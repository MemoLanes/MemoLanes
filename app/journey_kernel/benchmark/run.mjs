import { spawn } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

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
    browser = await chromium.launch({
      headless,
      args: [
        "--disable-background-timer-throttling",
        "--disable-renderer-backgrounding",
      ],
    });
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
    if (message.type() === "error") {
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

function startProcess(command, args, cwd, extraEnvironment = {}) {
  const child = spawn(command, args, {
    cwd,
    env: { ...process.env, ...extraEnvironment },
    detached: process.platform !== "win32",
    stdio: ["ignore", "pipe", "pipe"],
  });
  child.output = "";
  child.spawnError = null;
  const collect = (chunk) => {
    child.output = `${child.output}${chunk}`.slice(-40_000);
  };
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", collect);
  child.stderr.on("data", collect);
  child.on("error", (error) => {
    child.spawnError = error;
    collect(`${error.stack ?? error}\n`);
  });
  return child;
}

async function waitForHttp(url, child, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (hasProcessStopped(child)) {
      throw new Error(`Web bundle server exited early:\n${child.output}`);
    }
    try {
      const response = await fetch(url);
      if (response.ok) {
        return;
      }
    } catch {
      // Compilation or server startup is still in progress.
    }
    await delay(250);
  }
  throw new Error(`Timed out waiting for ${url}:\n${child.output}`);
}

function waitForOutput(child, pattern, timeoutMs) {
  return new Promise((resolve, reject) => {
    const inspect = () => {
      const match = child.output.match(pattern);
      if (match) {
        finish();
        resolve(match[1]);
        return;
      }
      if (hasProcessStopped(child)) {
        finish();
        reject(
          new Error(`Rust benchmark server exited early:\n${child.output}`),
        );
      }
    };
    const timeout = setTimeout(() => {
      finish();
      reject(
        new Error(`Timed out waiting for Rust benchmark URL:\n${child.output}`),
      );
    }, timeoutMs);
    const finish = () => {
      clearTimeout(timeout);
      child.stdout.off("data", inspect);
      child.stderr.off("data", inspect);
      child.off("error", inspect);
      child.off("exit", inspect);
    };

    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("error", inspect);
    child.once("exit", inspect);
    inspect();
  });
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
  params.set("low_power_mode", "false");
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

async function stopProcess(child) {
  if (hasProcessStopped(child)) {
    return;
  }
  signalProcess(child, "SIGINT");
  if (await waitForExit(child, 2_000)) {
    return;
  }
  signalProcess(child, "SIGTERM");
  if (await waitForExit(child, 2_000)) {
    return;
  }
  signalProcess(child, "SIGKILL");
}

function signalProcess(child, signal) {
  try {
    if (process.platform !== "win32" && child.pid) {
      process.kill(-child.pid, signal);
    } else {
      child.kill(signal);
    }
  } catch {
    // The process may have exited between the state check and the signal.
  }
}

function waitForExit(child, timeoutMs) {
  if (hasProcessStopped(child)) {
    return Promise.resolve(true);
  }
  return new Promise((resolve) => {
    const timeout = setTimeout(() => {
      child.off("exit", onExit);
      resolve(false);
    }, timeoutMs);
    const onExit = () => {
      clearTimeout(timeout);
      resolve(true);
    };
    child.once("exit", onExit);
  });
}

function hasProcessStopped(child) {
  return (
    child.spawnError !== null ||
    child.exitCode !== null ||
    child.signalCode !== null
  );
}

function delay(durationMs) {
  return new Promise((resolve) => setTimeout(resolve, durationMs));
}

function yarnCommand() {
  return process.platform === "win32" ? "yarn.cmd" : "yarn";
}
