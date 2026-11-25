/** Bounded real WebGL loss/recovery for tests sharing a live MapLibre instance. */
export async function withDeadline(promise, message, ms = 5000) {
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error(message)), ms);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

export async function loseContext(map) {
  const gl = map.getCanvas().getContext("webgl2");
  const extension = gl?.getExtension("WEBGL_lose_context");
  if (!extension) throw new Error("WEBGL_lose_context unavailable");
  const lost = map.once("webglcontextlost");
  extension.loseContext();
  await withDeadline(lost, "MapLibre did not observe real context loss");
  if (!gl.isContextLost() || map.getStyle()) {
    throw new Error("context loss did not retire the live style");
  }
  return async () => {
    const restored = map.once("webglcontextrestored");
    const styleLoaded = map.once("style.load");
    // Let the native loss event finish dispatching before requesting recovery.
    await new Promise((resolve) => setTimeout(resolve, 100));
    extension.restoreContext();
    await withDeadline(
      restored,
      "MapLibre did not observe real context restoration",
    );
    await withDeadline(
      styleLoaded,
      "same MapLibre instance did not restore its style",
    );
    if (gl.isContextLost() || !map.getStyle()) {
      throw new Error("restoration did not recover the live style");
    }
  };
}
