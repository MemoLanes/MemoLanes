import assert from "node:assert/strict";
import test from "node:test";
import { captureMapFrame } from "./map-screenshot.ts";

function fakeMap() {
  const listeners = new Map();
  let rendering = false;
  return {
    listeners,
    failEncoding: false,
    on: (name, callback) => listeners.set(name, callback),
    off: (name) => listeners.delete(name),
    triggerRepaint() {},
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

test("captures the next rendered frame even while the map is moving or loading", async () => {
  const map = Object.assign(fakeMap(), {
    isMoving: () => true,
    loaded: () => false,
  });
  const capture = captureMapFrame(map);
  map.emit("render");
  assert.equal(await capture, "data:image/png;base64,test");
  assert.equal(map.listeners.size, 0);
});

for (const event of ["remove", "webglcontextlost"]) {
  test(`${event} cancels the capture and releases listeners`, async () => {
    const map = fakeMap();
    const capture = captureMapFrame(map);
    map.emit(event);
    await assert.rejects(capture, /Map unavailable/);
    assert.equal(map.listeners.size, 0);
  });
}

test("encoding errors release listeners and reject instead of exporting a blank image", async () => {
  const map = fakeMap();
  map.failEncoding = true;
  const capture = captureMapFrame(map);
  map.emit("render");
  await assert.rejects(capture, /Tainted canvas/);
  assert.equal(map.listeners.size, 0);
});

test("a stalled render times out and releases listeners", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const map = fakeMap();
  const capture = captureMapFrame(map);
  t.mock.timers.tick(10000);
  await assert.rejects(capture, /timed out/);
  assert.equal(map.listeners.size, 0);
});
