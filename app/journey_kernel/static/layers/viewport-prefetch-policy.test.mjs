import assert from "node:assert/strict";
import test from "node:test";

import { getBoundedViewportPrefetchRange } from "./tile-range.ts";

test("adds a one-tile guard band inside the resource limits", () => {
  assert.deepEqual(
    getBoundedViewportPrefetchRange([100, 200, 2, 3, 9], 1, 24, 16),
    [99, 199, 4, 5, 9],
  );
});

test("falls back to the visible range when total tiles exceed the cap", () => {
  assert.deepEqual(
    getBoundedViewportPrefetchRange([100, 200, 3, 4, 9], 1, 24, 16),
    [100, 200, 3, 4, 9],
  );
});

test("falls back when the guard band adds too many tiles", () => {
  assert.deepEqual(
    getBoundedViewportPrefetchRange([100, 200, 2, 3, 9], 1, 24, 12),
    [100, 200, 2, 3, 9],
  );
});

test("does not request duplicate world copies at low zoom", () => {
  assert.deepEqual(
    getBoundedViewportPrefetchRange([1, 0, 1, 1, 1], 1, 24, 16),
    [1, 0, 2, 2, 1],
  );
  assert.deepEqual(
    getBoundedViewportPrefetchRange([0, 0, 1, 1, 0], 1, 24, 16),
    [0, 0, 1, 1, 0],
  );
});
