/**
 * Utility functions
 *
 * Note: Platform-specific utilities (like disableMagnifierIfIOS) have been
 * moved to platform.ts for better organization.
 */

import type { MapLocale } from "./map-locale";
import type { ProjectionType } from "./params";

const RUNTIME_LANGUAGE_METADATA_KEY = "memolanes:runtime-language";

function isRecord(value: unknown): value is Record<string, any> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

/**
 * Inject browser-selected language defaults only into styles that explicitly
 * opt into MemoLanes runtime localization. Other styles, including Mapbox,
 * retain their original state and label expressions.
 */
export function injectMapLocaleState(style: any, mapLocale: MapLocale): any {
  if (
    !isRecord(style) ||
    !isRecord(style.metadata) ||
    style.metadata[RUNTIME_LANGUAGE_METADATA_KEY] !== 1
  ) {
    return style;
  }

  const existingState = isRecord(style.state) ? style.state : {};
  const existingLanguageState = isRecord(existingState.mapLanguage)
    ? existingState.mapLanguage
    : {};
  const existingScriptsState = isRecord(existingState.mapTargetScripts)
    ? existingState.mapTargetScripts
    : {};

  return {
    ...style,
    state: {
      ...existingState,
      mapLanguage: {
        ...existingLanguageState,
        default: mapLocale.language,
      },
      mapTargetScripts: {
        ...existingScriptsState,
        default: [...mapLocale.targetScripts],
      },
    },
  };
}

/**
 * Transform map style with specified projection type
 * @param nextStyle - Next map style to apply
 * @param projection - Projection type ("mercator" or "globe")
 * @param mapLocale - Browser-selected locale for opted-in map styles
 * @returns Transformed style with specified projection
 */
export function transformStyleWithProjection(
  nextStyle: any,
  projection: ProjectionType,
  mapLocale: MapLocale,
): any {
  return {
    ...injectMapLocaleState(nextStyle, mapLocale),
    projection: { type: projection },
    sky: {
      "sky-color": "#080820",
      "horizon-color": "#2a2a3a",
      "atmosphere-blend": 0.8,
    },
  };
}

/**
 * Display a message on the webpage with consistent styling
 * Safe from XSS attacks by using DOM methods instead of innerHTML
 * @param heading - Main heading text to display
 * @param detail - Optional detailed message text
 * @param isError - Whether this is an error message (affects text color)
 */
export function displayPageMessage(
  heading: string,
  detail?: string,
  isError: boolean = true,
): void {
  const color = isError ? "#ff6f7d" : "#f1f5ef";

  // Create container div
  const container = document.createElement("div");
  container.style.padding = "20px";
  container.style.fontFamily = "Arial, sans-serif";
  container.style.color = color;
  document.body.style.backgroundColor = "#0b100d";

  // Create and add heading
  const h1 = document.createElement("h1");
  h1.textContent = heading; // textContent prevents XSS
  container.appendChild(h1);

  // Create and add detail paragraph if provided
  if (detail) {
    const p = document.createElement("p");
    p.textContent = detail; // textContent prevents XSS
    container.appendChild(p);
  }

  // Clear body and add new content
  document.body.innerHTML = "";
  document.body.appendChild(container);
}
