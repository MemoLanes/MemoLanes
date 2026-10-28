import { chromium, webkit } from "playwright";
import { browserLaunchOptions } from "../../tools/browser-launch.mjs";
import { serveBrowserBundle } from "../../tools/browser-bundle.mjs";

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
  const failure = new Promise((_, reject) => page.on("pageerror", reject));
  await Promise.race([
    failure,
    (async () => {
      await page.goto(bundle.url);
      await page.waitForFunction(() => window.renderingResult, undefined, {
        timeout: 30_000,
      });
    })(),
  ]);
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
