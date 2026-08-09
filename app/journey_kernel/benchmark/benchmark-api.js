const BENCHMARK_CENTER = [114.21247, 22.697006];
const PAN_DISTANCE_CSS_PIXELS = 140;
const TILE_BUFFER_TIMEOUT_MS = 10_000;

export function installJourneyBenchmark(map, tileProvider) {
  let activeRun = null;

  window.__journeyBenchmark = {
    run: () => {
      if (activeRun) {
        return activeRun;
      }

      const task = runBenchmark(map, tileProvider).finally(() => {
        if (activeRun === task) {
          activeRun = null;
        }
      });
      activeRun = task;
      return task;
    },
  };
  console.log("Journey benchmark ready");
}

async function runBenchmark(map, tileProvider) {
  const scenarios = [];
  scenarios.push(await runPanScenario(map, tileProvider, "low_zoom_pan", 8));
  scenarios.push(await runZoomScenario(map, tileProvider));
  scenarios.push(await runPanScenario(map, tileProvider, "high_zoom_pan", 14));

  return {
    schemaVersion: 1,
    createdAt: new Date().toISOString(),
    environment: readEnvironment(map),
    scenarios,
  };
}

async function runPanScenario(map, tileProvider, name, zoom) {
  await settleAt(map, tileProvider, zoom);
  await runPanSequence(map, 400);
  await settleAt(map, tileProvider, zoom);

  return measureScenario(map, name, () => runPanSequence(map, 1_200));
}

async function runZoomScenario(map, tileProvider) {
  await settleAt(map, tileProvider, 10);
  await runZoomSequence(map, 800);
  await settleAt(map, tileProvider, 10);

  return measureScenario(map, "zoom_transition", () =>
    runZoomSequence(map, 1_800),
  );
}

async function settleAt(map, tileProvider, zoom) {
  map.stop();
  const idle = waitForMapEvent(map, "idle", 10_000);
  map.jumpTo({
    center: BENCHMARK_CENTER,
    zoom,
    bearing: 0,
    pitch: 0,
  });
  map.triggerRepaint();
  await idle;
  await waitWithTimeout(
    tileProvider.waitForTileBufferUpdate(),
    TILE_BUFFER_TIMEOUT_MS,
    "Timed out waiting for journey tile buffer",
  );
  await nextAnimationFrames(2);
}

async function runPanSequence(map, legDurationMs) {
  const centerPoint = map.project(BENCHMARK_CENTER);
  const left = map.unproject([
    centerPoint.x - PAN_DISTANCE_CSS_PIXELS,
    centerPoint.y,
  ]);
  const right = map.unproject([
    centerPoint.x + PAN_DISTANCE_CSS_PIXELS,
    centerPoint.y,
  ]);

  await easeTo(map, left, map.getZoom(), legDurationMs);
  await easeTo(map, right, map.getZoom(), legDurationMs);
  await easeTo(map, BENCHMARK_CENTER, map.getZoom(), legDurationMs);
}

async function runZoomSequence(map, legDurationMs) {
  await easeTo(map, BENCHMARK_CENTER, 14, legDurationMs);
  await easeTo(map, BENCHMARK_CENTER, 10, legDurationMs);
}

async function easeTo(map, center, zoom, duration) {
  const moveEnd = waitForMapEvent(map, "moveend", duration + 5_000);
  map.easeTo({
    center,
    zoom,
    duration,
    easing: (value) => value,
    essential: true,
  });
  await moveEnd;
}

