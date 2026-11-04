import assert from "node:assert/strict";
import test from "node:test";
import { resolveStyleResourceUrls } from "./utils.ts";

test("relative resources use the style URL and preserve templates and keys", () => {
  const style = {
    sources: { planet: { type: "vector", url: "/tiles.json?key=a+b" } },
    glyphs: "./fonts/{fontstack}/{range}.pbf",
    "font-faces": {
      Regular: "./fonts/regular.woff2",
      Regional: [
        { url: "../fonts/local.woff2", "unicode-range": ["U+0900-097F"] },
        { url: "https://fonts.example.org/remote.woff2" },
      ],
    },
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
  assert.deepEqual(style["font-faces"], {
    Regular: "https://example.com/maps/fonts/regular.woff2",
    Regional: [
      {
        url: "https://example.com/fonts/local.woff2",
        "unicode-range": ["U+0900-097F"],
      },
      { url: "https://fonts.example.org/remote.woff2" },
    ],
  });
});
