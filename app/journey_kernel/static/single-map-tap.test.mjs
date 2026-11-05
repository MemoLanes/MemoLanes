import assert from "node:assert/strict";
import test from "node:test";
import { observeSingleMapTap } from "./single-map-tap.ts";

function fixture(t) {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const canvas = new EventTarget();
  const mapEvents = new EventTarget();
  const previousWindow = globalThis.window;
  globalThis.window = new EventTarget();
  let taps = 0;
  const stop = observeSingleMapTap(
    {
      getCanvas: () => canvas,
      on: mapEvents.addEventListener.bind(mapEvents),
      off: mapEvents.removeEventListener.bind(mapEvents),
    },
    () => taps++,
  );
  t.after(() => {
    stop();
    if (previousWindow === undefined) delete globalThis.window;
    else globalThis.window = previousWindow;
  });
  function emit(type, properties = {}) {
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
    assert.equal(event.defaultPrevented, false);
  }
  return {
    emit,
    tap(duration = 50) {
      emit("pointerdown");
      emit("pointerup", { timeStamp: duration });
    },
    zoom: () => mapEvents.dispatchEvent(new Event("zoomstart")),
    taps: () => taps,
  };
}

test("a single tap toggles the record controls after the double-tap window", (t) => {
  const f = fixture(t);
  f.tap();
  f.emit("pointerleave");
  t.mock.timers.tick(420);
  assert.equal(f.taps(), 0);
  t.mock.timers.tick(80);
  assert.equal(f.taps(), 1);
});

test("a second tap after 420 ms cancels the mode change", (t) => {
  const f = fixture(t);
  f.tap();
  t.mock.timers.tick(420);
  f.tap();
  t.mock.timers.tick(500);
  assert.equal(f.taps(), 0);
});

test("map zoom cancels a tap even if its first press was too long to toggle controls", (t) => {
  const f = fixture(t);
  f.tap(400);
  t.mock.timers.tick(100);
  f.tap();
  f.zoom();
  t.mock.timers.tick(500);
  assert.equal(f.taps(), 0);
});

test("drags, pinches and cancelled touches leave the controls alone", (t) => {
  const f = fixture(t);
  f.emit("pointerdown");
  f.emit("pointermove", { clientX: 23 });
  f.emit("pointerup");
  t.mock.timers.tick(500);
  f.emit("pointerdown");
  f.emit("pointerdown", { pointerId: 2 });
  f.emit("pointerup", { pointerId: 2 });
  f.emit("pointerup");
  t.mock.timers.tick(500);
  f.emit("pointerdown");
  f.emit("pointercancel");
  f.emit("pointerup");
  t.mock.timers.tick(500);
  assert.equal(f.taps(), 0);
});
