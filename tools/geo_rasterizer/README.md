# geo_rasterizer

Offline build tool. Converts Natural Earth GeoJSON into the `geo_data_*.bin`
geo-reference data shipped in `app/assets/`. Run via the `app/` Justfile
(`just rasterize-geo`); it is not part of the app at runtime.

This README focuses on **`geo_entity_registry.toml`**, the one file in this
crate that is hand-curated state rather than a pure build artifact.

## What `geo_entity_registry.toml` is

It is the **frozen, append-only id registry** for geo entities. It assigns every
entity a small, permanent integer id:

- **continents** — keyed by continent code
- **countries** — keyed by [ADM0_A3](https://en.wikipedia.org/wiki/ISO_3166-1_alpha-3)
  country code (the `ADM0_A3` field in the Natural Earth source)

Each entry also stores a **representative point** — a `[lon, lat]` anchor (the
centroid of the entity's merged geometry) used by the identity audit (see
below). Points are kept **per POV**, because borders differ between worldviews.

Top level:

- `schema` — format version (currently `1`).
- `povs` — the POV universe, e.g. `["chn", "iso", "usa"]`. A bare `ref` with no
  per-entry `pov` list means "this point applies to every POV in `povs`".

Each `[[continent]]` / `[[country]]` entry is written in the most compact form
that is lossless, so a no-op source bump produces a zero-line diff:

```toml
# Same point in every POV → one inline ref.
[[country]]
code = "ARG"
id = 7
ref = [-65.1731, -35.3787]

# Present in only some POVs → ref + the covered subset.
[[country]]
code = "TWN"
id = 183
ref = [120.9499, 23.753]
pov = ["iso", "usa"]

# Genuinely different point per POV → explicit refs sub-table.
[[country]]
code = "CHN"
id = 18
[country.refs]
chn = [103.8162, 36.4588]
iso = [103.9277, 36.5645]
usa = [103.827, 36.5584]
```

Entries are sorted by `code` and points rounded to 4 dp; `id` is always an
explicit field, so sorting/rounding never changes an id. The full schema lives
in the `Registry` / `Entry` types in [`src/registry.rs`](src/registry.rs).

Unlike the generated `geo_data_*.bin` files and the downloaded
`natural_earth/*.geojson` sources (both git-ignored), **this TOML is committed**
— it is the source of truth.

## Why it exists

The `geo_data_*.bin` files refer to entities by these integer ids, not by name
or code, to stay compact. For that to be safe, **an id must mean the same place
forever**:

- **Stable across source bumps.** When the pinned Natural Earth data is updated,
  a country keeps the id it already had — existing bins and any persisted data
  stay valid.
- **Shared across worldviews (POVs).** The `iso`, `chn`, and `usa` POVs disagree
  on borders, but a given country code resolves to the **same id** in every POV,
  so per-POV bins share one id space.

To enforce "same id ⇒ same place", the registry stores the representative point
and the rasterizer runs an **identity audit** (`audit_identity` in
`src/main.rs`): if a code's location in a new source/POV drifts more than ~8°
from the registry's anchor, the build fails. That catches a code being silently
reassigned to a different place.

This is why the generator is **append-only**: it only ever *adds* ids for codes
it has never seen. It never renumbers or removes existing ids.

## How to update it

Update the registry whenever a new or changed source introduces a country code
the registry has not seen yet (e.g. bumping the Natural Earth pin in
`app/rust/geo_data_format/src/pov.rs`, or adding a POV).

From the `app/` directory:

```bash
just registry-gen     # union over every shipped POV (Pov::ALL); downloads
                      # the pinned sources if missing, then rewrites the TOML
```

`just rasterize-geo` depends on `registry-gen`, so the registry is always
brought up to date before any POV is rasterized — you normally don't need to run
it by hand.

Then **commit the updated `geo_entity_registry.toml` in the same PR** as the
source/POV change. Because generation is append-only, the only change should be
newly appended ids; existing ids must not move.

### Direct invocation

```bash
# From this crate dir. No args = union over Pov::ALL (same as `just registry-gen`).
cargo run --release --bin registry_gen

# Register one specific file under one POV (paths are repo-relative, POSIX):
cargo run --release --bin registry_gen -- --source iso:natural_earth/<file>.geojson
```

## CI guardrail

CI runs `just rasterize-geo` (which regenerates the registry) and then checks:

```bash
git diff --exit-code tools/geo_rasterizer/geo_entity_registry.toml
```

A non-empty diff fails the build — meaning a source/POV bump was made without
regenerating and committing the registry. So forgetting this step is caught
automatically rather than silently shipping stale ids.
