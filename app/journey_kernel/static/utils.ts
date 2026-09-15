/**
 * Utility functions
 *
 * Note: Platform-specific utilities (like disableMagnifierIfIOS) have been
 * moved to platform.ts for better organization.
 */

import { transformMapboxStyle } from "maplibregl-mapbox-request-transformer";
import { detectMapLocale, type MapLocale } from "./map-locale";
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
 * Transform map style and add globe projection (default behavior)
 * @param previousStyle - Previous map style
 * @param nextStyle - Next map style to apply
 * @param mapLocale - Browser-selected locale for opted-in map styles
 * @returns Transformed style with globe projection
 */
export function transformStyle(
  previousStyle: any,
  nextStyle: any,
  mapLocale: MapLocale = detectMapLocale(),
): any {
  return transformStyleWithProjection(
    previousStyle,
    nextStyle,
    "globe",
    mapLocale,
  );
}

/**
 * Transform map style with specified projection type
 * @param previousStyle - Previous map style
 * @param nextStyle - Next map style to apply
 * @param projection - Projection type ("mercator" or "globe")
 * @param mapLocale - Browser-selected locale for opted-in map styles
 * @returns Transformed style with specified projection
 */
export function transformStyleWithProjection(
  previousStyle: any,
  nextStyle: any,
  projection: ProjectionType,
  mapLocale: MapLocale,
): any {
  const convertedStyle = injectMapLocaleState(
    transformMapboxStyle(previousStyle, nextStyle),
    mapLocale,
  );
  // Use Mapbox's projection transition to prevent GPU precision issues at large zoom levels.
  // TODO: remove this workaround once upstream issues are fixed.
  // https://github.com/mapbox/mapbox-gl-js/issues/13395
  // https://github.com/maplibre/maplibre-gl-js/issues/7419
  const projectionValue =
    projection === "globe"
      ? [
          "interpolate",
          ["linear"],
          ["zoom"],
          5,
          "vertical-perspective",
          6,
          "mercator",
        ]
      : projection;
  return {
    ...convertedStyle,
    projection: { type: projectionValue },
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
