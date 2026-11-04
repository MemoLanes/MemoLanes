import type { Map as MaplibreMap } from "maplibre-gl";
import { getScreenResolvedSourceTileZoom } from "./source-tile-zoom";
import { getBoundedViewportPrefetchRange, type TileRange } from "./tile-range";
import { getViewportTileRangeAtZoom } from "./utils";
import { getMapSamplingZoom } from "./map-resolution";

// Shared with the Rust tile_range endpoint's per-side request limit.
const MAX_TILE_RANGE_SIDE = 20;

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
): TileRange | null {
  const mapZoom = getMapSamplingZoom(map);
  let sourceZoom =
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
  let visibleRange = getViewportTileRangeAtZoom(
    map,
    isGlobeProjection,
    sourceZoom,
  );
  // A globe can expose every meridian at once. Reduce LOD until the complete
  // visible Mercator range fits the endpoint, rather than cropping coverage.
  while (visibleRange && !fitsTileRequest(visibleRange) && sourceZoom > 0) {
    visibleRange = getViewportTileRangeAtZoom(
      map,
      isGlobeProjection,
      --sourceZoom,
    );
  }
  if (!visibleRange) return null;
  const prefetch = policy.prefetch;
  const requestedRange = prefetch
    ? getBoundedViewportPrefetchRange(
        visibleRange,
        prefetch.padding,
        prefetch.maxTileCount,
        prefetch.maxExtraTileCount,
      )
    : visibleRange;
  return fitsTileRequest(requestedRange) ? requestedRange : visibleRange;
}

function fitsTileRequest(range: TileRange): boolean {
  return Math.max(range[2], range[3]) <= MAX_TILE_RANGE_SIDE;
}
