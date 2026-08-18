//! Assertions against the REAL pinned admin-1 source. Requires
//! `just fetch-natural-earth`; a missing source is a hard failure because a
//! gate that skips is a gate that passes.

use std::path::{Path, PathBuf};

use geo_data_format::Worldview;
use geo_rasterizer::admin1::{is_unassigned_remainder, parse_admin1};

fn source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("natural_earth")
        .join(geo_data_format::ADMIN1_SOURCE_FILENAME)
}

#[test]
fn filters_select_the_expected_counts() {
    let path = source();
    assert!(
        path.exists(),
        "{} is absent — run `just fetch-natural-earth`",
        path.display()
    );
    assert_eq!(parse_admin1(&path, Worldview::Iso).unwrap().len(), 4587);
    assert_eq!(parse_admin1(&path, Worldview::Usa).unwrap().len(), 4587);
    assert_eq!(parse_admin1(&path, Worldview::Chn).unwrap().len(), 4534);
}

#[test]
fn the_sentinel_predicate_still_selects_exactly_the_nameless_features() {
    // The `<ADM0>+99?` rule is a convention, not a guarantee. If a pin bump
    // ever decouples it from namelessness, this fails rather than shipping a
    // province with no name or dropping a real one.
    let raw = std::fs::read_to_string(source()).unwrap();
    let collection: geojson::FeatureCollection = serde_json::from_str(&raw).unwrap();
    let mut sentinels = 0;
    let mut nameless = 0;
    for f in &collection.features {
        let props = f.properties.as_ref().unwrap();
        let code = props.get("adm1_code").and_then(|v| v.as_str()).unwrap();
        let has_name = props.get("name_en").and_then(|v| v.as_str()).is_some();
        if is_unassigned_remainder(code) {
            sentinels += 1;
            assert!(!has_name, "{code} is a sentinel but carries a name");
        }
        if !has_name {
            nameless += 1;
            assert!(
                is_unassigned_remainder(code),
                "{code} is nameless but not a sentinel"
            );
        }
    }
    assert_eq!(sentinels, 7);
    assert_eq!(nameless, 7);
}

#[test]
fn adm1_code_is_unique() {
    // Over the RAW collection, not a worldview-filtered parse. The registry maps
    // `adm1_code → id` for the union across worldviews, and `collect_provinces`
    // merges geometry under that key, so a duplicate among features that pass
    // only the usa or only the chn POV column would silently fuse two distinct
    // provinces into one permanent id with one name. Checking the raw features
    // also covers codes that pass no column at all, so no future filter change
    // can narrow this gate.
    let raw = std::fs::read_to_string(source()).unwrap();
    let collection: geojson::FeatureCollection = serde_json::from_str(&raw).unwrap();
    let mut codes: Vec<&str> = collection
        .features
        .iter()
        .map(|f| {
            f.properties
                .as_ref()
                .and_then(|props| props.get("adm1_code"))
                .and_then(|v| v.as_str())
                .expect("every admin-1 feature carries an adm1_code")
        })
        .collect();
    assert_eq!(codes.len(), 4596, "raw admin-1 feature count changed");
    codes.sort_unstable();
    let before = codes.len();
    codes.dedup();
    assert_eq!(
        before,
        codes.len(),
        "adm1_code must be unique — it is the identity key"
    );
}

