import assert from "node:assert/strict";
import test from "node:test";
import { captureMapFrame } from "./map-screenshot.ts";

function fakeMap(toDataURL) {
  const listeners = new Map();
  return {
    listeners,
    on: (name, callback) => listeners.set(name, callback),
    off: (name) => listeners.delete(name),
    triggerRepaint() {},
    getCanvas: () => ({ width: 100, height: 200, toDataURL }),
    emit: (name) => listeners.get(name)?.(),
  };
}

test("reads pixels during the first render with fresh tracks", async () => {
  let ready = false;
  let rendering = false;
  const png = "data:image/png;base64,test";
  const map = fakeMap(() => {
    assert.equal(rendering, true);
    return png;
  });
  const capture = captureMapFrame(map, () => ready);
  map.emit("render");
  assert.equal(map.listeners.has("render"), true);
  ready = true;
  rendering = true;
  map.emit("render");
  rendering = false;
  assert.equal(await capture, png);
  assert.equal(map.listeners.size, 0);
});

test("failed captures release their listeners", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  for (const event of ["remove", "webglcontextlost", "render", "timeout"]) {
    const map = fakeMap(() => {
      throw new Error("Tainted canvas");
    });
    const capture = captureMapFrame(map, () => event === "render");
    if (event === "timeout") t.mock.timers.tick(10000);
    else map.emit(event);
    await assert.rejects(capture, /Map unavailable|Tainted canvas|timed out/);
    assert.equal(map.listeners.size, 0);
  }
});
