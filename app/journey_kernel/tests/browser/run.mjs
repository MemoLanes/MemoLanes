import assert from "node:assert/strict";
import { setTimeout } from "node:timers/promises";
import { chromium, webkit } from "playwright";
import { browserLaunchOptions } from "../../tools/browser-launch.mjs";
import { serveBrowserBundle } from "../../tools/browser-bundle.mjs";
import { encodeTileRangeResponse } from "../support/tile-range-fixture.js";

const browserName = process.env.RENDER_TEST_BROWSER || "chromium";
const browserType = { chromium, webkit }[browserName];
if (!browserType)
  throw new Error(`Unsupported rendering test browser: ${browserName}`);
let bundle, browser;
try {
  bundle = await serveBrowserBundle(
    "./tests/browser/entry.js",
    `<!doctype html><meta charset="utf-8"><body><script src="./bundle.js"></script>`,
  );
  browser = await browserType.launch(
    browserLaunchOptions(browserName, {
      headless: process.env.BENCHMARK_HEADLESS === "true",
    }),
  );
  const page = await browser.newPage({ viewport: { width: 800, height: 600 } });
  await page.route("**/tile_range?*", async (route) => {
    const q = new URL(route.request().url()).searchParams;
    const range = ["x", "y", "width", "height", "z"].map((key) =>
      Number(q.get(key)),
    );
    await route.fulfill({
      body: Buffer.from(
        encodeTileRangeResponse(range, [], Number(q.get("buffer_size_power"))),
      ),
      headers: { "X-Tile-Version": "1" },
    });
  });
  // Navigation regressions keep source tiles pending until their map is removed.
  await page.route("**/slow-basemap/**", () => {});
  await page.route("**/missing-basemap/**", (route) => route.abort());
  const liveStyleRequests = { builtin: 0, custom: 0 };
  await page.route("**/live-appearance/*.json", async (route) => {
    if (route.request().url().endsWith("custom.json")) {
      liveStyleRequests.custom++;
      await route.abort();
    } else {
      liveStyleRequests.builtin++;
      await route.fulfill({
        json: {
          version: 8,
          center: [42, 42],
          zoom: 5,
          sources: {},
          layers: [{ id: "live-builtin", type: "background" }],
        },
      });
    }
  });
  let emptyStyleRequests = 0;
  await page.route("**/empty-style/custom.json", async (route) => {
    emptyStyleRequests++;
    await route.fulfill({ json: { version: 8, sources: {}, layers: [] } });
  });
  let stalledStyleRequests = 0;
  let cancelledStalledRequests = 0;
  page.on("requestfailed", (request) => {
    if (request.url().endsWith("/stalled-style/builtin.json"))
      cancelledStalledRequests++;
  });
  await page.route("**/stalled-style/builtin.json", async (route) => {
    if (++stalledStyleRequests < 3) return;
    await route.fulfill({
      json: {
        version: 8,
        sources: {},
        layers: [{ id: "retried-basemap", type: "background" }],
      },
    });
  });
  let slowStyleRequests = 0;
  await page.route("**/slow-style/builtin.json", async (route) => {
    slowStyleRequests++;
    // Hold the style document itself beyond the production 8s retry interval.
    await setTimeout(10_000);
    if (!page.isClosed()) {
      await route.fulfill({
        json: {
          version: 8,
          sources: {
            provider: {
              type: "geojson",
              attribution: "Local Provider",
              data: { type: "FeatureCollection", features: [] },
            },
          },
          layers: [
            { id: "remote-basemap", type: "circle", source: "provider" },
          ],
        },
      });
    }
  });
  const recoveryRequests = { builtin: [], custom: [] };
  const requestWaiters = { builtin: [], custom: [] };
  await page.route("**/restore-style/*.json", async (route) => {
    const kind = route.request().url().endsWith("custom.json")
      ? "custom"
      : "builtin";
    recoveryRequests[kind].push(route);
    for (const notify of requestWaiters[kind]) notify();
  });
  await page.exposeFunction(
    "waitForRecoveryRequest",
    (kind, count) =>
      new Promise((resolve) => {
        const notify = () => {
          if (recoveryRequests[kind].length >= count) resolve();
        };
        requestWaiters[kind].push(notify);
        notify();
      }),
  );
  await page.exposeFunction("releaseRecoveryRequest", async (kind, index) => {
    const route = recoveryRequests[kind][index];
    if (kind === "custom") await route.abort();
    else
      await route.fulfill({
        json: {
          version: 8,
          sources: {},
          layers: [
            {
              id: index === 0 ? "stale-basemap" : "restored-remote-basemap",
              type: "background",
              paint: { "background-color": "#ddd" },
            },
          ],
        },
      });
  });
  await page.exposeFunction(
    "recoveryRequestCount",
    (kind) => recoveryRequests[kind].length,
  );
  const failure = new Promise((_, reject) => page.on("pageerror", reject));
  await Promise.race([
    failure,
    (async () => {
      await page.goto(bundle.url);
      // The live appearance check observes an additional full 8-second
      // retry interval, and the tap check waits for two tap windows.
      await page.waitForFunction(() => window.renderingResult, undefined, {
        timeout: 105_000,
      });
    })(),
  ]);
  assert.deepEqual(
    liveStyleRequests,
    { builtin: 1, custom: 1 },
    "one request per live selection; repeats and fog changes never reload a basemap",
  );
  assert.equal(emptyStyleRequests, 1, "one real empty custom style request");
  assert.equal(
    stalledStyleRequests,
    3,
    "two retries replaced consecutive stalled requests",
  );
  assert.equal(
    cancelledStalledRequests,
    2,
    "each retry cancels the earlier fetch",
  );
  assert.equal(
    slowStyleRequests,
    1,
    "the retry did not restart a pending style request",
  );
  assert.equal(
    recoveryRequests.builtin.length,
    2,
    "one replacement after restore",
  );
  assert.equal(
    recoveryRequests.custom.length,
    1,
    "custom style does not auto-retry",
  );
  console.log(
    JSON.stringify(await page.evaluate(() => window.renderingResult), null, 2),
  );
} finally {
  try {
    await browser?.close();
  } finally {
    await bundle?.close();
  }
}
