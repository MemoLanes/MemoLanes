use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use geo_data_format::{shipped_worldviews, write_geo_data, Pov};
use geo_rasterizer::{
    area::populate_total_areas,
    atomic_write::write_atomically,
    cache::{compute_provenance_hash, read_existing_hash},
    download::ensure_geojson,
    entities::assemble_entities,
    parse::{parse_geojson, validate_no_antimeridian_span},
    rasterize::rasterize,
    registry::{audit_identity, merged_representative_points, Registry},
};

/// Offline rasterizer. With no `--pov` it rasterizes every shipped POV using
/// repo-relative defaults (no other args needed); pass `--pov <id>` to run a
/// single POV and optionally override its input/output paths.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Which POV to rasterize. Absent ⇒ batch over every `Pov::ALL`.
    #[arg(long)]
    pov: Option<String>,

    /// Override the countries GeoJSON path. Requires `--pov`.
    #[arg(long, requires = "pov")]
    countries: Option<PathBuf>,

    /// Override the frozen geo-entity id registry path. Requires `--pov`.
    #[arg(long, requires = "pov")]
    registry: Option<PathBuf>,

    /// Override the output `geo_data.bin` path. Requires `--pov`.
    #[arg(long, requires = "pov")]
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

fn default_countries(pov: Pov) -> PathBuf {
    manifest()
        .join("natural_earth")
        .join(pov.spec().source_filename)
}

fn default_registry() -> PathBuf {
    manifest().join("geo_entity_registry.toml")
}

fn default_output(pov: Pov) -> PathBuf {
    manifest()
        .join("../../app/assets")
        .join(format!("geo_data_{}.bin", pov.spec().id))
}

fn main() -> Result<()> {
    let args = Args::parse();
    let started = Instant::now();
    eprintln!("[geo_rasterizer] started");

    match &args.pov {
        // Single mode: resolve the one POV, honoring any path overrides.
        Some(id) => {
            let pov = Pov::from_id(id)?;
            rasterize_one(
                pov,
                args.countries.unwrap_or_else(|| default_countries(pov)),
                args.registry.unwrap_or_else(default_registry),
                args.output.unwrap_or_else(|| default_output(pov)),
                args.ensure_source,
                args.download_only,
            )?;
        }
        // Batch mode: every shipped POV with derived paths. A new POV in
        // `Pov::ALL` is rasterized automatically.
        None => {
            for &pov in Pov::ALL {
                rasterize_one(
                    pov,
                    default_countries(pov),
                    default_registry(),
                    default_output(pov),
                    args.ensure_source,
                    args.download_only,
                )?;
            }
        }
    }

    eprintln!("[geo_rasterizer] done in {:.1?}", started.elapsed());
    Ok(())
}

/// Rasterize one POV. Returns early (this fn only — never aborting a batch
/// loop) after `ensure_source` when `download_only` is set.
fn rasterize_one(
    pov: Pov,
    countries: PathBuf,
    registry_path: PathBuf,
    output: PathBuf,
    ensure_source: bool,
    download_only: bool,
) -> Result<()> {
    let started = Instant::now();
    eprintln!("[geo_rasterizer] pov={}", pov.spec().id);

    if ensure_source {
        ensure_geojson(&countries, pov)?;
    }
    if download_only {
        eprintln!("[geo_rasterizer] --download-only: source ensured, skipping rasterize");
        return Ok(());
    }

    // Worldviews are derived once and fed to BOTH the provenance hash and the
    // embedded list, so the hashed content always matches what's written.
    let worldviews = shipped_worldviews();

    // 1. Smart skip — provenance hash (inputs + GEO_DATA_VERSION salt)
    //    vs. existing bin's embedded hash.
    let provenance_hash = compute_provenance_hash(&countries, &registry_path, &worldviews)?;
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
    let features = parse_geojson(&countries)?;
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
    audit_identity(&present, &registry, pov.spec().id, 8.0)?;

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

    // TODO(geo-C): Phase 2 — instead of one bin per run, iterate the
    // shipped POV files and emit a shared base + per-POV delta sections.
    // The registry already gives cross-POV-stable ids.

    // 6. Serialize (sectioned format) + atomic write.
    let bytes = write_geo_data(
        &model.entities,
        &worldviews,
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
