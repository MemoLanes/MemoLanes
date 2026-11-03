import { LngLat, type TransformConstrainFunction } from "maplibre-gl";

/** MapLibre's hybrid globe flattens at high zoom. Keep a spherical camera
 * near a cap, independently of data LOD; hysteresis avoids projection flicker. */
export function resolvePolarGlobeLock(
  projection: string,
  latitude: number,
  locked: boolean,
): boolean {
  return projection === "globe" && Math.abs(latitude) >= (locked ? 75 : 80);
}

/** The default globe constraint retreats to the Mercator edge when zoomed in
 * because its basemap has no polar data. Polar journeys can reach ±90°; keep
 * that geographic centre while retaining the configured zoom bounds. Stay a
 * fraction of a native pixel off ±90° to keep Mercator fallback matrices finite. */
export function createPolarCameraConstrain(
  minZoom: number,
  maxZoom: number,
): TransformConstrainFunction {
  return (center, zoom) => ({
    center: new LngLat(
      center.lng,
      Math.max(-89.999999, Math.min(89.999999, center.lat)),
    ),
    zoom: Math.max(minZoom, Math.min(maxZoom, zoom)),
  });
}
