import type { Map } from "maplibre-gl";

export function captureMapScreenshot(
  map: Pick<Map, "on" | "off" | "triggerRepaint" | "getCanvas">,
): Promise<string> {
  return new Promise((resolve, reject) => {
    const cleanup = () => {
      clearTimeout(timeout);
      map.off("render", onRender);
    };
    const onRender = () => {
      cleanup();
      try {
        // Read in the render callback before WebGL clears the drawing buffer.
        resolve(map.getCanvas().toDataURL("image/png"));
      } catch (error) {
        reject(error);
      }
    };
    const timeout = setTimeout(() => {
      cleanup();
      reject(new Error("Map screenshot timed out"));
    }, 10000);
    map.on("render", onRender);
    map.triggerRepaint();
  });
}
