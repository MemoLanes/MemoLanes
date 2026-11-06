import assert from "node:assert/strict";
import test from "node:test";

import { captureMapScreenshot } from "./map-screenshot.ts";

function fakeMap(readPixels) {
  const listeners = new Map();
  return {
    listeners,
    repaintRequested: false,
    on(type, listener) {
      listeners.set(type, listener);
    },
    off(type, listener) {
      assert.equal(listeners.get(type), listener);
      listeners.delete(type);
    },
    triggerRepaint() {
      this.repaintRequested = true;
    },
    getCanvas() {
      return { toDataURL: readPixels };
    },
    emit(type) {
      listeners.get(type)?.();
    },
  };
}

test("capture reads pixels synchronously inside render", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const png = "data:image/png;base64,test";
  let rendering = false;
  let reads = 0;
  const map = fakeMap((type) => {
    assert.equal(type, "image/png");
    assert.equal(rendering, true);
    reads++;
    return png;
  });
  const frame = captureMapScreenshot(map);
  assert.equal(map.repaintRequested, true);
  assert.equal(reads, 0);
  rendering = true;
  map.emit("render");
  rendering = false;
  assert.equal(reads, 1);
  assert.equal(await frame, png);
  assert.equal(map.listeners.size, 0);
  t.mock.timers.tick(10000);
});

test("read failures and timeouts release the capture listener", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  for (const event of ["render", "timeout"]) {
    const map = fakeMap(() => {
      throw new Error("Tainted canvas");
    });
    const frame = captureMapScreenshot(map);
    if (event === "timeout") t.mock.timers.tick(10000);
    else map.emit(event);
    await assert.rejects(
      frame,
      event === "timeout" ? /timed out/ : /Tainted canvas/,
    );
    assert.equal(map.listeners.size, 0);
    map.emit("render");
    t.mock.timers.tick(10000);
  }
});
