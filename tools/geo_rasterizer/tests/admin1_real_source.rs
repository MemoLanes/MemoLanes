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
/// These are the parsed counts minus the provinces whose parent country holds
/// none of their land (see `entities::UNPARENTED_PROVINCES`): iso drops 9 and
/// usa drops 1, because Natural Earth's iso admin-0 excises disputed land from
/// both claimants.
#[test]
fn shipped_bins_carry_the_expected_province_counts() {
    use geo_data_format::{read_geo_data, GeoEntityKind};

    for (worldview, expected) in [
        (Worldview::Iso, 4578usize),
        (Worldview::Usa, 4586),
        (Worldview::Chn, 4534),
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
            .filter(|e| matches!(e.kind, GeoEntityKind::Province))
            .count();
        assert_eq!(provinces, expected, "{}", worldview.spec().id);

        // Every province must be parented to a country that exists in this bin.
        let countries: std::collections::BTreeSet<_> = data
            .entities
            .iter()
            .filter(|e| matches!(e.kind, GeoEntityKind::Country))
            .map(|e| e.id)
            .collect();
        for province in data
            .entities
            .iter()
            .filter(|e| matches!(e.kind, GeoEntityKind::Province))
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
        .find(|e| matches!(e.kind, GeoEntityKind::Province) && e.canonical_code == code)?;
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

    // The Paracels: unclaimed in iso and usa, awarded to China in chn.
    assert_eq!(shipped_parent(Worldview::Iso, "PFA+00?"), None);
    assert_eq!(shipped_parent(Worldview::Usa, "PFA+00?"), None);
    assert!(shipped_parent(Worldview::Chn, "PFA+00?").is_some());
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
        // Reviewed overrides (MASK_OVERRULES_DECLARED_PARENT).
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
        (Worldview::Chn, "HKG-5153", "CHN"),
        (Worldview::Chn, "MAC+00?", "CHN"),
    ] {
        assert_eq!(
            shipped_parent(worldview, code).as_deref(),
            Some(expected),
            "{}/{code}",
            worldview.spec().id
        );
    }
}
