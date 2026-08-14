# geo_rasterizer

Offline build tool. Converts Natural Earth GeoJSON into the geo-reference data
shipped in `app/assets/geo/`:

- `geo_data_<worldview>.bin` — the packed entity/tile data (one per worldview).
- `region_names.<locale>.json` — the localized region-name maps (one per
  locale), resolved from Unicode CLDR (see "Region names" below).

Run via the `app/` Justfile (`just rasterize-geo`); it is not part of the app at
runtime. Both outputs are git-ignored and reproducible from the pinned source.

Three things in this crate are **hand-curated state**, committed as source of
truth: `geo_entity_registry/` (frozen ids), `geo_policy.toml` (editorial
policy), and `geo_names_overrides.toml` (name overrides). This README covers
the registry first, then policy, then names.

## What `geo_entity_registry/` is

It is the **frozen, append-only id registry** for geo entities. It assigns every
entity a small, permanent integer id, and is split into one file per level so a
country-level diff is not drowned by the ~4.6k provinces:

- `continents.toml` — keyed by continent code
- `countries.toml` — keyed by [ADM0_A3](https://en.wikipedia.org/wiki/ISO_3166-1_alpha-3)
  country code (the `ADM0_A3` field in the Natural Earth source)
- `provinces.toml` — keyed by `adm1_code` (the admin-1 source's own code)

Each **country** and **province** entry also stores a `point` — a `[lon, lat]`
representative point (the entity's **union centroid**: its geometry merged across
every worldview), rounded to 4 decimals. It is informational, not a gate — it makes
the committed registry diff carry an identity signal (see [below](#why-it-exists)).
Continents carry no `point` — a continent's identity is its code.

Each file is a standalone document: a `schema` key (format version, currently `1`;
all three must agree) plus one array of inline tables, one line per entity.

```toml
# countries.toml
schema = 1

country = [
  { code = "ARG", id = 7, point = [-65.1731, -35.3787] },
]
```

```toml
# continents.toml — identity is the code, so no point.
schema = 1

continent = [
  { code = "SA", id = 3 },
]
```

Entries are sorted by `code` and points rounded to 4 dp; `id` is always an
explicit field, so sorting never changes an id. The full schema lives in the
`Registry` / `Entry` types in [`src/registry.rs`](src/registry.rs).

Unlike the generated `geo_data_*.bin` files and the downloaded
`natural_earth/*.geojson` sources (both git-ignored), **these TOMLs are committed**
— they are the source of truth.

## Why it exists

The `geo_data_*.bin` files refer to entities by these integer ids, not by name
or code, to stay compact. For that to be safe, **an id must mean the same place
forever**:

- **Stable across source bumps.** When the pinned Natural Earth data is updated,
  a country keeps the id it already had — existing bins and any persisted data
  stay valid.
- **Shared across worldviews.** The `iso`, `chn`, and `usa` worldviews disagree
  on borders, but a given country code resolves to the **same id** in every worldview,
  so per-worldview bins share one id space.

Identity is **not gated automatically**. Instead, each country's `point` is
re-baselined to its current union centroid on every regen, and these TOMLs are
committed — so a code silently reassigned to a different place shows up as a large
`point` move in the registry diff of the source-bump PR. The CI guardrail
(`git diff --exit-code` on the directory, run after `registry-gen`) forces that diff
to be regenerated and reviewed; a reviewer scans it for a gross jump. Rounding to
~11 m keeps benign coastline refinement out of the diff, so only real movement
shows. A border move barely nudges the centroid; a reassignment to another place
moves it tens of degrees.

`assemble_entities` still hard-fails on any source code missing from the registry
(the unknown-code gate) — that is the correctness-critical check and the one thing
that *is* enforced automatically.

The generator is **append-only**: it only ever *adds* ids for codes it has never
seen. It never renumbers or removes existing ids.

## How to update it

Update the registry whenever a new or changed source introduces a country code
the registry has not seen yet (e.g. bumping the Natural Earth pin in
`app/rust/geo_data_format/src/worldview.rs`, or adding a worldview).

From the `app/` directory:

```bash
just registry-gen     # union over every shipped worldview (Worldview::ALL); downloads
                      # the pinned sources if missing, then rewrites every file
```

`just rasterize-geo` depends on `registry-gen`, so the registry is always
brought up to date before any worldview is rasterized — you normally don't need to run
it by hand.

Then **commit the updated `geo_entity_registry/` in the same PR** as the
source/worldview change. Because generation is append-only, the only change should be
newly appended ids; existing ids must not move.

### Direct invocation

```bash
# From this crate dir. No args = union over Worldview::ALL (same as `just registry-gen`).
cargo run --release --bin registry_gen

# Register one specific file under one worldview (paths are repo-relative, POSIX):
cargo run --release --bin registry_gen -- --source iso:natural_earth/<file>.geojson
```

## CI guardrail

CI runs `just rasterize-geo` (which regenerates the registry) and then checks:

```bash
git diff --exit-code tools/geo_rasterizer/geo_entity_registry
```

A non-empty diff fails the build — meaning a source/worldview bump was made without
regenerating and committing the registry. So forgetting this step is caught
automatically rather than silently shipping stale ids.

## Editorial policy (`geo_policy.toml`)

What ships, and at what tier, lives in the committed `geo_policy.toml` — data,
not code, so it is a provenance-hash input like the registry: editing it
rebuilds the bins. Its row shape is level-free (a code's namespace determines
the level it acts on), so future lower-level rows need no schema change. The
tables:

- **`absorb`** — a worldview that does not recognize a territory as a country
  either **demotes** it (the feature survives as an Admin1 entity under
  `into`, keeping its `country.<code>` name key and its own ISO code — the
  prefix and the registry namespace record where a code came from, not what
  tier it occupies) or **merges** it (geometry only, no entity; `attribute_to`
  optionally assigns the land to an admin-1 unit). In both modes the parent's
  polygon gains the geometry: the coarse mask is authoritative in `refine`, so
  a demoted entity outside its parent's mask would be clipped to nothing.
  Merging a `TYPE == "Country"` feature fails the build — that is a
  misclassification, usually from a pin bump.
- **`synthesize`** — the demote-equivalent for territories a worldview's
  admin-0 erased outright, leaving nothing to demote (Taiwan under chn): an
  Admin1 entity under `into` is built from the union of the admin-1 units
  declaring `code`, read ignoring their `FCLASS_*` suppression — which is what
  removed them from that worldview in the first place. `iso_a3_eh` is authored
  because the member units carry none. Fails the build if the code still ships
  as admin-0 here (should be a demote), the target is no country, or no member
  units exist.
- **`merge`** — the admin-1 analogue: features that are not administrative
  units in their own right merge into a real admin-1 unit (the Paracels and
  Spratlys into Hainan, which administers them as Sansha). A merge that
  contributes no block under the target's country mask fails the build (the
  zero-block gate in `main.rs`) — merged features no longer take the parenting
  path where the landless check would have caught them.
- **`drop_admin2`** — admin-2 features present in the admin-1 source (Hong
  Kong's 18 districts) with no tier to live in until ADM2 data ships. Their
  registry ids stay frozen for later re-use.
- **`unparented`** / **`overrule`** — the parenting escape hatches; see the
  ladder below.

One editorial rule is logic rather than a table — the coextensive `+00?` rule
in `apply_admin1_policy`: a `+00?` whole-territory unit that is its territory's
only admin-1 feature, where the territory is already an entity in that
worldview, is an admin-1 leaf identical to its own parent and is dropped. Where
the territory is *not* an entity, the `+00?` is its only representation —
Natural Earth's own admin-1 mechanism (Somaliland, Northern Cyprus, Baikonur) —
and survives. This and demotion are the two routes to the admin-1 tier; the
duality is forced by the source (NE's iso file has no Somaliland admin-0
feature to demote). Because no file input can see a logic change, editing such
a rule bumps `GEO_DATA_VERSION`, whose contract covers format layout and
pipeline logic alike.

### Three removal stages, and where a new rule belongs

| stage | rules | question answered |
| --- | --- | --- |
| parse (`admin1.rs`) | `+99?` remainder, `FCLASS_*` | is this an admin-1 unit at all, per Natural Earth? |
| editorial (`apply_admin1_policy`, `absorb`) | the tables above | should it ship, and at what tier? |
| post-raster (`resolve_province_parents`) | `unparented`, `overrule` | did the mask leave it any land, and to whom? |

The placement test: **does `registry_gen` need to see it?** Parse-stage rules
are visible to it and shape the frozen registry. Editorial rules must not be —
ids keep being minted for every source feature, so a reversed decision costs no
registry diff. Post-raster rules cannot run earlier: they depend on coverage
that does not exist until the rasters are merged. Every code a curated table
lists must exist post-policy in its worldview — enforced by
`validate_curated_tables` via the real-source tests, because the stale-*apply*
checks inside the resolver never visit a code that vanished entirely.

### The parenting ladder

Which country owns a province, in precedence order:

1. `overrule` — a reviewed override, where the mask covers the province in
   full;
2. the declared `adm0_a3` where it names a country here — for a demoted or
   synthesized territory this is its policy-row `into`;
3. `block_majority` over the country mask;
4. landless — dropped if listed in `unparented`, a hard error otherwise.

## Region names (`geo_names_overrides.toml`)

Each entity carries its display name as an l10n *key*, not a string — `entities.rs`
mints `continent.<code>` / `country.<ADM0_A3>` / `province.<adm1_code>` into
the `.bin` (a demoted or synthesized territory keeps its `country.` key — the
prefix names the source namespace, not the tier). The
rasterizer resolves those keys to display strings and writes one
`region_names.<locale>.json` per locale (`app/assets/geo/`), nested like the UI
translation files (`{"country": {"CHN": …}}`), which the app merges
into easy_localization via a custom `AssetLoader` — so a region name resolves
through the same `.tr()` path as every other string (see
`app/lib/common/app_translation_loader.dart`, `RegionEntity.displayName`).

The app never sees a name key as a bare `String`: `RegionEntity.nameKey` is a
`RegionNameKey` wrapper, so `entity.nameKey.tr()` doesn't compile. That forces
resolution through `RegionEntity.displayName(worldviewId)`, the one place that
unwraps `.value` and prefers a worldview-scoped override — a raw `.tr()` would
silently skip it.

Resolution per name, in order (continents resolve by override alone):

1. worldview-scoped override → a `<worldview>.<name_key>` key (see below),
2. worldview-agnostic override,
3. `country.*`: the CLDR territory name for the group's sovereign member's
   `ISO_A2_EH`; `province.*`: the CLDR subdivision name for its `iso_3166_2`,
   then Natural Earth's `name_en` / `name_zh`,
4. hard error — never a silent English fallback.

**Names come from CLDR, not Natural Earth.** Unicode CLDR
(`cldr-localenames-full`, pinned in `geo_data_format::cldr`) is the canonical,
per-locale authority for territory names, keyed by ISO 3166-1 alpha-2. Natural
Earth supplies only the geometry and the `ISO_A2_EH` code that joins a feature to
its CLDR name. A CLDR name depends on the alpha-2 alone, so a country resolves to
the same name across every worldview by construction — the map is keyed by
`name_key`, unioned across worldviews, and a given `ADM0_A3` must carry the same
`ISO_A2_EH` in every worldview (generation fails otherwise).

**Collision gate.** Natural Earth stamps some sub-features (disputed regions,
sovereign bases, outlying islands) with their *sovereign's* `ISO_A2_EH`, so CLDR
would name each after its sovereign — two distinct regions both reading
"Georgia". Generation requires at most one CLDR-resolved entity per alpha-2; the
sub-features must carry an override, or the build fails listing the collision.
The coverage gate can't catch this (the name is non-empty), so this gate does.

`geo_names_overrides.toml` is the names pipeline's only hand-authored part. An
override exists where CLDR cannot give the name we ship:

- **No CLDR territory / no alpha-2.** Continents are synthesized (no feature, so
  **every** continent name is authored here); NE-only aggregates (the Spratlys)
  and `-99`-sentinel entities (Bir Tawil, the Cyprus base areas) have no CLDR
  entry.
- **Sub-feature sharing a sovereign's alpha-2** (see the collision gate) —
  Abkhazia, South Ossetia, Clipperton Island, ….
- **CLDR's name is not the form we ship** — e.g. CLDR's verbose "Hong Kong SAR
  China" shortened to "Hong Kong".

A key is a locale string; a per-worldview override is a sub-table:

```toml
["country.HKG"]        # default: every worldview
en-US = "Hong Kong"

["country.AAA".chn]    # chn worldview only; emitted as `chn.country.AAA`
zh-CN = "…"
```

### Regenerating after an overrides edit

`just rasterize-geo` — the names pass always reruns and picks up the edit (the
`.bin`s hash the geojson sources + registry + `geo_policy.toml`, not the name
overrides, so they skip; CLDR sources are downloaded on demand and verified
against the pin, same as the geojson).
Generation reports every unresolved gap and every alpha-2 collision in one run,
so overrides can be authored in one pass. Then `just test-geo` runs the coverage
gate (`tests/names_coverage.rs`): every entity in every worldview must resolve to
a non-empty name in every locale. Commit only the `.toml` — the JSON are
git-ignored build artifacts.

## Future work

- **Per-worldview names.** The worldview-scoped override path
  (`["…".<worldview>]` → a `<worldview>.<name_key>` key the app prefers) is
  implemented but **unused** — no shipped name differs by worldview yet. It
  exists for disputed regions that legitimately carry a different name per
  political view (e.g. Arunachal Pradesh vs Zangnan in the chn worldview).
- **ADM2.** The tier below admin-1 (`GeoEntityKind::Admin2`) is plumbed but
  empty — no source ships. Landing it re-adds `drop_admin2`'s Hong Kong
  districts under their frozen ids and reuses the tier-agnostic refine/area
  machinery with the admin-1 raster as the coarse side.
