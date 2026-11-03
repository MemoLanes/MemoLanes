import { MapController } from "../../static/map-controller";
import { createReactiveParams } from "../../static/params";
import {
  createMapContainer as container,
  waitForIdle as idle,
  waitForRender,
} from "./map-fixture";
import { MAX_MAP_ZOOM } from "../../static/layer-config";

async function checkCamera(sign, check) {
  const element = container();
  const params = createReactiveParams({
    cgi_endpoint: ".",
    lat: String(sign * 70),
    lng: "0",
    zoom: "14",
    projection: "globe",
  });
  const controller = new MapController({
    containerId: element.id,
    params,
    DisableAutoRefresh: true,
  });
  const map = controller.getMap();
  try {
    await new Promise((resolve) => map.once("load", resolve));
    for (const method of ["jumpTo", "easeTo", "flyTo"]) {
      map.jumpTo({ center: [0, sign * 70], zoom: 14 });
      await idle(map);
      controller[method]({
        center: [0, sign * 89],
        zoom: 14,
        duration: 80,
        essential: true,
      });
      await idle(map);
      check(
        Math.abs(map.getCenter().lat - sign * 89) < 1e-6,
        `${method} must retain the polar destination from an ordinary latitude, got ${map.getCenter().lat}`,
      );
      check(
        map.getProjection().type === "vertical-perspective",
        "a polar destination locks the sphere",
      );
    }
    for (const [latitude, projection] of [
      [78, "vertical-perspective"],
      [74, "globe"],
      [81, "vertical-perspective"],
    ]) {
      map.jumpTo({ center: [0, sign * latitude], zoom: 14 });
      await idle(map);
      check(
        map.getProjection().type === projection,
        "the production camera preserves projection hysteresis",
      );
    }
    controller.jumpTo({ center: [0, sign * 70], zoom: 14 });
    await idle(map);
    controller.fitBounds(
      [
        [-1, sign * 88.99],
        [1, sign * 89.01],
      ],
      {
        padding: 16,
        maxZoom: MAX_MAP_ZOOM,
        duration: 80,
        essential: true,
      },
    );
    await idle(map);
    check(
      Math.abs(map.getCenter().lat - sign * 89) < 0.01,
      `journey bounds retain their polar center, got ${map.getCenter().lat}, zoom ${map.getZoom()}`,
    );
    for (const latitude of [88.99, 89.01])
      for (const longitude of [-1, 1]) {
        const p = map.project([longitude, sign * latitude]);
        check(
          p.x >= 14 && p.x <= 242 && p.y >= 14 && p.y <= 242,
          "polar journey corners fit inside the padded viewport",
        );
      }
    for (const [south, north, west, east] of [
      [88.99, 89.01, -1, 1],
      [85.1, 90, -180, 180],
    ]) {
      controller.fitBounds(
        [
          [west, sign * south],
          [east, sign * north],
        ],
        {
          padding: { top: 16, bottom: 80, left: 16, right: 16 },
          maxZoom: MAX_MAP_ZOOM,
          duration: 0,
          essential: true,
        },
      );
      await idle(map);
      for (let i = 0; i <= 12; i++)
        for (let j = 0; j <= 4; j++) {
          const p = map.project([
            west + ((east - west) * i) / 12,
            sign * (south + ((north - south) * j) / 4),
          ]);
          check(
            p.x >= 14 && p.x <= 242 && p.y >= 14 && p.y <= 178,
            `polar journey ${south}/${north} fits above the detail card, got ${p.x},${p.y}; camera ${map.getCenter().toArray()} z${map.getZoom()}`,
          );
        }
    }
    controller.flyTo({
      center: [0, sign * 70],
      zoom: 14,
      duration: 0,
      essential: true,
    });
    await idle(map);
    check(
      map.getPadding().bottom === 0 && map.getPadding().top === 0,
      "leaving journey focus restores the original viewport padding",
    );

    // These real raster requests are held by the Playwright runner. Camera
    // events must keep working while source tiles prevent the map going idle.
    map.addSource("slow-basemap", {
      type: "raster",
      tiles: [`${new URL("slow-basemap/", location.href).href}{z}/{x}/{y}.png`],
      tileSize: 256,
    });
    map.addLayer({
      id: "slow-basemap",
      type: "raster",
      source: "slow-basemap",
    });
    map.jumpTo({ center: [0, sign * 79], zoom: 14 });
    await waitForRender(map);
    check(
      !map.isStyleLoaded(),
      "basemap tiles are pending before crossing 80 degrees",
    );
    for (const [latitude, projection] of [
      [81, "vertical-perspective"],
      [89, "vertical-perspective"],
      [78, "vertical-perspective"],
      [74, "globe"],
    ]) {
      // Raw MapLibre camera updates exercise the move listener used by
      // gestures, without the programmatic navigation wrapper preparing it.
      map.jumpTo({ center: [0, sign * latitude], zoom: 14 });
      await waitForRender(map);
      check(
        map.getProjection().type === projection &&
          Math.abs(map.getCenter().lat - sign * latitude) < 1e-6,
        `pending basemap tiles must preserve ${sign * latitude} degrees and projection hysteresis, got ${map.getCenter().lat} with ${map.getProjection().type}`,
      );
    }
  } finally {
    controller.destroy();
    element.remove();
  }
  const initialElement = container();
  const initial = new MapController({
    containerId: initialElement.id,
    params: createReactiveParams({
      cgi_endpoint: ".",
      projection: "globe",
      lat: "0",
      lng: "0",
      west: -1,
      east: 1,
      south: sign * 88.99,
      north: sign * 89.01,
      fit_padding_top: 16,
      fit_padding_bottom: 80,
      fit_padding_left: 16,
      fit_padding_right: 16,
    }),
    DisableAutoRefresh: true,
  });
  try {
    await new Promise((resolve) => initial.getMap().once("load", resolve));
    await idle(initial.getMap());
    check(
      Math.abs(initial.getMap().getCenter().lat - sign * 89) < 0.01,
      "initial polar journey bounds use the same spherical fitting path",
    );
  } finally {
    initial.destroy();
    initialElement.remove();
  }
}

export async function checkPolarNavigation(check) {
  const results = [];
  for (const sign of [1, -1]) {
    await checkCamera(sign, check);
    results.push(
      `${sign > 0 ? "north" : "south"} camera navigation, slow basemap, projection hysteresis and journey bounds`,
    );
  }
  return results;
}
