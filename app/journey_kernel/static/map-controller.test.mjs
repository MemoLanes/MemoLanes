import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { registerHooks } from "node:module";
import test from "node:test";

// Exercise the real controller, fetch/parse path and reactive params. Only the
// browser SDK is replaced so request completions can be ordered explicitly.
class FakeMap {
  requests = [];
  listeners = new Map();
  dragRotate = { disable() {} };
  touchZoomRotate = { disableRotation() {} };

  constructor(options) {
    this.style = options.style;
  }
  getStyle() {
    return this.style;
  }
  getSource(id) {
    return this.style?.sources[id];
  }
  setStyle(style) {
    this.requests.push(style);
  }
  complete(index) {
    this.style = this.requests[index];
    this.emit("style.load");
    this.emit("idle");
  }
  on(name, callback) {
    if (!this.listeners.has(name)) this.listeners.set(name, new Set());
    this.listeners.get(name).add(callback);
  }
  off(name, callback) {
    this.listeners.get(name)?.delete(callback);
  }
  emit(name, event) {
    for (const callback of [...(this.listeners.get(name) || [])])
      callback(event);
  }
  remove() {
    this.emit("remove");
  }
}

globalThis.__basemapTestMap = FakeMap;
globalThis.window = { location: { href: "http://localhost/" } };
const sdkStub = `data:text/javascript,${encodeURIComponent(
  "export const Map = globalThis.__basemapTestMap; export class LngLat {} export function setWorkerUrl() {}",
)}`;
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === "maplibre-gl") {
      return { url: sdkStub, shortCircuit: true };
    }
    if (specifier.endsWith(".css")) {
      return { url: "data:text/javascript,export {}", shortCircuit: true };
    }
    if (specifier.startsWith(".") && !/\.[a-z]+$/i.test(specifier)) {
      const url = new URL(`${specifier}.ts`, context.parentURL);
      if (existsSync(url)) return nextResolve(url.href, context);
    }
    return nextResolve(specifier, context);
  },
});
const { MapController } = await import("./map-controller.ts");
const { createReactiveParams } = await import("./params.ts");
const { collectMapAttributions } = await import("./map-attribution.ts");
delete globalThis.__basemapTestMap;

const settle = () => new Promise(setImmediate);

async function fixture(t, id = "custom:a") {
  t.mock.timers.enable({ apis: ["setInterval", "setTimeout"] });
  const failures = [];
  const fetches = [];
  // Allow aborted requests to complete: cancellation alone cannot protect
  // against a body/parse result already queued by the browser.
  t.mock.method(
    globalThis,
    "fetch",
    (url, options) =>
      new Promise((resolve) => {
        fetches.push({ url, signal: options.signal, resolve });
      }),
  );
  window.onBasemapLoadFailed = {
    postMessage: (message) => failures.push(JSON.parse(message)),
  };
  const params = createReactiveParams({
    cgi_endpoint: "http://localhost/api",
    basemap: {
      id,
      requestId: 1,
      style: "https://example.com/a.json?key=user-key",
    },
  });
  const controller = new MapController({ containerId: "map", params });
  // These are the same hooks registered during initialization, without the
  // unrelated journey-provider/GPU startup required by a browser map.
  controller.registerParamsHooks();
  controller.startBasemapLoad();
  t.after(() => controller.destroy());
  await settle();
  return { controller, params, map: controller.getMap(), failures, fetches };
}

function style(name) {
  return {
    version: 8,
    name,
    sources: {
      [name]: {
        type: "geojson",
        attribution: `Provider ${name}`,
        data: { type: "FeatureCollection", features: [] },
      },
    },
    layers: [],
  };
}

function reply(request, name) {
  request.resolve(new Response(JSON.stringify(style(name))));
}

test("relative resources use the fetched style URL, including after redirects", async (t) => {
  const { map, fetches } = await fixture(t);
  const response = new Response(
    JSON.stringify({
      ...style("relative"),
      glyphs: "../fonts/{fontstack}/{range}.pbf",
      sprite: [{ id: "default", url: "./sprite" }],
      sources: {
        planet: { type: "vector", url: "/maptiler/tiles/v4/tiles.json" },
        raster: {
          type: "raster",
          tiles: ["//tiles.example.com/{z}/{x}/{y}.png?key=a+b"],
        },
        places: { type: "geojson", data: "./places.geojson" },
      },
    }),
  );
  Object.defineProperty(response, "url", {
    value: "https://provider.example.com/maps/dataviz/style.json?v=1",
  });
  fetches[0].resolve(response);
  await settle();
  const applied = map.requests[0];
  assert.equal(
    applied.sources.planet.url,
    "https://provider.example.com/maptiler/tiles/v4/tiles.json",
  );
  assert.deepEqual(applied.sources.raster.tiles, [
    "https://tiles.example.com/{z}/{x}/{y}.png?key=a+b",
  ]);
  assert.equal(
    applied.sources.places.data,
    "https://provider.example.com/maps/dataviz/places.geojson",
  );
  assert.equal(
    applied.glyphs,
    "https://provider.example.com/maps/fonts/{fontstack}/{range}.pbf",
  );
  assert.deepEqual(applied.sprite, [
    { id: "default", url: "https://provider.example.com/maps/dataviz/sprite" },
  ]);
});

test("late A/B responses cannot replace selected C", async (t) => {
  const { params, map, fetches } = await fixture(t);
  params.basemap = { id: "custom:b", requestId: 2, style: "b.json" };
  await settle();
  params.basemap = { id: "custom:c", requestId: 3, style: "c.json" };
  await settle();
  assert.ok(fetches[0].signal.aborted);
  assert.ok(fetches[1].signal.aborted);
  reply(fetches[2], "c");
  await settle();
  map.complete(0);
  reply(fetches[1], "b");
  reply(fetches[0], "a");
  await settle();
  assert.equal(map.requests.length, 1);
  assert.equal(map.getStyle().name, "c");
});

