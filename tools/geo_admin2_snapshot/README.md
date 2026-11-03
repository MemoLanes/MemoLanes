# geo_admin2_snapshot

Slices CHN admin-2 (prefecture-level + 省直辖县级) polygons out of a pinned
[Overture Maps](https://overturemaps.org/) `divisions` release on S3 and writes
the committed snapshot `tools/geo_rasterizer/admin2/chn_admin2.geojson.zst` that
`geo_rasterizer::admin2` reads: compact GeoJSON, coordinates snapped to a 1e-4°
grid, zstd-compressed. Inspect it with `zstd -dc chn_admin2.geojson.zst`.

Overture keeps only its latest few releases on S3, so an older pin cannot be
re-fetched — the committed snapshot is the only copy of the release it names.

Run by hand, offline from the app build. Only re-run when deliberately
refreshing the pinned release.

## Deps

```bash
python3 -c "import pyarrow, shapely, zstandard" || pip install pyarrow 'shapely>=2.1' zstandard
```

Needs `shapely>=2.1` for `coverage_simplify`. Reads the Overture S3 bucket
anonymously (`overturemaps-us-west-2`, region `us-west-2`) — no credentials
required, just network access.

## Usage

```bash
python3 snapshot.py --release 2026-10-21.0
```

Takes a couple of minutes (the `division_area` scan over CN `county` +
`localadmin` rows dominates). On success it writes
`../geo_rasterizer/admin2/chn_admin2.geojson.zst` and prints its size and feature
count.

The script enforces a **membership gate**: the output must contain exactly
the 364 adcodes listed in `expected_members.toml`, no more, no fewer. If an
Overture release renames, splits, or drops a division such that the gate
can't resolve it, the script exits with `missing=[...] extra=[...]` instead
of writing a wrong snapshot.

## `expected_members.toml`

The human-reviewed list of the 364 CHN admin-2 units this snapshot covers: the
333 GB/T 2260 prefecture-level (地级) divisions minus 三沙市 (absent from
Overture's CN division data entirely), plus the 32 省直辖县级 county-level
divisions administered directly by their province. Sourced from MCA
administrative division data via DataV.GeoAtlas, plus 新星市 and 白杨市
(founded after that data), and reviewed by eye against MCA's published
行政区划代码.

Each row is `{ adcode, name_zh, name_en, wikidata }`. Both names ship as-is;
Overture's own English names are not used, being inconsistent across units.
`name_en` follows the unit's English Wikipedia article (`wikidata`) in the
style the file header describes. `snapshot.py` joins each row against
Overture by exact match on `division.names.common["zh"]`. If a future release makes that
join ambiguous or fails for a given row (renamed alternate name, homonym
elsewhere in China), pin it explicitly by adding a `gers_id` field with that
division's Overture `division.id` — pins always take priority over the name
match. As of the `2026-10-21.0` release no row needs a pin.

## Refresh flow

1. Bump `--release` to the new pinned date and re-run.
2. If the membership gate fails, resolve each `missing`/`extra` adcode by
   updating `expected_members.toml` (a genuine rename/split needs a human
   decision, not a code change) and re-run.
3. Review the change against the previous snapshot (decompress both) —
   feature count, size, names, and a few cities' shapes. `TOLERANCE_DEG` and
   `PRECISION_DEG` are bounded by the raster's half-block error budget, so
   don't raise them to shrink the file.
4. Include the gate output (or its absence of complaints) and the diff
   summary in the PR description.

## Known upstream data quality caveat

Overture's `division_area` land geometry (seen in `2026-08-19.0`, unchanged
in `2026-10-21.0`) is not fully
reliable for every unit: a handful of divisions (confirmed: Shenzhen 4403,
~19% of its reported area; suspected in several Xinjiang Production and
Construction Corps cities, e.g. 铁门关市/659006) carry extra polygon
fragments tens to hundreds of km from the unit's main body that don't match
MCA/DataV's boundary. This was found by comparing every feature's bounding
box against DataV reference data by hand, not by any check this script runs.
It doesn't fail the membership gate (which only checks *which* adcodes are
present, not the fidelity of each polygon) and isn't corrected here. A future
geometry-fidelity pass, if wanted, would need to land as its own tool rather
than a heuristic bolted onto this one.
