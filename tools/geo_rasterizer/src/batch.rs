use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::{
    admin0::{parse_admin0_with_attributions, validate_no_antimeridian_span},
    admin1::parse_admin1,
    admin2::Admin2Feature,
    area::populate_total_areas_planes,
    atomic_write::publish_atomically,
    cache::compute_provenance_hash,
    display::{build_display_records, validate_loaded_geometry, DisplaySource},
    download::{ensure_admin1, ensure_geojson},
    entities::{
        apply_admin1_policy, apply_synthesized, assemble_entities, attach_admin2_entities,
        attach_province_entities, collect_provinces, resolve_province_parents, validate_tier_order,
        EntityModel, MergedSource,
    },
    polar::validate_no_polar_provinces,
    rasterize::{rasterize, rasterize_in_plane},
    refine::{block_counts, measure_coverage, refine_raster},
    registry::Registry,
};
use anyhow::{Context, Result};
use bitmap_geometry::BitmapPlane;
use geo_data_format::{
    read_header, verify_display_ids, write_geo_data_planes_with_display, DisplayRecord, GeoData,
    GeoEntity, GeoEntityId, GeoEntityKind, PlaneRaster, Worldview,
};
use geo_types::MultiPolygon;

/// Execution mode for a batch with explicit input and output paths.
pub enum RasterizeMode {
    DownloadSources,
    Build { ensure_source: bool },
}

pub fn rasterize_batch(
    inputs: impl IntoIterator<Item = (Worldview, RasterizePaths)>,
    mode: RasterizeMode,
) -> Result<()> {
    let (ensure_source, download_only) = match mode {
        RasterizeMode::DownloadSources => (true, true),
        RasterizeMode::Build { ensure_source } => (ensure_source, false),
    };
    for (worldview, paths) in inputs {
        if let Some(plan) = plan_raster(worldview, paths, ensure_source, download_only)? {
            rasterize_one(&plan)?;
        }
    }
    Ok(())
}

/// Input/output paths for one worldview run.
pub struct RasterizePaths {
    pub countries: PathBuf,
    pub admin1: PathBuf,
    pub registry: PathBuf,
    pub admin2_dir: PathBuf,
    pub output: PathBuf,
}

struct RasterPlan {
    worldview: Worldview,
    paths: RasterizePaths,
    provenance_hash: [u8; 32],
}

fn plan_raster(
    worldview: Worldview,
    paths: RasterizePaths,
    ensure_source: bool,
    download_only: bool,
) -> Result<Option<RasterPlan>> {
    let started = Instant::now();
    let worldview_id = worldview.spec().id;
    eprintln!("[geo_rasterizer] checking sources and raster for {worldview_id}");

    if ensure_source {
        ensure_geojson(&paths.countries, worldview)?;
        ensure_admin1(&paths.admin1)?;
    }
    if download_only {
        eprintln!("[geo_rasterizer] --download-only: source ensured, skipping rasterize");
        return Ok(None);
    }

    // The asset embeds its own worldview id (self-describing); it also feeds the
    // provenance hash so a worldview retag alone still triggers a rebuild.
    let admin2_inputs = crate::admin2::provenance_inputs(&paths.admin2_dir);
    let provenance_hash = compute_provenance_hash(
        &paths.countries,
        &paths.admin1,
        &paths.registry,
        &crate::policy::default_path(),
        &admin2_inputs,
        worldview_id,
    )?;
    if read_header(&paths.output)?
        .is_some_and(|header| header.provenance_hash == provenance_hash && header.display.len != 0)
    {
        match open_display_asset(&paths.output, worldview, provenance_hash)
            .and_then(|asset| validate_cached_display(&asset))
        {
            Ok(()) => {
                eprintln!(
                    "[geo_rasterizer] {worldview_id} geo asset up to date in {:.1?}",
                    started.elapsed()
                );
                return Ok(None);
            }
            Err(error) => eprintln!("[geo_rasterizer] rebuilding {worldview_id}: {error:#}"),
        }
    }
    Ok(Some(RasterPlan {
        worldview,
        paths,
        provenance_hash,
    }))
}

