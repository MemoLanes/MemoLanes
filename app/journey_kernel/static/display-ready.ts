import type { Map } from "maplibre-gl";

/** Read a ready frame synchronously in render, before WebGL clears its buffer. */
export function waitForRenderedFrame<T>(
  map: Pick<Map, "on" | "off" | "triggerRepaint">,
  readFrame: () => T | undefined,
  captureTimeoutMs?: number,
): Promise<T> {
  return new Promise((resolve, reject) => {
    const cleanup = () => {
      map.off("render", onRender);
      map.off("remove", onUnavailable);
      if (timeout !== undefined) {
        clearTimeout(timeout);
        map.off("webglcontextlost", onUnavailable);
      }
    };
    const fail = (error: unknown) => {
      cleanup();
      reject(error);
    };
    const onUnavailable = () => {
      fail(
        new Error(
          captureTimeoutMs === undefined
            ? "Map removed before its first displayable frame"
            : "Map unavailable during screenshot",
        ),
      );
    };
    const onRender = () => {
      try {
        const frame = readFrame();
        if (frame === undefined) return;
        cleanup();
        resolve(frame);
      } catch (error) {
        fail(error);
      }
    };
    const timeout =
      captureTimeoutMs === undefined
        ? undefined
        : setTimeout(
            () => fail(new Error("Map screenshot timed out")),
            captureTimeoutMs,
          );
    map.on("render", onRender);
    map.on("remove", onUnavailable);
    // Initial display can wait for recovery. A capture needs this context.
    if (timeout !== undefined) map.on("webglcontextlost", onUnavailable);
    map.triggerRepaint();
  });
}
