import assert from "node:assert/strict";
import test from "node:test";
import { BasemapLoadGuard } from "./style-load-retry.ts";
import { resolveStyleResourceUrls } from "./utils.ts";
import {
  BASEMAP_REQUEST_ID_METADATA_KEY,
  collectMapAttributions,
} from "./map-attribution.ts";

function fixture(t, type = "vector") {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const listeners = new Map();
  const state = { failures: 0 };
  const map = {
    getStyle: () => ({ sources: { basemap: { type } } }),
    on(name, callback) {
      if (!listeners.has(name)) listeners.set(name, new Set());
      listeners.get(name).add(callback);
    },
    off(name, callback) {
      listeners.get(name)?.delete(callback);
    },
  };
  const guard = new BasemapLoadGuard(
    map,
    () => state.failures++,
    new Set(["basemap"]),
  );
  t.after(() => guard.dispose());
  const emit = (name, event) => {
    for (const callback of [...(listeners.get(name) || [])]) callback(event);
  };
  return { state, guard, emit };
}

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

test("a pending style ignores old SDK events and falls back after 20 seconds", async (t) => {
  const { state, emit } = fixture(t);
  emit("error", { error: new Error("old request failed") });
  emit("style.load");
  emit("idle");
  t.mock.timers.tick(19_999);
  await Promise.resolve();
  assert.equal(state.failures, 0);
  t.mock.timers.tick(1);
  await Promise.resolve();
  assert.equal(state.failures, 1);
});

test("invalid styles report one fallback after event dispatch", async (t) => {
  const { state, guard, emit } = fixture(t);
  guard.watchStyle();
  emit("error", { error: new Error("invalid style") });
  assert.equal(state.failures, 0);
  await Promise.resolve();
  assert.equal(state.failures, 1);
  t.mock.timers.tick(60_000);
  await Promise.resolve();
  assert.equal(state.failures, 1);
});

for (const [name, type, tile, failures] of [
  ["raster tile failure", "raster", { state: "errored" }, 1],
  [
    "vector 404",
    "vector",
    {
      state: "loaded",
      latestFeatureIndex: null,
      latestRawTileData: new ArrayBuffer(8),
    },
    1,
  ],
  [
    "valid empty vector tile",
    "vector",
    {
      state: "loaded",
      latestFeatureIndex: {},
      latestRawTileData: new ArrayBuffer(0),
    },
    0,
  ],
]) {
  test(name, async (t) => {
    const { state, guard, emit } = fixture(t, type);
    guard.watchStyle();
    emit("style.load");
    // Fog tiles do not establish that the basemap loaded successfully.
    emit("sourcedata", { sourceId: "journey", tile: { state: "loaded" } });
    emit("sourcedataloading", {
      sourceId: "basemap",
      tile: { state: "loading" },
    });
    emit("sourcedata", { sourceId: "basemap", tile });
    emit("idle");
    t.mock.timers.tick(60_000);
    await Promise.resolve();
    assert.equal(state.failures, failures);
  });
}

test("disposing a load cancels its already queued fallback", async (t) => {
  const { state, guard, emit } = fixture(t);
  guard.watchStyle();
  emit("error", { error: new Error("invalid style") });
  guard.dispose();
  t.mock.timers.tick(60_000);
  await Promise.resolve();
  assert.equal(state.failures, 0);
});
