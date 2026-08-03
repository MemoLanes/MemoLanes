use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use geo_data_format::{write_geo_data, GeoEntityId, GeoEntityKind, Locale, Worldview};
use geo_rasterizer::{
    admin0::{parse_admin0, validate_no_antimeridian_span},
    admin1::parse_admin1,
    area::populate_total_areas,
    atomic_write::write_atomically,
    cache::{compute_provenance_hash, read_existing_hash},
    cldr::{load_subdivisions, load_territories},
    download::{ensure_admin1, ensure_cldr, ensure_cldr_subdivisions, ensure_geojson},
    entities::{
        assemble_entities, attach_province_entities, collect_provinces, resolve_province_parents,
    },
    names::{build_region_names, write_region_names},
    overrides::Overrides,
    rasterize::rasterize,
    refine::{measure_coverage, refine_raster},
    registry::Registry,
};

/// Offline rasterizer. With no `--worldview` it rasterizes every shipped worldview using
/// repo-relative defaults (no other args needed); pass `--worldview <id>` to run a
/// single worldview and optionally override its input/output paths.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Which worldview to rasterize. Absent ⇒ batch over every `Worldview::ALL`.
    #[arg(long)]
    worldview: Option<String>,

    /// Override the countries GeoJSON path. Requires `--worldview`.
    #[arg(long, requires = "worldview")]
    countries: Option<PathBuf>,

    /// Override the admin-1 states/provinces GeoJSON path. Requires `--worldview`.
    #[arg(long, requires = "worldview")]
    admin1: Option<PathBuf>,

    /// Override the frozen geo-entity id registry path. Requires `--worldview`.
    #[arg(long, requires = "worldview")]
    registry: Option<PathBuf>,

    /// Override the output `geo_data.bin` path. Requires `--worldview`.
    #[arg(long, requires = "worldview")]
    output: Option<PathBuf>,

    /// Download the pinned Natural Earth GeoJSON if missing or hash-mismatched.
    /// Production builds set this; tests using synthetic fixtures leave it off.
    #[arg(long)]
    ensure_source: bool,

    /// Fetch/verify the source (with `--ensure-source`) then exit, before
    /// parse/registry/audit/assemble. Used to populate the geojson files the
    /// registry bootstrap reads, without needing a registry yet.
    #[arg(long, requires = "ensure_source")]
    download_only: bool,
}

/// Crate dir, baked at compile time — defaults resolve relative to it so the
/// tool runs with no args regardless of the caller's cwd.
fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn default_countries(worldview: Worldview) -> PathBuf {
    manifest()
        .join("natural_earth")
        .join(worldview.spec().source_filename)
}

fn default_admin1() -> PathBuf {
    manifest()
        .join("natural_earth")
        .join(geo_data_format::ADMIN1_SOURCE_FILENAME)
}

fn default_registry() -> PathBuf {
    manifest().join("geo_entity_registry.toml")
}

fn geo_assets_dir() -> PathBuf {
    manifest().join("../../app/assets/geo")
}

fn default_output(worldview: Worldview) -> PathBuf {
    geo_assets_dir().join(format!("geo_data_{}.bin", worldview.spec().id))
}

fn default_overrides() -> PathBuf {
    manifest().join("geo_names_overrides.toml")
}

fn default_cldr(locale: Locale) -> PathBuf {
    manifest()
        .join("cldr")
        .join(format!("territories.{}.json", locale.spec().cldr_tag))
}

fn default_cldr_subdivisions(locale: Locale) -> PathBuf {
    manifest()
        .join("cldr")
        .join(format!("subdivisions.{}.json", locale.spec().cldr_tag))
}

fn generate_region_names(ensure_source: bool) -> Result<()> {
    let overrides = Overrides::load(&default_overrides())?;
    let mut by_worldview = Vec::new();
    for &worldview in Worldview::ALL {
        let path = default_countries(worldview);
        if ensure_source {
            ensure_geojson(&path, worldview)?;
        }
        by_worldview.push((worldview, parse_admin0(&path, worldview.spec().id)?));
    }
    let mut admin1_by_worldview = Vec::new();
    for &worldview in Worldview::ALL {
        admin1_by_worldview.push((worldview, parse_admin1(&default_admin1(), worldview)?));
    }
    let mut cldr = BTreeMap::new();
    for &locale in Locale::ALL {
        let path = default_cldr(locale);
        if ensure_source {
            ensure_cldr(&path, locale)?;
        }
        cldr.insert(locale, load_territories(&path, locale.spec().cldr_tag)?);
    }
    let mut subdivisions = BTreeMap::new();
    for &locale in Locale::ALL {
        let path = default_cldr_subdivisions(locale);
        let fetched = if ensure_source {
            ensure_cldr_subdivisions(&path, locale)?
        } else {
            locale.spec().cldr_subdivisions_sha256.is_some()
        };
        if fetched {
            subdivisions.insert(locale, load_subdivisions(&path, locale.spec().cldr_tag)?);
        }
    }
    let names = build_region_names(
        &by_worldview,
        &admin1_by_worldview,
        &cldr,
        &subdivisions,
        &overrides,
    )?;
    for (locale, map) in &names {
        let path = write_region_names(&geo_assets_dir(), *locale, map)?;
        eprintln!(
            "[geo_rasterizer] wrote {} ({} names)",
            path.display(),
            map.len()
        );
    }
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let started = Instant::now();
    eprintln!("[geo_rasterizer] started");

    match &args.worldview {
        // Single mode: resolve the one worldview, honoring any path overrides.
        Some(id) => {
            let worldview = Worldview::from_id(id)?;
            rasterize_one(
                worldview,
                args.countries
                    .unwrap_or_else(|| default_countries(worldview)),
                args.admin1.clone().unwrap_or_else(default_admin1),
                args.registry.unwrap_or_else(default_registry),
                args.output.unwrap_or_else(|| default_output(worldview)),
                args.ensure_source,
                args.download_only,
            )?;
        }
        // Batch mode: every shipped worldview with derived paths. A new worldview in
        // `Worldview::ALL` is rasterized automatically.
        None => {
            for &worldview in Worldview::ALL {
                rasterize_one(
                    worldview,
                    default_countries(worldview),
                    args.admin1.clone().unwrap_or_else(default_admin1),
                    default_registry(),
                    default_output(worldview),
                    args.ensure_source,
                    args.download_only,
                )?;
            }
            if !args.download_only {
                generate_region_names(args.ensure_source)?;
            }
        }
    }

    eprintln!("[geo_rasterizer] done in {:.1?}", started.elapsed());
    Ok(())
}

