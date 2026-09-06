/** Normalized RGBA color used by map layers: [red, green, blue, alpha]. */
export type RGBAColor = [number, number, number, number];

export type FogStyleId = "dark" | "light";

export interface FogStyle {
  /** Normalized renderer color: [red, green, blue, alpha]. */
  readonly rgba: RGBAColor;
}

/** Canonical fog palette owned by the map renderer. */
export const FOG_STYLES: Record<FogStyleId, FogStyle> = {
  dark: { rgba: [0.0, 0.07, 0.16, 0.5] },
  light: { rgba: [0.75, 0.84, 0.89, 0.6] },
};

export const DEFAULT_FOG_STYLE: FogStyleId = "dark";
export const DEFAULT_FOG_RGBA: RGBAColor = [
  ...FOG_STYLES[DEFAULT_FOG_STYLE].rgba,
];

/** Resolve an external style ID without allowing invalid renderer input. */
export function normalizeFogStyleId(styleId?: string): FogStyleId {
  return styleId === "light" ? "light" : DEFAULT_FOG_STYLE;
}

export function getFogStyle(styleId: FogStyleId): FogStyle {
  const style = FOG_STYLES[styleId];
  // Return an independent style so a runtime density override cannot mutate
  // the shared palette used as the source of truth for future map instances.
  return { rgba: [...style.rgba] };
}
