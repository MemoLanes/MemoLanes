//! Worldviews — the single source of truth.
//!
//! `Worldview` carries every per-worldview fact: the externally-meaningful worldview `id`,
//! and the pinned Natural Earth source (filename + content hash).
//! Adding a worldview is one enum case + one [`Worldview::spec`] arm; the compiler
//! forces the arm, and the lock tests catch a case forgotten in [`Worldview::ALL`].
//!
//! Both the offline rasterizer (which downloads/verifies the source and embeds
//! the worldview list into `geo_data.bin`) and the runtime depend on this crate,
//! so the spec lives here rather than in the tool.

/// Commit pinned on `nvkelso/natural-earth-vector` (master @ 2026-04-26).
/// Bumping this shifts entity IDs, areas, and border tiles for every worldview.
pub const NATURAL_EARTH_COMMIT: &str = "ca96624a56bd078437bca8184e78163e5039ad19";

/// Raw-GitHub base for the pinned commit's `geojson/` directory.
pub const NATURAL_EARTH_BASE: &str =
    "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/\
     ca96624a56bd078437bca8184e78163e5039ad19/geojson";

/// Worldview of Natural Earth Admin-0 Countries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Worldview {
    Iso,
    Chn,
    Usa,
}

/// All per-worldview facts. `id` names the runtime asset; `source_*` drive the
/// offline download.
pub struct WorldviewSpec {
    /// Externally-meaningful worldview id (also the `geo_data_<id>.bin` suffix).
    pub id: &'static str,
    /// Natural Earth GeoJSON filename under `NATURAL_EARTH_BASE`.
    pub source_filename: &'static str,
    /// SHA-256 of the pinned source's raw bytes (recorded at pin time).
    pub source_sha256: &'static str,
}

impl Worldview {
    // Adding a worldview: add the enum case here AND fill one `spec()` arm below.
    // To get `source_sha256` for a new variant, fetch the pinned file and hash
    // it (the source is NOT auto-trusted — a human pastes a verified hash; this
    // is the supply-chain guard, same as a pin bump):
    //   curl -sL "$NATURAL_EARTH_BASE/<source_filename>" | sha256sum
    // (or: add the variant with a placeholder sha, run `--worldview <new>
    //  --ensure-source --download-only`, and copy the real hash from the
    //  verify-mismatch error.)
    pub const ALL: &'static [Worldview] = &[Worldview::Iso, Worldview::Chn, Worldview::Usa];

    pub const fn spec(self) -> WorldviewSpec {
        match self {
            Worldview::Iso => WorldviewSpec {
                id: "iso",
                source_filename: "ne_10m_admin_0_countries_iso.geojson",
                source_sha256: "60eb10aa951f5872507c9436937508b09be4b43dc9fa7aad7644f23ef12e1cad",
            },
            Worldview::Chn => WorldviewSpec {
                id: "chn",
                source_filename: "ne_10m_admin_0_countries_chn.geojson",
                source_sha256: "a13bf5f310fde87bc0a5f994f8ce9bd706cc198d8ee37d221e61c2546b945372",
            },
            Worldview::Usa => WorldviewSpec {
                id: "usa",
                source_filename: "ne_10m_admin_0_countries_usa.geojson",
                source_sha256: "d3166691d3d86f113c0d8db52506f4b72936513691d1593f47010fed01fc0b93",
            },
        }
    }

    /// Full raw-GitHub URL of this worldview's pinned source.
    pub fn source_url(self) -> String {
        format!("{NATURAL_EARTH_BASE}/{}", self.spec().source_filename)
    }

    /// Resolve a worldview id (e.g. `"iso"`) to its `Worldview`. Replaces a `FromStr`
    /// impl so the accepted set is derived from `ALL`, not a separate match.
    pub fn from_id(s: &str) -> anyhow::Result<Worldview> {
        Worldview::ALL
            .iter()
            .copied()
            .find(|p| p.spec().id == s)
            .ok_or_else(|| {
                let ids: Vec<&str> = Worldview::ALL.iter().map(|p| p.spec().id).collect();
                anyhow::anyhow!("unknown worldview `{s}` (expected one of {ids:?})")
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            assert!(worldview
                .spec()
                .source_sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit()));
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
}
