//! NE `ISO_A2_EH` to CLDR `ISO_3166-1_alpha-2`

use crate::Locale;

pub const CLDR_TAG: &str = "48.2.0";

pub const CLDR_BASE: &str = "https://raw.githubusercontent.com/unicode-org/cldr-json/\
     48.2.0/cldr-json/cldr-localenames-full/main";

impl Locale {
    pub fn cldr_source_url(self) -> String {
        format!("{CLDR_BASE}/{}/territories.json", self.spec().cldr_tag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cldr_table_is_consistent() {
        for &locale in Locale::ALL {
            let url = locale.cldr_source_url();
            assert!(
                url.starts_with(CLDR_BASE),
                "url not under pinned base: {url}"
            );
            assert!(url.ends_with("/territories.json"));
            let sha = locale.spec().cldr_source_sha256;
            assert_eq!(sha.len(), 64, "sha must be 64 hex chars");
            assert!(sha.bytes().all(|b| b.is_ascii_hexdigit()));
        }
        assert!(
            CLDR_BASE.starts_with("https://raw.githubusercontent.com/unicode-org/cldr-json/"),
            "base URL looks wrong: {CLDR_BASE}"
        );
        assert!(!CLDR_BASE.contains(' '), "base has embedded whitespace");
        // Pin desync guard: the base must embed the pinned tag, so a future
        // bump that updates only one of the two consts fails here.
        assert!(
            CLDR_BASE.contains(CLDR_TAG),
            "CLDR_BASE does not contain CLDR_TAG (pin desync)"
        );
    }
}
