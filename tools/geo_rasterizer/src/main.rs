use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Result;
use clap::Parser;
use geo_data_format::{Locale, Worldview};
use geo_rasterizer::{
    admin0::parse_admin0,
    admin1::parse_admin1,
    batch::{rasterize_batch, RasterizeMode, RasterizePaths},
    cldr::{load_subdivisions, load_territories},
    download::{ensure_cldr, ensure_cldr_subdivisions},
    entities::apply_admin1_policy,
    names::{build_region_names, write_region_names},
    overrides::Overrides,
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

    /// Override the frozen geo-entity id registry directory. Requires `--worldview`.
    #[arg(long, requires = "worldview")]
    registry: Option<PathBuf>,

    /// Override the directory holding the admin-2 snapshot and its province map.
    /// Requires `--worldview`.
    #[arg(long, requires = "worldview")]
    admin2_dir: Option<PathBuf>,

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

    /// Write a separate display asset including all worldviews.
    #[arg(long, conflicts_with_all = ["worldview", "download_only"])]
    display_output: Option<PathBuf>,
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
    manifest().join("geo_entity_registry")
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

fn generate_region_names(ensure_source: bool, admin2_dir: &Path) -> Result<()> {
    let overrides = Overrides::load(&default_overrides())?;
    let admin2 = geo_rasterizer::admin2::load(admin2_dir)?;
    let mut by_worldview = Vec::new();
    for &worldview in Worldview::ALL {
        let path = default_countries(worldview);
        by_worldview.push((worldview, parse_admin0(&path, worldview.spec().id)?));
    }
    let mut admin1_by_worldview = Vec::new();
    for (worldview, features) in &by_worldview {
        admin1_by_worldview.push((
            *worldview,
            apply_admin1_policy(
                parse_admin1(&default_admin1(), *worldview)?,
                *worldview,
                features,
            )?
            .features,
        ));
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
        &admin2,
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
    let admin2_dir = args
        .admin2_dir
        .unwrap_or_else(geo_rasterizer::admin2::default_dir);
    let mode = if args.download_only {
        RasterizeMode::DownloadSources
    } else if let Some(path) = &args.display_output {
        RasterizeMode::WithDisplay {
            ensure_source: args.ensure_source,
            display_output: path,
        }
    } else {
        RasterizeMode::RasterOnly {
            ensure_source: args.ensure_source,
        }
    };
    match &args.worldview {
        Some(id) => {
            let worldview = Worldview::from_id(id)?;
            rasterize_batch(
                [(
                    worldview,
                    RasterizePaths {
                        countries: args
                            .countries
                            .unwrap_or_else(|| default_countries(worldview)),
                        admin1: args.admin1.unwrap_or_else(default_admin1),
                        registry: args.registry.unwrap_or_else(default_registry),
                        admin2_dir,
                        output: args.output.unwrap_or_else(|| default_output(worldview)),
                    },
                )],
                mode,
            )?;
        }
        None => {
            rasterize_batch(
                Worldview::ALL.iter().map(|&worldview| {
                    (
                        worldview,
                        RasterizePaths {
                            countries: default_countries(worldview),
                            admin1: default_admin1(),
                            registry: default_registry(),
                            admin2_dir: admin2_dir.clone(),
                            output: default_output(worldview),
                        },
                    )
                }),
                mode,
            )?;
            if !args.download_only {
                generate_region_names(args.ensure_source, &admin2_dir)?;
            }
        }
    }
    eprintln!("[geo_rasterizer] done in {:.1?}", started.elapsed());
    Ok(())
}
