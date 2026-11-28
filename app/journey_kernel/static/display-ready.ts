import type { Map } from "maplibre-gl";

/** Share the WebView's wait/notify sequence with the real-map browser fixture. */
export async function notifyWhenDisplayReady(
  controller: { waitForDisplay(): Promise<void> },
  notify: () => void,
): Promise<void> {
  await controller.waitForDisplay();
  notify();
}

/** Wait for a completed map render, rather than guessing a rendering delay. */
export function waitForRenderedFrame(
  map: Pick<Map, "on" | "off" | "triggerRepaint">,
  isReady: () => boolean,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const cleanup = () => {
      map.off("render", onRender);
      map.off("remove", onRemove);
    };
    const onRender = () => {
      if (!isReady()) return;
      cleanup();
      resolve();
    };
    const onRemove = () => {
      cleanup();
      reject(new Error("Map removed before its first displayable frame"));
    };
    map.on("render", onRender);
    map.on("remove", onRemove);
    map.triggerRepaint();
  });
}
