import type { Map } from "maplibre-gl";
import type { TileRange } from "./layers/tile-range";
import {
  polarUV,
  polarLngLat,
  POLAR_SEAM_LATITUDE,
  type PolarHemisphere,
} from "./polar-projection";

export const POLAR_CANVAS_TILE_EXPONENT = 9;
const POLAR_NATIVE_EXPONENT = 20;
const MAX_CANVAS_SIDE = 2048;

function createCapProbes(hemisphere: PolarHemisphere) {
  const probes: { uv: [number, number]; ll: [number, number] }[] = [];
  for (const radius of [0, 0.5, 1]) {
    // Radius zero is one point, regardless of its meridian.
    for (let i = 0; i < (radius === 0 ? 1 : 32); i++) {
      const uv: [number, number] = [
        0.5 + 0.5 * radius * Math.sin((i * Math.PI) / 16),
        0.5 + 0.5 * radius * Math.cos((i * Math.PI) / 16),
      ];
      probes.push({ uv, ll: polarLngLat(...uv, hemisphere) });
    }
  }
  return probes;
}

const capProbes = {
  north: createCapProbes("north"),
  south: createCapProbes("south"),
};

interface PolarRangeCache {
  key: string | null;
  points: [number, number][];
  views: Partial<Record<PolarHemisphere, PolarCanvasView>>;
}

const rangeCache = new WeakMap<Map, PolarRangeCache>();

export interface PolarCanvasView {
  readonly range: TileRange;
  readonly visible: boolean;
}

/** Screen-space samples choose chart coverage; a derivative chooses native LOD.
 * Invisible caps retain an overview for prefetch, without blocking display. */
export function getPolarCanvasView(
  map: Map,
  hemisphere: PolarHemisphere,
): PolarCanvasView {
  const canvas = map.getCanvas();
  const width = canvas.clientWidth || canvas.width;
  const height = canvas.clientHeight || canvas.height;
  const center = map.getCenter?.();
  const padding = map.getPadding?.();
  const key = center
    ? [
        center.lng,
        center.lat,
        map.getZoom(),
        map.getPitch(),
        map.getBearing(),
        map.getRoll?.() ?? 0,
        map.getVerticalFieldOfView?.(),
        map.getCenterElevation?.(),
        width,
        height,
        padding?.top ?? 0,
        padding?.right ?? 0,
        padding?.bottom ?? 0,
        padding?.left ?? 0,
        map.getProjection?.()?.type,
        // A configured globe can still be rendering its Mercator fallback.
        map.style?.projection?.transitionState,
      ].join("/")
    : null;
  const cached = key === null ? undefined : rangeCache.get(map);
  const entry: PolarRangeCache =
    cached && cached.key === key ? cached : { key, points: [], views: {} };
  const view = entry.views[hemisphere];
  if (view) return view;
  const remember = (range: TileRange, visible: boolean): PolarCanvasView => {
    const view = { range, visible };
    if (key !== null) {
      entry.views[hemisphere] = view;
      rangeCache.set(map, entry);
    }
    return view;
  };
  const sign = hemisphere === "north" ? 1 : -1;
  const samples: [number, number][] = [];
  const add = (lng: number, lat: number) => {
    if (
      sign * lat >= POLAR_SEAM_LATITUDE &&
      Number.isFinite(lng) &&
      Number.isFinite(lat)
    )
      samples.push(polarUV(lng, lat));
  };
  // Both hemispheres use the same inverse viewport grid for this camera key.
  if (!entry.points.length) {
    for (let y = 0; y <= 12; y++) {
      for (let x = 0; x <= 12; x++) {
        const point = map.unproject([(x * width) / 12, (y * height) / 12]);
        entry.points.push([point.lng, point.lat]);
      }
    }
  }
  for (const [lng, lat] of entry.points) add(lng, lat);
  // Include a cap wholly contained between the inverse-projection samples.
  for (const { uv, ll } of capProbes[hemisphere]) {
    const p = map.project(ll);
    if (!Number.isFinite(p.x) || !Number.isFinite(p.y)) continue;
    if (p.x < 0 || p.x > width || p.y < 0 || p.y > height) continue;
    const visible = map.unproject(p);
    // project() also returns points behind the globe; reject that hemisphere.
    if (Math.abs(visible.lat - ll[1]) < 0.01) samples.push(uv);
  }
  if (!samples.length) return remember([0, 0, 1, 1, 0], false);
  let density = 0;
  const delta = 1e-5;
  for (const [u, v] of samples) {
    const p = map.project(polarLngLat(u, v, hemisphere));
    if (!Number.isFinite(p.x) || !Number.isFinite(p.y)) continue;
    for (const [du, dv] of [
      [delta, 0],
      [0, delta],
    ]) {
      const q = map.project(polarLngLat(u + du, v + dv, hemisphere));
      if (!Number.isFinite(q.x) || !Number.isFinite(q.y)) continue;
      density = Math.max(density, Math.hypot(q.x - p.x, q.y - p.y) / delta);
    }
  }
  // Keep source cells at most one CSS pixel wide, down to native detail.
  let z = Math.max(
    0,
    Math.min(
      POLAR_NATIVE_EXPONENT - POLAR_CANVAS_TILE_EXPONENT,
      Math.ceil(
        Math.log2(Math.max(1, density) / (1 << POLAR_CANVAS_TILE_EXPONENT)),
      ),
    ),
  );
  const minU = Math.max(0, Math.min(...samples.map((p) => p[0])));
  const maxU = Math.min(1, Math.max(...samples.map((p) => p[0])));
  const minV = Math.max(0, Math.min(...samples.map((p) => p[1])));
  const maxV = Math.min(1, Math.max(...samples.map((p) => p[1])));
  for (;;) {
    const n = 2 ** z;
    const x = Math.max(0, Math.floor(minU * n) - 1);
    const y = Math.max(0, Math.floor(minV * n) - 1);
    const w = Math.min(n, Math.floor(maxU * n) + 2) - x;
    const h = Math.min(n, Math.floor(maxV * n) + 2) - y;
    const tileSide = 2 ** POLAR_CANVAS_TILE_EXPONENT;
    // At most 16 MiB of RGBA pixels per hemisphere.
    if (
      (w * tileSide <= MAX_CANVAS_SIDE && h * tileSide <= MAX_CANVAS_SIDE) ||
      z === 0
    )
      return remember([x, y, w, h, z], true);
    z--;
  }
}
