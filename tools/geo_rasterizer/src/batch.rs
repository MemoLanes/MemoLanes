use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::{
    admin0::{parse_admin0_with_attributions, validate_no_antimeridian_span},
    admin1::parse_admin1,
    admin2::Admin2Feature,
    area::populate_total_areas_planes,
    atomic_write::write_atomically,
    cache::compute_provenance_hash,
    display::{build_display_records, DisplaySource},
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
use anyhow::{ensure, Context, Result};
use bitmap_geometry::BitmapPlane;
use geo_data_format::{
    read_provenance_hash, write_display_asset, write_geo_data_planes, DisplayRecord, GeoData,
    GeoEntity, GeoEntityId, GeoEntityKind, PlaneRaster, Worldview,
};

use crate::display_cache;

/// Execution mode for a batch with explicit input and output paths.
pub enum RasterizeMode<'a> {
    DownloadSources,
    RasterOnly {
        ensure_source: bool,
    },
    WithDisplay {
        ensure_source: bool,
        display_output: &'a Path,
    },
}

/// Run each worldview, planning the entire display batch before publishing anything.
pub fn rasterize_batch(
    inputs: impl IntoIterator<Item = (Worldview, RasterizePaths)>,
    mode: RasterizeMode<'_>,
) -> Result<()> {
    let (ensure_source, download_only, display_output) = match mode {
        RasterizeMode::DownloadSources => (true, true, None),
        RasterizeMode::RasterOnly { ensure_source } => (ensure_source, false, None),
        RasterizeMode::WithDisplay {
            ensure_source,
            display_output,
        } => (ensure_source, false, Some(display_output)),
    };
    let mut plans = Vec::new();
    for (worldview, paths) in inputs {
        if let Some(plan) = plan_raster(
            worldview,
            paths,
            ensure_source,
            download_only,
            display_output.is_some(),
        )? {
            if display_output.is_some() {
                plans.push(plan);
            } else {
                rasterize_one(&plan, None)?;
            }
        }
    }
    if let Some(path) = display_output {
        build_display_batch(path, &plans)?;
    }
    Ok(())
}

fn build_display_batch(path: &Path, plans: &[RasterPlan]) -> Result<()> {
    let raster_paths = || plans.iter().map(|p| p.paths.output.as_path());
    display_cache::validate_paths(path, raster_paths())?;
    let manifest: Vec<_> = plans
        .iter()
        .map(|p| (p.worldview, p.provenance_hash))
        .collect();
    let mut current_hash = display_cache::RasterEntitiesHash::default();
    let rasters_current = plans.iter().all(|p| {
        let RasterState::CurrentForDisplay(entities) = &p.state else {
            return false;
        };
        current_hash.add_worldview(p.worldview, entities);
        true
    });
    if rasters_current && display_cache::is_current(path, &manifest, current_hash.finish()) {
        eprintln!("[geo_rasterizer] display cache hit: {}", path.display());
        return Ok(());
    }
    if !rasters_current {
        // Even identical raster entities must not allow a hit after an
        // incomplete batch that published new rasters but no display.
        display_cache::invalidate(path)?;
    }

    let mut display = DisplayExport::default();
    for plan in plans {
        rasterize_one(plan, Some(&mut display))?;
    }
    // Newly created files can now be canonicalized in full.
    if !rasters_current {
        display_cache::validate_paths(path, raster_paths())?;
    }
    let bytes = write_display_asset(&manifest, &display.records)?;
    display_cache::publish(path, &bytes, display.raster_entities.finish())?;
    eprintln!(
        "[geo_rasterizer] wrote {} ({} records, {} bytes)",
        path.display(),
        display.records.len(),
        bytes.len()
    );
    Ok(())
}

#[derive(Default)]
struct DisplayExport {
    records: Vec<DisplayRecord>,
    raster_entities: display_cache::RasterEntitiesHash,
}

impl DisplayExport {
    fn add_raster(
        &mut self,
        worldview: Worldview,
        entities: &[GeoEntity],
        model: &EntityModel,
        admin2: Vec<Admin2Feature>,
    ) -> Result<()> {
        self.records
            .extend(export_records(worldview, entities, model, admin2)?);
        self.raster_entities.add_worldview(worldview, entities);
        Ok(())
    }
}

/// Input/output paths for one worldview run.
pub struct RasterizePaths {
    pub countries: PathBuf,
    pub admin1: PathBuf,
    pub registry: PathBuf,
    pub admin2_dir: PathBuf,
    pub output: PathBuf,
}

enum RasterState {
    Rebuild,
    Current,
    CurrentForDisplay(Vec<GeoEntity>),
}

struct RasterPlan {
    worldview: Worldview,
    paths: RasterizePaths,
    provenance_hash: [u8; 32],
    state: RasterState,
}

fn plan_raster(
    worldview: Worldview,
    paths: RasterizePaths,
    ensure_source: bool,
    download_only: bool,
    export_display: bool,
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
    let state = if read_provenance_hash(&paths.output)? != Some(provenance_hash) {
        RasterState::Rebuild
    } else if !export_display {
        RasterState::Current
    } else {
        // The display batch also needs the raster's embedded worldview and
        // entity metadata. Save the entities for export if the display misses.
        let entities = GeoData::open(&paths.output).and_then(|raster| {
            ensure!(
                raster.worldview_id == worldview_id && raster.provenance_hash == provenance_hash,
                "wrong worldview or provenance"
            );
            raster.entities()
        });
        match entities {
            Ok(entities) => RasterState::CurrentForDisplay(entities),
            Err(err) => {
                eprintln!(
                    "[geo_rasterizer] rebuilding {}: {err:#}",
                    paths.output.display()
                );
                RasterState::Rebuild
            }
        }
    };
    if !matches!(state, RasterState::Rebuild) {
        eprintln!(
            "[geo_rasterizer] {worldview_id} raster up to date in {:.0?}",
            started.elapsed()
        );
    }
    Ok(Some(RasterPlan {
        worldview,
        paths,
        provenance_hash,
        state,
    }))
}

fn rasterize_one(plan: &RasterPlan, display: Option<&mut DisplayExport>) -> Result<()> {
    if matches!(plan.state, RasterState::Current) {
        return Ok(());
    }
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
    if let RasterState::CurrentForDisplay(entities) = &plan.state {
        let admin2 = crate::admin2::load(admin2_dir)?;
        return display
            .expect("planned display export")
            .add_raster(worldview, entities, &model, admin2);
    }

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

    // TODO: Phase 2 — instead of one bin per run, iterate the
    // shipped worldview files and emit a shared base + per-worldview delta sections.
    // The registry already gives cross-worldview-stable ids.

    // 6. Serialize (sectioned format) + atomic write.
    let bytes = write_geo_data_planes(
        &model.entities,
        worldview_id,
        &rasters,
        plan.provenance_hash,
    )
    .context("serializing geo_data.bin")?;
    write_atomically(output, &bytes)?;

    eprintln!(
        "[geo_rasterizer] wrote {} ({} bytes) in {:.1?}",
        output.display(),
        bytes.len(),
        started.elapsed()
    );
    if let Some(display) = display {
        display.add_raster(worldview, &model.entities, &model, admin2_features)?;
    }
    Ok(())
}

// Policy resolution is the same with or without a current raster. Only the
// coverage-dependent inclusion and parent decisions come from raster metadata.
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

fn export_records(
    worldview: Worldview,
    entities: &[GeoEntity],
    model: &EntityModel,
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
            GeoEntityKind::Admin0 => model.geometry_for_country.get(code).cloned(),
            GeoEntityKind::Admin1 => model.geometry_for_province.get(code).cloned(),
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
