import * as maplibregl from "maplibre-gl";
import { JourneyCanvasLayer } from "../../static/layers/journey-canvas-layer";
import { createPolarCameraConstrain } from "../../static/polar-camera-policy";
import { polarUV, POLAR_SEAM_LATITUDE } from "../../static/polar-projection";
import { getTileRequestRange } from "../../static/layers/tile-request-policy";
import { createMapContainer, waitForIdle } from "./map-fixture";
import { encodeTileRangeResponse } from "../support/tile-range-fixture.js";

/** Full OSS Canvas path: binary tiles -> WASM pixel queries -> Canvas -> globe. */
export async function checkPolarCanvas(check) {
  const originalFetch = window.fetch,
    previousParams = window.EXTERNAL_PARAMS;
  const endpoint = new URL("canvas-test/", location.href).href;
  window.EXTERNAL_PARAMS = { cgi_endpoint: endpoint };
  let points = [],
    revision = 1;
  const refresh = new Set();
  const provider = {
    registerTileBufferCallback() {},
    unregisterTileBufferCallback() {},
    registerRefreshCallback(fn) {
      refresh.add(fn);
    },
    unregisterRefreshCallback(fn) {
      refresh.delete(fn);
    },
  };
  const requests = [];
  window.fetch = (input, options) => {
    const url = new URL(
      input instanceof Request ? input.url : input,
      location.href,
    );
    if (!url.href.startsWith(endpoint)) return originalFetch(input, options);
    requests.push(url);
    const q = url.searchParams;
    if (q.get("cached_version") === String(revision))
      return Promise.resolve(
        new Response(null, { headers: { "X-Not-Modified": "true" } }),
      );
    const range = ["x", "y", "width", "height", "z"].map((key) =>
      Number(q.get(key)),
    );
    const exponent = Number(q.get("buffer_size_power")),
      grid = 2 ** (range[4] + exponent);
    const cells = [];
    for (const [lng, lat] of points) {
      if (lat > 0 !== (q.get("plane") === "north")) continue;
      const uv = polarUV(lng, lat),
        x = Math.floor(uv[0] * grid),
        y = Math.floor(uv[1] * grid);
      for (let dy = -1; dy <= 1; dy++)
        for (let dx = -1; dx <= 1; dx++)
          if (x + dx >= 0 && y + dy >= 0 && x + dx < grid && y + dy < grid)
            cells.push([x + dx, y + dy]);
    }
    return Promise.resolve(
      new Response(encodeTileRangeResponse(range, cells, exponent), {
        headers: { "X-Tile-Version": String(revision) },
      }),
    );
  };
  const container = createMapContainer();
  const map = new maplibregl.Map({
    container,
    center: [0, 86],
    zoom: 14,
    maxZoom: 14,
    transformConstrain: createPolarCameraConstrain(0, 14),
    interactive: false,
    canvasContextAttributes: { antialias: false, preserveDrawingBuffer: true },
    style: {
      version: 8,
      projection: { type: "vertical-perspective" },
      sources: {},
      layers: [],
    },
  });
  let layer;
  const results = [];
  try {
    await new Promise((resolve) => map.once("load", resolve));
    layer = new JourneyCanvasLayer(map, provider);
    layer.initialize();
    const gl = map.getCanvas().getContext("webgl2");
    const read = () => {
      const pixels = new Uint8Array(256 * 256 * 4);
      gl.readPixels(0, 0, 256, 256, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
      check(
        gl.getError() === gl.NO_ERROR,
        "Canvas polar rendering has no GL errors",
      );
      return pixels;
    };
    const settle = async () => {
      await layer.globeLayer.data.waitForUpdates();
      await waitForIdle(map);
    };
    for (const sign of [1, -1]) {
      for (const [longitude, latitude] of [
        [7, 85.052],
        [7, 85.055],
        [7, 85.061],
        [7, 85.1],
        [90, 86],
        [120.5, 89.999],
        [0, 90],
      ]) {
        points = [[longitude, sign * latitude]];
        revision++;
        map.jumpTo({ center: points[0], zoom: 14 });
        if (latitude < 85.06) {
          const range = getTileRequestRange(map, true, 9, {
            maxSourceZoom: 13,
          });
          if (range)
            layer.redrawCanvas(...range, 9, {
              get_tile_pixels: () => new Uint16Array(),
            });
        }
        for (const fn of refresh) fn(true);
        await settle();
        const p = map.project(points[0]),
          pixels = read();
        let total = 0,
          sx = 0,
          sy = 0;
        for (let y = 0; y < 256; y++)
          for (let x = 0; x < 256; x++) {
            const alpha = pixels[((255 - y) * 256 + x) * 4 + 3];
            if (Math.abs(x - p.x) > 20 || Math.abs(y - p.y) > 20) {
              check(
                alpha >= 126 && alpha <= 130,
                `polar fog must cover once at ${longitude},${sign * latitude}; pixel ${x},${y}: ${alpha}`,
              );
            } else {
              const weight = Math.max(0, 128 - alpha);
              total += weight;
              sx += (x + 0.5) * weight;
              sy += (y + 0.5) * weight;
            }
          }
        check(
          total > 20,
          `Canvas must reveal tracks at ${longitude},${sign * latitude}`,
        );
        check(
          Math.hypot(sx / total - p.x, sy / total - p.y) < 3,
          `Canvas track drift at ${longitude},${sign * latitude}: ${sx / total},${sy / total} vs ${p.x},${p.y}`,
        );
      }
      points = [];
      revision++;
      for (const zoom of [2, 8, 14]) {
        map.jumpTo({ center: [7, sign * POLAR_SEAM_LATITUDE], zoom });
        const range = getTileRequestRange(map, true, 9, { maxSourceZoom: 13 });
        layer.redrawCanvas(...range, 9, {
          get_tile_pixels: () => new Uint16Array(),
        });
        for (const fn of refresh) fn(true);
        await settle();
        const pixels = read();
        for (const x of [96, 128, 160])
          for (const y of [96, 128, 160]) {
            const alpha = pixels[(y * 256 + x) * 4 + 3];
            check(
              alpha >= 126 && alpha <= 130,
              `Canvas seam ${sign}, zoom ${zoom} at ${x},${y}: alpha ${alpha}, geographic ${JSON.stringify(map.unproject([x + 0.5, 255 - y + 0.5]))}`,
            );
          }
      }
      results.push(
        `${sign > 0 ? "north" : "south"} Canvas tracks at native detail, pole and Mercator seams`,
      );
    }
    check(
      requests.some((url) => url.searchParams.has("plane")),
      "Canvas requests public polar tiles",
    );
    for (const [longitude, latitude, zoom] of [
      [179.99, 30, 14],
      [-179.99, -30, 14],
      [7, 84.99, 14],
      [60, 45, 3],
    ]) {
      map.jumpTo({ center: [longitude, latitude], zoom });
      const range = getTileRequestRange(map, true, 9, { maxSourceZoom: 13 });
      const n = 2 ** range[4],
        side = n * 512;
      const px = Math.floor(((longitude + 180) / 360) * side);
      const py = Math.floor(
        ((1 - Math.asinh(Math.tan((latitude * Math.PI) / 180)) / Math.PI) / 2) *
          side,
      );
      layer.redrawCanvas(...range, 9, {
        get_tile_pixels(x, y) {
          const cells = [];
          for (let dy = -1; dy <= 1; dy++)
            for (let dx = -1; dx <= 1; dx++)
              if (
                Math.floor((px + dx) / 512) === ((x % n) + n) % n &&
                Math.floor((py + dy) / 512) === y
              )
                cells.push((px + dx) % 512, (py + dy) % 512);
          return new Uint16Array(cells);
        },
      });
      await settle();
      const pixels = read(),
        p = map.project([longitude, latitude]);
      let total = 0,
        sx = 0,
        sy = 0;
      for (let y = 108; y < 148; y++)
        for (let x = 108; x < 148; x++) {
          const weight = Math.max(
            0,
            128 - pixels[((255 - y) * 256 + x) * 4 + 3],
          );
          total += weight;
          sx += (x + 0.5) * weight;
          sy += (y + 0.5) * weight;
        }
      check(
        total > 20 && Math.hypot(sx / total - p.x, sy / total - p.y) < 3,
        `Mercator Canvas track survives globe mapping at ${longitude},${latitude}, zoom ${zoom}`,
      );
    }
    map.jumpTo({ center: [0, 0], zoom: 0 });
    const worldRange = getTileRequestRange(map, true, 9, { maxSourceZoom: 13 });
    layer.redrawCanvas(...worldRange, 9, {
      get_tile_pixels: () => new Uint16Array(),
    });
    await settle();
    const overview = read();
    check(
      overview[(128 * 256 + 128) * 4 + 3] === 128,
      "globe overview fog is drawn once",
    );
    check(overview[3] === 0, "globe Canvas does not draw over the sky");
    results.push("Canvas Mercator tracks, antimeridian and globe horizon");

    const data = layer.globeLayer.data;
    const tileFetch = window.fetch;
    let releaseNorth;
    let holdNorth = true;
    window.fetch = async (input, options) => {
      const response = await tileFetch(input, options);
      if (holdNorth && new URL(input).searchParams.get("plane") === "north") {
        holdNorth = false;
        await new Promise((resolve) => {
          releaseNorth = resolve;
        });
      }
      return response;
    };
    const beforeNorth = data.get("north"),
      beforeSouth = data.get("south");
    revision++;
    for (const fn of refresh) fn(true);
    for (
      let i = 0;
      i < 100 && (!releaseNorth || data.get("south") === beforeSouth);
      i++
    )
      await new Promise((resolve) => setTimeout(resolve, 10));
    check(
      !!releaseNorth && data.get("south") !== beforeSouth,
      "a slow north cap does not block south updates",
    );
    check(
      data.get("north") === beforeNorth,
      "pending downloads retain the previous cap",
    );
    revision++;
    for (const fn of refresh) fn(true);
    releaseNorth();
    await data.waitForUpdates();
    check(
      data.get("north")?.version?.endsWith(`/${revision}`),
      "forced refresh replaces a stale in-flight response",
    );
    holdNorth = true;
    releaseNorth = null;
    revision++;
    for (const fn of refresh) fn(true);
    for (let i = 0; i < 100 && !releaseNorth; i++)
      await new Promise((resolve) => setTimeout(resolve, 10));
    check(!!releaseNorth, "disposal test has an in-flight request");
    layer.remove();
    layer = null;
    releaseNorth();
    await data.waitForUpdates();
    check(
      data.get("north") === null && data.get("south") === null,
      "disposed caps reject late responses",
    );
    results.push(
      "Canvas independent downloads, forced refresh and in-flight disposal",
    );
    check(refresh.size === 0, "Canvas removes its refresh listener");
    return results;
  } finally {
    layer?.remove();
    map.remove();
    container.remove();
    window.fetch = originalFetch;
    window.EXTERNAL_PARAMS = previousParams;
  }
}
