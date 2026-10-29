import {
  LngLatBounds,
  type Map,
  type LngLatBoundsLike,
  type FitBoundsOptions,
  type FlyToOptions,
} from "maplibre-gl";
import { POLAR_SEAM_LATITUDE } from "./polar-projection";

/** MapLibre seeds globe bounds fitting with a Mercator rectangle, which clips
 * polar latitudes. Fit the spherical rectangle directly, without moving the
 * live camera or depending on MapLibre's private transform implementation. */
export function getPolarBoundsCamera(
  map: Map,
  boundsLike: LngLatBoundsLike,
  options: FitBoundsOptions = {},
): FlyToOptions | null {
  const bounds = LngLatBounds.convert(boundsLike);
  const latitude = (bounds.getSouth() + bounds.getNorth()) / 2;
  if (Math.abs(latitude) < 80) return null;
  const west = bounds.getWest();
  const east =
    bounds.getEast() < west ? bounds.getEast() + 360 : bounds.getEast();
  const longitude = (west + east) / 2;
  const radians = Math.PI / 180;
  const centerLat = latitude * radians;
  const bearing = options.bearing ?? 0;
  const cosBearing = Math.cos(bearing * radians),
    sinBearing = Math.sin(bearing * radians);
  const padding =
    typeof options.padding === "number"
      ? {
          top: options.padding,
          bottom: options.padding,
          left: options.padding,
          right: options.padding,
        }
      : { top: 0, bottom: 0, left: 0, right: 0, ...options.padding };
  const base = {
    top: 0,
    bottom: 0,
    left: 0,
    right: 0,
    ...(options.absolutePadding ? {} : map.getPadding()),
  };
  const offset = options.offset ?? [0, 0];
  const [dx, dy] = Array.isArray(offset) ? offset : [offset.x, offset.y];
  const canvas = map.getCanvas();
  const halfWidth =
    (canvas.clientWidth -
      base.left -
      base.right -
      padding.left -
      padding.right) /
      2 -
    Math.abs(dx) -
    1;
  const halfHeight =
    (canvas.clientHeight -
      base.top -
      base.bottom -
      padding.top -
      padding.bottom) /
      2 -
    Math.abs(dy) -
    1;
  if (halfWidth <= 0 || halfHeight <= 0) return null;
  const distance =
    canvas.clientHeight /
    (2 * Math.tan((map.getVerticalFieldOfView() * radians) / 2));
  let radius = Infinity;
  // Samples include the parallel/meridian interiors, where a wide polar box
  // bulges beyond its four corners. A 2% margin covers sampling error.
  for (let j = 0; j <= 8; j++) {
    const lat =
      (bounds.getSouth() + ((bounds.getNorth() - bounds.getSouth()) * j) / 8) *
      radians;
    for (let i = 0; i <= 32; i++) {
      const delta = (west + ((east - west) * i) / 32 - longitude) * radians;
      const x = Math.cos(lat) * Math.sin(delta);
      const y =
        Math.cos(centerLat) * Math.sin(lat) -
        Math.sin(centerLat) * Math.cos(lat) * Math.cos(delta);
      const z =
        Math.sin(centerLat) * Math.sin(lat) +
        Math.cos(centerLat) * Math.cos(lat) * Math.cos(delta);
      for (const [coordinate, halfSide] of [
        [x * cosBearing + y * sinBearing, halfWidth],
        [y * cosBearing - x * sinBearing, halfHeight],
      ]) {
        // Perspective position is D*R*coordinate / (D + R*(1-z)).
        const denominator =
          distance * Math.abs(coordinate) - halfSide * (1 - z);
        if (denominator > 0)
          radius = Math.min(radius, (halfSide * distance) / denominator);
      }
      // Keep every requested point on the visible side of the globe's limb.
      if (z < 1) radius = Math.min(radius, (distance * z) / (1 - z));
    }
  }
  const scale = Math.cos(
    Math.min(Math.abs(latitude), POLAR_SEAM_LATITUDE) * radians,
  );
  const clampZoom = (value: number) =>
    Math.max(
      map.getMinZoom(),
      Math.min(options.maxZoom ?? map.getMaxZoom(), map.getMaxZoom(), value),
    );
  const zoom = clampZoom(
    Math.log2((radius * 0.98 * 2 * Math.PI * scale) / 512),
  );
  return {
    center: [longitude, latitude],
    zoom,
    bearing,
    pitch: 0,
    // A pole cannot move to an arbitrary screen position while keeping north
    // up. Use the actual padded viewport, and restore it on the next navigation.
    padding: {
      top: base.top + padding.top,
      bottom: base.bottom + padding.bottom,
      left: base.left + padding.left,
      right: base.right + padding.right,
    },
    offset,
  };
}
