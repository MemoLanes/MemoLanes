import { loseContext, withDeadline } from "./context-loss";
import { Map as MapLibreMap } from "maplibre-gl";
import { CanvasMeshLayer } from "../../static/layers/canvas-mesh-layer";
import { JOURNEY_LAYER_ID } from "../../static/layers/journey-layer-interface";
import { createAchievementRegionOverlay } from "../../static/achievement-region-overlay";
import { FlutterBridge, notifyFlutterReady } from "../../static/flutter-bridge";
import { notifyWhenDisplayReady } from "../../static/display-ready";
import {
  createMapContainer,
  createMapControllerFixture,
  waitForIdle,
  waitForRender,
} from "./map-fixture";

function createExplorerFixture(params, { documentId = 1, ...options } = {}) {
  const previous = window.EXTERNAL_PARAMS;
  window.EXTERNAL_PARAMS = {
    ...previous,
    achievement_map_document: documentId,
  };
  let restored = false;
  const restoreParams = () => {
    if (!restored) window.EXTERNAL_PARAMS = previous;
    restored = true;
  };
  try {
    const fixture = createMapControllerFixture(params, options);
    return {
      ...fixture,
      restoreParams,
      dispose() {
        try {
          fixture.dispose();
        } finally {
          restoreParams();
        }
      },
    };
  } catch (error) {
    restoreParams();
    throw error;
  }
}

function feature(id, west, south, east, north) {
  return {
    type: "Feature",
    id,
    properties: { region: id },
    geometry: {
      type: "Polygon",
      coordinates: [
        [
          [west, south],
          [east, south],
          [east, north],
          [west, north],
          [west, south],
        ],
      ],
    },
  };
}

const highlight = (map) =>
  map
    .getLayersOrder()
    .map((id) => map.getLayer(id))
    .find(
      (layer) =>
        layer?.type === "custom" &&
        layer.id.startsWith("memolanes-achievement-region-mask"),
    );

async function readPixelsAt(map, points) {
  let result;
  map.once("render", () => {
    const canvas = map.getCanvas();
    const gl = canvas.getContext("webgl2");
    result = points.map(({ x, y }) => {
      const rgba = new Uint8Array(4);
      gl.readPixels(
        Math.floor((x * gl.drawingBufferWidth) / canvas.clientWidth),
        gl.drawingBufferHeight -
          1 -
          Math.floor((y * gl.drawingBufferHeight) / canvas.clientHeight),
        1,
        1,
        gl.RGBA,
        gl.UNSIGNED_BYTE,
        rgba,
      );
      return Array.from(rgba);
    });
  });
  await waitForRender(map);
  return result;
}

export async function checkAchievementRevealPixels(check) {
  const reports = [];
  for (const type of ["mercator", "globe"]) {
    const container = createMapContainer(360, 240);
    const map = new MapLibreMap({
      container,
      center: [0, 0],
      zoom: 2,
      pixelRatio: 3,
      attributionControl: false,
      style: {
        version: 8,
        projection: { type },
        sources: {},
        layers: [
          {
            id: "background",
            type: "background",
            paint: { "background-color": "#304050" },
          },
        ],
      },
    });
    const project = (...positions) =>
      positions.map((position) => map.project(position));
    const assertPixel = (actual, expected, label) =>
      check(
        actual.every((v, i) => Math.abs(v - expected[i]) <= 1),
        `${type}: ${label}: expected ${expected}, got ${actual}`,
      );
    const params = window.EXTERNAL_PARAMS;
    window.EXTERNAL_PARAMS = { ...params, cgi_endpoint: "." };
    try {
      await waitForIdle(map);
      map.addLayer(
        new CanvasMeshLayer(
          map,
          JOURNEY_LAYER_ID,
          [0, 0, 0, 0.5],
          [0, 0, 0, 0],
          {
            registerRefreshCallback() {},
            unregisterRefreshCallback() {},
          },
        ),
      );
      const set = createAchievementRegionOverlay(map);
      const box = feature("box", -18, -15, 18, 15);
      box.geometry.coordinates.push(
        feature("hole", 4, -4, 9, 4).geometry.coordinates[0].slice().reverse(),
      );
      set(box);
      const boundary = map.project([-18, 0]);
      const [core, hole, outside, edge, shoulder] = await readPixelsAt(map, [
        ...project([0, 0], [6, 0], [24, 0]),
        { x: boundary.x + 3, y: boundary.y },
        { x: boundary.x + 9, y: boundary.y },
      ]);
      check(
        shoulder[2] - outside[2] < (core[2] - outside[2]) * 0.6,
        `${type}: the coast is not traced by a bright shoulder (${outside}, ${shoulder}, ${core})`,
      );
      assertPixel(
        core,
        [33, 44, 54, 255],
        "selection reduces existing fog without tinting the basemap",
      );
      assertPixel(hole, [24, 32, 40, 255], "a hole remains unchanged");
      assertPixel(
        outside,
        [24, 32, 40, 255],
        "the unselected fog remains unchanged",
      );
      check(
        edge[2] > outside[2] && edge[2] < core[2],
        `${type}: the lift tapers inside the boundary (${edge})`,
      );

      const thin = feature("thin", -11, -0.45, 11, 0.45);
      const left = feature("clipped", -70, -14, -20, 14);
      set({
        ...thin,
        geometry: {
          type: "MultiPolygon",
          coordinates: [thin.geometry.coordinates, left.geometry.coordinates],
        },
      });
      const [narrow, clipped] = await readPixelsAt(map, [
        map.project([0, 0]),
        { x: 1, y: 120 },
      ]);
      check(
        narrow[2] > outside[2] && narrow[2] < core[2],
        `${type}: a narrow component keeps some lift (${narrow})`,
      );
      assertPixel(
        clipped,
        core,
        "the edge of the viewport is not a region boundary",
      );
      const strip = feature("thin-high-dpr", -11, -0.09, 11, 0.09);
      set(strip);
      const [stripCore, stripOutside] = await readPixelsAt(
        map,
        project([0, 0], [0, 0.3]),
      );
      assertPixel(
        stripCore,
        outside,
        "a one-CSS-pixel strip does not become a bright line at DPR 3",
      );
      assertPixel(
        stripOutside,
        outside,
        "thin selections do not brighten outside their edge",
      );

      map.jumpTo({ center: [180, 0] });
      const east = feature("date", 155, -18, 180, 18);
      const west = feature("date", -180, -18, -155, 18);
      set({
        ...east,
        geometry: {
          type: "MultiPolygon",
          coordinates: [east.geometry.coordinates, west.geometry.coordinates],
        },
      });
      const positions = project([177, 0], [180, 0], [183, 0]);
      const dateline = await readPixelsAt(map, positions);
      dateline.forEach((pixel, i) =>
        assertPixel(
          pixel,
          core,
          `the joined dateline pieces have no inner seam (${JSON.stringify({ i, positions, dateline, visible: map.coveringTiles({ tileSize: 512 }).map((t) => [t.canonical.x, t.canonical.y, t.wrap]) })})`,
        ),
      );
      if (type === "globe") {
        map.jumpTo({ center: [0, 0] });
        const [front] = await readPixelsAt(map, project([0, 0]));
        assertPixel(
          front,
          outside,
          "geometry behind the globe cannot brighten its front",
        );
        map.jumpTo({ center: [20, 87], zoom: 2 });
        set(feature("south", -15, -37, 15, -17));
        const poles = project([45, 87], [-85, 87]);
        const [polarBase, unselectedBase] = await readPixelsAt(map, poles);
        set(feature("polar", 0, 73, 90, 90));
        const [polar, unselected] = await readPixelsAt(map, poles);
        check(
          polar[2] > polarBase[2],
          `the polar cap follows the selected meridians (${polarBase} → ${polar})`,
        );
        assertPixel(
          unselected,
          unselectedBase,
          "the other side of the pole remains unchanged",
        );
      }

      map.setStyle(
        { version: 8, projection: { type }, sources: {}, layers: [] },
        { diff: false },
      );
      await waitForIdle(map);
      set(feature("empty", -18, -15, 18, 15));
      map.jumpTo({ center: [0, 0] });
      const [transparent] = await readPixelsAt(map, project([0, 0]));
      assertPixel(
        transparent,
        [0, 0, 0, 0],
        "an empty style remains transparent",
      );
      reports.push(
        `Achievement ${type} fog reveal: boundaries, holes, islands, and seams`,
      );
    } finally {
      map.remove();
      container.remove();
      window.EXTERNAL_PARAMS = params;
    }
  }
  return reports;
}

