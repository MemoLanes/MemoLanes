import assert from "node:assert/strict";
import test from "node:test";

import { LocationFollowController } from "./location-follow-controller.ts";

class FakeMap {
  moving = false;
  zoom = 15;
  calls = [];
  listeners = new Map();

  on(name, callback) {
    this.listeners.set(name, callback);
  }

  off(name, callback) {
    if (this.listeners.get(name) === callback) this.listeners.delete(name);
  }

  emit(name) {
    this.listeners.get(name)?.();
  }

  isMoving() {
    return this.moving;
  }

  getZoom() {
    return this.zoom;
  }

  project(center) {
    return { x: center[0], y: center[1] };
  }

  getCanvas() {
    return { clientWidth: 200, clientHeight: 100 };
  }

  flyTo(options) {
    this.calls.push({ method: "flyTo", ...options });
    this.moving = true;
  }

  easeTo(options) {
    this.calls.push({ method: "easeTo", ...options });
    this.moving = true;
  }

  stop() {
    this.moving = false;
    this.emit("moveend");
  }
}

test("keeps only the latest fix while a camera update is active", () => {
  const map = new FakeMap();
  const controller = new LocationFollowController(map, {
    minUpdateIntervalMs: 0,
  });

  controller.update(120, 60, true);
  controller.update(130, 70, true);
  controller.update(140, 80, true);
  assert.equal(map.calls.length, 1);

  map.moving = false;
  map.emit("moveend");
  assert.equal(map.calls.length, 2);
  assert.deepEqual(map.calls[1].center, [140, 80]);
  controller.dispose();
});

test("eases nearby tracking updates instead of flying with a fixed duration", () => {
  const map = new FakeMap();
  const controller = new LocationFollowController(map, {
    minUpdateIntervalMs: 0,
  });

  controller.update(120, 60, true);
  assert.equal(map.calls.length, 1);
  assert.equal(map.calls[0].method, "easeTo");
  assert.equal(map.calls[0].duration, 900);
  assert.equal(map.calls[0].zoom, undefined);
  assert.ok(map.calls[0].easing(0.5) > 0.5);
  controller.dispose();
});

test("uses MapLibre's distance-based flyTo for a far locate", () => {
  const map = new FakeMap();
  const controller = new LocationFollowController(map, {
    minUpdateIntervalMs: 0,
  });

  controller.update(400, 50, true);
  assert.equal(map.calls.length, 1);
  assert.equal(map.calls[0].method, "flyTo");
  assert.deepEqual(map.calls[0].center, [400, 50]);
  assert.equal(map.calls[0].zoom, 15);
  assert.equal(map.calls[0].duration, undefined);
  assert.equal(map.calls[0].easing, undefined);
  controller.dispose();
});

test("flies and zooms in when tracking starts below the follow zoom", () => {
  const map = new FakeMap();
  map.zoom = 10;
  const controller = new LocationFollowController(map, {
    minUpdateIntervalMs: 0,
  });

  controller.update(120, 60, true);
  assert.equal(map.calls.length, 1);
  assert.equal(map.calls[0].method, "flyTo");
  assert.equal(map.calls[0].zoom, 14);
  assert.equal(map.calls[0].duration, undefined);
  controller.dispose();
});

test("skips sub-pixel camera updates at tracking zoom", () => {
  const map = new FakeMap();
  const controller = new LocationFollowController(map, {
    minUpdateIntervalMs: 0,
    minMovementPixels: 2,
  });

  controller.update(101, 50, true);
  assert.equal(map.calls.length, 0);
  controller.dispose();
});

test("a user cancellation stops only the active presentation animation", () => {
  const map = new FakeMap();
  const controller = new LocationFollowController(map, {
    minUpdateIntervalMs: 0,
  });

  controller.update(120, 60, true);
  controller.cancel();
  assert.equal(map.moving, false);

  map.emit("moveend");
  assert.equal(map.calls.length, 1);
  controller.dispose();
});