/// The shipped bins must contain the province counts the design predicts.
/// Requires `just rasterize-geo`, which `just test-geo` runs first.
///
/// These are the parsed counts minus the editorial drops (geo_policy.toml
/// `drop_admin1_in` −1075 everywhere; the coextensive sole-unit dedup −28 iso /
/// −33 chn / −35 usa) and minus the provinces whose parent country holds none
/// of their land (geo_policy.toml `unparented`): iso drops 9 and usa drops 1,
/// because Natural Earth's iso admin-0 excises disputed land from both
/// claimants — minus chn's two `merge` rows into Hainan, plus the two
/// territories it demotes (Hong Kong, Macau) and its synthesized Taiwan entity.
#[test]
fn shipped_bins_carry_the_expected_province_counts() {
    use geo_data_format::{read_geo_data, GeoEntityKind};

    for (worldview, expected) in [
        (Worldview::Iso, 3475usize),
        (Worldview::Usa, 3476),
        (Worldview::Chn, 3427),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../app/assets/geo")
            .join(format!("geo_data_{}.bin", worldview.spec().id));
        assert!(
            path.exists(),
            "{} is absent — run `just rasterize-geo`",
            path.display()
        );
        let data = read_geo_data(&std::fs::read(&path).unwrap()).unwrap();
        let provinces = data
            .entities
            .iter()
            .filter(|e| matches!(e.kind, GeoEntityKind::Admin1))
            .count();
        assert_eq!(provinces, expected, "{}", worldview.spec().id);

        // Every province must be parented to a country that exists in this bin.
        let countries: std::collections::BTreeSet<_> = data
            .entities
            .iter()
            .filter(|e| matches!(e.kind, GeoEntityKind::Admin0))
            .map(|e| e.id)
            .collect();
        for province in data
            .entities
            .iter()
            .filter(|e| matches!(e.kind, GeoEntityKind::Admin1))
        {
            let parent = province.parent_id.expect("province must have a parent");
            assert!(
                countries.contains(&parent),
                "{}: province {} is parented to {parent:?}, which is not a country in this bin",
                worldview.spec().id,
                province.canonical_code
            );
            assert!(
                province.total_area_m2 > 0,
                "{}: province {} has zero area",
                worldview.spec().id,
                province.canonical_code
            );
        }
    }
}

/// Every code the curated tables list must exist in the worldview it is listed
/// for — the gate that catches an NE pin bump (or an editorial change)
/// orphaning table entries, which the stale-skip checks cannot see (they only
/// visit codes present in `province_ids`).
#[test]
fn every_curated_table_entry_names_a_code_that_ships() {
    use geo_rasterizer::admin0::parse_admin0;
    use geo_rasterizer::entities::{apply_admin1_policy, validate_curated_tables};
    use std::collections::BTreeSet;

    for &worldview in Worldview::ALL {
        // Post-policy, matching the pipeline: the gate's contract is "listed
        // codes survive the editorial step".
        let admin0 = parse_admin0(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("natural_earth")
                .join(worldview.spec().source_filename),
            worldview.spec().id,
        )
        .unwrap();
        let codes: Vec<String> = apply_admin1_policy(
            parse_admin1(&source(), worldview).unwrap(),
            worldview,
            &admin0,
        )
        .unwrap()
        .features
        .into_iter()
        .map(|f| f.adm1_code)
        .collect();
        let live: BTreeSet<&str> = codes.iter().map(String::as_str).collect();
        validate_curated_tables(worldview, &live)
            .unwrap_or_else(|e| panic!("{}: {e}", worldview.spec().id));
    }
}

/// Every province `canonical_code` in the worldview's shipped bin.
fn shipped_provinces(worldview: Worldview) -> Vec<String> {
    use geo_data_format::{read_geo_data, GeoEntityKind};
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../app/assets/geo")
        .join(format!("geo_data_{}.bin", worldview.spec().id));
    assert!(
        path.exists(),
        "{} is absent — run `just rasterize-geo`",
        path.display()
    );
    read_geo_data(&std::fs::read(&path).unwrap())
        .unwrap()
        .entities
        .into_iter()
        .filter(|e| matches!(e.kind, GeoEntityKind::Admin1))
        .map(|e| e.canonical_code)
        .collect()
}

/// A country `drop_admin1_in` lists must ship no province at all, in any
/// worldview — the drop is all-or-nothing, so a survivor means a hole rendering
/// as the country beside a sibling rendering as a province.
///
/// Hong Kong is the case that named the rule: its districts are one tier below
/// the SAR, and under `chn` they used to land as siblings of Guangdong via
/// block majority.
#[test]
fn a_country_below_the_state_tier_ships_no_provinces() {
    let policy = geo_rasterizer::policy::get().unwrap();
    for &worldview in Worldview::ALL {
        for f in shipped_provinces(worldview) {
            // A demoted or synthesized territory carries a bare `ADM0_A3`; only
            // rows from the admin-1 source are subject to the drop.
            let Some((adm0, _)) = f.split_once(['-', '+']) else {
                continue;
            };
            assert!(
                !policy.drop_admin1_in.contains(adm0),
                "{}: {f} ships as a province, but `{adm0}` is dropped wholesale",
                worldview.spec().id
            );
        }
    }
}