async function measureScenario(map, name, action) {
  const animationTimestamps = [];
  const mapRenderTimestamps = [];
  let animationFrameId = 0;
  let collecting = true;

  const collectAnimationFrame = (timestamp) => {
    animationTimestamps.push(timestamp);
    if (collecting) {
      animationFrameId = requestAnimationFrame(collectAnimationFrame);
    }
  };
  const collectMapRender = () => {
    mapRenderTimestamps.push(performance.now());
  };

  const startTime = performance.now();
  const existingLongFrameCount = performance.getEntriesByType(
    "long-animation-frame",
  ).length;
  map.on("render", collectMapRender);
  animationFrameId = requestAnimationFrame(collectAnimationFrame);

  try {
    await action();
    await nextAnimationFrames(2);
  } finally {
    collecting = false;
    cancelAnimationFrame(animationFrameId);
    map.off("render", collectMapRender);
  }

  const endTime = performance.now();
  const longFrames = performance
    .getEntriesByType("long-animation-frame")
    .slice(existingLongFrameCount)
    .filter(
      (entry) => entry.startTime >= startTime && entry.startTime <= endTime,
    );

  return {
    name,
    durationMs: round(endTime - startTime),
    animationFrames: summarizeFrames(animationTimestamps),
    mapRenderFrames: summarizeFrames(mapRenderTimestamps),
    longAnimationFrameCount: longFrames.length,
    longestAnimationFrame: round(
      longFrames.reduce((max, entry) => Math.max(max, entry.duration), 0),
    ),
  };
}

function summarizeFrames(timestamps) {
  const intervals = [];
  for (let index = 1; index < timestamps.length; index++) {
    const duration = timestamps[index] - timestamps[index - 1];
    if (duration > 0) {
      intervals.push(duration);
    }
  }
  if (intervals.length === 0) {
    return {
      sampleCount: 0,
      averageFps: 0,
      frameTimeP50: 0,
      frameTimeP95: 0,
      frameTimeP99: 0,
      framesOver20msRatio: 0,
      framesOver33msRatio: 0,
      maxFrameTime: 0,
    };
  }

  const sorted = [...intervals].sort((left, right) => left - right);
  const average =
    intervals.reduce((total, duration) => total + duration, 0) /
    intervals.length;

  return {
    sampleCount: intervals.length,
    averageFps: round(1_000 / average),
    frameTimeP50: round(percentile(sorted, 0.5)),
    frameTimeP95: round(percentile(sorted, 0.95)),
    frameTimeP99: round(percentile(sorted, 0.99)),
    framesOver20msRatio: round(
      intervals.filter((duration) => duration > 20).length / intervals.length,
    ),
    framesOver33msRatio: round(
      intervals.filter((duration) => duration > 33.3).length / intervals.length,
    ),
    maxFrameTime: round(sorted[sorted.length - 1]),
  };
}

function percentile(sorted, fraction) {
  const index = Math.max(
    0,
    Math.min(sorted.length - 1, Math.ceil(sorted.length * fraction) - 1),
  );
  return sorted[index];
}

function readEnvironment(map) {
  const canvas = map.getCanvas();
  const gl = canvas.getContext("webgl2");
  const debugInfo = gl?.getExtension("WEBGL_debug_renderer_info");

  return {
    userAgent: navigator.userAgent,
    hardwareConcurrency: navigator.hardwareConcurrency,
    viewportWidth: canvas.clientWidth,
    viewportHeight: canvas.clientHeight,
    devicePixelRatio: window.devicePixelRatio,
    gpuVendor:
      gl && debugInfo
        ? String(gl.getParameter(debugInfo.UNMASKED_VENDOR_WEBGL))
        : null,
    gpuRenderer:
      gl && debugInfo
        ? String(gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL))
        : null,
  };
}

function waitForMapEvent(map, eventName, timeoutMs) {
  return new Promise((resolve, reject) => {
    const complete = () => {
      window.clearTimeout(timeout);
      map.off(eventName, complete);
      resolve();
    };
    const timeout = window.setTimeout(() => {
      map.off(eventName, complete);
      reject(new Error(`Timed out waiting for MapLibre ${eventName}`));
    }, timeoutMs);
    map.once(eventName, complete);
  });
}

function waitWithTimeout(task, timeoutMs, message) {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error(message));
    }, timeoutMs);
    task.then(
      (value) => {
        window.clearTimeout(timeout);
        resolve(value);
      },
      (error) => {
        window.clearTimeout(timeout);
        reject(error);
      },
    );
  });
}

function nextAnimationFrames(count) {
  return new Promise((resolve) => {
    const next = () => {
      if (count <= 0) {
        resolve();
        return;
      }
      count--;
      requestAnimationFrame(next);
    };
    next();
  });
}

function round(value) {
  return Math.round(value * 100) / 100;
}
