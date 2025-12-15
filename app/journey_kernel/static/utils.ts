/**
 * Utility functions
 *
 * Note: Platform-specific utilities (like disableMagnifierIfIOS) have been
 * moved to platform.ts for better organization.
 */

import { transformMapboxStyle } from "maplibregl-mapbox-request-transformer";

/**
 * Transform map style and add globe projection
 * @param previousStyle - Previous map style
 * @param nextStyle - Next map style to apply
 * @returns Transformed style with globe projection
 */
export function transformStyle(previousStyle: any, nextStyle: any): any {
  const convertedStyle = transformMapboxStyle(previousStyle, nextStyle);
  return {
    ...convertedStyle,
    projection: { type: "globe" },
  };
}
