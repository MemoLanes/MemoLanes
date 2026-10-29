import {
  startProcess,
  waitForHttp,
  waitForOutput,
  stopProcess,
} from "../tools/process.mjs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { browserLaunchOptions } from "../tools/browser-launch.mjs";

const benchmarkDirectory = path.dirname(fileURLToPath(import.meta.url));
const journeyKernelDirectory = path.resolve(benchmarkDirectory, "..");
const rustDirectory = path.resolve(journeyKernelDirectory, "../rust");
const resultDirectory = path.join(benchmarkDirectory, "results");
const staticPort = Number(process.env.BENCHMARK_STATIC_PORT ?? 8080);
const staticUrl = `http://127.0.0.1:${staticPort}`;
const viewport = {
  width: Number(process.env.BENCHMARK_WIDTH ?? 390),
  height: Number(process.env.BENCHMARK_HEIGHT ?? 844),
};
const deviceScaleFactor = Number(process.env.BENCHMARK_DPR ?? 2);
const headless = process.env.BENCHMARK_HEADLESS !== "false";

const children = [];
let browser;
let interrupted = false;

process.once("SIGINT", () => {
  interrupted = true;
  process.exitCode = 130;
  void cleanup();
});
process.once("SIGTERM", () => {
  interrupted = true;
  process.exitCode = 143;
  void cleanup();
});

try {
  process.stderr.write("Building and starting benchmark web bundle...\n");
  const staticServer = startProcess(
    yarnCommand(),
    ["benchmark:serve", "--host", "127.0.0.1", "--port", String(staticPort)],
    journeyKernelDirectory,
  );
  children.push(staticServer);
  await waitForHttp(`${staticUrl}/index.html`, staticServer, 120_000);

  process.stderr.write("Starting deterministic Rust benchmark server...\n");
  const rustServer = startProcess(
    "cargo",
    ["run", "--release", "--example", "benchmark_server"],
    rustDirectory,
    { DEV_SERVER: staticUrl },
  );
  children.push(rustServer);
  const rawMapUrl = await waitForOutput(
    rustServer,
    /BENCHMARK_URL=(https?:\/\/\S+)/,
    120_000,
  );
  const benchmarkUrl = buildBenchmarkUrl(rawMapUrl);

  try {
    browser = await chromium.launch(
      browserLaunchOptions("chromium", {
        headless,
        args: [
          "--disable-background-timer-throttling",
          "--disable-renderer-backgrounding",
        ],
      }),
    );
  } catch (error) {
    if (String(error).includes("Executable doesn't exist")) {
      throw new Error(
        "Chromium is not installed. Run `yarn benchmark:install` once, then retry.",
      );
    }
    throw error;
  }

  const context = await browser.newContext({
    viewport,
    deviceScaleFactor,
  });
  const page = await context.newPage();
  page.setDefaultTimeout(120_000);

  const browserErrors = [];
  page.on("pageerror", (error) => browserErrors.push(String(error)));
  page.on("console", (message) => {
    // A headed Chromium also asks for a favicon, which the bundle server does
    // not serve. That 404 says nothing about rendering.
    if (
      message.type() === "error" &&
      !(message.location()?.url ?? "").endsWith("/favicon.ico")
    ) {
      browserErrors.push(message.text());
    }
  });

  process.stderr.write("Running journey rendering scenarios...\n");
  await page.goto(benchmarkUrl, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(
    () => typeof window.__journeyBenchmark?.run === "function",
  );
  const pageReport = await page.evaluate(() => window.__journeyBenchmark.run());

  if (browserErrors.length > 0) {
    throw new Error(`Browser errors detected:\n${browserErrors.join("\n")}`);
  }

  const report = {
    ...pageReport,
    runner: {
      browserVersion: browser.version(),
      headless,
      browserErrors,
    },
  };

  await mkdir(resultDirectory, { recursive: true });
  const serialized = `${JSON.stringify(report, null, 2)}\n`;
  const timestamp = new Date().toISOString().replaceAll(":", "-");
  const resultPath = path.join(resultDirectory, `${timestamp}.json`);
  await Promise.all([
    writeFile(resultPath, serialized),
    writeFile(path.join(resultDirectory, "latest.json"), serialized),
  ]);

  process.stdout.write(serialized);
  process.stderr.write(`Saved benchmark result to ${resultPath}\n`);
} catch (error) {
  if (!interrupted) {
    process.stderr.write(`${error instanceof Error ? error.stack : error}\n`);
    process.exitCode = 1;
  }
} finally {
  await cleanup();
}

function buildBenchmarkUrl(rawUrl) {
  const [baseUrl, rawHash = ""] = rawUrl.split("#", 2);
  const params = new URLSearchParams(rawHash);
  for (const key of ["west", "south", "east", "north"]) {
    params.delete(key);
  }
  params.set("map_style", `${staticUrl}/benchmark-style.json`);
  params.set("debug", "false");
  params.set("render", "canvas");
  params.set("projection", "mercator");
  params.set("lng", "114.21247");
  params.set("lat", "22.697006");
  params.set("zoom", "8");
  return `${baseUrl}#${params.toString()}`;
}

async function cleanup() {
  if (browser) {
    await browser.close().catch(() => undefined);
    browser = undefined;
  }
  await Promise.all(children.splice(0).map(stopProcess));
}

function yarnCommand() {
  return process.platform === "win32" ? "yarn.cmd" : "yarn";
}
