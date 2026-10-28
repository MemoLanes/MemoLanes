import type { Map as MaplibreMap } from "maplibre-gl";

const MERCATOR_MAX_LATITUDE = 85.0511287798066;

/** Resolution of visible Mercator coverage, independent of the stroke width.
 * MapLibre enlarges the globe by 1/cos(center latitude), then adjusts zoom
 * during a pan to keep its apparent size. Raw zoom therefore underestimates
 * ordinary tracks away from the centre when looking towards a pole. */
export function getMapSamplingZoom(map: MaplibreMap): number {
  const zoom = map.getZoom();
  const transition = map.style?.projection?.transitionState ?? 0;
  if (transition <= 0) return zoom;

  const bounds = map.getBounds();
  const south = bounds.getSouthWest().lat;
  const north = bounds.getNorthEast().lat;
  const nearestEquator = Math.max(0, south, -north);
  // Match MapLibre's globe radius: latitude stops changing its scale at the
  // Mercator edge, including when the camera itself reaches either pole.
  const scale = (latitude: number) =>
    Math.cos(
      (Math.min(Math.abs(latitude), MERCATOR_MAX_LATITUDE) * Math.PI) / 180,
    );
  const globeScale = Math.max(
    1,
    scale(nearestEquator) / scale(map.getCenter().lat),
  );
  return zoom + Math.log2(1 + transition * (globeScale - 1));
}

export function getMapSamplingWorldSize(map: MaplibreMap): number {
  return 512 * 2 ** getMapSamplingZoom(map);
}