export async function checkAchievementKeepsMapSymbols(check) {
  const container = createMapContainer(360, 240);
  const map = new MapLibreMap({
    container,
    center: [0, 0],
    zoom: 3,
    attributionControl: false,
    style: {
      version: 8,
      sources: {},
      layers: [
        {
          id: "base",
          type: "background",
          paint: { "background-color": "#304050" },
        },
      ],
    },
  });
  try {
    await waitForIdle(map);
    const data = new Uint8Array(8 * 8 * 4);
    for (let i = 0; i < data.length; i += 4) data.set([120, 80, 60, 255], i);
    map.addImage("icon", { width: 8, height: 8, data });
    map.addSource("place", {
      type: "geojson",
      data: { type: "Point", coordinates: [0, 0] },
    });
    map.addLayer({
      id: "label",
      type: "symbol",
      source: "place",
      layout: { "icon-image": "icon" },
    });
    await waitForIdle(map);
    const points = [map.project([0, 0]), map.project([3, 0])];
    const [icon, ground] = await readPixelsAt(map, points);
    check(
      icon.join() === "120,80,60,255",
      "symbol is visible before selection",
    );
    const set = createAchievementRegionOverlay(map);
    set(feature("country", -7, -7, 7, 7));
    await waitForIdle(map);
    const [selectedIcon, selectedGround] = await readPixelsAt(map, points);
    check(
      selectedIcon.join() === icon.join(),
      "the highlight does not recolor the basemap symbol",
    );
    check(
      selectedGround.join() === ground.join(),
      "selection does not change a basemap with no Journey fog",
    );
    return "Achievement leaves basemap and symbols unchanged without Journey fog";
  } finally {
    map.remove();
    container.remove();
  }
}

export async function checkAchievementResizeBeforeFit(check) {
  const { container, controller, map, dispose } = createExplorerFixture(
    {
      projection: "mercator",
      custom_basemap: true,
      map_style: { version: 8, sources: {}, layers: [] },
    },
    { height: 120, documentId: 31 },
  );
  try {
    await controller.initialize();
    new FlutterBridge(controller).initialize();
    const bounds = { west: 16, south: 30, east: 24, north: 38 };
    const request = (generation, selected = true) =>
      window.applyAchievementRegionCommand({
        document: 31,
        generation,
        revision: generation,
        featureGeoJson: selected
          ? JSON.stringify(feature("resized", 16, 30, 24, 38))
          : null,
        bounds: selected ? bounds : null,
        padding: { top: 24, right: 24, bottom: 180, left: 24 },
        view: null,
      });
    const centered = () => Math.abs(map.getCenter().lng - 20) < 0.1;
    check(
      request(1) && !centered(),
      "the drawing is accepted before the native view has room for the fit",
    );
    container.style.height = "380px";
    map.resize();
    await waitForIdle(map);
    check(
      centered(),
      "the accepted selection fits once the native viewport catches up",
    );
    map.jumpTo({ center: [1, 2] });
    container.style.height = "420px";
    map.resize();
    await waitForIdle(map);
    check(
      map.getCenter().lng === 1,
      "a completed selection is not fitted again on resize",
    );
    container.style.height = "120px";
    map.resize();
    request(2);
    request(3, false);
    container.style.height = "380px";
    map.resize();
    await waitForIdle(map);
    check(
      map.getCenter().lng === 1,
      "clearing a deferred selection retires its fit",
    );
    return "Achievement retries only an unfinished fit when the WebView resizes";
  } finally {
    dispose();
  }
}

