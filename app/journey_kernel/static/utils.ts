/**
 * Utility functions
 *
 * Note: Platform-specific utilities (like disableMagnifierIfIOS) have been
 * moved to platform.ts for better organization.
 */

import { transformMapboxStyle } from "maplibregl-mapbox-request-transformer";
import type { ProjectionType } from "./params";

/**
 * Transform map style and add globe projection (default behavior)
 * @param previousStyle - Previous map style
 * @param nextStyle - Next map style to apply
 * @returns Transformed style with globe projection
 */
export function transformStyle(previousStyle: any, nextStyle: any): any {
  return transformStyleWithProjection(previousStyle, nextStyle, "globe");
}

/**
 * Transform map style with specified projection type
 * @param previousStyle - Previous map style
 * @param nextStyle - Next map style to apply
 * @param projection - Projection type ("mercator" or "globe")
 * @returns Transformed style with specified projection
 */
export function transformStyleWithProjection(
  previousStyle: any,
  nextStyle: any,
  projection: ProjectionType,
): any {
  const convertedStyle = transformMapboxStyle(previousStyle, nextStyle);
  return {
    ...convertedStyle,
    projection: { type: projection },
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
  const color = isError ? "red" : "#333";

  // Create container div
  const container = document.createElement("div");
  container.style.padding = "20px";
  container.style.fontFamily = "Arial, sans-serif";
  container.style.color = color;

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