fn rasterize_one(plan: &RasterPlan) -> Result<()> {
    let RasterizePaths {
        countries,
        admin1: admin1_path,
        registry: registry_dir,
        admin2_dir,
        output,
    } = &plan.paths;
    let worldview = plan.worldview;
    let worldview_id = worldview.spec().id;
    let started = Instant::now();
    eprintln!("[geo_rasterizer] worldview={worldview_id}");
    let registry = Registry::load(registry_dir)?;
    let (mut model, merged) = assemble_geometry(worldview, countries, admin1_path, &registry)?;

    eprintln!(
        "[geo_rasterizer] {} countries + {} provinces",
        model.geometry_for_country.len(),
        model.geometry_for_province.len(),
    );

    validate_no_polar_provinces(&model.geometry_for_province)?;

    // 4. Rasterize countries in every plane; refine provinces only in Mercator.
    eprintln!("[geo_rasterizer] rasterizing countries...");
    let country_ids: BTreeMap<String, GeoEntityId> = model
        .entities
        .iter()
        .filter(|e| matches!(e.kind, GeoEntityKind::Admin0))
        .map(|e| (e.canonical_code.clone(), e.id))
        .collect();
    let mut mercator_raster = rasterize(&model.geometry_for_country, &country_ids);
    let north_polar_raster = rasterize_in_plane(
        &model.geometry_for_country,
        &country_ids,
        BitmapPlane::NorthPolar,
    );
    let south_polar_raster = rasterize_in_plane(
        &model.geometry_for_country,
        &country_ids,
        BitmapPlane::SouthPolar,
    );

    // Zero-block gate: a merge whose land the country mask does not cover is silently clipped by `refine`
    if !merged.is_empty() {
        let base = model.entities.iter().map(|e| e.id.0).max().unwrap_or(0) + 1;
        let mut probe_geoms = BTreeMap::new();
        let mut probe_ids = BTreeMap::new();
        for (i, m) in merged.iter().enumerate() {
            probe_geoms.insert(m.code.clone(), m.geometry.clone());
            probe_ids.insert(m.code.clone(), GeoEntityId(base + i as u32));
        }
        let probe = rasterize(&probe_geoms, &probe_ids);
        let cov = measure_coverage(
            (&mercator_raster.0, &mercator_raster.1),
            (&probe.0, &probe.1),
        );
        for (i, m) in merged.iter().enumerate() {
            let declared = model
                .province_declared_adm0
                .get(&m.target)
                .with_context(|| format!("merge target `{}` has no declared country", m.target))?;
            let country = country_ids.get(declared).with_context(|| {
                format!("merge target `{}`: `{declared}` is no country", m.target)
            })?;
            let blocks = cov
                .get((base + i as u32) as usize)
                .and_then(|by| by.get(country).copied())
                .unwrap_or(0);
            if blocks == 0 {
                anyhow::bail!(
                    "merged `{}` contributes no block to `{}` under its country mask — the merge \
                     is dead; drop the entry or fix the mask",
                    m.code,
                    m.target
                );
            }
        }
    }

    eprintln!("[geo_rasterizer] rasterizing provinces...");
    let province_raster = rasterize(&model.geometry_for_province, &model.province_ids);

    eprintln!("[geo_rasterizer] resolving province parents...");
    let tally = measure_coverage(
        (&mercator_raster.0, &mercator_raster.1),
        (&province_raster.0, &province_raster.1),
    );
    let province_blocks = block_counts((&province_raster.0, &province_raster.1));
    let parents = resolve_province_parents(&model, &tally, &province_blocks, worldview)?;
    attach_province_entities(&mut model, &parents);
    validate_tier_order(&model.entities)?;
    mercator_raster = refine_raster(
        (&mercator_raster.0, &mercator_raster.1),
        (&province_raster.0, &province_raster.1),
        &parents,
    );

    let admin2_features = crate::admin2::load(admin2_dir)?;
    if !admin2_features.is_empty() {
        eprintln!(
            "[geo_rasterizer] rasterizing {} admin-2 units...",
            admin2_features.len()
        );
        let mut admin2_geoms = BTreeMap::new();
        let mut admin2_ids = BTreeMap::new();
        let mut admin2_parents: BTreeMap<GeoEntityId, GeoEntityId> = BTreeMap::new();
        for f in &admin2_features {
            let id = registry.id_for_admin2(&f.canonical_code)?;
            // `parents` holds only the provinces that actually shipped, so a
            // code present in `province_ids` but dropped as landless or
            // unparented is rejected here rather than orphaning the child.
            let parent = model
                .province_ids
                .get(&f.parent_adm1)
                .copied()
                .filter(|p| parents.contains_key(p))
                .with_context(|| {
                    format!(
                        "admin-2 `{}` parents to `{}`, which does not ship in worldview \
                         `{worldview_id}`",
                        f.canonical_code, f.parent_adm1
                    )
                })?;
            admin2_ids.insert(f.canonical_code.clone(), id);
            admin2_geoms.insert(f.canonical_code.clone(), f.geometry.clone());
            admin2_parents.insert(id, parent);
        }
        let admin2_raster = rasterize(&admin2_geoms, &admin2_ids);

        let cov = measure_coverage(
            (&mercator_raster.0, &mercator_raster.1),
            (&admin2_raster.0, &admin2_raster.1),
        );
        for (code, &id) in &admin2_ids {
            let parent = admin2_parents[&id];
            let blocks = cov
                .get(id.0 as usize)
                .and_then(|by| by.get(&parent))
                .copied()
                .unwrap_or(0);
            if blocks == 0 {
                anyhow::bail!(
                    "admin-2 `{code}` has no block inside its parent's mask — landless entity"
                );
            }
        }

        attach_admin2_entities(&mut model, &admin2_ids, &admin2_parents);
        validate_tier_order(&model.entities)?;
        mercator_raster = refine_raster(
            (&mercator_raster.0, &mercator_raster.1),
            (&admin2_raster.0, &admin2_raster.1),
            &admin2_parents,
        );
    }

    let rasters = [
        (BitmapPlane::Mercator, &mercator_raster),
        (BitmapPlane::NorthPolar, &north_polar_raster),
        (BitmapPlane::SouthPolar, &south_polar_raster),
    ]
    .map(|(plane, raster)| PlaneRaster {
        plane,
        tiles: &raster.0,
        blocks: &raster.1,
    });
    eprintln!(
        "[geo_rasterizer] merged: {} entities, {} border tiles",
        model.entities.len(),
        rasters.iter().map(|r| r.blocks.len()).sum::<usize>()
    );

    // 5. Areas.
    populate_total_areas_planes(&mut model, &rasters);

    let entities = model.entities;
    let display = export_records(
        worldview,
        &entities,
        model.geometry_for_country,
        model.geometry_for_province,
        admin2_features,
    )?;
    verify_display_ids(&entities, display.iter().map(|record| record.entity_id))
        .with_context(|| format!("[display:{worldview_id}] entity parity"))?;
    let bytes = write_geo_data_planes_with_display(
        &entities,
        worldview_id,
        &rasters,
        plan.provenance_hash,
        &display,
    )
    .context("serializing geo_data.bin")?;
    publish_asset(
        output,
        &bytes,
        worldview,
        plan.provenance_hash,
        &entities,
        &display,
    )
    .with_context(|| format!("[display:{worldview_id}] publishing geo asset"))?;

    eprintln!(
        "[geo_rasterizer] wrote {} ({} bytes) in {:.1?}",
        output.display(),
        bytes.len(),
        started.elapsed()
    );
    Ok(())
}

