export type PolarHemisphere = "north" | "south";
export const POLAR_SEAM_LATITUDE = 85.0511287798066;
export const POLAR_SEAM_COLATITUDE =
  ((90 - POLAR_SEAM_LATITUDE) * Math.PI) / 180;
export const POLAR_SEAM_RADIUS = 2 * Math.sin(POLAR_SEAM_COLATITUDE / 2);

export function polarUV(lng: number, lat: number): [number, number] {
  const radius =
    (2 * Math.sin(((90 - Math.abs(lat)) * Math.PI) / 360)) / POLAR_SEAM_RADIUS;
  const angle = (lng * Math.PI) / 180;
  return [
    0.5 + 0.5 * radius * Math.sin(angle),
    0.5 - Math.sign(lat) * 0.5 * radius * Math.cos(angle),
  ];
}

export function polarLngLat(
  u: number,
  v: number,
  hemisphere: PolarHemisphere,
): [number, number] {
  const x = (u - 0.5) * 2;
  const y = (v - 0.5) * 2;
  const sign = hemisphere === "north" ? 1 : -1;
  const lat =
    90 -
    (2 *
      Math.asin(Math.min(1, (Math.hypot(x, y) * POLAR_SEAM_RADIUS) / 2)) *
      180) /
      Math.PI;
  return [(Math.atan2(x, -sign * y) * 180) / Math.PI, sign * lat];
}