/// Rasterize one worldview. Returns early (this fn only — never aborting a batch
/// loop) after `ensure_source` when `download_only` is set.
fn rasterize_one(
    worldview: Worldview,
    countries: PathBuf,
    admin1_path: PathBuf,
    registry_path: PathBuf,
    output: PathBuf,
    ensure_source: bool,
    download_only: bool,
) -> Result<()> {
    let started = Instant::now();
    eprintln!("[geo_rasterizer] worldview={}", worldview.spec().id);

    if ensure_source {
        ensure_geojson(&countries, worldview)?;
        ensure_admin1(&admin1_path)?;
    }
    if download_only {
        eprintln!("[geo_rasterizer] --download-only: source ensured, skipping rasterize");
        return Ok(());
    }

    // The asset embeds its own worldview id (self-describing); it also feeds the
    // provenance hash so a worldview retag alone still triggers a rebuild.
    let worldview_id = worldview.spec().id;

    // 1. Smart skip — provenance hash (inputs + GEO_DATA_VERSION salt)
    //    vs. existing bin's embedded hash.
    let provenance_hash =
        compute_provenance_hash(&countries, &admin1_path, &registry_path, worldview_id)?;
    if let Some(existing) = read_existing_hash(&output)? {
        if existing == provenance_hash {
            eprintln!(
                "[geo_rasterizer] inputs unchanged (hash match) — output up to date in {:.0?}",
                started.elapsed()
            );
            return Ok(());
        }
    }

    // 2. Parse + validate.
    eprintln!("[geo_rasterizer] parsing inputs...");
    let features = parse_admin0(&countries, worldview_id)?;
    eprintln!("[geo_rasterizer] parsed {} features", features.len());
    validate_no_antimeridian_span(&features)?;
    let registry = Registry::load(&registry_path)?;

    // 3. Entity assembly.
    eprintln!("[geo_rasterizer] assembling entity model...");
    let mut model = assemble_entities(&features, &registry)?;
    let admin1_features = parse_admin1(&admin1_path, worldview)?;
    collect_provinces(&mut model, &admin1_features, &registry)?;
    eprintln!(
        "[geo_rasterizer] {} countries + {} provinces",
        model.geometry_for_country.len(),
        model.geometry_for_province.len(),
    );

    // 4. Rasterize countries, then provinces, then merge inside the country mask.
    eprintln!("[geo_rasterizer] rasterizing countries...");
    let country_ids: BTreeMap<String, GeoEntityId> = model
        .entities
        .iter()
        .filter(|e| matches!(e.kind, GeoEntityKind::Country))
        .map(|e| (e.canonical_code.clone(), e.id))
        .collect();
    let country_raster = rasterize(&model.geometry_for_country, &country_ids);

    eprintln!("[geo_rasterizer] rasterizing provinces...");
    let province_raster = rasterize(&model.geometry_for_province, &model.province_ids);

    eprintln!("[geo_rasterizer] resolving province parents...");
    let tally = measure_coverage(
        (&country_raster.0, &country_raster.1),
        (&province_raster.0, &province_raster.1),
    );
    let parents = resolve_province_parents(&model, &tally, worldview)?;
    attach_province_entities(&mut model, &parents);
    let (tile_lookup, block_lookup) = refine_raster(
        (&country_raster.0, &country_raster.1),
        (&province_raster.0, &province_raster.1),
        &parents,
    );
    eprintln!(
        "[geo_rasterizer] merged: {} entities, {} border tiles",
        model.entities.len(),
        block_lookup.len()
    );

    // 5. Areas.
    populate_total_areas(&mut model, &tile_lookup, &block_lookup);

    // TODO: Phase 2 — instead of one bin per run, iterate the
    // shipped worldview files and emit a shared base + per-worldview delta sections.
    // The registry already gives cross-worldview-stable ids.

    // 6. Serialize (sectioned format) + atomic write.
    let bytes = write_geo_data(
        &model.entities,
        worldview_id,
        &tile_lookup,
        &block_lookup,
        provenance_hash,
    )
    .context("serializing geo_data.bin")?;
    write_atomically(&output, &bytes)?;

    eprintln!(
        "[geo_rasterizer] wrote {} ({} bytes) in {:.1?}",
        output.display(),
        bytes.len(),
        started.elapsed()
    );
    Ok(())
}