export async function checkAchievementPolarResizeBeforeFit(check) {
  const { container, controller, map, dispose } = createExplorerFixture(
    {
      projection: "globe",
      custom_basemap: true,
      map_style: { version: 8, sources: {}, layers: [] },
    },
    { height: 120, documentId: 32 },
  );
  try {
    await controller.initialize();
    new FlutterBridge(controller).initialize();
    map.jumpTo({ center: [0, 20], zoom: 7 });
    const bounds = { west: 16, south: 82, east: 24, north: 87 };
    const request = window.applyAchievementRegionCommand({
      document: 32,
      generation: 1,
      revision: 1,
      featureGeoJson: JSON.stringify(feature("polar", 16, 82, 24, 87)),
      bounds,
      padding: { top: 24, right: 24, bottom: 180, left: 24 },
      view: null,
    });
    check(
      request && Math.abs(map.getCenter().lat - 20) < 1,
      "polar fit is initially deferred",
    );
    container.style.height = "380px";
    map.resize();
    await Promise.resolve();
    check(
      map.getProjection().type === "vertical-perspective",
      "resize does not drop the polar projection lock before the flight",
    );
    await waitForIdle(map);
    check(
      map.getCenter().lat > 80,
      "the deferred flight reaches the polar selection",
    );
    return "Achievement deferred polar fit retains its projection lock across native resize";
  } finally {
    dispose();
  }
}

export async function startAchievementStalledBasemap(check) {
  const { controller, map, dispose, restoreParams } = createExplorerFixture(
    {
      projection: "mercator",
      map_style: new URL("stalled-style/builtin.json", location.href).href,
    },
    { id: "stalled-basemap-regression" },
  );
  try {
    await controller.initialize();
    await withDeadline(
      controller.waitForDisplay(),
      "stalled style blocked the local frame",
    );
    check(
      map.getLayer("offline-basemap"),
      "the hung style leaves a usable local map",
    );
  } catch (error) {
    dispose();
    throw error;
  } finally {
    restoreParams();
  }
  const result = withDeadline(
    map.once("style.load"),
    "the stalled built-in style was never replaced",
    38_000,
  )
    .then(async () => {
      await withDeadline(
        controller.waitForDisplay(),
        "retry did not restore journey display",
      );
      check(
        map.getLayer("retried-basemap"),
        "the retry installs the replacement style",
      );
      return "Achievement retries a stalled built-in style";
    })
    .finally(dispose);
  return { result };
}

export async function checkAchievementEmptyCustomStyle(check) {
  const { controller, map, dispose } = createExplorerFixture({
    projection: "mercator",
    custom_basemap: true,
    fog_density: "0",
    map_style: new URL("empty-style/custom.json", location.href).href,
  });
  const previousReady = window.readyForDisplay;
  let emptyFrames = 0;
  let sawLocalBackground = false;
  const signals = [];
  const isEmptyRemoteStyle = () =>
    !!map.getStyle() &&
    !map.getStyle().metadata?.["memolanes:placeholder"] &&
    map.getLayersOrder().join() === JOURNEY_LAYER_ID;
  map.on("render", () => {
    if (isEmptyRemoteStyle()) emptyFrames++;
  });
  window.readyForDisplay = {
    postMessage: (message) => signals.push({ message, emptyFrames }),
  };
  // Subscribe before initialize can replace the local background. Do not start
  // the readiness observer until the actual remote empty style has rendered.
  const remoteLoaded = new Promise((resolve) => {
    const onStyleLoad = () => {
      if (map.getLayer("offline-basemap")) sawLocalBackground = true;
      if (!isEmptyRemoteStyle()) return;
      map.off("style.load", onStyleLoad);
      resolve();
    };
    map.on("style.load", onStyleLoad);
  });
  try {
    await withDeadline(
      controller.initialize(),
      "empty style initialization timed out",
    );
    new FlutterBridge(controller).initialize();
    await withDeadline(remoteLoaded, "remote empty custom style never loaded");
    await withDeadline(waitForRender(map), "remote empty style never rendered");
    check(
      sawLocalBackground &&
        isEmptyRemoteStyle() &&
        !map.getLayer("offline-basemap") &&
        emptyFrames > 0 &&
        signals.length === 0,
      "remote empty style replaced the background and rendered before first readiness observation",
    );
    await withDeadline(
      notifyWhenDisplayReady(controller, notifyFlutterReady),
      "rendered empty custom style did not report ready",
      2500,
    );
    check(
      signals.length === 1 &&
        signals[0].message === "" &&
        signals[0].emptyFrames > 0,
      "empty custom style reports real Flutter readiness once after rendering",
    );
    check(
      selectFixtureRegion("empty-style"),
      "selection command accepted only after empty style readiness",
    );
    await withDeadline(
      waitForIdle(map),
      "empty style selection did not settle",
    );
    await withDeadline(
      waitForRender(map),
      "empty style selection frame missing",
    );
    check(
      !!highlight(map) &&
        (await readPixelsAt(map, [map.project([0, 38])]))[0].every(
          (value) => value === 0,
        ) &&
        signals.length === 1,
      "selection fits without adding color to the empty style or duplicating readiness",
    );
    return "Achievement rendered empty custom style readiness and selection";
  } finally {
    dispose();
    window.readyForDisplay = previousReady;
  }
}

function selectFixtureRegion(id) {
  return window.applyAchievementRegionCommand({
    document: 1,
    generation: 1,
    revision: 1,
    featureGeoJson: JSON.stringify(feature(id, -4, 34, 4, 42)),
    bounds: { west: -4, south: 34, east: 4, north: 42 },
    padding: { top: 24, right: 24, bottom: 24, left: 24 },
    view: null,
  });
}

export async function checkAchievementBasemapReady(check) {
  const tileUrl = new URL("slow-basemap/tile.png", location.href).href;
  const { controller, map, dispose } = createExplorerFixture({
    projection: "mercator",
    custom_basemap: true,
    map_style: {
      version: 8,
      sources: {
        tiles: { type: "raster", tiles: [tileUrl], tileSize: 256 },
      },
      layers: [
        {
          id: "base",
          type: "background",
          paint: { "background-color": "#eee" },
        },
        { id: "tiles", type: "raster", source: "tiles" },
      ],
    },
  });
  const previousReady = window.readyForDisplay;
  const signals = [];
  let completedFrames = 0;
  map.on("render", () => {
    if (
      map.getLayer("base") &&
      !map.getStyle().metadata?.["memolanes:placeholder"]
    )
      completedFrames++;
  });
  window.readyForDisplay = {
    postMessage: (message) => signals.push({ message, completedFrames }),
  };
  try {
    const setup = (async () => {
      await controller.initialize();
      const bridge = new FlutterBridge(controller);
      bridge.initialize();
      await notifyWhenDisplayReady(controller, notifyFlutterReady);
    })();
    check(
      signals.length === 0,
      "no readiness notification before a completed frame",
    );
    await withDeadline(setup, "basemap readiness was not signalled", 2500);
    check(
      signals.length === 1 &&
        signals[0].message === "" &&
        signals[0].completedFrames > 0,
      "the real Flutter callback fires once after a completed basemap frame",
    );
    check(
      !map.areTilesLoaded(),
      "remote tiles remain unavailable at readiness",
    );
    check(
      map.getLayersOrder().join() === `base,tiles,${JOURNEY_LAYER_ID}`,
      "the remote basemap and journey layer share one globe",
    );
    const movement = map.once("moveend");
    check(
      selectFixtureRegion("selected"),
      "selection accepted with remote tiles pending",
    );
    await movement;
    await waitForRender(map);
    check(
      !!highlight(map),
      "selection draws after readiness while remote tiles are pending",
    );
    check(
      Math.abs(map.getCenter().lng) < 0.01 &&
        map.getCenter().lat > 37 &&
        map.getCenter().lat < 39 &&
        map.getZoom() > 1,
      "selection fits after readiness while remote tiles are pending",
    );
    check(
      signals.length === 1,
      "later selection does not notify readiness again",
    );
    return "Achievement basemap readiness without remote tiles";
  } finally {
    dispose();
    window.readyForDisplay = previousReady;
  }
}