/// Every country `drop_admin1_in` names must exist in the source. A pin bump
/// that renames or retires one would otherwise leave a row that silently drops
/// nothing.
#[test]
fn every_dropped_country_names_a_country_in_the_source() {
    use std::collections::BTreeSet;

    let policy = geo_rasterizer::policy::get().unwrap();
    let raw = std::fs::read_to_string(source()).unwrap();
    let collection: geojson::FeatureCollection = serde_json::from_str(&raw).unwrap();
    let present: BTreeSet<&str> = collection
        .features
        .iter()
        .filter_map(|f| f.properties.as_ref()?.get("adm0_a3")?.as_str())
        .collect();
    let dead: Vec<&String> = policy
        .drop_admin1_in
        .iter()
        .filter(|c| !present.contains(c.as_str()))
        .collect();
    assert!(
        dead.is_empty(),
        "geo_policy.toml `drop_admin1_in` names {dead:?}, which the admin-1 source has no rows \
         for — drop the dead entries"
    );
}

/// The tier is all of a country or none of it. A country that keeps some of its
/// units and loses others renders partly as itself and partly as provinces,
/// which is what a drop keyed on anything but the country produces.
///
/// Per-code removals stay possible, but each is a reviewed row (`unparented`,
/// `merge`) or the coextensive `+00?` dedup, so each is excluded from both
/// sides of the count. Nothing else may thin a country's tier.
#[test]
fn the_province_tier_is_never_partial_for_a_country() {
    use geo_rasterizer::admin0::parse_admin0;
    use geo_rasterizer::entities::apply_admin1_policy;
    use std::collections::{BTreeMap, BTreeSet};

    let policy = geo_rasterizer::policy::get().unwrap();
    let mut partial: Vec<String> = Vec::new();
    for &worldview in Worldview::ALL {
        let wv = worldview.spec().id;
        let reviewed: BTreeSet<&str> = policy
            .unparented
            .iter()
            .filter(|u| u.worldview == wv)
            .map(|u| u.code.as_str())
            .chain(
                policy
                    .merge
                    .iter()
                    .filter(|m| m.worldview == wv)
                    .map(|m| m.code.as_str()),
            )
            .collect();
        let counts = |features: &[geo_rasterizer::admin1::Admin1Feature]| {
            let mut by_country: BTreeMap<String, usize> = BTreeMap::new();
            for f in features {
                if reviewed.contains(f.adm1_code.as_str()) || f.adm1_code.ends_with("+00?") {
                    continue;
                }
                *by_country.entry(f.adm0_a3.clone()).or_default() += 1;
            }
            by_country
        };

        let parsed = parse_admin1(&source(), worldview).unwrap();
        let source_units = counts(&parsed);
        let admin0 = parse_admin0(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("natural_earth")
                .join(worldview.spec().source_filename),
            wv,
        )
        .unwrap();
        let shipped = counts(
            &apply_admin1_policy(parsed, worldview, &admin0)
                .unwrap()
                .features,
        );

        for (adm0, &in_source) in &source_units {
            let kept = shipped.get(adm0).copied().unwrap_or(0);
            if kept != 0 && kept != in_source {
                partial.push(format!("{wv}: {adm0} keeps {kept} of {in_source} units"));
            }
        }
    }
    assert!(
        partial.is_empty(),
        "the province tier is partial for:\n  {}",
        partial.join("\n  ")
    );
}

/// `adm1_code → the ADM0_A3 that province ships under`, or `None` when the bin
/// carries no such province.
fn shipped_parent(worldview: Worldview, code: &str) -> Option<String> {
    use geo_data_format::{read_geo_data, GeoEntityKind};

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../app/assets/geo")
        .join(format!("geo_data_{}.bin", worldview.spec().id));
    assert!(
        path.exists(),
        "{} is absent — run `just rasterize-geo`",
        path.display()
    );
    let data = read_geo_data(&std::fs::read(&path).unwrap()).unwrap();
    let province = data
        .entities
        .iter()
        .find(|e| matches!(e.kind, GeoEntityKind::Admin1) && e.canonical_code == code)?;
    Some(
        data.entities
            .iter()
            .find(|e| Some(e.id) == province.parent_id)
            .expect("a shipped province's parent must be in the same bin")
            .canonical_code
            .clone(),
    )
}

