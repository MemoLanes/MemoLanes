import assert from "node:assert/strict";
import test from "node:test";
import { observeSingleMapTap } from "./single-map-tap.ts";

function fixture(t) {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const canvas = new EventTarget();
  const previousWindow = globalThis.window;
  globalThis.window = new EventTarget();
  let taps = 0;
  const stop = observeSingleMapTap(canvas, () => taps++);
  t.after(() => {
    stop();
    if (previousWindow === undefined) delete globalThis.window;
    else globalThis.window = previousWindow;
  });
  return {
    stop,
    get taps() {
      return taps;
    },
    emit(type, properties = {}) {
      const event = new Event(type, { cancelable: true });
      for (const [key, value] of Object.entries({
        pointerId: 1,
        clientX: 20,
        clientY: 20,
        button: 0,
        timeStamp: 0,
        ...properties,
      }))
        Object.defineProperty(event, key, { value });
      canvas.dispatchEvent(event);
      assert.equal(event.defaultPrevented, false, "Map gestures stay enabled");
    },
  };
}

test("a short tap fires once, including touch pointerleave after release", (t) => {
  const f = fixture(t);
  f.emit("pointerdown");
  f.emit("pointerup", { timeStamp: 100 });
  f.emit("pointerleave", { timeStamp: 100 });
  assert.equal(f.taps, 0);
  t.mock.timers.tick(350);
  assert.equal(f.taps, 1);
  t.mock.timers.tick(1000);
  assert.equal(f.taps, 1);
});

test("a second tap cancels single-tap mode changes", (t) => {
  const f = fixture(t);
  f.emit("pointerdown");
  f.emit("pointerup", { timeStamp: 50 });
  t.mock.timers.tick(100);
  f.emit("pointerdown", { timeStamp: 150 });
  f.emit("pointerup", { timeStamp: 200 });
  t.mock.timers.tick(500);
  assert.equal(f.taps, 0);
});

test("small map drags are not taps, but sub-tolerance jitter is allowed", (t) => {
  const f = fixture(t);
  for (const distance of [3, 5, 8]) {
    f.emit("pointerdown");
    f.emit("pointermove", { clientX: 20 + distance });
    f.emit("pointerup", { timeStamp: 100 });
    t.mock.timers.tick(500);
    assert.equal(f.taps, 0, `A ${distance}px drag must not toggle controls`);
  }
  f.emit("pointerdown");
  f.emit("pointerup", { clientX: 22, timeStamp: 100 });
  t.mock.timers.tick(500);
  assert.equal(f.taps, 1);
});

test("dragging out and back, long press and right click are not taps", (t) => {
  const f = fixture(t);
  f.emit("pointerdown");
  f.emit("pointermove", { clientX: 50 });
  f.emit("pointerup", { timeStamp: 100 });
  t.mock.timers.tick(500);
  f.emit("pointerdown");
  f.emit("pointerup", { timeStamp: 1000 });
  t.mock.timers.tick(500);
  f.emit("pointerdown", { button: 2 });
  f.emit("pointerup", { timeStamp: 100 });
  t.mock.timers.tick(500);
  assert.equal(f.taps, 0);
});

test("two-finger gestures and cancelled pointers never toggle the mode", (t) => {
  const f = fixture(t);
  f.emit("pointerdown");
  f.emit("pointerdown", { pointerId: 2 });
  f.emit("pointerup", { pointerId: 2, timeStamp: 100 });
  f.emit("pointerup", { timeStamp: 150 });
  t.mock.timers.tick(500);
  f.emit("pointerdown");
  f.emit("pointercancel");
  f.emit("pointerup", { timeStamp: 100 });
  t.mock.timers.tick(500);
  assert.equal(f.taps, 0);
});

test("leaving mid-gesture or disposing cancels callbacks and listeners", (t) => {
  const f = fixture(t);
  f.emit("pointerdown");
  f.emit("pointerleave");
  f.emit("pointerup", { timeStamp: 100 });
  t.mock.timers.tick(500);
  assert.equal(f.taps, 0);
  f.emit("pointerdown");
  f.emit("pointerup", { timeStamp: 100 });
  f.stop();
  t.mock.timers.tick(500);
  f.emit("pointerdown");
  f.emit("pointerup", { timeStamp: 100 });
  t.mock.timers.tick(500);
  assert.equal(f.taps, 0);
});
