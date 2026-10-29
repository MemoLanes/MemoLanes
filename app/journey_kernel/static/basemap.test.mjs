import assert from "node:assert/strict";
import test from "node:test";
import { resolveStyleResourceUrls } from "./utils.ts";
import {
  BASEMAP_REQUEST_ID_METADATA_KEY,
  collectMapAttributions,
} from "./map-attribution.ts";

test("relative resources use the style URL and preserve templates and keys", () => {
  const style = {
    sources: { planet: { type: "vector", url: "/tiles.json?key=a+b" } },
    glyphs: "./fonts/{fontstack}/{range}.pbf",
  };
  resolveStyleResourceUrls(style, "https://example.com/maps/style.json");
  assert.equal(
    style.sources.planet.url,
    "https://example.com/tiles.json?key=a+b",
  );
  assert.equal(
    style.glyphs,
    "https://example.com/maps/fonts/{fontstack}/{range}.pbf",
  );
});

test("attribution comes from loaded sources and keeps the applied request ID", () => {
  let style;
  const map = {
    getStyle: () => style,
    getSource: (id) =>
      id === "pending" ? undefined : { attribution: " Provider " },
  };
  assert.equal(collectMapAttributions(map), undefined);
  style = {
    metadata: { [BASEMAP_REQUEST_ID_METADATA_KEY]: 7 },
    sources: { roads: {}, water: {}, pending: {} },
  };
  assert.deepEqual(collectMapAttributions(map), {
    requestId: 7,
    entries: ["Provider"],
  });
});
