import assert from "node:assert/strict";
import test from "node:test";
import { captureCenteredMapFrame, captureMapFrame } from "./map-screenshot.ts";

for (const fails of [false, true]) {
  test(`centered capture keeps zoom and restores camera after ${fails ? "failure" : "success"}`, async () => {
    const original = {
      center: [120, 30],
      zoom: 12,
      bearing: 20,
      pitch: 25,
      padding: { top: 20, bottom: 240, left: 20, right: 20 },
    };
    let camera = { ...original };
    const bounds = [
      [179, 30],
      [181, 40],
    ];
    const map = {
      getCenter: () => ({ toArray: () => [...camera.center] }),
      getZoom: () => camera.zoom,
      getBearing: () => camera.bearing,
      getPitch: () => camera.pitch,
      getPadding: () => camera.padding,
      cameraForBounds: (actualBounds, options) => {
        assert.deepEqual(actualBounds, bounds);
        assert.equal(options.padding, 0);
        assert.equal(options.absolutePadding, true);
        assert.equal(options.bearing, original.bearing);
        return { center: [180, 35], zoom: 6 };
      },
      jumpTo: (options) => {
        camera = { ...camera, ...options };
      },
    };
    const capture = captureCenteredMapFrame(map, bounds, async () => {
      await Promise.resolve();
      assert.deepEqual(camera, {
        ...original,
        center: [180, 35],
        padding: { top: 0, bottom: 0, left: 0, right: 0 },
      });
      if (fails) throw new Error("Capture failed");
      return "centered PNG";
    });
    if (fails) await assert.rejects(capture, /Capture failed/);
    else assert.equal(await capture, "centered PNG");
    assert.deepEqual(camera, original);
  });
}

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
