import type { Map as MaplibreMap } from "maplibre-gl";

/** Reverse single-finger tap-and-drag zoom. */
export function configureMapTouchGestures(
  map: Pick<MaplibreMap, "touchZoomRotate">,
): void {
  // touchZoomRotate.setZoomRate() also changes pinch zoom.
  // shortcut: Internal handler, switch when MapLibre exposes a separate rate.
  const tapDragZoom = map.touchZoomRotate._tapDragZoom;
  if (typeof tapDragZoom?.setZoomRate !== "function") {
    console.warn(
      "[MapTouchGestures] MapLibre tap-and-drag zoom rate is unavailable",
    );
    return;
  }

  tapDragZoom.setZoomRate(-1);
}