export async function checkAchievementOfflineStyle(check, customBasemap) {
  const styleFile = customBasemap ? "custom.json" : "builtin.json";
  const { controller, map, dispose } = createExplorerFixture({
    custom_basemap: customBasemap,
    map_style: new URL(`missing-basemap/${styleFile}`, location.href).href,
  });
  const previousReady = window.readyForDisplay;
  let renderedFallback = false;
  const signals = [];
  map.on("render", () => {
    if (map.getLayer("offline-basemap")) renderedFallback = true;
  });
  window.readyForDisplay = {
    postMessage: (message) => signals.push({ message, renderedFallback }),
  };
  try {
    const setup = (async () => {
      await controller.initialize();
      new FlutterBridge(controller).initialize();
      await notifyWhenDisplayReady(controller, notifyFlutterReady);
    })();
    check(
      signals.length === 0,
      "unavailable style URL cannot notify before a drawable frame",
    );
    await withDeadline(setup, `${styleFile} did not become drawable`, 3000);
    check(
      signals.length === 1 &&
        signals[0].message === "" &&
        signals[0].renderedFallback,
      "offline fallback notifies the real Flutter callback once after rendering",
    );
    check(
      map.getLayersOrder().join() === `offline-basemap,${JOURNEY_LAYER_ID}`,
      "offline fallback retains the journey layer",
    );
    check(
      map.getProjection()?.type === "globe",
      "offline fallback retains the configured default globe projection",
    );
    check(
      selectFixtureRegion("offline"),
      "selection accepted on the offline globe",
    );
    await waitForIdle(map);
    check(
      map.getProjection()?.type === "globe" &&
        Math.abs(map.getCenter().lng) < 0.01 &&
        map.getCenter().lat > 37 &&
        map.getCenter().lat < 39 &&
        map.getZoom() > 1,
      "offline selection fits in the default globe projection",
    );
    check(
      !!highlight(map),
      "an offline selection can draw on the fallback style",
    );
    const before = window.getCurrentMapView();
    map.setStyle(
      {
        version: 8,
        sources: {},
        layers: [
          {
            id: "recovered",
            type: "background",
            paint: { "background-color": "#ddd" },
          },
        ],
      },
      { diff: false },
    );
    await waitForIdle(map);
    check(
      window.getCurrentMapView() === before && !!highlight(map),
      "style recovery restores the outline without refitting the camera",
    );
    return `Achievement offline ${customBasemap ? "custom" : "built-in"} style URL fallback and recovery`;
  } finally {
    dispose();
    window.readyForDisplay = previousReady;
  }
}

