import assert from "node:assert/strict";
import test from "node:test";

import { waitForRenderedFrame } from "./display-ready.ts";

function fakeMap() {
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
    emit(type) {
      listeners.get(type)?.();
    },
  };
}

test("initial display waits for a ready render, including after context loss", async () => {
  const map = fakeMap();
  let ready = false;
  const frame = waitForRenderedFrame(map, () => ready || undefined);
  assert.equal(map.repaintRequested, true);
  map.emit("render");
  assert.equal(map.listeners.has("render"), true);
  map.emit("webglcontextlost");
  ready = true;
  map.emit("render");
  assert.equal(await frame, true);
  assert.equal(map.listeners.size, 0);
});

test("capture reads pixels synchronously inside render", async () => {
  const map = fakeMap();
  const png = "data:image/png;base64,test";
  let rendering = false;
  const frame = waitForRenderedFrame(
    map,
    () => {
      assert.equal(rendering, true);
      return png;
    },
    10000,
  );
  rendering = true;
  map.emit("render");
  rendering = false;
  assert.equal(await frame, png);
  assert.equal(map.listeners.size, 0);
});

test("failed frame reads release their listeners", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  for (const [event, timeout, error] of [
    ["remove", undefined, /Map removed/],
    ["webglcontextlost", 10, /Map unavailable/],
    ["render", 10, /Tainted canvas/],
    ["timeout", 10, /timed out/],
  ]) {
    const map = fakeMap();
    const frame = waitForRenderedFrame(
      map,
      () => {
        throw new Error("Tainted canvas");
      },
      timeout,
    );
    if (event === "timeout") t.mock.timers.tick(timeout);
    else map.emit(event);
    await assert.rejects(frame, error);
    assert.equal(map.listeners.size, 0);
  }
});
