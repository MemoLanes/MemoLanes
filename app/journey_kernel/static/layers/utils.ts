import maplibregl, { LngLat } from "maplibre-gl";

export function lngLatToTileXY(lngLat: LngLat, zoom: number): [number, number] {
  const n = Math.pow(2, zoom);
  const x = Math.floor(((lngLat.lng + 180) / 360) * n);
  const latRad = (lngLat.lat * Math.PI) / 180;
  const y = Math.floor(
    ((1 - Math.log(Math.tan(latRad) + 1 / Math.cos(latRad)) / Math.PI) / 2) * n,
  );
  return [x, y] as [number, number];
}

export function tileXYToLngLat(x: number, y: number, zoom: number): LngLat {
  const n = Math.pow(2, zoom);
  const lng = (x / n) * 360 - 180;
  const latRad = Math.atan(Math.sinh(Math.PI * (1 - (2 * y) / n)));
  const lat = (latRad * 180) / Math.PI;
  return new LngLat(lng, lat);
}

/* Get the range of tiles that covers the current map viewport
 * @param {Object} map - Mapbox map object
 * @returns {Array} - Array of [x, y, w, h, z] representing the tile range
 */
export function getViewportTileRange(
  map: maplibregl.Map,
  isGlobeProjection: boolean,
): [number, number, number, number, number] {
  // Get the current zoom level
  const z = Math.max(0, Math.floor(map.getZoom()));

  // Get the bounds of the map
  const bounds = map.getBounds();
  const sw = bounds.getSouthWest();
  const ne = bounds.getNorthEast();

  // Convert to tile coordinates
  const [swX, swY] = lngLatToTileXY(sw, z);
  const [neX, neY] = lngLatToTileXY(ne, z);

  // Calculate the minimum x and y coordinates
  const x = Math.min(swX, neX);
  // for maplibre, the calculated y may be out of range, we need to crop is accordingly
  const y = Math.min(Math.max(0, Math.min(neY, swY)), (1 << z) - 1);

  // Calculate the width and height
  let w = Math.max(1, Math.abs(neX - swX) + 1);
  if (isGlobeProjection) {
    w = Math.min(w, 1 << z);
  }
  // w=1;
  // on special case when map is at border, make sure h will not exceed the limit.
  const h = Math.min(Math.max(1, Math.abs(swY - neY) + 1), (1 << z) - y);

  return [x, y, w, h, z];
}

/**
 * Convert tile X, Y, Z coordinates to a string key
 * @param {Number} x - X tile coordinate
 * @param {Number} y - Y tile coordinate
 * @param {Number} z - Zoom level
 * @returns {String} - Tile key in the format "z/x/y"
 */
export function tileXYZToKey(x: number, y: number, z: number): string {
  return `${z}/${x}/${y}`;
}

/**
 * Convert a tile key string to X, Y, Z coordinates
 * @param {String} key - Tile key in the format "z/x/y"
 * @returns {Object} - Object with x, y, z properties
 */
export function keyToTileXYZ(key: string): { x: number; y: number; z: number } {
  const [z, x, y] = key.split("/").map(Number);
  return { x, y, z };
}