export async function checkAchievementRegionOverlay(check) {
  const style = (color) => ({
    version: 8,
    sources: {},
    layers: [
      {
        id: "basemap",
        type: "background",
        paint: { "background-color": color },
      },
    ],
  });
  const { controller, map, dispose } = createExplorerFixture(
    {
      projection: "mercator",
      map_style: style("#eee"),
      custom_basemap: true,
    },
    { documentId: 2 },
  );
  const baselineLayers = `basemap,${JOURNEY_LAYER_ID}`;

  const failures = [];
  const regression = (condition, message) => {
    if (!condition) failures.push(message);
  };
  try {
    await controller.initialize();
    const bridge = new FlutterBridge(controller);
    bridge.initialize();
    await waitForIdle(map);
    check(
      map.getLayersOrder().join() === baselineLayers,
      "initialization shows the basemap and journey layer without a selection",
    );

    const bounds = { west: -4, south: 34, east: 4, north: 42 };
    const padding = { top: 24, right: 24, bottom: 24, left: 24 };
    const first = feature("first", -4, 34, -0.5, 42);
    const second = feature("second", 0.5, 34, 4, 42);
    window.setAchievementRegion(first);
    window.flyToBounds(bounds, padding);
    await waitForIdle(map);
    check(!!highlight(map), "the first selection installs the mask");
    const fitted = JSON.parse(window.getCurrentMapView());
    const atFit = () => {
      const live = JSON.parse(window.getCurrentMapView());
      return ["lng", "lat", "zoom"].every(
        (key) => Math.abs(live[key] - fitted[key]) < 1e-6,
      );
    };
    const order = map.getLayersOrder().join();
    const firstLayer = highlight(map);
    window.setAchievementRegion(second);
    window.flyToBounds(bounds, padding);
    check(
      highlight(map) !== firstLayer,
      "the previous selection is retired immediately",
    );
    await waitForIdle(map);
    check(!!highlight(map), "the replacement installs one mask");
    check(
      map.getLayersOrder().join() === order,
      "replacement does not stack layers",
    );
    map.jumpTo({ center: [0.5, 38.5], zoom: map.getZoom() + 0.3 });
    await waitForIdle(map);
    const panned = window.getCurrentMapView();
    check(!atFit(), "a manual camera update remains possible");
    map.setStyle(style("#ccc"), { diff: false });
    await waitForIdle(map);
    check(
      !!highlight(map),
      "style replacement restores the last selected polygon",
    );
    check(
      map.getLayersOrder().join() === order,
      "style replacement restores the selection mask",
    );
    check(
      window.getCurrentMapView() === panned,
      "style restoration does not fit again",
    );
    map.setStyle(style("#ddd"));
    await waitForIdle(map);
    check(!!highlight(map), "style diff also restores the selected polygon");
    check(
      window.getCurrentMapView() === panned,
      "style diff does not fit again",
    );

    window.flyToBounds(bounds, padding);
    await waitForIdle(map);
    check(
      atFit(),
      "an explicit fit to the same bounds restores the fitted camera",
    );
    window.setAchievementRegion(null);
    await waitForIdle(map);
    check(
      !highlight(map) && map.getLayersOrder().join() === baselineLayers,
      "clearing removes the selection without changing the basemap",
    );
    check(atFit(), "clearing does not fit");

    let fits = 0;
    const fitBounds = controller.fitBounds.bind(controller);
    controller.fitBounds = (...args) => {
      fits++;
      return fitBounds(...args);
    };
    const request = (generation, revision, feature, bounds, document = 2) => {
      window.applyAchievementRegionCommand({
        document,
        generation,
        revision,
        featureGeoJson: feature === null ? null : JSON.stringify(feature),
        bounds,
        padding,
        view: null,
      });
    };
    const olderBounds = { west: -30, south: 10, east: 30, north: 65 };
    request(2, 2, second, bounds);
    request(1, 1, first, olderBounds);
    request(100, 1, first, olderBounds, 1);
    await waitForIdle(map);
    regression(
      !!highlight(map) && atFit(),
      `older commands must not replace B or move the camera (${fits} fits)`,
    );
    regression(fits === 1, `only B should fit (found ${fits} fits)`);

    map.jumpTo({ center: [0.5, 38.5], zoom: map.getZoom() + 0.3 });
    await waitForIdle(map);
    const manuallyMoved = window.getCurrentMapView();
    const overlay = highlight(map);
    request(3, 2, second, bounds); // Retry with an unobserved acknowledgment.
    regression(
      highlight(map) === overlay,
      "a same-target retry does not replace the highlight",
    );
    map.setStyle(style("#bbb"), { diff: false });
    await waitForIdle(map);
    regression(
      !!highlight(map) &&
        window.getCurrentMapView() === manuallyMoved &&
        fits === 1,
      "a same-revision retry and style reload restore B without refitting",
    );

    request(5, 3, null, null);
    request(4, 3, first, olderBounds);
    map.setStyle(style("#aaa"), { diff: false });
    await waitForIdle(map);
    regression(
      !highlight(map) &&
        map.getLayersOrder().join() === baselineLayers &&
        window.getCurrentMapView() === manuallyMoved &&
        fits === 1,
      "a clear and style reload cannot revive an older draw or fit",
    );
    controller.fitBounds = fitBounds;

    check(failures.length === 0, failures.join("; "));
    return "Achievement single-region overlay, ordered updates, and drawing-only style restoration";
  } finally {
    dispose();
  }
}

export async function checkAchievementContextRecovery(check) {
  const failures = [];
  for (const target of [null, { ...feature("B", 2, -2, 6, 2), id: 202 }]) {
    const label = target ? "replace B" : "clear";

    const { controller, map, dispose } = createExplorerFixture({
      projection: "mercator",
      custom_basemap: true,
      map_style: {
        version: 8,
        sources: {},
        layers: [
          {
            id: "local-base",
            type: "background",
            paint: { "background-color": "#eee" },
          },
        ],
      },
    });
    const previousReady = window.readyForDisplay;
    const signals = [];
    window.readyForDisplay = {
      postMessage: (message) => signals.push(message),
    };
    try {
      await withDeadline(
        controller.initialize(),
        `${label}: initialization timed out`,
      );
      new FlutterBridge(controller).initialize();
      await withDeadline(
        notifyWhenDisplayReady(controller, notifyFlutterReady),
        "local readiness timed out",
      );
      map.jumpTo({ center: [0, 0], zoom: 4 });
      let fits = 0;
      const fitBounds = controller.fitBounds.bind(controller);
      controller.fitBounds = (...args) => {
        fits++;
        return fitBounds(...args);
      };
      const request = (generation, selection) =>
        window.applyAchievementRegionCommand({
          document: 1,
          generation,
          revision: generation,
          featureGeoJson: selection === null ? null : JSON.stringify(selection),
          bounds: null,
          padding: { top: 0, right: 0, bottom: 0, left: 0 },
          view: null,
        });
      check(
        request(1, { ...feature("A", -6, -2, -2, 2), id: 101 }),
        "initial drawing command accepted",
      );
      await withDeadline(waitForIdle(map), "initial A did not render");
      check(!!highlight(map), "the selection is installed before context loss");
      const view = window.getCurrentMapView();
      const restore = await loseContext(map);
      check(
        request(2, target),
        `${label}: latest drawing command accepted during loss`,
      );
      await restore();
      await withDeadline(
        waitForIdle(map),
        `${label}: restored map did not become idle`,
      );
      await withDeadline(
        waitForRender(map),
        `${label}: restored frame missing`,
      );
      check(
        controller.getMap() === map && !!map.getLayer("local-base"),
        `${label}: same map restored the inline basemap`,
      );
      if (Boolean(highlight(map)) !== Boolean(target)) {
        failures.push(`${label}: restored layers ${map.getLayersOrder()}`);
      }
      check(
        fits === 0 &&
          window.getCurrentMapView() === view &&
          signals.length === 1,
        `${label}: recovery neither refits nor reports ready again`,
      );
    } finally {
      dispose();
      window.readyForDisplay = previousReady;
    }
  }
  check(failures.length === 0, failures.join("; "));
  return [
    "Achievement real context recovery after clear",
    "Achievement real context recovery after replacement",
  ];
}