/// The provinces the skip list drops must be absent from the bin that drops
/// them and present in the bins that do not — otherwise the list is silently
/// over- or under-reaching.
#[test]
fn skipped_provinces_are_absent_only_where_they_are_unparentable() {
    // Crimea: unclaimed in iso's admin-0, awarded to Ukraine in chn and usa.
    assert_eq!(shipped_parent(Worldview::Iso, "RUS-283"), None);
    assert!(shipped_parent(Worldview::Chn, "RUS-283").is_some());
    assert!(shipped_parent(Worldview::Usa, "RUS-283").is_some());
}

/// The Paracels, the Spratlys and Scarborough Shoal are administered as Sansha,
/// a prefecture-level city under Hainan — so in chn their land merges into Hainan
/// shipping as pseudo-provinces of CHN. Elsewhere they are untouched:
/// landless-dropped via `unparented` (PFA in iso/usa, PGA in iso) or
/// deduplicated (PGA in usa, a country there) — those entries stay live.
#[test]
fn the_south_china_sea_features_merge_into_hainan_under_chn() {
    assert_eq!(shipped_parent(Worldview::Chn, "PFA+00?"), None);
    assert_eq!(shipped_parent(Worldview::Chn, "PGA+00?"), None);
    assert_eq!(
        shipped_parent(Worldview::Chn, "CHN-1775").as_deref(),
        Some("CHN")
    );
    assert_eq!(shipped_parent(Worldview::Iso, "PFA+00?"), None);
    assert_eq!(shipped_parent(Worldview::Usa, "PFA+00?"), None);
}

/// A demoted territory ships exactly once per worldview: as a Province of its
/// claimant where demoted, as a Country where recognized — never both.
#[test]
fn demoted_territories_ship_as_provinces_of_their_claimant() {
    assert_eq!(
        shipped_parent(Worldview::Chn, "HKG").as_deref(),
        Some("CHN")
    );
    assert_eq!(
        shipped_parent(Worldview::Chn, "MAC").as_deref(),
        Some("CHN")
    );
    // Where recognized they stay countries — no province leaf, and the dedup
    // rule now sees the demoted MAC entity and drops MAC+00? in chn too.
    assert_eq!(shipped_parent(Worldview::Iso, "HKG"), None);
    assert_eq!(shipped_parent(Worldview::Chn, "MAC+00?"), None);
}

/// A unit that is its territory's only admin-1 feature, where the territory is
/// already an entity in this worldview, duplicates its own parent. The rule
/// keys on the unit count, not on the `+00?` suffix — the kept cases are each
/// territory's ONLY representation in that worldview.
#[test]
fn coextensive_whole_country_units_are_deduplicated() {
    // Dropped: the territory is an entity here, so its sole unit duplicates it.
    // `FRO-1443` and `PRI-5260` are the numbered form of the same thing, and
    // Faroe's is drawn over the whole country while naming itself Eysturoyar.
    for (worldview, code) in [
        (Worldview::Iso, "MAC+00?"),
        (Worldview::Iso, "GIB+00?"),
        (Worldview::Iso, "VAT+00?"),
        (Worldview::Iso, "FRO-1443"),
        (Worldview::Iso, "PRI-5260"),
        (Worldview::Usa, "USG+00?"),
        (Worldview::Usa, "PGA+00?"),
    ] {
        assert_eq!(
            shipped_parent(worldview, code),
            None,
            "{}/{code} should be deduplicated",
            worldview.spec().id
        );
    }
    // Kept: the +00? is the territory's only representation in this worldview.
    for (worldview, code, expected) in [
        (Worldview::Iso, "SOL+00?", "SOM"),
        (Worldview::Iso, "CYN+00?", "CYP"),
        (Worldview::Iso, "KAB+00?", "KAZ"),
        (Worldview::Iso, "USG+00?", "CUB"),
    ] {
        assert_eq!(shipped_parent(worldview, code).as_deref(), Some(expected));
    }
    // Kept: not coextensive — two units under PSX, and one AUS unit of many.
    assert!(shipped_parent(Worldview::Iso, "GAZ+00?").is_some());
    assert!(shipped_parent(Worldview::Iso, "WEB+00?").is_some());
    assert!(shipped_parent(Worldview::Iso, "AUS+00?").is_some());
}

