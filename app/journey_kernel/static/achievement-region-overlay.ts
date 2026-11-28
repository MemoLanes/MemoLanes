import type { Feature, MultiPolygon, Polygon } from "geojson";
import type { Map as MapLibreMap, StyleSpecification } from "maplibre-gl";
import { AchievementRegionMask } from "./layers/achievement-region-mask";
import { JOURNEY_LAYER_ID } from "./layers/journey-layer-interface";

export type AchievementRegionFeature = Feature<Polygon | MultiPolygon>;

const preferredId = "memolanes-achievement-region-mask";
const reassignOnCollision = new WeakMap<
  MapLibreMap,
  (style: StyleSpecification) => void
>();

export function prepareAchievementRegionStyle(
  map: MapLibreMap,
  nextStyle: StyleSpecification,
): void {
  reassignOnCollision.get(map)?.(nextStyle);
}

/** A drawing-only overlay. Camera changes remain separate Flutter commands. */
export function createAchievementRegionOverlay(map: MapLibreMap) {
  let current: AchievementRegionFeature | null = null;
  let installed: {
    id: string;
    layer: NonNullable<ReturnType<MapLibreMap["getLayer"]>>;
  } | null = null;

  const ownsLiveLayer = () =>
    installed !== null && map.getLayer(installed.id) === installed.layer;

  const retire = () => {
    if (ownsLiveLayer()) map.removeLayer(installed!.id);
    installed = null;
  };

  const sync = (reserved?: ReadonlySet<string>) => {
    const style = map.getStyle();
    if (!style || !current) return;
    const before = map.getLayer(JOURNEY_LAYER_ID)
      ? JOURNEY_LAYER_ID
      : undefined;
    if (installed && ownsLiveLayer()) {
      if (!reserved?.has(installed.id)) {
        const layers = map.getLayersOrder();
        if (
          layers.indexOf(installed.id) !==
          (before ? layers.indexOf(before) : layers.length) - 1
        ) {
          map.moveLayer(installed.id, before);
        }
        return;
      }
      retire();
    }
    let id = preferredId;
    for (let i = 1; reserved?.has(id) || map.getLayer(id); i++) {
      id = `${preferredId}-${i}`;
    }
    map.addLayer(new AchievementRegionMask(id, current), before);
    const layer = map.getLayer(id);
    if (layer) installed = { id, layer };
  };
  const restore = () => sync();

  reassignOnCollision.set(map, (nextStyle) => {
    if (
      installed &&
      nextStyle.layers.some((layer) => layer.id === installed!.id)
    ) {
      // Custom layers aren't serialized for a style diff, so release the incoming
      // name before MapLibre tries to add a basemap layer with that name.
      sync(new Set(nextStyle.layers.map((layer) => layer.id)));
    }
  });
  map.on("style.load", restore);
  map.once("remove", () => {
    map.off("style.load", restore);
    reassignOnCollision.delete(map);
  });

  return (feature: AchievementRegionFeature | null) => {
    retire();
    current = feature;
    if (current) sync();
  };
}