export async function checkAchievementStyleRetryAfterWebGLRestore(check) {
  for (const kind of ["builtin", "custom"]) {
    const { controller, map, dispose } = createExplorerFixture(
      {
        projection: "mercator",
        custom_basemap: kind === "custom",
        map_style: new URL(`restore-style/${kind}.json`, location.href).href,
      },
      { disableAutoRefresh: false },
    );
    const previousReady = window.readyForDisplay;
    const signals = [];
    window.readyForDisplay = {
      postMessage: (message) => signals.push(message),
    };
    let fits = 0;
    const fitBounds = controller.fitBounds.bind(controller);
    controller.fitBounds = (...args) => {
      fits++;
      return fitBounds(...args);
    };
    const showsSelection = () => !!highlight(map);
    try {
      await withDeadline(
        controller.initialize(),
        `${kind}: initialization timed out`,
      );
      new FlutterBridge(controller).initialize();
      await withDeadline(
        notifyWhenDisplayReady(controller, notifyFlutterReady),
        `${kind}: no local readiness`,
      );
      await withDeadline(
        window.waitForRecoveryRequest(kind, 1),
        `${kind}: initial request missing`,
      );
      check(
        map.getLayer("offline-basemap") && signals.length === 1,
        `${kind}: local background ready before remote style`,
      );
      check(
        window.applyAchievementRegionCommand({
          document: 1,
          generation: 1,
          revision: 1,
          featureGeoJson: JSON.stringify(feature("restore", -4, 34, 4, 42)),
          bounds: { west: -4, south: 34, east: 4, north: 42 },
          padding: { top: 24, right: 24, bottom: 24, left: 24 },
          view: null,
        }),
        `${kind}: selection accepted`,
      );
      await withDeadline(
        waitForIdle(map),
        `${kind}: initial outline did not render`,
      );
      check(
        showsSelection() && fits === 1,
        `${kind}: local outline rendered and fitted once`,
      );
      map.jumpTo({ center: [0.5, 38.5], zoom: map.getZoom() + 0.3 });
      const panned = window.getCurrentMapView();
      const restore = await loseContext(map);
      await restore();
      await withDeadline(
        waitForIdle(map),
        `${kind}: restored placeholder did not render`,
      );
      check(
        controller.getMap() === map &&
          map.getLayer("offline-basemap") &&
          showsSelection(),
        `${kind}: same map restores local background and outline`,
      );
      if (kind === "builtin") {
        await withDeadline(
          window.waitForRecoveryRequest(kind, 2),
          "no replacement built-in style request after real WebGL restore",
          10_000,
        );
        // Release the canceled response only after its replacement exists.
        await window.releaseRecoveryRequest(kind, 0);
        // A late canceled response must not retire the replacement's guard.
        // Observe a full retry interval while that replacement is still pending.
        await new Promise((resolve) => setTimeout(resolve, 8500));
        check(
          (await window.recoveryRequestCount(kind)) === 2 &&
            !map.getLayer("stale-basemap"),
          "late canceled response neither loads nor restarts the pending replacement",
        );
        const loaded = map.once("style.load");
        await window.releaseRecoveryRequest(kind, 1);
        await withDeadline(loaded, "replacement style did not load");
        await withDeadline(
          waitForIdle(map),
          "replacement style did not render",
        );
        check(
          map.getLayer("restored-remote-basemap") &&
            !map.getLayer("stale-basemap"),
          "replacement renders, canceled response cannot overwrite it",
        );
      } else {
        await window.releaseRecoveryRequest(kind, 0);
        // Negative observation spans the unchanged production 8s retry interval.
        await new Promise((resolve) => setTimeout(resolve, 8500));
        check(
          (await window.recoveryRequestCount(kind)) === 1 &&
            map.getLayer("offline-basemap"),
          "failed custom URL does not auto-retry after restoration",
        );
      }
      check(
        showsSelection() &&
          window.getCurrentMapView() === panned &&
          fits === 1 &&
          signals.length === 1,
        `${kind}: recovery preserves selection and panned view without duplicate fit or ready`,
      );
    } finally {
      dispose();
      window.readyForDisplay = previousReady;
    }
  }
  return "Achievement built-in retry and custom no-retry after real WebGL restore";
}

export async function checkAchievementOverlayPreservesCollidingBasemap(check) {
  const preferred = "memolanes-achievement-region-mask";
  const style = (id) => ({
    version: 8,
    sources: {},
    layers: [
      { id, type: "background", paint: { "background-color": "#304050" } },
    ],
  });
  const { controller, map, dispose } = createExplorerFixture({
    projection: "mercator",
    custom_basemap: true,
    fog_density: "0",
    map_style: style(preferred),
  });
  try {
    await controller.initialize();
    new FlutterBridge(controller).initialize();
    map.jumpTo({ center: [0, 0], zoom: 4 });
    const read = async () =>
      (await readPixelsAt(map, [map.project([-4, 0]), map.project([4, 0])]))
        .map(([r]) => r)
        .join();
    window.setAchievementRegion(feature("A", -6, -2, -2, 2));
    await waitForIdle(map);
    const active = highlight(map);
    check(
      active &&
        active.id !== preferred &&
        map.getLayer(preferred).type === "background" &&
        (await read()) === "48,48",
      "a basemap matching the preferred name remains intact and the selection has its own mask",
    );
    const incoming = style(active.id);
    const originalStyle = map.style;
    controller.applyMapAppearance({
      map_style: incoming,
      custom_basemap: true,
      fog_style: "dark",
      fog_density: 0,
      access_key: null,
    });
    await waitForIdle(map);
    check(
      map.style === originalStyle &&
        map.getLayer(active.id)?.type === "background" &&
        (await read()) === "48,48",
      "live style diff preserves the incoming layer and A, even when the basemap claims A's live ID",
    );
    const restore = await loseContext(map);
    window.setAchievementRegion(feature("B", 2, -2, 6, 2));
    await restore();
    await waitForIdle(map);
    check(
      map.getLayer(active.id)?.type === "background" &&
        !!highlight(map) &&
        (await read()) === "48,48",
      "real restoration keeps the incoming basemap and restores the mask",
    );
    window.setAchievementRegion(null);
    await waitForIdle(map);
    check(
      map.getLayersOrder().join() === `${active.id},${JOURNEY_LAYER_ID}` &&
        (await read()) === "48,48",
      "clear retains the foreign layer, its original colors, and the journey layer",
    );
    return "Achievement style diff and context recovery preserve a basemap claiming the overlay ID";
  } finally {
    dispose();
  }
}