test("old HTTP/JSON and SDK errors cannot fail C while its URL is pending", async (t) => {
  const { params, map, fetches, failures } = await fixture(t);
  params.basemap = { id: "custom:b", requestId: 2, style: "b.json" };
  await settle();
  params.basemap = { id: "custom:c", requestId: 3, style: "c.json" };
  await settle();
  fetches[0].resolve(new Response("offline", { status: 503 }));
  fetches[1].resolve(new Response("{invalid JSON"));
  map.emit("error", { error: new Error("old sprite request failed") });
  map.emit("style.load");
  map.emit("idle");
  await settle();
  assert.deepEqual(failures, []);
  assert.equal(map.requests.length, 0);
  reply(fetches[2], "c");
  await settle();
  map.complete(0);
  t.mock.timers.tick(30_000);
  await settle();
  assert.deepEqual(failures, []);
  assert.equal(map.getStyle().name, "c");
});

test("a slow custom style request keeps its URL and is not restarted", async (t) => {
  const { map, failures, fetches } = await fixture(t);
  assert.equal(fetches[0].url, "https://example.com/a.json?key=user-key");
  t.mock.timers.tick(14_000);
  assert.equal(fetches.length, 1);
  assert.equal(fetches[0].signal.aborted, false);
  reply(fetches[0], "slow");
  await settle();
  map.complete(0);
  t.mock.timers.tick(60_000);
  await settle();
  assert.deepEqual(failures, []);
  assert.equal(fetches.length, 1);
});

for (const [name, response] of [
  ["HTTP", () => new Response("offline", { status: 503 })],
  ["JSON", () => new Response("{invalid JSON")],
]) {
  test(`the current ${name} failure reports its own request once`, async (t) => {
    const { failures, fetches } = await fixture(t);
    fetches[0].resolve(response());
    await settle();
    assert.deepEqual(failures, [{ requestId: 1 }]);
    assert.ok(fetches[0].signal.aborted);
    t.mock.timers.tick(60_000);
    await settle();
    assert.equal(failures.length, 1);
  });
}

test("default 503 retries once, waits for a slow response and stops after success", async (t) => {
  const { params, map, fetches, failures } = await fixture(t);
  reply(fetches[0], "old-custom");
  await settle();
  map.complete(0);
  params.basemap = { id: "openfreemap", requestId: 2, style: "default.json" };
  await settle();
  fetches[1].resolve(new Response("offline", { status: 503 }));
  await settle();
  assert.equal(map.getStyle().name, "old-custom");
  t.mock.timers.tick(8_000);
  await settle();
  assert.equal(fetches.length, 3);
  t.mock.timers.tick(24_000);
  assert.equal(fetches.length, 3);
  assert.equal(fetches[2].signal.aborted, false);
  reply(fetches[2], "default");
  await settle();
  map.complete(1);
  t.mock.timers.tick(60_000);
  await settle();
  assert.equal(map.getStyle().name, "default");
  assert.equal(fetches.length, 3);
  assert.deepEqual(failures, []);
});

test("built-in tiles arriving after 20 seconds do not schedule a style rebuild", async (t) => {
  const { map, fetches } = await fixture(t, "openfreemap");
  reply(fetches[0], "default");
  await settle();
  map.style = map.requests[0];
  map.emit("style.load");
  map.emit("sourcedataloading", {
    sourceId: "default",
    tile: { state: "loading" },
  });
  t.mock.timers.tick(21_000);
  await settle();
  map.emit("sourcedata", { sourceId: "default", tile: { state: "loaded" } });
  map.emit("idle");
  t.mock.timers.tick(60_000);
  await settle();
  assert.equal(fetches.length, 1);
  assert.equal(map.requests.length, 1);
});

test("attribution belongs to the applied style while a new selection is pending", async (t) => {
  const { params, map, fetches } = await fixture(t);
  reply(fetches[0], "a");
  await settle();
  map.complete(0);
  params.basemap = { id: "custom:b", requestId: 2, style: "b.json" };
  await settle();
  // A's delayed TileJSON can still add credits before B replaces its style.
  map.getSource("a").attribution = "Late Provider A";
  assert.deepEqual(collectMapAttributions(map), {
    requestId: 1,
    entries: ["Late Provider A"],
  });
  reply(fetches[1], "b");
  await settle();
  map.complete(1);
  assert.deepEqual(collectMapAttributions(map), {
    requestId: 2,
    entries: ["Provider b"],
  });
});

test("selection changes cancel a scheduled default retry", async (t) => {
  const { params, fetches } = await fixture(t, "openfreemap");
  fetches[0].resolve(new Response("offline", { status: 503 }));
  await settle();
  params.basemap = { id: "custom:b", requestId: 2, style: "b.json" };
  await settle();
  t.mock.timers.tick(8_000);
  await settle();
  assert.equal(fetches.length, 2);
  assert.equal(fetches[1].signal.aborted, false);
});

test("destroying the map cancels the request and discards its late result", async (t) => {
  const { controller, map, fetches, failures } = await fixture(t);
  controller.destroy();
  assert.ok(fetches[0].signal.aborted);
  reply(fetches[0], "a");
  await settle();
  t.mock.timers.tick(60_000);
  await settle();
  assert.equal(map.requests.length, 0);
  assert.deepEqual(failures, []);
});
