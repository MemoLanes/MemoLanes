import type { Map } from "maplibre-gl";
import type { JourneyTileProvider } from "./journey-tile-provider";
import { MainThreadTileBufferBackend } from "./tile-buffer-backend";
import {
  createCanvasTrackColors,
  drawCanvasTileRange,
} from "./layers/canvas-raster";
import type { RGBAColor } from "./layers/journey-layer-interface";
import { tileRangeContains, type TileRange } from "./layers/tile-range";
import type { PolarHemisphere } from "./polar-projection";
import {
  getPolarCanvasView,
  POLAR_CANVAS_TILE_EXPONENT,
} from "./polar-canvas-request";

export interface PolarCanvasImage {
  canvas: HTMLCanvasElement;
  range: TileRange;
  version: string | null;
}
interface Cap {
  image: PolarCanvasImage | null;
  demand: string;
  pending: Promise<void> | null;
  queued: boolean;
}

/** Independent cap downloads never block Mercator or the other hemisphere. */
export class PolarCanvasProvider {
  private readonly caps: Record<PolarHemisphere, Cap> = {
    north: {
      image: null,
      demand: "",
      pending: null,
      queued: false,
    },
    south: {
      image: null,
      demand: "",
      pending: null,
      queued: false,
    },
  };
  private readonly colors: Uint32Array;
  private readonly endpoint: string;
  private epoch = 0;
  private disposed = false;
  private readonly updateView = () => this.update(false);
  private readonly refresh = (force: boolean) => {
    if (force) this.epoch++;
    this.update(true);
    return this.waitForUpdates();
  };

  private readonly map: Map;
  private readonly tracks: JourneyTileProvider;

  constructor(
    map: Map,
    tracks: JourneyTileProvider,
    bg: RGBAColor,
    fg: RGBAColor,
  ) {
    this.map = map;
    this.tracks = tracks;
    this.colors = createCanvasTrackColors(bg, fg);
    this.endpoint = new URL(
      `${window.EXTERNAL_PARAMS.cgi_endpoint || "."}/tile_range`,
      window.location.href,
    ).href;
    map.on("move", this.updateView);
    map.on("render", this.updateView);
    tracks.registerRefreshCallback(this.refresh);
    this.update(false);
  }

  get(hemisphere: PolarHemisphere): PolarCanvasImage | null {
    return this.caps[hemisphere].image;
  }

  /** Require image coverage only for caps that are actually on screen. */
  isReadyForDisplay(): boolean {
    if (this.disposed) return false;
    return (Object.keys(this.caps) as PolarHemisphere[]).every((hemisphere) => {
      const { visible, range } = getPolarCanvasView(this.map, hemisphere);
      const image = this.caps[hemisphere].image;
      return (
        !visible || (image !== null && tileRangeContains(image.range, range))
      );
    });
  }

  async waitForUpdates(): Promise<void> {
    await Promise.all(Object.values(this.caps).map((cap) => cap.pending));
  }

  private isGlobe(): boolean {
    return this.map.getProjection().type !== "mercator";
  }

  private update(checkVersion: boolean): void {
    if (this.disposed || !this.isGlobe()) return;
    for (const hemisphere of ["north", "south"] as const) {
      const cap = this.caps[hemisphere];
      const { range } = getPolarCanvasView(this.map, hemisphere);
      const demand = `${this.epoch}/${range.join("/")}`;
      if (cap.pending && cap.demand === demand) continue;
      if (!checkVersion && cap.demand === demand) continue;
      cap.demand = demand;
      cap.queued = true;
      if (!cap.pending) cap.pending = this.drain(hemisphere);
    }
  }

  private async drain(hemisphere: PolarHemisphere): Promise<void> {
    const cap = this.caps[hemisphere];
    try {
      while (cap.queued && !this.disposed && this.isGlobe()) {
        cap.queued = false;
        const epoch = this.epoch;
        const { range } = getPolarCanvasView(this.map, hemisphere);
        const [x, y, width, height, z] = range;
        const current = () =>
          !this.disposed &&
          this.isGlobe() &&
          epoch === this.epoch &&
          getPolarCanvasView(this.map, hemisphere).range.every(
            (v, i) => v === range[i],
          );
        const url = new URL(this.endpoint);
        for (const [key, value] of Object.entries({
          plane: hemisphere,
          x,
          y,
          width,
          height,
          z,
          buffer_size_power: POLAR_CANVAS_TILE_EXPONENT,
        }))
          url.searchParams.set(key, String(value));
        if (
          cap.image?.range.every((v, i) => v === range[i]) &&
          cap.image.version?.startsWith(`${epoch}/`)
        ) {
          url.searchParams.set(
            "cached_version",
            cap.image.version.slice(`${epoch}/`.length),
          );
        }
        try {
          const response = await fetch(url.href, { cache: "no-cache" });
          if (!current() || response.headers.get("X-Not-Modified") === "true")
            continue;
          if (!response.ok)
            throw new Error(`Polar tile request: ${response.status}`);
          const bytes = await response.arrayBuffer();
          if (!bytes.byteLength || !current()) continue;
          // Allow input/paint between bounded cap decodes on the Canvas path.
          await new Promise((resolve) => setTimeout(resolve, 0));
          if (!current()) continue;
          const source = await new MainThreadTileBufferBackend().decode(
            0,
            bytes,
          );
          try {
            if (!current()) continue;
            const canvas = document.createElement("canvas");
            const context = canvas.getContext("2d");
            if (!context) throw new Error("Canvas 2D is unavailable");
            drawCanvasTileRange(
              canvas,
              context,
              this.colors,
              undefined,
              range,
              POLAR_CANVAS_TILE_EXPONENT,
              source.mainThreadBuffer!,
            );
            const version = response.headers.get("X-Tile-Version");
            cap.image = {
              canvas,
              range,
              version: version === null ? null : `${epoch}/${version}`,
            };
            this.map.triggerRepaint();
          } finally {
            source.release();
          }
        } catch (error) {
          if (!this.disposed)
            console.error(`Unable to load ${hemisphere} Canvas cap`, error);
        }
      }
    } finally {
      cap.pending = null;
    }
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.map.off("move", this.updateView);
    this.map.off("render", this.updateView);
    this.tracks.unregisterRefreshCallback(this.refresh);
    for (const cap of Object.values(this.caps)) cap.image = null;
  }
}
