import assert from "node:assert/strict";
import test from "node:test";
import {
  BasemapLoadGuard,
  PLACEHOLDER_STYLE_METADATA_KEY,
} from "./style-load-retry.ts";

function fixture(t, watchStyle = true) {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const listeners = new Map();
  const state = {
    style: {
      metadata: { [PLACEHOLDER_STYLE_METADATA_KEY]: true },
      sources: {},
    },
    fallbacks: 0,
  };
  const map = {
    getStyle: () => state.style,
    on(name, callback) {
      if (!listeners.has(name)) listeners.set(name, new Set());
      listeners.get(name).add(callback);
    },
    off(name, callback) {
      listeners.get(name)?.delete(callback);
    },
  };
  const emit = (name, event) =>
    [...(listeners.get(name) || [])].forEach((fn) => fn(event));
  const guard = new BasemapLoadGuard(
    map,
    () => state.fallbacks++,
    new Set(["basemap"]),
  );
  if (watchStyle) guard.watchStyle();
  t.after(() => guard.dispose());
  return { state, guard, emit, listeners };
}

test("SDK validation errors report one fallback after event dispatch and clean up", async (t) => {
  const { state, emit, listeners } = fixture(t);
  emit("error", { error: new Error("version: expected 8") });
  assert.equal(state.fallbacks, 0);
  await Promise.resolve();
  assert.equal(state.fallbacks, 1);
  emit("error", { error: new Error("fallback is offline too") });
  t.mock.timers.tick(60_000);
  await Promise.resolve();
  assert.equal(state.fallbacks, 1);
  assert.ok([...listeners.values()].every((entries) => entries.size === 0));
});

test("old SDK events cannot settle a pending URL or prevent its 20-second timeout", async (t) => {
  const { state, emit } = fixture(t, false);
  emit("error", { error: new Error("old sprite failed") });
  emit("style.load");
  emit("idle");
  await Promise.resolve();
  assert.equal(state.fallbacks, 0);

  t.mock.timers.tick(19_999);
  await Promise.resolve();
  assert.equal(state.fallbacks, 0);
  t.mock.timers.tick(1);
  await Promise.resolve();
  assert.equal(state.fallbacks, 1);
});

for (const reportError of [true, false]) {
  test(`raster tiles failing ${reportError ? "with" : "without"} error events trigger fallback`, async (t) => {
    const { state, emit } = fixture(t);
    state.style = {
      sources: { basemap: { type: "raster" }, journey: { type: "raster" } },
    };
    emit("style.load");
    emit("sourcedata", { sourceId: "basemap", sourceDataType: "content" });
    if (reportError) {
      emit("error", { sourceId: "basemap", error: new Error("tile 404") });
    } else {
      emit("sourcedataloading", {
        sourceId: "basemap",
        tile: { state: "loading" },
      });
    }
    // Loaded fog tiles and basemap metadata are not usable basemap tiles.
    emit("sourcedata", { sourceId: "journey", tile: { state: "loaded" } });
    emit("idle");
    await Promise.resolve();
    assert.equal(state.fallbacks, 1);
  });
}

test("vector 404s marked loaded cannot cancel fallback, even with stale raw bytes", async (t) => {
  const { state, emit } = fixture(t);
  state.style = { sources: { basemap: { type: "vector" } } };
  emit("style.load");
  // A 404 clears the parsed feature index but may leave stale raw bytes.
  emit("sourcedata", {
    sourceId: "basemap",
    tile: {
      state: "loaded",
      latestFeatureIndex: null,
      latestRawTileData: new ArrayBuffer(8),
    },
  });
  emit("idle");
  await Promise.resolve();
  assert.equal(state.fallbacks, 1);
});

test("an empty but parsed vector tile survives partial tile and font failures", async (t) => {
  const { state, emit } = fixture(t);
  state.style = { sources: { basemap: { type: "vector" } } };
  emit("style.load");
  emit("error", { error: new Error("font missing") });
  emit("error", { sourceId: "basemap", error: new Error("one tile missing") });
  emit("sourcedata", {
    sourceId: "basemap",
    tile: {
      state: "loaded",
      latestFeatureIndex: {},
      latestRawTileData: new ArrayBuffer(0),
    },
  });
  emit("idle");
  t.mock.timers.tick(60_000);
  await Promise.resolve();
  assert.equal(state.fallbacks, 0);
});

test("a valid style without tile sources completes at idle", async (t) => {
  const { state, emit } = fixture(t);
  state.style = { sources: {}, layers: [{ type: "background" }] };
  emit("style.load");
  emit("idle");
  t.mock.timers.tick(60_000);
  await Promise.resolve();
  assert.equal(state.fallbacks, 0);
});

for (const event of ["remove", "webglcontextlost"]) {
  test(`${event} cancels the pending fallback`, async (t) => {
    const { state, emit } = fixture(t);
    emit(event);
    t.mock.timers.tick(60_000);
    await Promise.resolve();
    assert.equal(state.fallbacks, 0);
  });
}

test("disposal cancels an already queued fallback", async (t) => {
  const { state, emit, guard } = fixture(t);
  emit("error", { error: new Error("style missing") });
  guard.dispose();
  await Promise.resolve();
  assert.equal(state.fallbacks, 0);
});
