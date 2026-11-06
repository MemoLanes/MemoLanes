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
    stop,
    tap(duration = 50) {
      emit("pointerdown");
      emit("pointerup", { timeStamp: duration });
    },
    fireMapEvent(type, inputType) {
      const event = new Event(type);
      if (inputType) event.originalEvent = { type: inputType };
      mapEvents.dispatchEvent(event);
    },
    taps: () => taps,
  };
}

test("a single tap waits for the double-tap window and a second tap cancels it", (t) => {
  const f = fixture(t);
  f.tap();
  f.emit("pointerleave");
  f.fireMapEvent("movestart");
  t.mock.timers.tick(420);
  assert.equal(f.taps(), 0);
  t.mock.timers.tick(80);
  assert.equal(f.taps(), 1);

  f.tap();
  t.mock.timers.tick(420);
  f.tap();
  t.mock.timers.tick(500);
  assert.equal(f.taps(), 1);
});

test("leaving the map tab cancels the pending tap and detaches the observer", (t) => {
  const f = fixture(t);
  f.tap();
  f.stop();
  f.tap();
  t.mock.timers.tick(500);
  assert.equal(f.taps(), 0);
});

test("recognized zoom gestures cancel a tap after a longer first press", (t) => {
  const f = fixture(t);
  for (const [type, inputType] of [
    ["zoomstart"],
    ["movestart", "touchend"],
    ["movestart", "dblclick"],
  ]) {
    f.tap(400);
    t.mock.timers.tick(100);
    f.tap();
    f.fireMapEvent(type, inputType);
    t.mock.timers.tick(500);
    assert.equal(f.taps(), 0, inputType ?? type);
  }
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
