import type { Map } from "maplibre-gl";

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
