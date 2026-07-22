use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use geo_data_format::{write_geo_data, Worldview};
use geo_rasterizer::{
    area::populate_total_areas,
    atomic_write::write_atomically,
    cache::{compute_provenance_hash, read_existing_hash},
    download::ensure_geojson,
    entities::assemble_entities,
    names::{build_region_names, write_region_names},
    overrides::Overrides,
    parse::{parse_geojson, validate_no_antimeridian_span},
    rasterize::rasterize,
    registry::{audit_identity, merged_representative_points, Registry},
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

fn generate_region_names(ensure_source: bool) -> Result<()> {
    let overrides = Overrides::load(&default_overrides())?;
    let mut by_worldview = Vec::new();
    for &worldview in Worldview::ALL {
        let path = default_countries(worldview);
        if ensure_source {
            ensure_geojson(&path, worldview)?;
        }
        by_worldview.push((worldview, parse_geojson(&path, worldview.spec().id)?));
    }
    let names = build_region_names(&by_worldview, &overrides)?;
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
    registry_path: PathBuf,
    output: PathBuf,
    ensure_source: bool,
    download_only: bool,
) -> Result<()> {
    let started = Instant::now();
    eprintln!("[geo_rasterizer] worldview={}", worldview.spec().id);

    if ensure_source {
        ensure_geojson(&countries, worldview)?;
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
    let provenance_hash = compute_provenance_hash(&countries, &registry_path, worldview_id)?;
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
    let features = parse_geojson(&countries, worldview_id)?;
    eprintln!("[geo_rasterizer] parsed {} features", features.len());
    validate_no_antimeridian_span(&features)?;
    let registry = Registry::load(&registry_path)?;
    // CI gate 2: a code must denote the same place across worldviews/bumps.
    // Use the merged-geometry centroid per ADM0_A3 so that multi-part
    // countries (e.g. FRA with overseas territories) are not falsely flagged
    // by fragments that fall far from the registry's reference point.
    let present: Vec<(String, (f64, f64))> = merged_representative_points(
        features
            .iter()
            .map(|f| (f.adm0_a3.clone(), false, f.geometry.clone())),
    )
    .into_iter()
    .map(|(code, _is_cont, pt)| (code, pt))
    .collect();
    audit_identity(&present, &registry, worldview.spec().id, 8.0)?;

    // 3. Entity assembly.
    eprintln!("[geo_rasterizer] assembling entity model...");
    let mut model = assemble_entities(&features, &registry)?;
    eprintln!(
        "[geo_rasterizer] {} entities ({} continents + {} countries)",
        model.entities.len(),
        model
            .entities
            .iter()
            .filter(|e| matches!(e.kind, geo_data_format::GeoEntityKind::Continent))
            .count(),
        model
            .entities
            .iter()
            .filter(|e| matches!(e.kind, geo_data_format::GeoEntityKind::Country))
            .count(),
    );

    // 4. Rasterize.
    eprintln!("[geo_rasterizer] rasterizing...");
    let raster_started = Instant::now();
    let (tile_lookup, block_lookup) = rasterize(&features, &model);
    eprintln!(
        "[geo_rasterizer] rasterization done in {:.1?} ({} border tiles)",
        raster_started.elapsed(),
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
