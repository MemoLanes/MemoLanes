//! Bootstrap / extend the frozen geo-entity id registry.
//!
//! APPEND ONLY: never renumbers or removes ids. With no `--source`, it unions
//! every shipped worldview (`Worldview::ALL`) from repo-relative defaults, downloading the
//! pinned Natural Earth source if missing — so the registry is the union across
//! all worldviews. Pass `--source <worldview-id>:<path>` to register specific files
//! instead. Commit the resulting geo_entity_registry.toml in the same PR as the
//! source bump.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use clap::Parser;
use geo_data_format::Worldview;
use geo_rasterizer::download::ensure_geojson;
use geo_rasterizer::entities::continent_code_pub;
use geo_rasterizer::parse::parse_geojson;
use geo_rasterizer::registry::{
    merged_representative_points, register_worldview, to_toml_sorted, Registry,
};

#[derive(Parser, Debug)]
#[command(version, about = "Append-only geo-entity id registry generator")]
struct Args {
    /// Explicit labeled worldview sources: `<worldview-id>:<path>`. When omitted, every
    /// shipped worldview (`Worldview::ALL`) is unioned from repo-relative defaults.
    /// Processed in given order; first source's codes get the lowest ids
    /// (stable).
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

/// Register every ADM0_A3 (and its continent) found in `path` under `worldview`.
fn register_source(reg: &mut Registry, worldview: Worldview, path: &Path) -> Result<()> {
    let features = parse_geojson(path)?;
    let mut items: Vec<(String, bool, geo_types::MultiPolygon<f64>)> = Vec::new();
    for f in &features {
        items.push((
            continent_code_pub(&f.continent).to_string(),
            true,
            f.geometry.clone(),
        ));
        items.push((f.adm0_a3.clone(), false, f.geometry.clone()));
    }
    let points = merged_representative_points(items);
    register_worldview(reg, worldview.spec().id, &points);
    Ok(())
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
        }
    };

    let before = reg.next_id();
    if args.sources.is_empty() {
        // Default: union every shipped worldview from repo-relative defaults,
        // downloading the pinned Natural Earth source if missing.
        for &worldview in Worldview::ALL {
            let path = default_countries(worldview);
            ensure_geojson(&path, worldview)?;
            register_source(&mut reg, worldview, &path)?;
        }
    } else {
        for source in &args.sources {
            // Relies on POSIX repo-relative paths (no Windows drive-letter colons).
            let (worldview_str, path_str) = match source.split_once(':') {
                Some(pair) => pair,
                None => bail!("--source must be in worldview:PATH form, got: {source}"),
            };
            let worldview = Worldview::from_id(worldview_str)?;
            register_source(&mut reg, worldview, &PathBuf::from(path_str))?;
        }
    }

    reg.validate_unique_ids()?;
    let after = reg.next_id();
    std::fs::write(&registry_path, to_toml_sorted(&reg)?)?;
    eprintln!(
        "[registry_gen] {} → {} ids ({} new); wrote {}",
        before,
        after,
        after - before,
        registry_path.display()
    );
    Ok(())
}
