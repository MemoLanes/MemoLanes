import assert from "node:assert/strict";
import test from "node:test";
import { captureMapFrame } from "./map-screenshot.ts";

function fakeMap() {
  const listeners = new Map();
  let rendering = false;
  return {
    listeners,
    moving: false,
    ready: true,
    failEncoding: false,
    on: (name, callback) => listeners.set(name, callback),
    off: (name) => listeners.delete(name),
    triggerRepaint() {},
    isMoving() {
      return this.moving;
    },
    loaded() {
      return this.ready;
    },
    getCanvas() {
      return {
        width: 100,
        height: 200,
        toDataURL: () => {
          assert.equal(
            rendering,
            true,
            "Read pixels before the render callback returns",
          );
          if (this.failEncoding) throw new Error("Tainted canvas");
          return "data:image/png;base64,test";
        },
      };
    },
    emit(name) {
      rendering = name === "render";
      listeners.get(name)?.();
      rendering = false;
    },
  };
}

test("captures synchronously only after camera, map and tracks are ready", async () => {
  const map = fakeMap();
  let tracksReady = false;
  let resolved = false;
  const capture = captureMapFrame(map, () => tracksReady).then((value) => {
    resolved = true;
    return value;
  });
  map.emit("render");
  tracksReady = true;
  map.moving = true;
  map.emit("render");
  map.moving = false;
  map.ready = false;
  map.emit("render");
  await Promise.resolve();
  assert.equal(resolved, false);
  map.ready = true;
  map.emit("render");
  assert.equal(await capture, "data:image/png;base64,test");
  assert.equal(map.listeners.size, 0);
});

for (const event of ["remove", "webglcontextlost"]) {
  test(`${event} cancels the capture and releases listeners`, async () => {
    const map = fakeMap();
    const capture = captureMapFrame(map, () => false);
    map.emit(event);
    await assert.rejects(capture, /Map unavailable/);
    assert.equal(map.listeners.size, 0);
  });
}

test("encoding errors release listeners and reject instead of exporting a blank image", async () => {
  const map = fakeMap();
  map.failEncoding = true;
  const capture = captureMapFrame(map, () => true);
  map.emit("render");
  await assert.rejects(capture, /Tainted canvas/);
  assert.equal(map.listeners.size, 0);
});

test("a stalled render times out and releases listeners", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const map = fakeMap();
  const capture = captureMapFrame(map, () => false);
  t.mock.timers.tick(10000);
  await assert.rejects(capture, /timed out/);
  assert.equal(map.listeners.size, 0);
});