async function checkSharedMapTapLease(check, map) {
  const previousTap = window.onMapTap;
  const taps = [];
  window.onMapTap = { postMessage: (message) => taps.push(message) };
  try {
    const canvas = map.getCanvas();
    const tap = () => {
      for (const type of ["pointerdown", "pointerup"])
        canvas.dispatchEvent(
          new PointerEvent(type, { pointerId: 1, button: 0 }),
        );
    };
    window.setMapTapEnabled(21, 0, true);
    tap();
    window.setMapTapEnabled(21, 1, false);
    window.setMapTapEnabled(21, 0, true); // Out-of-order Record enable.
    tap(); // Explorer pointer input must not arm a timer.
    window.setMapTapEnabled(21, 2, true);
    await new Promise((resolve) => setTimeout(resolve, 550));
    check(taps.length === 0, "retired and disabled taps never reach Record");
    window.setMapTapEnabled(20, 3, false); // Retired document.
    tap();
    await new Promise((resolve) => setTimeout(resolve, 550));
    check(
      taps.length === 1 &&
        JSON.parse(taps[0]).document === 21 &&
        JSON.parse(taps[0]).lease === 2,
      "Record posts the captured document and tap lease",
    );
  } finally {
    window.onMapTap = previousTap;
  }
}

async function checkSharedMapLiveAppearance(check, map, pixel, darkFog) {
  const builtin = new URL("live-appearance/builtin.json", location.href).href;
  const missing = new URL("live-appearance/custom.json", location.href).href;
  const camera = window.getCurrentMapView();
  let moves = 0;
  const onMove = () => moves++;
  map.on("move", onMove);
  try {
    const apply = (style, custom, fog = "light") =>
      window.applyMapAppearance(21, {
        map_style: style,
        custom_basemap: custom,
        fog_style: fog,
        fog_density: 0.5,
        access_key: null,
      });
    const inline = { version: 8, sources: {}, layers: [] };
    apply(inline, true);
    const lightFog = await withDeadline(
      pixel(0.8),
      "updated fog did not render",
    );
    check(
      lightFog[0] > darkFog[0] && lightFog[3] === darkFog[3],
      `live fog changes the rendered palette without changing density (${darkFog} → ${lightFog})`,
    );
    const remote = new Promise((resolve) => {
      const listener = () => {
        if (!map.getLayer("live-builtin")) return;
        map.off("styledata", listener);
        resolve();
      };
      map.on("styledata", listener);
    });
    apply(builtin, false);
    apply(builtin, false);
    await withDeadline(remote, "live built-in style was not displayed");
    await withDeadline(waitForIdle(map), "live built-in style did not settle");
    const failed = new Promise((resolve) => map.once("error", resolve));
    apply(missing, true);
    apply(missing, true, "dark"); // A fog-only change cannot retry the URL.
    await withDeadline(failed, "live custom style was not requested");
    await withDeadline(waitForIdle(map), "local fallback did not settle");
    check(
      map.getLayer("offline-basemap") && !map.getLayer("live-builtin"),
      "unavailable live custom style falls back to the local globe",
    );
    await new Promise((resolve) => setTimeout(resolve, 8_150));
    check(
      moves === 0 && window.getCurrentMapView() === camera,
      "appearance changes do not move or fit the retained camera",
    );
    apply(inline, true, "dark");
    await withDeadline(
      waitForIdle(map),
      "replacement inline style did not settle",
    );
    check(
      !map.getLayer("offline-basemap"),
      "a further live selection replaces the fallback",
    );
  } finally {
    map.off("move", onMove);
  }
}

/** The retained main renderer, not a basemap-only Explorer substitute. */
export async function checkSharedExplorerHandoff(check) {
  const { encodeTileRangeResponse } =
    await import("../support/tile-range-fixture.js");
  const originalFetch = window.fetch;
  const previousParams = window.EXTERNAL_PARAMS;
  const previousViewChannel = window.onMapViewChanged;
  window.onMapViewChanged = {
    postMessage: (json) => {
      const view = JSON.parse(json);
      if (view.owner === 2)
        check(
          Math.abs(view.lng) < 1e-6 &&
            Math.abs(view.lat) < 1e-6 &&
            view.zoom === 14,
          "home-owner pushes never contain Explorer coordinates during scoped exit",
        );
    },
  };
  const endpoint = new URL("shared-explorer/", location.href).href;
  window.EXTERNAL_PARAMS = {
    cgi_endpoint: endpoint,
    achievement_map_document: 21,
  };
  window.fetch = async (input, options) => {
    const url = new URL(
      input instanceof Request ? input.url : input,
      location.href,
    );
    if (!url.href.startsWith(endpoint)) return originalFetch(input, options);
    const q = url.searchParams;
    const range = ["x", "y", "width", "height", "z"].map((k) =>
      Number(q.get(k)),
    );
    const exponent = Number(q.get("buffer_size_power"));
    const grid = 2 ** (range[4] + exponent);
    const cells = [];
    for (let y = -3; y <= 3; y++)
      for (let x = -3; x <= 3; x++)
        cells.push([Math.floor(grid / 2) + x, Math.floor(grid / 2) + y]);
    return new Response(encodeTileRangeResponse(range, cells, exponent), {
      headers: { "X-Tile-Version": "1" },
    });
  };
  const { controller, map, dispose } = createMapControllerFixture({
    cgi_endpoint: endpoint,
    lng: "0",
    lat: "0",
    zoom: "14",
    custom_basemap: true,
    map_style: { version: 8, sources: {}, layers: [] },
  });
  const home = { lng: 0, lat: 0, zoom: 14 };
  try {
    await controller.initialize();
    new FlutterBridge(controller).initialize();
    await controller.waitForDisplay();
    const pixel = async (x) => {
      let result;
      map.once("render", () => {
        const gl = map.getCanvas().getContext("webgl2");
        result = new Uint8Array(4);
        gl.readPixels(
          Math.floor(gl.drawingBufferWidth * x),
          Math.floor(gl.drawingBufferHeight / 2),
          1,
          1,
          gl.RGBA,
          gl.UNSIGNED_BYTE,
          result,
        );
      });
      await waitForRender(map);
      return result;
    };
    const journey = await pixel(0.5),
      fog = await pixel(0.8);
    check(
      controller.getTileProvider() !== null && journey[3] < fog[3],
      `real journey tile clears shading on retained globe (${journey}, ${fog})`,
    );
    await checkSharedMapLiveAppearance(check, map, pixel, fog);
    await checkSharedMapTapLease(check, map);
    const request = (generation, owner, selection, view = null) =>
      window.applyAchievementRegionCommand({
        document: 21,
        generation,
        owner,
        revision: 1,
        featureGeoJson: selection === null ? null : JSON.stringify(selection),
        bounds: null,
        padding: { top: 24, right: 24, bottom: 24, left: 24 },
        view,
        restore: selection === null && view !== null,
      });
    const selection = feature("shared", -0.003, -0.003, 0.006, 0.003);
    const ack = request(1, 1, selection, home);
    check(
      ack.owner === 1 && ack.document === 21 && ack.generation === 1,
      "acknowledgment identifies document and owner",
    );
    await waitForIdle(map);
    check(!!highlight(map), "selection layer is installed on the shared globe");
    const layers = map.getLayersOrder();
    check(
      layers.at(-1) === "memolanes-journey-layer" &&
        layers.at(-2) === highlight(map).id,
      "the journey renderer sits above the selection",
    );
    const insideFog = await pixel(0.8);
    check(
      Math.abs(insideFog[3] - 82) <= 1,
      `selection eases existing fog inside the region (${fog} → ${insideFog})`,
    );
    check(
      (await pixel(0.5)).join() === journey.join(),
      "fully explored pixels stay unchanged",
    );
    check(
      (await pixel(0.1)).join() === fog.join(),
      "fog outside the selected region is unchanged",
    );
    const restore = await loseContext(map);
    await restore();
    await waitForIdle(map);
    check(!!highlight(map), "real context restore retains shared selection");
    map.jumpTo({ center: [0.02, 0], zoom: 14 });
    window.flyToView({ lng: 0.03, lat: 0, zoom: 14 });
    request(2, 2, null, home);
    const returned = JSON.parse(window.getCurrentMapView());
    check(
      Math.abs(returned.lng - home.lng) < 1e-6 &&
        Math.abs(returned.lat - home.lat) < 1e-6 &&
        Math.abs(returned.zoom - home.zoom) < 1e-6,
      "scoped exit atomically restores home before acknowledgment",
    );
    check(
      !map.isMoving(),
      "scoped exit cancels active Explorer animation without animating the home restore",
    );
    await waitForIdle(map);
    check(!highlight(map), "exit clears only Explorer drawing");
    const cleared = await pixel(0.5);
    check(cleared[3] === journey[3], "journey remains visible after clear");
    check(
      (await pixel(0.8)).join() === fog.join(),
      "clearing restores ordinary fog",
    );
    request(3, 3, selection, { lng: 0.001, lat: 0, zoom: 14 });
    await waitForIdle(map);
    check(
      Math.abs(map.getCenter().lng - 0.001) < 1e-6,
      "new owner fits repeated revision",
    );
    check(
      request(4, 1, selection, home) === false,
      "retired owner cannot reclaim camera",
    );
    check(
      controller.getMap() === map,
      "handoff and recovery retain the same globe",
    );
    return "Shared Explorer real journey, live basemap and fog, tap leases, ordered overlay, scoped return and context recovery";
  } finally {
    dispose();
    window.fetch = originalFetch;
    window.EXTERNAL_PARAMS = previousParams;
    window.onMapViewChanged = previousViewChannel;
  }
}

