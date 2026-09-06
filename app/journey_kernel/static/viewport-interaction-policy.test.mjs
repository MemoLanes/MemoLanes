import assert from "node:assert/strict";
import test from "node:test";
import {
  shouldDeferDataViewportUpdate,
  resolveZoomingOutInteractionState,
} from "./viewport-interaction-policy.ts";

test("covered pans can defer but finer or uncovered viewports fetch immediately", () => {
  assert.equal(shouldDeferDataViewportUpdate(true, true, true, true), true);
  for (const inputs of [
    [false, true, true, true],
    [true, false, true, true],
    [true, true, false, true],
    [true, true, true, false],
  ]) {
    assert.equal(shouldDeferDataViewportUpdate(...inputs), false);
  }
});
test("pan-only frames preserve zoom-out direction until reversal or moveend", () => {
  assert.equal(
    resolveZoomingOutInteractionState(true, false, 100, 90, 1e-6),
    true,
  );
  assert.equal(
    resolveZoomingOutInteractionState(true, true, 90, 90, 1e-6),
    true,
  );
  assert.equal(
    resolveZoomingOutInteractionState(true, true, 90, 100, 1e-6),
    false,
  );
  assert.equal(
    resolveZoomingOutInteractionState(false, true, 90, 90, 1e-6),
    false,
  );
});
