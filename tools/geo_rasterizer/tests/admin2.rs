use std::path::Path;

use geo_rasterizer::admin2::load;

#[test]
fn snapshot_features_parent_by_their_adcode_province_prefix() {
    let feats = load(Path::new("tests/fixtures/admin2_chn_sample")).unwrap();
    assert_eq!(feats.len(), 3);
    let find = |code| feats.iter().find(|f| f.canonical_code == code).unwrap();

    let sz = find("CN-4403");
    assert_eq!(sz.name_zh, "深圳市");
    assert_eq!(sz.parent_adm1, "CHN-4818");
    // A 6-digit 省直辖县级 adcode parents by the same two-digit prefix.
    assert_eq!(find("CN-469002").parent_adm1, "CHN-1775");
}
