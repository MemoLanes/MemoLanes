const MAP_TILE_SIZE_EXPONENT = 9;

/**
 * Select the lowest source tile zoom whose native cells remain sub-pixel at
 * the live fractional map zoom.
 */
export function getScreenResolvedSourceTileZoom(
  mapZoom: number,
  bufferSizePower: number,
  maxSourceZoom: number,
  maxSourceCellWidthCss: number,
): number {
  const safeMapZoom = Number.isFinite(mapZoom) ? Math.max(0, mapZoom) : 0;
  const floorZoom = Math.floor(safeMapZoom);
  const safeMaxZoom = Number.isFinite(maxSourceZoom)
    ? Math.max(0, Math.floor(maxSourceZoom))
    : floorZoom;
  if (
    !Number.isFinite(bufferSizePower) ||
    bufferSizePower < 0 ||
    !Number.isFinite(maxSourceCellWidthCss) ||
    maxSourceCellWidthCss <= 0
  ) {
    return Math.min(floorZoom, safeMaxZoom);
  }

  const requiredZoom = Math.ceil(
    MAP_TILE_SIZE_EXPONENT +
      safeMapZoom -
      Math.floor(bufferSizePower) -
      Math.log2(maxSourceCellWidthCss),
  );
  return Math.min(safeMaxZoom, Math.max(floorZoom, requiredZoom));
}