fn assemble_geometry(
    worldview: Worldview,
    countries: &Path,
    admin1_path: &Path,
    registry: &Registry,
) -> Result<(EntityModel, Vec<MergedSource>)> {
    // 2. Parse + validate.
    eprintln!("[geo_rasterizer] parsing inputs...");
    let parse = parse_admin0_with_attributions(countries, worldview.spec().id)?;
    let features = parse.features;
    eprintln!("[geo_rasterizer] parsed {} features", features.len());
    validate_no_antimeridian_span(&features)?;

    // 3. Entity assembly.
    eprintln!("[geo_rasterizer] assembling entity model...");
    let mut model = assemble_entities(&features, registry)?;
    let policy = apply_admin1_policy(parse_admin1(admin1_path, worldview)?, worldview, &features)?;
    let mut merged = policy.merged_sources;
    collect_provinces(&mut model, &policy.features, registry)?;
    apply_synthesized(&mut model, worldview, admin1_path, registry)?;
    for (target, geometry) in parse.province_attributions {
        model
            .geometry_for_province
            .get_mut(&target)
            .with_context(|| {
                format!("absorption attributes land to `{target}`, which does not ship here")
            })?
            .0
            .extend(geometry.0.iter().cloned());
        merged.push(MergedSource {
            code: format!("absorbed→{target}"),
            target,
            geometry,
        });
    }
    Ok((model, merged))
}

fn open_display_asset(path: &Path, worldview: Worldview, provenance: [u8; 32]) -> Result<GeoData> {
    let asset = GeoData::open(path)?;
    anyhow::ensure!(
        asset.worldview_id == worldview.spec().id,
        "expected worldview {}, found {}",
        worldview.spec().id,
        asset.worldview_id
    );
    anyhow::ensure!(
        asset.provenance_hash == provenance,
        "asset provenance mismatch"
    );
    Ok(asset)
}

