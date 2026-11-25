import { MapController } from "../../static/map-controller";
import { createReactiveParams } from "../../static/params";

/** Small shared pieces for tests that need a real MapLibre camera. */
export function createMapContainer(width = 256, height = 256) {
  const element = document.createElement("div");
  element.id = "rendering-regression";
  element.style.cssText = `width:${width}px;height:${height}px;position:fixed;left:0;top:0`;
  document.body.append(element);
  return element;
}

export function createMapControllerFixture(
  params,
  { height = 240, id, disableAutoRefresh = true } = {},
) {
  const container = createMapContainer(360, height);
  if (id) container.id = id;
  try {
    const controller = new MapController({
      containerId: container.id,
      params: createReactiveParams({ cgi_endpoint: ".", ...params }),
      DisableAutoRefresh: disableAutoRefresh,
    });
    return {
      container,
      controller,
      map: controller.getMap(),
      dispose() {
        try {
          controller.destroy();
        } finally {
          container.remove();
        }
      },
    };
  } catch (error) {
    container.remove();
    throw error;
  }
}

export function waitForIdle(map) {
  return new Promise((resolve) => {
    map.once("idle", resolve);
    map.triggerRepaint();
  });
}

export function waitForRender(map) {
  return new Promise((resolve) => {
    map.once("render", resolve);
    map.triggerRepaint();
  });
}