/// Natural Earth's declared `adm0_a3` is what a province ships under. These are
/// the provinces where the country mask disagrees loudly enough that block
/// majority used to hand them to a neighbour — Ladakh to China, Belize's Toledo
/// District to Guatemala, Adjara to Turkey.
#[test]
fn a_contested_province_ships_under_the_country_that_declares_it() {
    for (worldview, code, expected) in [
        (Worldview::Iso, "BLZ-1356", "BLZ"), // Toledo District, not GTM
        (Worldview::Iso, "GUY-676", "GUY"),  // Pomeroon-Supenaam, not VEN
        (Worldview::Iso, "MDA-1637", "MDA"), // Grigoriopol, not UKR
        (Worldview::Iso, "MDA-1645", "MDA"), // Rezina, not UKR
        (Worldview::Usa, "GEO-3027", "GEO"), // Adjara, not TUR
        (Worldview::Chn, "IND-20012", "IND"),
        (Worldview::Usa, "IND-20012", "IND"),
    ] {
        assert_eq!(
            shipped_parent(worldview, code).as_deref(),
            Some(expected),
            "{}/{code}",
            worldview.spec().id
        );
    }

    // Ladakh and Barima-Waini have no Indian/Guyanese land at all in iso — that
    // file excises the whole of disputed Kashmir and the Essequibo — so they are
    // dropped there rather than shipped under China and Venezuela.
    assert_eq!(shipped_parent(Worldview::Iso, "IND-20012"), None);
    assert_eq!(shipped_parent(Worldview::Iso, "GUY-675"), None);
}

/// The two ways a province reaches a country other than the one it declares:
/// a reviewed override where the mask covers it in full, and block majority
/// where the declared code is no country in that worldview.
#[test]
fn a_province_reaches_another_country_only_by_review_or_by_absence() {
    for (worldview, code, expected) in [
        // Reviewed overrides (geo_policy.toml `overrule`).
        (Worldview::Chn, "RUS-283", "UKR"),
        (Worldview::Chn, "RUS-5482", "UKR"),
        (Worldview::Usa, "RUS-283", "UKR"),
        (Worldview::Usa, "RUS-5482", "UKR"),
        (Worldview::Usa, "GEO-3015", "B35"),
        // Declared codes that are no country in that worldview → block majority.
        (Worldview::Iso, "KOS-5886", "SRB"),
        (Worldview::Iso, "SOL+00?", "SOM"),
        (Worldview::Iso, "CYN+00?", "CYP"),
        (Worldview::Iso, "KAB+00?", "KAZ"),
        (Worldview::Iso, "USG+00?", "CUB"),
        (Worldview::Iso, "WSB-5133", "CYP"),
        (Worldview::Iso, "ESB-5132", "CYP"),
        (Worldview::Iso, "CSI+00?", "AUS"),
        (Worldview::Iso, "KAS+00?", "CHN"),
    ] {
        assert_eq!(
            shipped_parent(worldview, code).as_deref(),
            Some(expected),
            "{}/{code}",
            worldview.spec().id
        );
    }
}

/// A country's first-order units keep shipping whatever Natural Earth's
/// `TYPE_EN` calls them — the class string is not evidence of tier. `County` is
/// first-order in Sweden, Liberia, Romania, Estonia and Norway and second-order
/// in the UK; `District` is first-order in Eswatini and second-order in Sri
/// Lanka.
#[test]
fn first_order_units_ship_whatever_their_class_string_says() {
    for code in [
        "SWE-3429", // Skåne, County
        "LBR-785",  // Grand Bassa, County
        "ROU-296",  // Cluj, County
        "EST-1654", // Harju, County
        "NOR-75",   // Nordland, County
        "SWZ-534",  // Hhohho, District
        "TLS-546",  // Baucau, District|Regencies
        "USA-3556", // District of Columbia, Federal District
        "BRA-599",  // Distrito Federal, Federal District
    ] {
        assert!(
            shipped_parent(Worldview::Iso, code).is_some(),
            "iso/{code} must ship"
        );
    }
}
