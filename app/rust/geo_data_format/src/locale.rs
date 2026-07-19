#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Locale {
    EnUs,
    ZhCn,
}

pub struct LocaleSpec {
    /// BCP-47 tag. Must match a `LocaleConstants.supportedLocales` entry in
    /// `app/lib/constants/locale_constants.dart`
    pub tag: &'static str,
    /// Natural Earth property holding this locale's name (e.g. `NAME_ZH`).
    pub ne_field: &'static str,
}

impl Locale {
    pub const ALL: &'static [Locale] = &[Locale::EnUs, Locale::ZhCn];

    pub const fn spec(self) -> LocaleSpec {
        match self {
            Locale::EnUs => LocaleSpec {
                tag: "en-US",
                ne_field: "NAME_EN",
            },
            Locale::ZhCn => LocaleSpec {
                tag: "zh-CN",
                ne_field: "NAME_ZH",
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
}