export async function checkSharedExplorerColdLocalReady(check) {
  const previousParams = window.EXTERNAL_PARAMS;
  const originalFetch = window.fetch;
  const previousCredits = window.onMapAttributionChanged;
  const credits = [];
  window.onMapAttributionChanged = {
    postMessage: (value) => credits.push(value),
  };
  const endpoint = new URL("held-shared-tiles/", location.href).href;
  window.EXTERNAL_PARAMS = {
    cgi_endpoint: endpoint,
    achievement_map_document: 22,
  };
  let release;
  const held = new Promise((resolve) => {
    release = resolve;
  });
  window.fetch = async (input, options) => {
    const url = new URL(
      input instanceof Request ? input.url : input,
      location.href,
    );
    if (!url.href.startsWith(endpoint)) return originalFetch(input, options);
    await held;
    return new Response("offline", { status: 503 });
  };
  const { controller, map, dispose } = createMapControllerFixture(
    {
      cgi_endpoint: endpoint,
      zoom: "4",
      map_style: new URL("slow-style/builtin.json", location.href).href,
    },
    { disableAutoRefresh: false },
  );
  let initialized = false;
  let local;
  const ready = new Promise((resolve) => {
    local = resolve;
  });
  const initialization = controller
    .initialize(() => {
      new FlutterBridge(controller).initialize();
      controller.waitForLocalDisplay().then(local);
    })
    .then(() => {
      initialized = true;
    });
  try {
    await withDeadline(
      ready,
      "shared cold local frame waited for remote data",
      2500,
    );
    check(
      !initialized && !!map.getLayer("offline-basemap"),
      "shared local readiness precedes held journey tiles and remote style",
    );
    const ack = window.applyAchievementRegionCommand({
      document: 22,
      owner: 1,
      generation: 1,
      revision: 1,
      featureGeoJson: JSON.stringify(feature("local-shared", -4, 34, 4, 42)),
      bounds: { west: -4, south: 34, east: 4, north: 42 },
      padding: { top: 24, right: 24, bottom: 24, left: 24 },
      view: null,
    });
    check(
      ack.owner === 1,
      "offline selection acknowledged on cold shared document",
    );
    await withDeadline(map.once("moveend"), "offline selection failed to fit");
    await waitForRender(map);
    check(
      !initialized &&
        !!highlight(map) &&
        Math.abs(map.getCenter().lat - 38) < 1,
      "selection draws and fits while the style and journey tiles are held",
    );
    map.jumpTo({ center: [0.5, 38.5], zoom: map.getZoom() + 0.3 });
    const panned = window.getCurrentMapView();
    await withDeadline(
      map.once("style.load"),
      "slow built-in style never loaded",
      12_000,
    );
    await withDeadline(waitForRender(map), "late basemap did not render");
    const layers = map.getLayersOrder();
    check(
      layers.includes("remote-basemap") &&
        !!highlight(map) &&
        layers.indexOf(highlight(map).id) < layers.indexOf(JOURNEY_LAYER_ID) &&
        window.getCurrentMapView() === panned,
      "late basemap preserves the outline below the journey layer and does not refit",
    );
    check(
      credits.includes("true") &&
        window.getMapAttribution() === "Local Provider" &&
        !initialized,
      "the loaded style exposes credits before full journey readiness",
    );
    return "Shared cold readiness, offline selection, and late style recovery with held journey tiles";
  } finally {
    release();
    await initialization;
    dispose();
    window.fetch = originalFetch;
    window.EXTERNAL_PARAMS = previousParams;
    window.onMapAttributionChanged = previousCredits;
  }
}
