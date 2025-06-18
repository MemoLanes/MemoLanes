/**
 * Convert longitude/latitude coordinates to tile XY coordinates at a specific zoom level
 * @param {Array} lngLat - Array of [longitude, latitude]
 * @param {Number} zoom - Zoom level
 * @returns {Array} - Array of [x, y] tile coordinates
 */
export function lngLatToTileXY([lng, lat], zoom) {
    const n = Math.pow(2, zoom);
    const x = Math.floor((lng + 180) / 360 * n);
    const latRad = lat * Math.PI / 180;
    const y = Math.floor((1 - Math.log(Math.tan(latRad) + 1 / Math.cos(latRad)) / Math.PI) / 2 * n);
    return [x, y];
}

/**
 * Convert tile XY coordinates to longitude/latitude at a specific zoom level
 * @param {Array} tileXY - Array of [x, y] tile coordinates
 * @param {Number} zoom - Zoom level
 * @returns {Array} - Array of [longitude, latitude]
 */
export function tileXYToLngLat([x, y], zoom) {
    const n = Math.pow(2, zoom);
    const lng = x / n * 360 - 180;
    const latRad = Math.atan(Math.sinh(Math.PI * (1 - 2 * y / n)));
    const lat = latRad * 180 / Math.PI;
    return [lng, lat];
}

/**
 * Get the range of tiles that covers the current map viewport
 * @param {Object} map - Mapbox map object
 * @returns {Array} - Array of [x, y, w, h, z] representing the tile range
 */
export function getViewportTileRange(map) {
    // Get the current zoom level
    const z = Math.max(0, Math.floor(map.getZoom()));
    
    // Get the bounds of the map
    const bounds = map.getBounds();
    const sw = bounds.getSouthWest();
    const ne = bounds.getNorthEast();
    
    // Convert to tile coordinates
    const [swX, swY] = lngLatToTileXY([sw.lng, sw.lat], z);
    const [neX, neY] = lngLatToTileXY([ne.lng, ne.lat], z);
    
    // Calculate the minimum x and y coordinates
    const x = Math.min(swX, neX);
    const y = Math.min(neY, swY);
    
    // Calculate the width and height
    const w = Math.max(1, Math.abs(neX - swX)+1);
    // on special case when map is at border, make sure h will not exceed the limit.
    const h = Math.min(Math.max(1, Math.abs(swY - neY)+1), (1 << z) - y);
    
    return [x, y, w, h, z];
}

/**
 * Convert tile X, Y, Z coordinates to a string key
 * @param {Number} x - X tile coordinate
 * @param {Number} y - Y tile coordinate
 * @param {Number} z - Zoom level
 * @returns {String} - Tile key in the format "z/x/y"
 */
export function tileXYZToKey(x, y, z) {
    return `${z}/${x}/${y}`;
}

/**
 * Convert a tile key string to X, Y, Z coordinates
 * @param {String} key - Tile key in the format "z/x/y"
 * @returns {Object} - Object with x, y, z properties
 */
export function keyToTileXYZ(key) {
    const [z, x, y] = key.split('/').map(Number);
    return { x, y, z };
} 