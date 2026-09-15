import assert from "node:assert/strict";
import test from "node:test";

import { StyleLoadRetry } from "./style-load-retry.ts";

function fixture(t) {
  t.mock.method(console, "error", () => {});
  t.mock.timers.enable({ apis: ["setInterval"] });
  const previousWindow = globalThis.window;
  globalThis.window = { location: { href: "https://app.test/map/index.html" } };
  const listeners = new Map();
  const map = {
    on(type, listener) {
      listeners.set(type, listener);
    },
    off(type, listener) {
      if (listeners.get(type) === listener) listeners.delete(type);
    },
  };
  let attempts = 0;
  let contextAvailable = true;
  const retry = new StyleLoadRetry(
    map,
    () => {
      attempts++;
      retry.requestStarted("./style.json");
    },
    () => contextAvailable,
  );
  t.after(() => {
    retry.dispose();
    globalThis.window = previousWindow;
  });
  return {
    retry,
    listeners,
    attempts: () => attempts,
    contextAvailable: (value) => {
      contextAvailable = value;
    },
    fail: (url) =>
      listeners.get("error")?.({
        error: Object.assign(new Error("failed"), { url }),
      }),
  };
}

test("retries failed style downloads, then stops after a successful response", (t) => {
  const f = fixture(t);
  f.retry.requestStarted("./style.json");
  f.fail("https://app.test/map/style.json");
  t.mock.timers.tick(7_999);
  assert.equal(f.attempts(), 0);
  t.mock.timers.tick(1);
  assert.equal(f.attempts(), 1);
  f.fail("./style.json");
  t.mock.timers.tick(8_000);
  assert.equal(f.attempts(), 2);
  f.retry.responseReceived();
  f.fail("./style.json");
  t.mock.timers.tick(80_000);
  assert.equal(f.attempts(), 2);
});

test("slow requests and unrelated resource failures do not reload the style", (t) => {
  const f = fixture(t);
  f.retry.requestStarted("./style.json");
  for (const url of ["./tile.pbf", "./sprite.json", "./font.pbf", undefined]) {
    f.fail(url);
  }
  t.mock.timers.tick(80_000);
  assert.equal(f.attempts(), 0);
  f.retry.responseReceived();
  t.mock.timers.tick(80_000);
  assert.equal(f.attempts(), 0);
});

test("tracks the transformed request URL and ignores obsolete requests", (t) => {
  const f = fixture(t);
  f.retry.requestStarted("./style.json");
  f.fail("./style.json");
  f.retry.requestStarted(
    "https://api.mapbox.com/styles/v1/example/style?access_token=test",
  );
  f.fail("./style.json");
  t.mock.timers.tick(8_000);
  assert.equal(f.attempts(), 0);
  f.fail("https://api.mapbox.com/styles/v1/example/style?access_token=test");
  t.mock.timers.tick(8_000);
  assert.equal(f.attempts(), 1);
});

test("defers retries during context loss and resumes when available", (t) => {
  const f = fixture(t);
  f.retry.requestStarted("./style.json");
  f.fail("./style.json");
  f.contextAvailable(false);
  t.mock.timers.tick(16_000);
  assert.equal(f.attempts(), 0);
  f.contextAvailable(true);
  t.mock.timers.tick(8_000);
  assert.equal(f.attempts(), 1);
});

test("disposing removes the error listener and pending retry timer", (t) => {
  const f = fixture(t);
  f.retry.requestStarted("./style.json");
  f.fail("./style.json");
  f.retry.dispose();
  assert.equal(f.listeners.size, 0);
  t.mock.timers.tick(16_000);
  assert.equal(f.attempts(), 0);
});
