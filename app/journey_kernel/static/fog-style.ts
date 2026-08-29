import type { RGBAColor } from "./layers/journey-layer-interface";

/** Fallback used by the standalone web map when Flutter supplies no style. */
export const DEFAULT_FOG_COLOR = "#001228";
export const DEFAULT_FOG_OPACITY = 0.5;

const HEX_RGB_PATTERN = /^#?([\da-f]{2})([\da-f]{2})([\da-f]{2})$/i;

/** Return a canonical #RRGGBB fog color, falling back on invalid input. */
export function normalizeFogColor(color?: string): string {
  if (!color) return DEFAULT_FOG_COLOR;

  const match = HEX_RGB_PATTERN.exec(color.trim());
  if (!match) {
    console.warn(
      `[FogStyle] Invalid fog color '${color}', using ${DEFAULT_FOG_COLOR}.`,
    );
    return DEFAULT_FOG_COLOR;
  }

  return `#${match[1]}${match[2]}${match[3]}`.toUpperCase();
}

/** Combine a validated CSS hex color and opacity for a journey layer. */
export function createFogRgba(color: string, opacity: number): RGBAColor {
  const normalized = normalizeFogColor(color);
  const match = HEX_RGB_PATTERN.exec(normalized)!;

  return [
    Number.parseInt(match[1], 16) / 255,
    Number.parseInt(match[2], 16) / 255,
    Number.parseInt(match[3], 16) / 255,
    Math.max(0, Math.min(1, opacity)),
  ];
}
