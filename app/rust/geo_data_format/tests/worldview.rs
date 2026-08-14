use geo_data_format::{Worldview, NATURAL_EARTH_BASE, NATURAL_EARTH_COMMIT};

#[test]
fn worldview_table_is_consistent() {
    for &worldview in Worldview::ALL {
        let url = worldview.source_url();
        assert!(
            url.starts_with(NATURAL_EARTH_BASE),
            "url not under pinned base: {url}"
        );
        assert!(url.ends_with(".geojson"));
        assert_eq!(
            worldview.spec().source_sha256.len(),
            64,
            "sha must be 64 hex chars"
        );
        // Canonical lowercase, not merely "is hex". The rasterizer verifies a
        // download by comparing the digest's lowercase-hex spelling to this pin
        // as TEXT, so an uppercase paste — a perfectly valid SHA-256, and what
        // several tools emit — would never match, and would surface at download
        // time as a supply-chain mismatch blaming Natural Earth for what is
        // really a bad constant here. Fail on the constant instead.
        assert!(
            worldview
                .spec()
                .source_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "pin must be canonical lowercase hex: {}",
            worldview.spec().source_sha256
        );
    }
    // Guard NATURAL_EARTH_BASE itself (the per-worldview url assertions above
    // are tautological w.r.t. the base, so spot-check the base directly).
    assert!(
        NATURAL_EARTH_BASE.starts_with("https://raw.githubusercontent.com/nvkelso/"),
        "base URL looks wrong: {NATURAL_EARTH_BASE}"
    );
    assert!(
        !NATURAL_EARTH_BASE.contains(' '),
        "base has embedded whitespace"
    );
    // Pin desync guard: the base must embed the pinned commit, so a
    // future bump that updates only one of the two consts fails here.
    assert!(
        NATURAL_EARTH_BASE.contains(NATURAL_EARTH_COMMIT),
        "NATURAL_EARTH_BASE does not contain NATURAL_EARTH_COMMIT (pin desync)"
    );
    assert_eq!(
        Worldview::Iso.spec().source_sha256,
        "60eb10aa951f5872507c9436937508b09be4b43dc9fa7aad7644f23ef12e1cad"
    );
    assert_eq!(Worldview::from_id("chn").unwrap(), Worldview::Chn);
    assert!(Worldview::from_id("bogus").is_err());
}

#[test]
fn all_round_trips_through_from_id() {
    // The only guard against a variant added to the enum but forgotten in
    // ALL (the compiler can't catch that).
    assert_eq!(Worldview::ALL.len(), 3);
    for &p in Worldview::ALL {
        assert_eq!(Worldview::from_id(p.spec().id).unwrap(), p);
    }
}
