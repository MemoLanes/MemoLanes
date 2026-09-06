/**
 * Existing decoded tiles remain displayable while panning inside their
 * desired coverage (including any bounded guard band). Zooming in to a finer
 * tile zoom, or consuming that coverage, must fetch before moveend.
 */
export function shouldDeferDataViewportUpdate(
  mapMoving: boolean,
  hasLoadedTileBuffer: boolean,
  loadedCoversDesiredRange: boolean,
  loadedTileZoomIsSufficient: boolean,
): boolean {
  return (
    mapMoving &&
    hasLoadedTileBuffer &&
    loadedCoversDesiredRange &&
    loadedTileZoomIsSufficient
  );
}

/**
 * Tracks zoom-out direction across move frames in the same zoom interaction.
 * A pinch can emit a pan-only frame, so an unchanged world size retains the
 * previous direction until zooming reverses or the interaction ends.
 */
export function resolveZoomingOutInteractionState(
  mapZooming: boolean,
  wasZoomingOut: boolean,
  previousWorldSize: number,
  currentWorldSize: number,
  epsilon: number,
): boolean {
  if (!mapZooming) return false;
  if (currentWorldSize < previousWorldSize - epsilon) return true;
  if (currentWorldSize > previousWorldSize + epsilon) return false;
  return wasZoomingOut;
}