fn publish_asset(
    output: &Path,
    bytes: &[u8],
    worldview: Worldview,
    provenance: [u8; 32],
    entities: &[GeoEntity],
    records: &[DisplayRecord],
) -> Result<()> {
    publish_atomically(
        output,
        |file| Ok(file.write_all(bytes)?),
        |staged| {
            let asset = open_display_asset(staged, worldview, provenance)?;
            anyhow::ensure!(
                asset.entities()? == entities,
                "serialized raster entities differ from the final entity table"
            );
            verify_display_ids(entities, asset.display_ids()?)?;
            for record in records {
                let id = record.entity_id;
                let decoded = asset
                    .display_geometry(id)
                    .with_context(|| format!("decoding staged display ID {}", id.0))?
                    .with_context(|| format!("missing staged display ID {}", id.0))?;
                anyhow::ensure!(
                    decoded.bounds == record.bounds && decoded.polygons == record.polygons,
                    "serialized outline differs from validated display ID {}",
                    id.0
                );
            }
            Ok(())
        },
    )
}

fn validate_cached_display(asset: &GeoData) -> Result<()> {
    asset.validate_display_parity()?;
    asset.validate_display(|id, geometry| {
        validate_loaded_geometry(geometry)
            .with_context(|| format!("invalid cached display ID {}", id.0))
    })
}

fn export_records(
    worldview: Worldview,
    entities: &[GeoEntity],
    mut countries: BTreeMap<String, MultiPolygon<f64>>,
    mut provinces: BTreeMap<String, MultiPolygon<f64>>,
    admin2: Vec<Admin2Feature>,
) -> Result<Vec<DisplayRecord>> {
    let mut admin2: BTreeMap<_, _> = admin2
        .into_iter()
        .map(|f| (f.canonical_code, f.geometry))
        .collect();
    let mut sources = Vec::new();
    for entity in entities {
        let code = &entity.canonical_code;
        let polygons = match entity.kind {
            GeoEntityKind::Continent => continue,
            GeoEntityKind::Admin0 => countries.remove(code),
            GeoEntityKind::Admin1 => provinces.remove(code),
            GeoEntityKind::Admin2 => admin2.remove(code),
        }
        .with_context(|| {
            format!(
                "display: raster entity {:?} ({code}) has no source",
                entity.id
            )
        })?;
        sources.push(DisplaySource {
            entity_id: entity.id,
            polygons,
        });
    }
    build_display_records(worldview, &sources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_data_format::{DisplayBounds, DisplayPolygons};

    #[test]
    fn a_valid_but_substituted_staged_outline_preserves_the_previous_asset() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("asset.bin");
        let id = GeoEntityId(17);
        let entities = [GeoEntity {
            id,
            kind: GeoEntityKind::Admin0,
            canonical_code: "TST".into(),
            iso_a3_eh: None,
            name_key: "country.TST".into(),
            parent_id: None,
            total_area_m2: 1,
        }];
        let record = DisplayRecord {
            entity_id: id,
            worldview: Worldview::Iso,
            bounds: DisplayBounds {
                west: 0.,
                south: 0.,
                east: 1.,
                north: 1.,
            },
            polygons: DisplayPolygons::Grid(vec![vec![vec![
                [0, 0],
                [100000, 0],
                [0, 100000],
                [0, 0],
            ]]]),
        };
        let records = [record.clone()];
        let encode = |records: &[DisplayRecord]| {
            write_geo_data_planes_with_display(&entities, "iso", &[], [4; 32], records)
        };
        let previous = encode(&records)?;
        publish_asset(
            &path,
            &previous,
            Worldview::Iso,
            [4; 32],
            &entities,
            &records,
        )?;
        let mut substituted = record;
        substituted.polygons = DisplayPolygons::Grid(vec![vec![vec![
            [0, 0],
            [100000, 100000],
            [0, 100000],
            [0, 0],
        ]]]);
        let wrong = encode(&[substituted])?;
        let error =
            publish_asset(&path, &wrong, Worldview::Iso, [4; 32], &entities, &records).unwrap_err();
        assert!(format!("{error:#}").contains("differs from validated display ID 17"));
        assert_eq!(std::fs::read(path)?, previous);
        assert_eq!(std::fs::read_dir(dir.path())?.count(), 1);
        Ok(())
    }
}
