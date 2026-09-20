import assert from "node:assert/strict";
import test from "node:test";

import { waitForRenderedFrame } from "./display-ready.ts";

function fakeMap() {
  const listeners = new Map();
  const calls = [];
  return {
    calls,
    on(type, listener) {
      listeners.set(type, listener);
    },
    off(type, listener) {
      assert.equal(listeners.get(type), listener);
      listeners.delete(type);
      calls.push(`off:${type}`);
    },
    triggerRepaint() {
      calls.push("repaint");
    },
    emit(type) {
      listeners.get(type)?.();
    },
  };
}

test("an unready render does not release the display gate", async () => {
  const map = fakeMap();
  let ready = false;
  let settled = false;
  const frame = waitForRenderedFrame(map, () => ready).then(() => {
    settled = true;
  });

  map.emit("render");
  await Promise.resolve();
  assert.equal(settled, false);

  ready = true;
  map.emit("render");
  await frame;
  assert.deepEqual(map.calls, ["repaint", "off:render", "off:remove"]);
});

test("a ready map still waits for a real render and then removes listeners", async () => {
  const map = fakeMap();
  let settled = false;
  const frame = waitForRenderedFrame(map, () => true).then(() => {
    settled = true;
  });

  await Promise.resolve();
  assert.equal(settled, false);
  map.emit("render");
  await frame;
  assert.deepEqual(map.calls, ["repaint", "off:render", "off:remove"]);
});

test("map removal rejects and cleans up both listeners", async () => {
  const map = fakeMap();
  const frame = waitForRenderedFrame(map, () => false);

  map.emit("remove");
  await assert.rejects(frame, {
    message: "Map removed before its first displayable frame",
  });
  assert.deepEqual(map.calls, ["repaint", "off:render", "off:remove"]);
});
