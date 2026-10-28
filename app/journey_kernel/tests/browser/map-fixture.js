/** Small shared pieces for tests that need a real MapLibre camera. */
export function createMapContainer(width = 256, height = 256) {
  const element = document.createElement("div");
  element.id = "rendering-regression";
  element.style.cssText = `width:${width}px;height:${height}px;position:fixed;left:0;top:0`;
  document.body.append(element);
  return element;
}

export function waitForIdle(map) {
  return new Promise((resolve) => {
    map.once("idle", resolve);
    map.triggerRepaint();
  });
}
