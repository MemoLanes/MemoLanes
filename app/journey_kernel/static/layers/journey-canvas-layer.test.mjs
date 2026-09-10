import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { registerHooks } from "node:module";
import test from "node:test";

registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier.startsWith(".") && !/\.[a-z]+$/i.test(specifier)) {
      const url = new URL(`${specifier}.ts`, context.parentURL);
      if (existsSync(url)) return nextResolve(url.href, context);
    }
    return nextResolve(specifier, context);
  },
});
const { JourneyCanvasLayer } = await import("./journey-canvas-layer.ts");

function fixture(t, bg, fg) {
  const previousDocument = globalThis.document;
  let output;
  let allocations = 0;
  const ctx = {
    createImageData(width, height) {
      allocations++;
      return { width, height, data: new Uint8ClampedArray(width * height * 4) };
    },
    putImageData(image) {
      output = { ...image, data: image.data.slice() };
    },
  };
  const canvas = { width: 300, height: 150, getContext: () => ctx };
  globalThis.document = { createElement: () => canvas };
  t.after(() => {
    if (previousDocument === undefined) delete globalThis.document;
    else globalThis.document = previousDocument;
  });
  t.mock.method(console, "log", () => {});
  const source = { setCoordinates() {}, play() {}, pause() {} };
  const map = {
    getSource: () => source,
    getLayer: () => undefined,
    removeSource() {},
  };
  const layer = new JourneyCanvasLayer(map, {}, "canvas-test", bg, fg);
  return {
    layer,
    allocations: () => allocations,
    pixel: (x, y) =>
      Array.from(
        output.data.slice(
          (y * output.width + x) * 4,
          (y * output.width + x + 1) * 4,
        ),
      ),
    draw(points, { w = 1, h = 1, z = 4, power = 3 } = {}) {
      const size = 2 ** power;
      layer.redrawCanvas(0, 0, w, h, z, power, {
        get_tile_pixels(x, y) {
          return new Uint16Array(
            points
              .filter(
                ([px, py]) =>
                  Math.floor(px / size) === x && Math.floor(py / size) === y,
              )
              .flatMap(([px, py]) => [px % size, py % size]),
          );
        },
      });
      return output.data;
    },
  };
}

test("feather keeps solid track cores and untouched fog outside one cell", (t) => {
  const f = fixture(t);
  f.draw([[3, 3]]);
  assert.equal(f.pixel(3, 3)[3], 0);
  assert.equal(f.pixel(5, 3)[3], 128);
  assert.ok(f.pixel(4, 3)[3] > 0);
  assert.ok(f.pixel(4, 3)[3] < f.pixel(4, 4)[3]);
  assert.ok(f.pixel(4, 4)[3] < 128);
});

test("diagonal support smooths notches independently of order and duplicates", (t) => {
  const f = fixture(t);
  f.draw([[3, 3]]);
  const singleNeighborAlpha = f.pixel(4, 3)[3];
  const points = [
    [3, 3],
    [4, 4],
    [5, 5],
  ];
  const forward = f.draw(points);
  assert.ok(f.pixel(4, 3)[3] < singleNeighborAlpha);
  assert.equal(f.pixel(4, 4)[3], 0);
  assert.deepEqual(f.draw([...points].reverse()), forward);
  assert.deepEqual(f.draw([...points, ...points]), forward);
});

test("tile seams match a single raster, and only full worlds wrap", (t) => {
  const f = fixture(t);
  const points = [
    [7, 3],
    [8, 4],
  ];
  const split = f.draw(points, { w: 2 });
  const combined = f.draw(points, { power: 4 });
  assert.deepEqual(split, combined.slice(0, split.length));
  f.draw([[0, 3]], { z: 0 });
  assert.equal(f.pixel(7, 3)[3], f.pixel(1, 3)[3]);
  f.draw([[0, 3]]);
  assert.equal(f.pixel(7, 3)[3], 128);
  f.draw([[3, 0]], { z: 0 });
  assert.equal(f.pixel(3, 7)[3], 128);
});

test("custom foregrounds retain alpha and use premultiplied edge colors", (t) => {
  const f = fixture(t, [0, 0, 1, 0.5], [1, 0, 0, 0.25]);
  f.draw([[3, 3]]);
  assert.deepEqual(f.pixel(3, 3), [255, 0, 0, 64]);
  assert.deepEqual(f.pixel(5, 3), [0, 0, 255, 128]);
  const [red, green, blue, alpha] = f.pixel(4, 3);
  assert.equal(green, 0);
  assert.ok(red > 0 && red < 46);
  assert.ok(blue > 209 && blue < 255);
  assert.ok(alpha > 64 && alpha < 128);
});

test("reused rasters clear old tracks and resize to the new viewport", (t) => {
  const f = fixture(t);
  const initial = f.draw([[3, 3]]);
  f.draw([]);
  assert.equal(f.pixel(3, 3)[3], 128);
  assert.equal(f.allocations(), 1);
  assert.deepEqual(f.draw([[3, 3]]), initial);
  assert.equal(f.allocations(), 1);
  assert.equal(f.draw([[9, 3]], { w: 2 }).length, 16 * 8 * 4);
  assert.equal(f.pixel(9, 3)[3], 0);
  assert.equal(f.allocations(), 2);
  f.layer.remove();
  f.draw([]);
  assert.equal(f.allocations(), 3);
});
