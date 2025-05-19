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