#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Locale {
    EnUs,
    ZhCn,
}

pub struct LocaleSpec {
    /// BCP-47 tag. Must match a `LocaleConstants.supportedLocales` entry in
    /// `app/lib/constants/locale_constants.dart`
    pub tag: &'static str,
    pub cldr_tag: &'static str,
    pub cldr_source_sha256: &'static str,
    pub cldr_subdivisions_sha256: Option<&'static str>,
}

impl Locale {
    pub const ALL: &'static [Locale] = &[Locale::EnUs, Locale::ZhCn];

    pub const fn spec(self) -> LocaleSpec {
        match self {
            Locale::EnUs => LocaleSpec {
                tag: "en-US",
                cldr_tag: "en",
                cldr_source_sha256:
                    "158c1d575308f7e46912edbeda435c8fe2ef5dad280798231f3a432e406b1807",
                cldr_subdivisions_sha256: Some(
                    "f27f24be40f4a76f025e909ff0f277a58ebb39236e7d9692b43722e4c2b8126a",
                ),
            },
            Locale::ZhCn => LocaleSpec {
                tag: "zh-CN",
                cldr_tag: "zh",
                cldr_source_sha256:
                    "18f1426f1e8981a671517a4857a8d4e56060909906c4cfd9b9a43a8a18b9ab6f",
                cldr_subdivisions_sha256: None,
            },
        }
    }

    pub fn from_tag(s: &str) -> anyhow::Result<Locale> {
        Locale::ALL
            .iter()
            .copied()
            .find(|l| l.spec().tag == s)
            .ok_or_else(|| {
                let tags: Vec<&str> = Locale::ALL.iter().map(|l| l.spec().tag).collect();
                anyhow::anyhow!("unknown locale `{s}` (expected one of {tags:?})")
            })
    }

    /// Used only for admin-1 names, continents and countries resolve through CLDR
    pub fn ne_name_field(self) -> &'static str {
        match self {
            Locale::EnUs => "name_en",
            Locale::ZhCn => "name_zh",
        }
    }
}
