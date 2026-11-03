#!/usr/bin/env python3
"""Slice CHN admin-2 from a pinned Overture divisions release into the
committed, zstd-compressed snapshot geojson. Deps: pyarrow>=16,
shapely>=2.1, zstandard.
Usage: python3 snapshot.py --release 2026-10-21.0
"""
import argparse
import json
import sys
import tomllib
from pathlib import Path

import pyarrow.dataset as ds
import pyarrow.fs as pafs
import shapely
import shapely.wkb
import zstandard

HERE = Path(__file__).parent
OUT = HERE / "../geo_rasterizer/admin2/chn_admin2.geojson.zst"
TOLERANCE_DEG = 0.002  # ≈200 m: max deviation stays under half a z16 block
# ≈11 m. Snapping spends a small fraction of the half-block budget the
# simplification already uses; a 1e-3 grid would add up to ~30% of it at
# China's northern edge.
PRECISION_DEG = 1e-4
ZSTD_LEVEL = 19

# CHN admin-2 members live in two Overture division subtypes: `county` (地级
# divisions — prefecture-level cities, autonomous prefectures, leagues,
# prefectures) and `localadmin` (the 32 省直辖县级 divisions administered
# directly by their province, which Overture files one tier down from
# `county`).
SUBTYPES = ["county", "localadmin"]


def load_expected() -> dict[str, dict]:
    """adcode -> {adcode, name_zh, name_en, wikidata, [gers_id]} from
    `expected_members.toml`."""
    with open(HERE / "expected_members.toml", "rb") as f:
        doc = tomllib.load(f)
    return {row["adcode"]: row for row in doc["member"]}


def common_names(names: dict | None) -> dict[str, str]:
    """`division.names.common` decodes from Arrow's map type as a list of
    (language, value) pairs, not a dict — collapse it to a dict keyed by
    language tag."""
    if not names:
        return {}
    return dict(names.get("common") or [])


def fetch_divisions(fs: pafs.S3FileSystem, release: str) -> list[dict]:
    """CN `county`/`localadmin` division rows: id + names, the join inputs."""
    base = f"overturemaps-us-west-2/release/{release}/theme=divisions/type=division"
    dataset = ds.dataset(base, filesystem=fs)
    flt = (ds.field("country") == "CN") & ds.field("subtype").isin(SUBTYPES)
    cols = ["id", "subtype", "names"]
    have = [c for c in cols if c in dataset.schema.names]
    return dataset.to_table(filter=flt, columns=have).to_pylist()


def fetch_areas(fs: pafs.S3FileSystem, release: str) -> list[dict]:
    """CN `county`/`localadmin` division_area rows: division_id + geometry.
    Every coastal division carries a second `is_land=False` row for its
    territorial-sea claim (bounds run tens of km offshore) alongside its
    `is_land=True` land boundary; keep only the land rows or the union below
    bakes maritime claims into the admin-2 polygons. A division can still have
    more than one land row (multi-part/archipelago coverage); all are unioned
    per adcode in `build_features`."""
    base = f"overturemaps-us-west-2/release/{release}/theme=divisions/type=division_area"
    dataset = ds.dataset(base, filesystem=fs)
    # `ds.field(...) == True` builds a pyarrow comparison expression, not a
    # Python bool check, so `is True` here would be a bug (and `.isin(True)`
    # doesn't apply to a boolean field).
    flt = (
        (ds.field("country") == "CN")
        & ds.field("subtype").isin(SUBTYPES)
        & (ds.field("is_land") == True)
    )
    cols = ["id", "division_id", "subtype", "geometry"]
    have = [c for c in cols if c in dataset.schema.names]
    return dataset.to_table(filter=flt, columns=have).to_pylist()


def resolve_adcode(
    div: dict,
    by_gers: dict[str, str],
    by_name: dict[str, str],
) -> str | None:
    """Map an Overture division to its adcode in `expected`, or None if it's
    not a member (para-zones, ordinary districts, out-of-scope localadmin
    units). A `gers_id` pin in `expected_members.toml` always wins; otherwise
    fall back to an exact match on the Chinese common name."""
    pinned = by_gers.get(div["id"])
    if pinned is not None:
        return pinned
    zh = common_names(div["names"]).get("zh") or (div["names"] or {}).get("primary")
    return by_name.get(zh)


def build_features(divisions: list[dict], areas: list[dict], expected: dict[str, dict]):
    by_gers = {row["gers_id"]: adcode for adcode, row in expected.items() if "gers_id" in row}
    by_name = {row["name_zh"]: adcode for adcode, row in expected.items()}

    div_by_id = {}
    for div in divisions:
        adcode = resolve_adcode(div, by_gers, by_name)
        if adcode is not None:
            div_by_id[div["id"]] = (adcode, div)

    feats: dict[str, dict] = {}
    for row in areas:
        hit = div_by_id.get(row["division_id"])
        if hit is None:
            continue
        adcode, div = hit
        geom = shapely.wkb.loads(row["geometry"])
        feats.setdefault(adcode, {"div": div, "geoms": []})["geoms"].append(geom)

    missing = sorted(set(expected) - set(feats))
    extra = sorted(set(feats) - set(expected))
    if missing or extra:
        sys.exit(f"membership gate failed: missing={missing} extra={extra}")
    return feats


def write_out(feats: dict[str, dict], expected: dict[str, dict], release: str):
    order = sorted(feats)
    merged = {adcode: shapely.union_all(feats[adcode]["geoms"]) for adcode in order}
    simplified = shapely.coverage_simplify([merged[a] for a in order], TOLERANCE_DEG)

    out = {
        "type": "FeatureCollection",
        "overture_release": release,
        "tolerance_deg": TOLERANCE_DEG,
        "features": [],
    }
    for adcode, geom in zip(order, simplified):
        div = feats[adcode]["div"]
        geom = shapely.geometry.mapping(shapely.set_precision(geom, PRECISION_DEG))
        out["features"].append(
            {
                "type": "Feature",
                "properties": {
                    "adcode": adcode,
                    "gers_id": div["id"],
                    "name_zh": expected[adcode]["name_zh"],
                    "name_en": expected[adcode]["name_en"],
                },
                "geometry": geom,
            }
        )
    raw = (json.dumps(out, ensure_ascii=False, separators=(",", ":")) + "\n").encode()
    OUT.write_bytes(zstandard.ZstdCompressor(level=ZSTD_LEVEL).compress(raw))
    print(f"wrote {OUT.resolve()} ({OUT.stat().st_size / 1e6:.1f} MB, {len(order)} features)")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--release", required=True)
    args = ap.parse_args()

    expected = load_expected()
    fs = pafs.S3FileSystem(anonymous=True, region="us-west-2")
    divisions = fetch_divisions(fs, args.release)
    areas = fetch_areas(fs, args.release)
    feats = build_features(divisions, areas, expected)
    write_out(feats, expected, args.release)


if __name__ == "__main__":
    main()
