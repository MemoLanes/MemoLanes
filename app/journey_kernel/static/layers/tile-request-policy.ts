import type { Map as MaplibreMap } from "maplibre-gl";
import { getScreenResolvedSourceTileZoom } from "./source-tile-zoom";
import { getBoundedViewportPrefetchRange, type TileRange } from "./tile-range";
import { getViewportTileRangeAtZoom } from "./utils";

export interface ViewportPrefetchPolicy {
  padding: number;
  maxTileCount: number;
  maxExtraTileCount: number;
}

/** Data-resolution and coverage policy for one rendering layer. */
export interface TileRequestPolicy {
  maxSourceZoom: number;
  /**
   * When set, advance the source zoom before an integer zoom boundary so a
   * native source cell never grows wider than this many CSS pixels.
   */
  maxSourceCellWidthCss?: number;
  prefetch?: ViewportPrefetchPolicy;
}

/** Resolve the complete tile request range from the active layer's policy. */
export function getTileRequestRange(
  map: MaplibreMap,
  isGlobeProjection: boolean,
  bufferSizePower: number,
  policy: TileRequestPolicy,
): TileRange {
  const mapZoom = map.getZoom();
  const sourceZoom =
    policy.maxSourceCellWidthCss === undefined
      ? Math.min(
          Math.max(0, Math.floor(mapZoom)),
          Math.max(0, Math.floor(policy.maxSourceZoom)),
        )
      : getScreenResolvedSourceTileZoom(
          mapZoom,
          bufferSizePower,
          policy.maxSourceZoom,
          policy.maxSourceCellWidthCss,
        );
  const visibleRange = getViewportTileRangeAtZoom(
    map,
    isGlobeProjection,
    sourceZoom,
  );
  const prefetch = policy.prefetch;
  return prefetch
    ? getBoundedViewportPrefetchRange(
        visibleRange,
        prefetch.padding,
        prefetch.maxTileCount,
        prefetch.maxExtraTileCount,
      )
    : visibleRange;
}
