# Natural Earth — admin 0 countries (ISO worldview)

Source: <https://www.naturalearthdata.com/downloads/10m-cultural-vectors/10m-admin-0-countries/>
Repository: <https://github.com/nvkelso/natural-earth-vector>
File: `ne_10m_admin_0_countries_iso.geojson`
Resolution: 1:10m
Worldview: ISO/UN-recognized perspective

## License

Natural Earth data is in the **public domain**. See the project license:
<https://www.naturalearthdata.com/about/terms-of-use/>.

## Why this file

This tool rasterizes country polygons into a tile/block lookup table. The
"iso" variant is the most diplomatically neutral perspective shipped with
Natural Earth.

## Where the pin lives

The exact upstream commit, raw URL base, and per-worldview SHA-256s are pinned in
`app/rust/geo_data_format/src/worldview.rs`:

- `NATURAL_EARTH_COMMIT` — git SHA on `nvkelso/natural-earth-vector`
- `NATURAL_EARTH_BASE` — raw.githubusercontent.com base URL at that commit
- `Worldview::spec().source_sha256` — SHA-256 of each worldview file's raw bytes

The download/verify logic lives in `tools/geo_rasterizer/src/download.rs`.

The files themselves are **not checked into the repo** — `just rasterize-geo`
(run as part of `just pre-build`) downloads them on demand to this directory
and verifies the hashes. The rasterized `app/assets/geo/geo_data_*.bin` outputs are
also generated, not committed.

Two caches make repeat runs fast:

1. **GeoJSON hash check** — skips download if the local file already matches
   the worldview's `source_sha256`.
2. **Bin hash check** — skips re-rasterization if the existing
   `geo_data_<id>.bin` embeds a provenance hash matching the current GeoJSON +
   registry + worldview id.

## Updating the pin

1. Pick a new commit on `nvkelso/natural-earth-vector`.
2. Fetch each worldview file from the new commit's raw URL and recompute its SHA-256
   (`curl -sL "$NATURAL_EARTH_BASE/<file>" | sha256sum`).
3. Update `NATURAL_EARTH_COMMIT`, `NATURAL_EARTH_BASE`, and each
   `source_sha256` in `app/rust/geo_data_format/src/worldview.rs`.
4. Run `just rasterize-geo` and review the diff in `app/assets/geo/geo_data_*.bin`
   (entity IDs, areas, and border tiles can shift).
