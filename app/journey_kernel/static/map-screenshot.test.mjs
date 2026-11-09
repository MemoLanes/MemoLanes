import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";

import { captureMapScreenshot } from "./map-screenshot.ts";

function fakeMap(t, readPixels) {
  return Object.assign(new EventEmitter(), {
    triggerRepaint: t.mock.fn(),
    getCanvas: () => ({ toDataURL: readPixels }),
  });
}

test("capture reads pixels synchronously inside render", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const png = "data:image/png;base64,test";
  const readPixels = t.mock.fn(() => png);
  const map = fakeMap(t, readPixels);
  const frame = captureMapScreenshot(map);
  assert.equal(map.triggerRepaint.mock.callCount(), 1);
  assert.equal(readPixels.mock.callCount(), 0);
  map.emit("render");
  assert.equal(readPixels.mock.callCount(), 1);
  assert.equal(await frame, png);
  assert.equal(map.listenerCount("render"), 0);
});

test("read failures and timeouts release the capture listener", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  for (const event of ["render", "timeout"]) {
    const map = fakeMap(t, () => {
      throw new Error("Tainted canvas");
    });
    const frame = captureMapScreenshot(map);
    if (event === "timeout") t.mock.timers.tick(10000);
    else map.emit(event);
    await assert.rejects(
      frame,
      event === "timeout" ? /timed out/ : /Tainted canvas/,
    );
    assert.equal(map.listenerCount("render"), 0);
  }
});
