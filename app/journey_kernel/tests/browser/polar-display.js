import { MapController } from "../../static/map-controller";
import { createReactiveParams } from "../../static/params";
import { POLAR_SEAM_LATITUDE } from "../../static/polar-projection";
import { createMapContainer, waitForIdle } from "./map-fixture";
import { encodeTileRangeResponse } from "../support/tile-range-fixture.js";

function deferred() {
  let resolve;
  const promise = new Promise((done) => (resolve = done));
  return { promise, resolve };
}

/** Run the production initialization with independently delayed tile planes. */
async function withMap(latitude, check, run) {
  const originalFetch = window.fetch;
  const previousParams = window.EXTERNAL_PARAMS;
  const endpoint = new URL("display-test/", location.href).href;
  window.EXTERNAL_PARAMS = { cgi_endpoint: endpoint };
  const requests = [];
  const holds = new Map();
  const hold = (plane) => {
    const started = deferred(),
      response = deferred();
    const gate = { started: started.promise, release: response.resolve };
    holds.set(plane, { started, response });
    return gate;
  };
  window.fetch = async (input, options) => {
    const url = new URL(
      input instanceof Request ? input.url : input,
      location.href,
    );
    if (!url.href.startsWith(endpoint)) return originalFetch(input, options);
    const q = url.searchParams;
    const plane = q.get("plane") ?? "mercator";
    const range = ["x", "y", "width", "height", "z"].map((key) =>
      Number(q.get(key)),
    );
    requests.push({ plane, range });
    // Match the real Rust endpoint; invalid requests must not produce a buffer.
    if (range[2] > 20 || range[3] > 20)
      return new Response("Dimensions too large", { status: 500 });
    const gate = holds.get(plane);
    if (gate) {
      gate.started.resolve();
      await gate.response.promise;
    }
    return new Response(
      encodeTileRangeResponse(range, [], Number(q.get("buffer_size_power"))),
      {
        headers: { "X-Tile-Version": "1" },
      },
    );
  };
  const container = createMapContainer(390, 560);
  const style =
    "data:application/json," +
    encodeURIComponent(
      JSON.stringify({
        version: 8,
        sources: {},
        layers: [
          {
            id: "base",
            type: "background",
            paint: { "background-color": "white" },
          },
        ],
      }),
    );
  const controller = new MapController({
    containerId: container.id,
    params: createReactiveParams({
      cgi_endpoint: endpoint,
      map_style: style,
      projection: "globe",
      lat: String(latitude),
      lng: "0",
      zoom: "14",
    }),
    DisableAutoRefresh: true,
  });
  try {
    await run({ controller, map: controller.getMap(), requests, hold });
    check(
      requests.every(({ range }) => range[2] <= 20 && range[3] <= 20),
      "every production tile request fits the Rust endpoint",
    );
  } finally {
    for (const gate of holds.values()) gate.response.resolve();
    controller.destroy();
    container.remove();
    window.fetch = originalFetch;
    window.EXTERNAL_PARAMS = previousParams;
  }
}

function watchDisplay(controller) {
  const promise = controller.waitForDisplay();
  let ready = false;
  promise.then(
    () => {
      ready = true;
    },
    () => {},
  );
  return { promise, isReady: () => ready };
}

export async function checkPolarDisplay(check) {
  const results = [];
  for (const sign of [1, -1]) {
    const hemisphere = sign > 0 ? "north" : "south";
    const opposite = sign > 0 ? "south" : "north";
    await withMap(
      sign * 90,
      check,
      async ({ controller, map, requests, hold }) => {
        const visible = hold(hemisphere),
          invisible = hold(opposite);
        await controller.initialize();
        await visible.started;
        const display = watchDisplay(controller);
        await waitForIdle(map);
        check(!display.isReady(), "first display waits for the visible cap");
        check(
          !requests.some(({ plane }) => plane === "mercator"),
          "a polar-only viewport sends no Mercator request",
        );
        visible.release();
        await display.promise;
        // The hidden hemisphere is still downloading when first display completes.
        invisible.release();
        await controller.refreshMapData();

        const refreshGate = hold(hemisphere);
        let refreshed = false;
        const refresh = controller.refreshMapData().then(() => {
          refreshed = true;
        });
        await refreshGate.started;
        await waitForIdle(map);
        check(!refreshed, "an explicit refresh waits for the polar download");
        refreshGate.release();
        await refresh;

        // Leaving and re-entering a polar-only view reactivates Mercator demand.
        controller.jumpTo({ center: [0, sign * 70], zoom: 14 });
        await controller.getTileProvider().waitForTileBufferUpdate();
        await controller.waitForDisplay();
        check(
          controller.getTileProvider().getViewRange() !== null,
          "ordinary navigation restores Mercator demand",
        );
        controller.jumpTo({ center: [0, sign * 89], zoom: 14 });
        await controller.waitForDisplay();
        check(
          controller.getTileProvider().getViewRange() === null,
          "returning to the cap clears Mercator demand",
        );
        controller.getParams().projection = "mercator";
        await waitForIdle(map);
        check(
          controller.getTileProvider().getViewRange() !== null,
          "switching to a flat map restores Mercator demand after projection migration",
        );
        await controller.waitForDisplay();
      },
    );
    results.push(
      `${hemisphere} production initialization, visible-plane readiness and refresh`,
    );

    await withMap(
      sign * POLAR_SEAM_LATITUDE,
      check,
      async ({ controller, map, hold }) => {
        const mercator = hold("mercator"),
          polar = hold(hemisphere);
        const initialized = controller.initialize();
        await Promise.all([mercator.started, polar.started]);
        const display = watchDisplay(controller);
        polar.release();
        await waitForIdle(map);
        check(
          !display.isReady(),
          "a seam viewport also waits for Mercator data",
        );
        mercator.release();
        await initialized;
        await display.promise;
      },
    );
    results.push(`${hemisphere} seam display waits for both visible planes`);
  }
  return results;
}
