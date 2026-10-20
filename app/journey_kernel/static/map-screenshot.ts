import type { LngLatBoundsLike, Map } from "maplibre-gl";

/** Capture a journey without the detail card's camera offset, then restore it. */
export async function captureCenteredMapFrame(
  map: Pick<
    Map,
    | "cameraForBounds"
    | "getCenter"
    | "getZoom"
    | "getBearing"
    | "getPitch"
    | "getPadding"
    | "jumpTo"
  >,
  bounds: LngLatBoundsLike,
  capture: () => Promise<string>,
): Promise<string> {
  const original = {
    center: map.getCenter().toArray(),
    zoom: map.getZoom(),
    bearing: map.getBearing(),
    pitch: map.getPitch(),
    padding: { ...map.getPadding() },
  };
  const center = map.cameraForBounds(bounds, {
    bearing: original.bearing,
    padding: 0,
    absolutePadding: true,
  })?.center;
  if (!center) throw new Error("Journey center is unavailable");
  try {
    // Only move the camera: retain the user's zoom, bearing and pitch.
    map.jumpTo({ center, padding: { top: 0, bottom: 0, left: 0, right: 0 } });
    return await capture();
  } finally {
    map.jumpTo(original);
  }
}

/** Read inside the render callback, before WebGL discards its drawing buffer. */
export function captureMapFrame(
  map: Pick<
    Map,
    "on" | "off" | "triggerRepaint" | "getCanvas" | "isMoving" | "loaded"
  >,
  isReady: () => boolean,
): Promise<string> {
  return new Promise((resolve, reject) => {
    const cleanup = () => {
      clearTimeout(timeout);
      map.off("render", onRender);
      map.off("remove", onUnavailable);
      map.off("webglcontextlost", onUnavailable);
    };
    const onUnavailable = () => {
      cleanup();
      reject(new Error("Map unavailable during screenshot"));
    };
    const onRender = () => {
      try {
        if (map.isMoving() || !map.loaded() || !isReady()) return;
        const canvas = map.getCanvas();
        if (!canvas.width || !canvas.height)
          throw new Error("Empty map canvas");
        const png = canvas.toDataURL("image/png");
        cleanup();
        resolve(png);
      } catch (error) {
        cleanup();
        reject(error);
      }
    };
    const timeout = setTimeout(() => {
      cleanup();
      reject(new Error("Map screenshot timed out"));
    }, 10000);
    map.on("render", onRender);
    map.on("remove", onUnavailable);
    map.on("webglcontextlost", onUnavailable);
    map.triggerRepaint();
  });
}
