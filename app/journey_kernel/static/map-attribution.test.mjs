import assert from "node:assert/strict";
import test from "node:test";
import {
  BASEMAP_REQUEST_ID_METADATA_KEY,
  collectMapAttributions,
} from "./map-attribution.ts";

test("attribution comes from loaded TileJSON sources and tolerates incomplete style loads", () => {
  let style;
  const sources = {
    roads: { attribution: '<a href="https://example.com">Provider</a>' },
    water: { attribution: ' <a href="https://example.com">Provider</a> ' },
    terrain: { attribution: "Terrain data" },
  };
  const map = { getStyle: () => style, getSource: (id) => sources[id] };
  assert.equal(collectMapAttributions(map), undefined);
  // Attribution is often only in the loaded TileJSON, not in this style.
  style = { sources: { roads: {}, water: {}, terrain: {}, pending: {} } };
  assert.equal(collectMapAttributions(map), undefined);
  style.metadata = { [BASEMAP_REQUEST_ID_METADATA_KEY]: 1 };
  assert.deepEqual(collectMapAttributions(map), {
    requestId: 1,
    entries: [sources.roads.attribution, "Terrain data"],
  });
});
