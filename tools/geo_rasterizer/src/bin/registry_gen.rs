//! Bootstrap / extend the frozen geo-entity id registry.
//!
//! APPEND ONLY: never renumbers or removes ids. With no `--source`, it unions
//! every shipped worldview (`Worldview::ALL`) from repo-relative defaults, downloading the
//! pinned Natural Earth sources if missing — so the registry is the union across
//! all worldviews. Pass `--source <worldview-id>:<path>` to register specific
//! country files instead.
//!
//! Either way both levels are registered: countries from the admin-0 file(s),
//! provinces from the pinned admin-1 file. Natural Earth ships one
//! worldview-independent admin-1 file, so `--source` overrides the admin-0 side
//! only and the admin-1 side stays pinned — a registry without provinces would
//! make every `rasterize-geo` fail on an unknown `adm1_code`.
//!
//! Commit the resulting geo_entity_registry.toml in the same PR as the source
//! bump.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use clap::Parser;
use geo_data_format::Worldview;
use geo_rasterizer::admin0::parse_admin0;
use geo_rasterizer::admin1::parse_admin1;
use geo_rasterizer::download::{ensure_admin1, ensure_geojson};
use geo_rasterizer::registry::{
    admin0_point_items, admin1_point_items, merged_representative_points, register_worldview,
    to_toml_sorted, Namespace, Registry,
};

#[derive(Parser, Debug)]
#[command(version, about = "Append-only geo-entity id registry generator")]
struct Args {
    /// Explicit labeled admin-0 sources: `<worldview-id>:<path>`. When omitted, every
    /// shipped worldview (`Worldview::ALL`) is unioned from repo-relative defaults.
    /// Processed in given order; first source's codes get the lowest ids
    /// (stable). The admin-1 source stays pinned either way.
    #[arg(long = "source", value_name = "worldview:PATH", num_args = 1..)]
    sources: Vec<String>,
    /// Registry TOML to create or extend. Defaults to the crate's frozen
    /// geo_entity_registry.toml regardless of the caller's cwd.
    #[arg(long)]
    registry: Option<PathBuf>,
}

/// Crate dir, baked at compile time — defaults resolve relative to it so the
/// tool runs with no args regardless of the caller's cwd.
fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn default_registry() -> PathBuf {
    manifest().join("geo_entity_registry.toml")
}

fn default_countries(worldview: Worldview) -> PathBuf {
    manifest()
        .join("natural_earth")
        .join(worldview.spec().source_filename)
}

fn source_items(
    worldview: Worldview,
    path: &Path,
) -> Result<Vec<(String, Namespace, geo_types::MultiPolygon<f64>)>> {
    let features = parse_admin0(path, worldview.spec().id)?;
    Ok(admin0_point_items(&features))
}

fn main() -> Result<()> {
    let args = Args::parse();
    let registry_path = args.registry.unwrap_or_else(default_registry);

    let mut reg = if registry_path.exists() {
        Registry::load(&registry_path)?
    } else {
        Registry {
            schema: 1,
            continents: vec![],
            countries: vec![],
            provinces: vec![],
        }
    };
    let start_id = reg.next_id();

    let admin1_path = manifest()
        .join("natural_earth")
        .join(geo_data_format::ADMIN1_SOURCE_FILENAME);
    ensure_admin1(&admin1_path)?;

    let mut items: Vec<(String, Namespace, geo_types::MultiPolygon<f64>)> = Vec::new();
    if args.sources.is_empty() {
        // Default: union every shipped worldview from repo-relative defaults,
        // downloading the pinned Natural Earth source if missing.
        for &worldview in Worldview::ALL {
            let path = default_countries(worldview);
            ensure_geojson(&path, worldview)?;
            items.extend(source_items(worldview, &path)?);
            items.extend(admin1_point_items(&parse_admin1(&admin1_path, worldview)?));
        }
    } else {
        for source in &args.sources {
            // Relies on POSIX repo-relative paths (no Windows drive-letter colons).
            let (worldview_str, path_str) = match source.split_once(':') {
                Some(pair) => pair,
                None => bail!("--source must be in worldview:PATH form, got: {source}"),
            };
            let worldview = Worldview::from_id(worldview_str)?;
            items.extend(source_items(worldview, &PathBuf::from(path_str))?);
            items.extend(admin1_point_items(&parse_admin1(&admin1_path, worldview)?));
        }
    }

    register_worldview(&mut reg, &merged_representative_points(items));

    reg.validate_unique_ids()?;
    let after_id = reg.next_id();
    std::fs::write(&registry_path, to_toml_sorted(&reg)?)?;
    eprintln!(
        "[registry_gen] {} → {} ids ({} new); wrote {}",
        start_id,
        after_id,
        after_id - start_id,
        registry_path.display()
    );
    Ok(())
}
