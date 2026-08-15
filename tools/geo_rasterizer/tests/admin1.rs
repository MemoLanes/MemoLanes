use std::path::Path;

use geo_data_format::Worldview;
use geo_rasterizer::admin1::{is_unassigned_remainder, parse_admin1};

const FIXTURE: &str = "tests/fixtures/synthetic_admin1.geojson";

#[test]
fn drops_the_unassigned_remainder_sentinel() {
    let features = parse_admin1(Path::new(FIXTURE), Worldview::Iso).unwrap();
    assert!(
        features.iter().all(|f| !f.adm1_code.ends_with("+99?")),
        "NE's `<ADM0>+99?` sentinel is the country's leftover geometry, not a province"
    );
    assert_eq!(features.len(), 3);
}

#[test]
fn honours_the_worldview_fclass_column() {
    let iso = parse_admin1(Path::new(FIXTURE), Worldview::Iso).unwrap();
    let chn = parse_admin1(Path::new(FIXTURE), Worldview::Chn).unwrap();
    let usa = parse_admin1(Path::new(FIXTURE), Worldview::Usa).unwrap();

    assert!(iso.iter().any(|f| f.adm1_code == "AAA-002"));
    assert!(usa.iter().any(|f| f.adm1_code == "AAA-002"));
    assert!(
        !chn.iter().any(|f| f.adm1_code == "AAA-002"),
        "FCLASS_CN is non-null for AAA-002, so the chn worldview must drop it"
    );
    assert_eq!(chn.len(), 2);
}

#[test]
fn every_surviving_feature_carries_both_names() {
    for &wv in Worldview::ALL {
        for f in parse_admin1(Path::new(FIXTURE), wv).unwrap() {
            assert!(
                f.name_en.is_some(),
                "{}: {} has no name_en",
                wv.spec().id,
                f.adm1_code
            );
            assert!(
                f.name_zh.is_some(),
                "{}: {} has no name_zh",
                wv.spec().id,
                f.adm1_code
            );
        }
    }
}

#[test]
fn sentinel_predicate_matches_only_the_plus_99_form() {
    assert!(is_unassigned_remainder("MEX+99?"));
    assert!(is_unassigned_remainder("ATA+99?"));
    assert!(!is_unassigned_remainder("ATA+00?"));
    assert!(!is_unassigned_remainder("MYS-1186"));
}
