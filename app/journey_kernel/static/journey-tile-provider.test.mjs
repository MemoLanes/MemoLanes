import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { registerHooks } from "node:module";
import { setImmediate } from "node:timers/promises";
import test from "node:test";

// Production imports omit extensions for Webpack. Resolve those same modules
// for Node's TypeScript runner without mocking the provider implementation.
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier.startsWith(".") && !/\.[a-z]+$/i.test(specifier)) {
      const url = new URL(`${specifier}.ts`, context.parentURL);
      if (existsSync(url)) return nextResolve(url.href, context);
    }
    return nextResolve(specifier, context);
  },
});
const { JourneyTileProvider } = await import("./journey-tile-provider.ts");
const { AVAILABLE_LAYERS } = await import("./layer-config.ts");
const { MainThreadTileBufferBackend } =
  await import("./tile-buffer-backend.ts");

function deferred() {
  let resolve;
  const promise = new Promise((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
async function until(condition) {
  for (let i = 0; i < 100 && !condition(); i++) await setImmediate();
  assert.ok(condition(), "asynchronous work did not reach the expected state");
}
class FakeMap {
  x = 100;
  zoom = 8;
  listeners = new Map();
  getZoom() {
    return this.zoom;
  }
  isMoving() {
    return false;
  }
  isZooming() {
    return false;
  }
  on(event, callback) {
    this.listeners.set(event, callback);
  }
  off(event, callback) {
    if (this.listeners.get(event) === callback) this.listeners.delete(event);
  }
  getBounds() {
    const lng = (x) => (x / 2 ** this.zoom) * 360 - 180;
    const lat = (y) =>
      (Math.atan(Math.sinh(Math.PI * (1 - (2 * y) / 2 ** this.zoom))) * 180) /
      Math.PI;
    return {
      getSouthWest: () => ({ lng: lng(this.x + 0.1), lat: lat(100.9) }),
      getNorthEast: () => ({ lng: lng(this.x + 0.9), lat: lat(100.1) }),
    };
  }
}
function fixture(t) {
  const previousWindow = globalThis.window;
  const previousFetch = globalThis.fetch;
  globalThis.window = {
    EXTERNAL_PARAMS: {},
    location: { href: "http://localhost/" },
    dispatchEvent() {},
  };
  const requests = [];
  const decodes = [];
  let backendDisposals = 0;
  let unsubscribeCount = 0;
  let invalidate;
  const backend = {
    decode(id) {
      const pending = deferred();
      const source = {
        id,
        mainThreadBuffer: { id },
        releases: 0,
        release() {
          this.releases++;
        },
      };
      decodes.push({ ...pending, source });
      return pending.promise;
    },
    dispose() {
      backendDisposals++;
    },
  };
  AVAILABLE_LAYERS.test = {
    ...AVAILABLE_LAYERS.canvas,
    createTileBufferBackend(callback) {
      invalidate = callback;
      return backend;
    },
  };
  globalThis.fetch = (url) => {
    const pending = deferred();
    requests.push({ ...pending, url });
    return pending.promise;
  };
  const map = new FakeMap();
  const provider = new JourneyTileProvider(map, {
    renderMode: "test",
    on() {
      return () => {
        unsubscribeCount++;
      };
    },
  });
  t.after(() => {
    provider.dispose();
    delete AVAILABLE_LAYERS.test;
    globalThis.window = previousWindow;
    globalThis.fetch = previousFetch;
  });
  function respond(index, version = "1", unchanged = false) {
    requests[index].resolve(
      new Response(unchanged ? null : new Uint8Array([1]), {
        headers: unchanged
          ? { "X-Not-Modified": "true" }
          : { "X-Tile-Version": version },
      }),
    );
  }
  async function commit(index, version = "1") {
    respond(index, version);
    await until(() => decodes.length > index);
    decodes[index].resolve(decodes[index].source);
    await provider.waitForTileBufferUpdate();
  }
  return {
    provider,
    map,
    requests,
    decodes,
    respond,
    commit,
    invalidate: () => invalidate(),
    backendDisposals: () => backendDisposals,
    unsubscribeCount: () => unsubscribeCount,
  };
}

test("superseded decoded sources are released and callbacks describe committed coverage", async (t) => {
  const f = fixture(t);
  const callbacks = [];
  f.provider.registerTileBufferCallback((...args) => callbacks.push(args));
  assert.equal(callbacks.length, 0);
  f.respond(0);
  await until(() => f.decodes.length === 1);
  f.map.x = 110;
  f.provider.tryUpdateViewRange();
  f.decodes[0].resolve(f.decodes[0].source);
  await until(() => f.requests.length === 2);
  assert.equal(f.decodes[0].source.releases, 1);
  assert.equal(callbacks.length, 0);
  await f.commit(1, "2");
  assert.deepEqual(callbacks[0].slice(0, 6), [110, 100, 1, 1, 8, 8]);
  assert.equal(callbacks[0][6], f.decodes[1].source.mainThreadBuffer);
  assert.equal(f.provider.getLoadedTileBuffer().generation, 1);
});

test("disposal releases late decodes and detaches the provider exactly once", async (t) => {
  const f = fixture(t);
  f.respond(0);
  await until(() => f.decodes.length === 1);
  f.provider.dispose();
  f.provider.dispose();
  f.decodes[0].resolve(f.decodes[0].source);
  await f.provider.waitForTileBufferUpdate();
  await until(() => f.decodes[0].source.releases === 1);
  assert.equal(f.provider.getLoadedTileBuffer(), null);
  assert.equal(f.backendDisposals(), 1);
  assert.equal(f.unsubscribeCount(), 1);
  assert.equal(f.map.listeners.size, 0);
});

test("unchanged polling retains the source and backend invalidation bypasses version caching", async (t) => {
  const f = fixture(t);
  await f.commit(0);
  const original = f.provider.getLoadedTileBuffer();
  const poll = f.provider.pollForJourneyUpdates();
  assert.equal(
    new URL(f.requests[1].url).searchParams.get("cached_version"),
    "1",
  );
  assert.equal(await f.provider.pollForJourneyUpdates(), false);
  assert.equal(f.requests.length, 2);
  f.respond(1, "1", true);
  await poll;
  assert.equal(f.provider.getLoadedTileBuffer(), original);
  assert.equal(f.decodes[0].source.releases, 0);
  f.invalidate();
  assert.equal(
    new URL(f.requests[2].url).searchParams.has("cached_version"),
    false,
  );
  f.respond(2, "2");
  await until(() => f.decodes.length === 2);
  f.decodes[1].resolve(f.decodes[1].source);
  await f.provider.waitForTileBufferUpdate();
  assert.equal(f.decodes[0].source.releases, 1);
  assert.equal(f.provider.getLoadedTileBuffer().generation, 2);
});

test("Canvas backend decodes OSS WASM pixels and permits idempotent release", async () => {
  const wasm = await import("../pkg/journey_kernel.js");
  await wasm.default({
    module_or_path: await readFile(
      new URL("../pkg/journey_kernel_bg.wasm", import.meta.url),
    ),
  });
  // One z2 tile with a 4x4 base bitmap and a complete OR-reduced pyramid.
  const bytes = new Uint8Array(39);
  const view = new DataView(bytes.buffer);
  bytes[0] = 2;
  bytes[1] = 2;
  for (const offset of [12, 14, 16, 18]) view.setUint16(offset, 1, true);
  bytes[20] = 1;
  view.setUint16(21, 3, true);
  view.setUint32(23, 16, true);
  bytes[28] = 2;
  view.setUint32(29, 4, true);
  bytes[33] = 4;
  view.setUint32(34, 1, true);
  bytes[38] = 1;
  const backend = new MainThreadTileBufferBackend();
  const source = await backend.decode(7, bytes.buffer);
  assert.equal(source.id, 7);
  assert.deepEqual(
    [...source.mainThreadBuffer.get_tile_pixels(0, 0, 2, 2)],
    [1, 2],
  );
  source.release();
  source.release();
  backend.dispose();
});

test("Canvas keeps native source detail through map zoom 14", async () => {
  const { getTileRequestRange } =
    await import("./layers/tile-request-policy.ts");
  const map = new FakeMap();
  map.zoom = 14;
  const canvas = AVAILABLE_LAYERS.canvas;
  assert.deepEqual(
    getTileRequestRange(
      map,
      false,
      canvas.bufferSizePower,
      canvas.tileRequestPolicy,
    ),
    [100, 100, 1, 1, 14],
  );
});
