use std::path::PathBuf;
use std::str::FromStr;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use geo_data_format::{write_geo_data, Worldview};
use geo_rasterizer::{
    area::populate_total_areas,
    atomic_write::{acquire_lock, write_atomically},
    cache::{compute_provenance_hash, read_existing_hash},
    download::{ensure_geojson, Pov},
    entities::assemble_entities,
    parse::{parse_geojson, parse_worldviews, validate_no_antimeridian_span},
    rasterize::rasterize,
    registry::{audit_identity, merged_representative_points, Registry},
};

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Path to Natural Earth countries GeoJSON (1:10m)
    #[arg(long)]
    countries: PathBuf,

    /// Path to Natural Earth provinces/states GeoJSON (1:10m).
    /// Optional: not used by the MVP; reserved for follow-up provinces work.
    #[arg(long)]
    provinces: Option<PathBuf>,

    /// Path to worldviews.toml
    #[arg(long)]
    worldviews: PathBuf,

    /// Path to the frozen geo-entity id registry.
    #[arg(long, default_value = "geo_entity_registry.toml")]
    registry: PathBuf,

    /// Output path for geo_data.bin
    #[arg(long, default_value = "geo_data.bin")]
    output: PathBuf,

    /// Download the pinned Natural Earth GeoJSON to --countries if missing
    /// or hash-mismatched. Production builds set this; tests using synthetic
    /// fixtures leave it off.
    #[arg(long)]
    ensure_source: bool,

    /// Which Natural Earth POV `--ensure-source` should fetch/verify.
    #[arg(long, default_value = "iso")]
    pov: String,

    /// Fetch/verify the source (with --ensure-source) then exit, before
    /// parse/registry/audit/assemble. Used to populate the geojson files
    /// the registry bootstrap reads, without needing a registry yet.
    #[arg(long)]
    download_only: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let pov = Pov::from_str(&args.pov)?;
    let started = Instant::now();
    eprintln!("[geo_rasterizer] started");

    let _lock = acquire_lock(&args.output)
        .with_context(|| format!("acquiring lock for {}", args.output.display()))?;

    if args.download_only && !args.ensure_source {
        anyhow::bail!("--download-only requires --ensure-source (nothing to do otherwise)");
    }
    if args.ensure_source {
        ensure_geojson(&args.countries, pov)?;
    }
    if args.download_only {
        eprintln!("[geo_rasterizer] --download-only: source ensured, exiting before parse");
        return Ok(());
    }

    if let Some(p) = &args.provinces {
        eprintln!(
            "[geo_rasterizer] --provinces ignored (out of MVP scope): {}",
            p.display()
        );
    }

    // 1. Smart skip — provenance hash (inputs + GEO_DATA_VERSION salt)
    //    vs. existing bin's embedded hash.
    let provenance_hash =
        compute_provenance_hash(&args.countries, &args.worldviews, &args.registry)?;
    if let Some(existing) = read_existing_hash(&args.output)? {
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
    let features = parse_geojson(&args.countries)?;
    eprintln!("[geo_rasterizer] parsed {} features", features.len());
    validate_no_antimeridian_span(&features)?;
    let registry = Registry::load(&args.registry)?;
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
    audit_identity(&present, &registry, pov.id(), 8.0)?;
    let parsed_worldviews = parse_worldviews(&args.worldviews)?;
    if parsed_worldviews.is_empty() {
        anyhow::bail!("worldviews.toml has no [[worldview]] entries");
    }
    let worldviews: Vec<Worldview> = parsed_worldviews
        .into_iter()
        .map(|w| Worldview {
            id: w.id,
            name_key: w.name_key,
            description_key: w.description_key,
        })
        .collect();

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
    write_atomically(&args.output, &bytes)?;

    eprintln!(
        "[geo_rasterizer] wrote {} ({} bytes) in {:.1?}",
        args.output.display(),
        bytes.len(),
        started.elapsed()
    );
    Ok(())
}
