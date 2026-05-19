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

The exact upstream commit, raw URL, and SHA-256 are pinned as constants in
`tools/geo_rasterizer/src/download.rs`:

- `NATURAL_EARTH_COMMIT` — git SHA on `nvkelso/natural-earth-vector`
- `NATURAL_EARTH_URL` — raw.githubusercontent.com URL at that commit
- `NATURAL_EARTH_SHA256` — SHA-256 of the file's raw bytes

The file itself is **not checked into the repo** — `just rasterize-geo` (run
as part of `just pre-build`) downloads it on demand to this directory and
verifies the hash. The rasterized output `app/assets/geo_data.bin` is also
generated, not committed.

Two caches make repeat runs fast:

1. **GeoJSON hash check** — skips download if the local file already matches
   `NATURAL_EARTH_SHA256`.
2. **Bin hash check** — skips re-rasterization if the existing
   `geo_data.bin` embeds an input hash matching the current GeoJSON +
   `worldviews.toml`.

## Updating the pin

1. Pick a new commit on `nvkelso/natural-earth-vector`.
2. Fetch the file from the new commit's raw URL and recompute its SHA-256.
3. Update all three constants in `tools/geo_rasterizer/src/download.rs`.
4. Run `just rasterize-geo` and review the diff in `app/assets/geo_data.bin`
   (entity IDs, areas, and border tiles can shift).
