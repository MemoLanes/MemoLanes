/**
 * Browser-derived map label locale.
 *
 * This is intentionally independent from Flutter's UI locale. The value is
 * resolved once when MapController is created and is not updated while the
 * map is running.
 */

export interface MapLocale {
  /** Protomaps `name:*` suffix, for example `fr` or `zh-Hant`. */
  readonly language: string;
  /** Protomaps script names treated as local for this language. */
  readonly targetScripts: readonly string[];
}

const FALLBACK_MAP_LOCALE: MapLocale = {
  language: "en",
  targetScripts: ["Latin"],
};

// Languages that the current renderer can display without an RTL plugin or
// positioned-glyph handling. Protomaps also ships Arabic/Hebrew and
// Devanagari translations, which can be added once those render paths exist.
interface MapLanguageConfig {
  readonly targetScripts: readonly string[];
}

const LATIN_LANGUAGE_CONFIG: MapLanguageConfig = {
  targetScripts: ["Latin"],
};

const CYRILLIC_LANGUAGE_CONFIG: MapLanguageConfig = {
  targetScripts: ["Cyrillic"],
};

const MAP_LANGUAGE_CONFIG: Record<string, MapLanguageConfig> = {
  bg: CYRILLIC_LANGUAGE_CONFIG,
  "zh-Hans": { targetScripts: ["Han"] },
  "zh-Hant": { targetScripts: ["Han"] },
  hr: LATIN_LANGUAGE_CONFIG,
  cs: LATIN_LANGUAGE_CONFIG,
  da: LATIN_LANGUAGE_CONFIG,
  nl: LATIN_LANGUAGE_CONFIG,
  en: LATIN_LANGUAGE_CONFIG,
  et: LATIN_LANGUAGE_CONFIG,
  fi: LATIN_LANGUAGE_CONFIG,
  fr: LATIN_LANGUAGE_CONFIG,
  de: LATIN_LANGUAGE_CONFIG,
  el: { targetScripts: ["Greek"] },
  hu: LATIN_LANGUAGE_CONFIG,
  id: LATIN_LANGUAGE_CONFIG,
  ga: LATIN_LANGUAGE_CONFIG,
  it: LATIN_LANGUAGE_CONFIG,
  ja: {
    targetScripts: ["Han", "Hiragana", "Katakana", "Mixed-Japanese"],
  },
  ko: { targetScripts: ["Hangul"] },
  lv: LATIN_LANGUAGE_CONFIG,
  lt: LATIN_LANGUAGE_CONFIG,
  mt: LATIN_LANGUAGE_CONFIG,
  no: LATIN_LANGUAGE_CONFIG,
  pl: LATIN_LANGUAGE_CONFIG,
  pt: LATIN_LANGUAGE_CONFIG,
  ro: LATIN_LANGUAGE_CONFIG,
  ru: CYRILLIC_LANGUAGE_CONFIG,
  sk: LATIN_LANGUAGE_CONFIG,
  sl: LATIN_LANGUAGE_CONFIG,
  es: LATIN_LANGUAGE_CONFIG,
  sv: LATIN_LANGUAGE_CONFIG,
  tr: LATIN_LANGUAGE_CONFIG,
  uk: CYRILLIC_LANGUAGE_CONFIG,
  vi: LATIN_LANGUAGE_CONFIG,
};

const TRADITIONAL_CHINESE_REGIONS = new Set(["HK", "MO", "TW"]);

function resolveMapLanguage(locale: Intl.Locale): string {
  if (locale.language === "zh") {
    if (locale.script === "Hans" || locale.script === "Hant") {
      return `zh-${locale.script}`;
    }
    return locale.region !== undefined &&
      TRADITIONAL_CHINESE_REGIONS.has(locale.region)
      ? "zh-Hant"
      : "zh-Hans";
  }

  return locale.language === "nb" || locale.language === "nn"
    ? "no"
    : locale.language;
}

function cloneMapLocale(locale: MapLocale): MapLocale {
  return {
    language: locale.language,
    targetScripts: [...locale.targetScripts],
  };
}

/**
 * Resolve the first browser language supported by the map tiles.
 * Except for Chinese variants, explicit script subtags fall back to the base
 * language because the tiles do not expose script-specific name fields.
 */
export function resolveMapLocale(
  preferredLanguages: readonly string[],
): MapLocale {
  for (const preferredLanguage of preferredLanguages) {
    try {
      const locale = new Intl.Locale(preferredLanguage).maximize();
      const language = resolveMapLanguage(locale);

      const languageConfig = MAP_LANGUAGE_CONFIG[language];
      if (!languageConfig) {
        continue;
      }

      return {
        language,
        targetScripts: [...languageConfig.targetScripts],
      };
    } catch {
      // Ignore malformed browser preferences and try the next locale.
    }
  }

  return cloneMapLocale(FALLBACK_MAP_LOCALE);
}

/** Read the browser preference list once for a new map instance. */
export function detectMapLocale(): MapLocale {
  if (typeof navigator === "undefined") {
    return cloneMapLocale(FALLBACK_MAP_LOCALE);
  }

  const preferredLanguages =
    navigator.languages.length > 0
      ? navigator.languages
      : navigator.language
        ? [navigator.language]
        : [];

  return resolveMapLocale(preferredLanguages);
}
