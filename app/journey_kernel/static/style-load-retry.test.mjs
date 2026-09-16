import assert from "node:assert/strict";
import test from "node:test";

import {
  PLACEHOLDER_STYLE_METADATA_KEY,
  StyleLoadRetry,
} from "./style-load-retry.ts";

function fixture(t) {
  t.mock.timers.enable({ apis: ["setInterval"] });
  const state = {
    style: {
      metadata: { [PLACEHOLDER_STYLE_METADATA_KEY]: true },
      layers: [{ id: "background" }, { id: "journey" }, { id: "poles" }],
    },
    contextAvailable: true,
    attempts: 0,
  };
  const retry = new StyleLoadRetry(
    { getStyle: () => state.style },
    () => {
      state.attempts++;
    },
    () => state.contextAvailable,
  );
  t.after(() => retry.dispose());
  return { state, retry };
}

test("retries the placeholder despite overlay layers and stops after a real style replaces it", (t) => {
  const { state } = fixture(t);
  const placeholder = state.style;
  t.mock.timers.tick(7_999);
  assert.equal(state.attempts, 0);
  t.mock.timers.tick(1);
  assert.equal(state.attempts, 1);
  t.mock.timers.tick(8_000);
  assert.equal(state.attempts, 2);

  // A valid basemap can contain just one background layer.
  state.style = { layers: [{ id: "background" }] };
  t.mock.timers.tick(8_000);
  state.style = placeholder;
  t.mock.timers.tick(16_000);
  assert.equal(state.attempts, 2);
});

test("waits through context/style restoration and cancels pending retries on disposal", (t) => {
  const { state, retry } = fixture(t);
  const placeholder = state.style;
  state.contextAvailable = false;
  t.mock.timers.tick(8_000);
  assert.equal(state.attempts, 0);

  state.contextAvailable = true;
  state.style = undefined;
  t.mock.timers.tick(8_000);
  assert.equal(state.attempts, 0);

  state.style = placeholder;
  t.mock.timers.tick(8_000);
  assert.equal(state.attempts, 1);
  retry.dispose();
  t.mock.timers.tick(16_000);
  assert.equal(state.attempts, 1);
});
