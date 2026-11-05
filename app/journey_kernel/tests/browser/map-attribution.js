import { FlutterBridge } from "../../static/flutter-bridge";
import { MapController } from "../../static/map-controller";
import { createReactiveParams } from "../../static/params";
import { createMapContainer, waitForIdle } from "./map-fixture";

export async function checkMapAttribution(check) {
  const container = createMapContainer();
  const controller = new MapController({
    containerId: container.id,
    params: createReactiveParams({ cgi_endpoint: ".", projection: "mercator" }),
    DisableAutoRefresh: true,
  });
  const map = controller.getMap();
  const changes = [];
  window.onMapAttributionChanged = {
    postMessage: (available) => changes.push(available),
  };
  const bridge = new FlutterBridge(controller);
  bridge.setupFlutterCallableMethods();
  bridge.observeMapAttribution();
  try {
    check(window.getMapAttribution() === null, "placeholder has no credits");
    check(changes.length === 0, "the button starts hidden");
    map.setStyle(
      {
        version: 8,
        sources: {
          provider: {
            type: "geojson",
            data: { type: "FeatureCollection", features: [] },
            attribution:
              '<a href="https://example.com/terms?lang=en&amp;view=full"><b>[Provider]</b></a> &amp; &lt;source&gt; *literal*',
          },
        },
        layers: [{ id: "provider", type: "circle", source: "provider" }],
      },
      { diff: false },
    );
    await waitForIdle(map);
    check(
      window.getMapAttribution() ===
        String.raw`[\[Provider\]](<https://example.com/terms?lang=en&amp;view=full>) \& \<source\> \*literal\*`,
      "the bridge preserves links and escapes literal credit text",
    );
    check(changes.join() === "true", "the button shows when credits arrive");
    map.setStyle({ version: 8, sources: {}, layers: [] }, { diff: false });
    await waitForIdle(map);
    check(
      window.getMapAttribution() === null,
      "a style without sources leaves no credit",
    );
    check(changes.join() === "true,false", "the button hides without credits");
    return "map attribution bridge";
  } finally {
    delete window.onMapAttributionChanged;
    controller.destroy();
    container.remove();
  }
}
