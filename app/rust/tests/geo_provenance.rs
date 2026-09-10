#![cfg(geo_bins_present)]
//! The compiled-in provenance hashes must match the assets Flutter bundles.
//! `build.rs` reads them from those bins' headers, so a mismatch means the
//! generated module went stale against `app/assets/geo/`.
//!
//! Compiled only when `assets/geo/geo_data_*.bin` are present (`just rasterize-geo`).
//! Without them this binary compiles to zero tests rather than to passing ones, so a
//! bare clone never reports coverage it did not run. CI always has the bins
//! (`rasterize-geo` is ungated), so its coverage is never narrowed.

use geo_data_format::Worldview;
use memolanes_core::geo::{GeoIndex, GeoLookup};
use memolanes_core::geo_provenance::bundled_provenance_hash;

#[test]
fn compiled_in_hashes_match_the_shipped_assets() {
    for worldview in Worldview::ALL {
        let id = worldview.spec().id;
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../assets/geo")
            .join(format!("geo_data_{id}.bin"));
        let shipped = GeoIndex::open(&path).unwrap().provenance_hash();
        assert_eq!(
            bundled_provenance_hash(*worldview),
            Some(shipped),
            "compiled-in hash for {id} does not match {}",
            path.display()
        );
    }
}
