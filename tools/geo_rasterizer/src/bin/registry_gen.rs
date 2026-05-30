//! Bootstrap / extend the frozen geo-entity id registry.
//!
//! APPEND ONLY: run this when a new POV GeoJSON introduces an ADM0_A3 the
//! registry has never seen. It never renumbers or removes ids. Commit the
//! resulting geo_entity_registry.toml in the same PR as the source bump.
//!
//! TODO(geo-C): Phase 2 passes every shipped POV file here so the registry
//! is the union across all worldviews before any delta is built.

use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::Parser;
use geo_data_format::Pov;
use geo_rasterizer::entities::continent_code_pub;
use geo_rasterizer::parse::parse_geojson;
use geo_rasterizer::registry::{
    merged_representative_points, register_pov, to_toml_sorted, Registry,
};

#[derive(Parser, Debug)]
#[command(version, about = "Append-only geo-entity id registry generator")]
struct Args {
    /// One or more labeled POV sources: `<pov-id>:<path>`. Processed in
    /// given order; first source's codes get the lowest ids (stable).
    #[arg(long = "source", value_name = "POV:PATH", num_args = 1.., required = true)]
    sources: Vec<String>,
    /// Registry TOML to create or extend.
    #[arg(long, default_value = "geo_entity_registry.toml")]
    registry: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut reg = if args.registry.exists() {
        Registry::load(&args.registry)?
    } else {
        Registry {
            schema: 1,
            continents: vec![],
            countries: vec![],
        }
    };

    let before = reg.next_id();
    for source in &args.sources {
        // Relies on POSIX repo-relative paths (no Windows drive-letter colons).
        let (pov_str, path_str) = match source.split_once(':') {
            Some(pair) => pair,
            None => bail!("--source must be in POV:PATH form, got: {source}"),
        };
        let pov = Pov::from_id(pov_str)?;
        let path = PathBuf::from(path_str);
        let features = parse_geojson(&path)?;
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
        register_pov(&mut reg, pov.spec().id, &points);
    }

    reg.validate_unique_ids()?;
    let after = reg.next_id();
    std::fs::write(&args.registry, to_toml_sorted(&reg)?)?;
    eprintln!(
        "[registry_gen] {} → {} ids ({} new); wrote {}",
        before,
        after,
        after - before,
        args.registry.display()
    );
    Ok(())
}
