/**
 * Tile range covering a rectangular area: [x, y, width, height, zoom].
 *
 * `x` is not normalized into a single world copy: a range that crosses the
 * antimeridian stays contiguous and may fall outside [0, 2^zoom). `y` is
 * always clamped to the world.
 */
export type TileRange = readonly [
  x: number,
  y: number,
  width: number,
  height: number,
  zoom: number,
];

/** A tile range rescaled to a zoom level it shares with another range. */
interface TileRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

// Once the requested range is lined up with the coverage, only the copy that
// starts just before it and the one just after it can still overlap.
const WORLD_COPY_SHIFTS = [-1, 0, 1];

/** Grows `range` by `padding` tiles on every side, clamped to the world. */
export function padTileRange(range: TileRange, padding: number): TileRange {
  const [rangeX, rangeY, width, height, zoom] = range;
  const tiles = Math.max(0, Math.floor(padding));
  const tileCount = Math.pow(2, zoom);
  const y = Math.max(0, rangeY - tiles);
  return [
    rangeX - tiles,
    y,
    width + tiles * 2,
    Math.min(tileCount, rangeY + height + tiles) - y,
    zoom,
  ];
}

/**
 * Add a small data guard band without allowing viewport requests to grow
 * without bound. The visible range is always returned unchanged when the
 * padded candidate exceeds either resource limit.
 */
export function getBoundedViewportPrefetchRange(
  visibleRange: TileRange,
  padding: number,
  maxTileCount: number,
  maxExtraTileCount: number,
): TileRange {
  const safePadding = Math.max(0, Math.floor(padding));
  if (safePadding === 0) {
    return visibleRange;
  }

  const [x, , width, height, zoom] = visibleRange;
  let candidate = padTileRange(visibleRange, safePadding);

  // A TileBuffer can serve modular-equivalent world copies. Do not transfer
  // duplicate copies merely to create horizontal padding at very low zoom.
  const worldTileCount = Math.pow(2, zoom);
  if (width <= worldTileCount && candidate[2] > worldTileCount) {
    const candidateWidth = worldTileCount;
    const addedWidth = candidateWidth - width;
    const leftPadding = Math.min(safePadding, Math.floor(addedWidth / 2));
    candidate = [
      x - leftPadding,
      candidate[1],
      candidateWidth,
      candidate[3],
      zoom,
    ];
  }

  const visibleTileCount = width * height;
  const candidateTileCount = candidate[2] * candidate[3];
  const extraTileCount = candidateTileCount - visibleTileCount;
  if (
    candidateTileCount <= visibleTileCount ||
    candidateTileCount > Math.max(visibleTileCount, maxTileCount) ||
    extraTileCount > Math.max(0, maxExtraTileCount)
  ) {
    return visibleRange;
  }

  return candidate;
}

/** Whether every tile in `requested` is available in `coverage`. */
export function tileRangeContains(
  coverage: TileRange,
  requested: TileRange,
): boolean {
  const overlap = tileRangeIntersect(coverage, requested);
  if (!overlap) {
    return false;
  }
  // The overlap is always a subset of `requested`, so matching sizes at a
  // shared zoom level means nothing was clipped away.
  const scale = Math.pow(2, overlap[4] - requested[4]);
  return (
    overlap[2] === requested[2] * scale && overlap[3] === requested[3] * scale
  );
}

/** Tiles present in both ranges, or `null` if they do not overlap. */
export function tileRangeIntersect(
  coverage: TileRange,
  requested: TileRange,
): TileRange | null {
  const aligned = alignTileRanges(coverage, requested);
  if (!aligned) {
    return null;
  }
  const { zoom, coverage: covered, requested: needed } = aligned;

  const y = Math.max(covered.y, needed.y);
  const yEnd = Math.min(covered.y + covered.height, needed.y + needed.height);
  if (yEnd <= y) {
    return null;
  }

  // Either range may be expressed in any world copy, so move the requested one
  // next to the coverage first: `alignedX` is its leftmost placement that does
  // not start before the coverage.
  const worldWidth = Math.pow(2, zoom);
  const alignedX =
    needed.x + Math.ceil((covered.x - needed.x) / worldWidth) * worldWidth;
  let x = 0;
  let width = 0;
  for (const shift of WORLD_COPY_SHIFTS) {
    const copyX = alignedX + shift * worldWidth;
    const overlapX = Math.max(covered.x, copyX);
    const overlapEnd = Math.min(
      covered.x + covered.width,
      copyX + needed.width,
    );
    if (overlapEnd - overlapX > width) {
      x = overlapX;
      width = overlapEnd - overlapX;
    }
  }
  if (width <= 0) {
    return null;
  }

  return [x, y, width, yEnd - y, zoom];
}

/** Rescales both ranges to their deeper zoom level so they can be compared. */
function alignTileRanges(
  coverage: TileRange,
  requested: TileRange,
): {
  zoom: number;
  coverage: TileRect;
  requested: TileRect;
} | null {
  const [, , coverageWidth, coverageHeight, coverageZoom] = coverage;
  const [, , requestedWidth, requestedHeight, requestedZoom] = requested;
  if (
    coverageWidth <= 0 ||
    coverageHeight <= 0 ||
    requestedWidth <= 0 ||
    requestedHeight <= 0
  ) {
    return null;
  }

  const zoom = Math.max(coverageZoom, requestedZoom);
  return {
    zoom,
    coverage: scaleTileRange(coverage, zoom),
    requested: scaleTileRange(requested, zoom),
  };
}

function scaleTileRange(range: TileRange, zoom: number): TileRect {
  const [x, y, width, height, rangeZoom] = range;
  const scale = Math.pow(2, zoom - rangeZoom);
  return {
    x: x * scale,
    y: y * scale,
    width: width * scale,
    height: height * scale,
  };
}
