import assert from "node:assert/strict";
import test from "node:test";
import "../test-support/register-typescript.mjs";
const { getMapSamplingZoom } = await import("./map-resolution.ts");
const { getTileRequestRange } = await import("./tile-request-policy.ts");

const edge = 85.0511287798066;
const scale = (lat) =>
  Math.cos((Math.min(Math.abs(lat), edge) * Math.PI) / 180);
function view(latitude, transition = 1) {
  return {
    // Keep MapLibre's globe radius unchanged while moving to the pole.
    getZoom: () => 1.8 + Math.log2(scale(latitude)),
    getCenter: () => ({ lat: latitude }),
    getBounds: () => ({
      getSouthWest: () => ({ lng: -180, lat: latitude > 0 ? 30 : -90 }),
      getNorthEast: () => ({ lng: 180, lat: latitude > 0 ? 90 : -30 }),
    }),
    style: { projection: { transitionState: transition } },
  };
}

test("globe sampling preserves visible ordinary-track detail towards both poles", () => {
  for (const sign of [1, -1]) {
    const zooms = [70, 80, edge, 89.999, 90].map((lat) =>
      getMapSamplingZoom(view(sign * lat)),
    );
    assert.ok(zooms.every((zoom) => Math.abs(zoom - zooms[0]) < 1e-10));
    assert.ok(
      zooms[0] > 1.5,
      "negative camera zoom must retain sub-pixel detail",
    );
    const ranges = [70, 80, edge, 90].map((lat) =>
      getTileRequestRange(view(sign * lat), true, 10, {
        maxSourceZoom: 12,
        maxSourceCellWidthCss: 0.25,
      }),
    );
    assert.ok(ranges.every((range) => range[4] === ranges[0][4]));
    assert.ok(ranges.every((range) => range[2] <= 20 && range[3] <= 20));
  }
});

test("sampling uses raw zoom in Mercator and increases sampling detail through the globe transition", () => {
  const map = view(80, 0);
  assert.equal(getMapSamplingZoom(map), map.getZoom());
  const zooms = [0, 0.25, 0.5, 0.75, 1].map((transition) =>
    getMapSamplingZoom(view(80, transition)),
  );
  assert.ok(zooms.every((zoom, i) => i === 0 || zoom > zooms[i - 1]));
  assert.equal(getMapSamplingZoom({ getZoom: () => 8.5 }), 8.5);
});

test("a close polar view does not fetch detail for invisible lower latitudes", () => {
  for (const sign of [1, -1]) {
    const map = view(sign * 90);
    map.getZoom = () => 14;
    map.getBounds = () => ({
      getSouthWest: () => ({ lat: sign > 0 ? 89.99 : -90 }),
      getNorthEast: () => ({ lat: sign > 0 ? 90 : -89.99 }),
    });
    assert.equal(getMapSamplingZoom(map), 14);
    assert.equal(
      getTileRequestRange(map, true, 9, { maxSourceZoom: 13 }),
      null,
    );
  }
});

test("Mercator requests clip polar bounds and reduce LOD to the endpoint limit", () => {
  for (const sign of [1, -1]) {
    const map = view(sign * 85);
    map.getZoom = () => 14;
    map.getBounds = () => ({
      getSouthWest: () => ({ lng: -180, lat: sign > 0 ? 84 : -90 }),
      getNorthEast: () => ({ lng: 180, lat: sign > 0 ? 90 : -84 }),
    });
    const range = getTileRequestRange(map, true, 9, { maxSourceZoom: 13 });
    assert.ok(range[2] <= 20 && range[3] <= 20);
    assert.equal(range[2], 2 ** range[4], "all meridians remain covered");
    assert.ok(range[3] < 2 ** range[4], "only the Mercator edge is fetched");
  }
});
