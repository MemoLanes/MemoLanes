import { waitForIdle } from "./map-fixture";

/** Empty journey data and a transparent basemap should leave one 50% fog cover. */
export async function checkFogFrames(map, changeView, check, label) {
  const gl = map.getCanvas().getContext("webgl2");
  const pixel = new Uint8Array(4);
  const frames = [];
  const sample = () => {
    const alphas = [];
    for (const x of [0.375, 0.5, 0.625])
      for (const y of [0.375, 0.5, 0.625]) {
        gl.readPixels(
          Math.floor(x * gl.drawingBufferWidth),
          Math.floor(y * gl.drawingBufferHeight),
          1,
          1,
          gl.RGBA,
          gl.UNSIGNED_BYTE,
          pixel,
        );
        alphas.push(pixel[3]);
      }
    frames.push([Math.min(...alphas), Math.max(...alphas)]);
  };
  map.on("render", sample);
  try {
    changeView();
    await waitForIdle(map);
  } finally {
    map.off("render", sample);
  }
  check(
    frames.length > 0 && frames.every(([min, max]) => min >= 126 && max <= 130),
    `${label}: fog must cover once on every frame; alpha ranges ${JSON.stringify(frames)}`,
  );
}
