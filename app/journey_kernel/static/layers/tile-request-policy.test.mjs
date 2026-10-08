import assert from "node:assert/strict";
import test from "node:test";

import { getScreenResolvedSourceTileZoom } from "./source-tile-zoom.ts";

test("prefetches a finer source LOD before fractional cells become jagged", () => {
  assert.equal(getScreenResolvedSourceTileZoom(8, 10, 12, 0.7), 8);
  assert.equal(getScreenResolvedSourceTileZoom(8.4, 10, 12, 0.7), 8);
  assert.equal(getScreenResolvedSourceTileZoom(8.9, 10, 12, 0.7), 9);
  assert.equal(getScreenResolvedSourceTileZoom(9.12, 10, 12, 0.7), 9);
  assert.equal(getScreenResolvedSourceTileZoom(11.9, 10, 12, 0.7), 12);
  assert.equal(getScreenResolvedSourceTileZoom(12.9, 10, 12, 0.7), 12);
});
