import { TileBuffer } from "../pkg/journey_kernel.js";
import { getViewportTileRange } from "./layers/utils";
import type maplibregl from "maplibre-gl";
import { AVAILABLE_LAYERS, type ReactiveParams } from "./params";

/**
 * View range tuple: [x, y, width, height, zoom]
 */
type ViewRange = [number, number, number, number, number];

/**
 * Tile buffer callback function type
 */
type TileBufferCallback = (
  x: number,
  y: number,
  w: number,
  h: number,
  z: number,
  bufferSizePower: number,
  tileBuffer: TileBuffer | null,
) => void;

/**
 * Request parameters for tile buffer fetch
 */
interface TileBufferRequestParams {
  x: number;
  y: number;
  z: number;
  width: number;
  height: number;
  buffer_size_power: number;
  cached_version?: string;
}

/**
 * Extended window interface for external parameters
 */
declare global {
  interface Window {
    EXTERNAL_PARAMS: {
      cgi_endpoint?: string;
      [key: string]: any;
    };
  }
}

function buildUrl(
  endpoint: string,
  resource: string,
  params?: Record<string, any>,
): string {
  const url = new URL(`${endpoint}/${resource}`, window.location.href);
  if (params) {
    for (const [key, value] of Object.entries(params)) {
      if (value !== undefined && value !== null) {
        url.searchParams.set(key, String(value));
      }
    }
  }
  return url.toString();
}

export class JourneyTileProvider {
  private map: maplibregl.Map;
  private params: ReactiveParams;
  private currentVersion: string | null; // Store the current version
  private viewRange: ViewRange | null; // Store the current viewport tile range [x, y, w, h, z]
  // TODO: evaluate whether we need to make this public (also the bufferSizePower)
  tileBuffer: TileBuffer | null; // Store the tile buffer data
  private viewRangeUpdated: boolean; // Flag indicating view range has been updated
  private downloadInProgress: boolean; // Flag indicating download is in progress
  bufferSizePower: number;
  private isGlobeProjection: boolean; // Flag indicating if globe projection is used
  private tileBufferCallbacks: TileBufferCallback[]; // Array to store tile buffer update callbacks
  private cgiEndpoint: string;

  constructor(
    map: maplibregl.Map,
    params: ReactiveParams,
    isGlobeProjection: boolean = false,
  ) {
    this.map = map;
    this.params = params;
    this.currentVersion = null;
    this.viewRange = null;
    this.tileBuffer = null;
    this.viewRangeUpdated = false;
    this.downloadInProgress = false;

    this.bufferSizePower = this.getBufferSizePowerFromRenderMode(
      params.renderMode,
    );

    // TODO: better handling of globe projection
    this.isGlobeProjection = isGlobeProjection;

    this.tileBufferCallbacks = [];

    this.cgiEndpoint = window.EXTERNAL_PARAMS.cgi_endpoint || ".";
    console.log(`JourneyTileProvider: endpoint: ${this.cgiEndpoint}`);

    this.params.on("renderMode", (newMode, _oldMode) => {
      const newBufferSizePower = this.getBufferSizePowerFromRenderMode(newMode);
      this.setBufferSizePower(newBufferSizePower);
      // TODO: should we also refresh the tile buffer?
    });

    this.map.on("move", () => this.tryUpdateViewRange());
    this.map.on("moveend", () => this.tryUpdateViewRange());
    this.tryUpdateViewRange();
  }

  private getBufferSizePowerFromRenderMode(renderMode: string): number {
    const layerConfig = AVAILABLE_LAYERS[renderMode];
    if (layerConfig) {
      return layerConfig.bufferSizePower;
    }
    return AVAILABLE_LAYERS["canvas"]?.bufferSizePower ?? 8;
  }

  // typically two use cases: if the original page detect a data change, then no cache (forceUpdate = true)
  // if it is just a periodic update or normal check, then use cache (forceUpdate = false)
  async pollForJourneyUpdates(
    forceUpdate: boolean = false,
  ): Promise<boolean | null> {
    try {
      // console.log("Checking for journey updates via tile buffer");

      // Force update view range and fetch tile buffer
      this.viewRangeUpdated = true;
      const tileBufferUpdated = await this.checkAndFetchTileBuffer(forceUpdate);

      return tileBufferUpdated;
    } catch (error) {
      console.error("Error while checking for journey updates:", error);
      return null;
    }
  }

  setBufferSizePower(bufferSizePower: number): void {
    if (this.bufferSizePower === bufferSizePower) {
      return;
    }

    console.log(
      `Switching buffer size power: ${this.bufferSizePower} -> ${bufferSizePower}`,
    );
    this.bufferSizePower = bufferSizePower;
    this.pollForJourneyUpdates(true);
  }

  // Try to update the current viewport tile range, only if it has changed
  tryUpdateViewRange(): ViewRange | null {
    const [x, y, w, h, z] = getViewportTileRange(
      this.map,
      this.isGlobeProjection,
    );

    // Skip update if the values haven't changed
    if (
      this.viewRange &&
      this.viewRange[0] === x &&
      this.viewRange[1] === y &&
      this.viewRange[2] === w &&
      this.viewRange[3] === h &&
      this.viewRange[4] === z
    ) {
      return this.viewRange;
    }

    // Update only when values have changed
    this.viewRange = [x, y, w, h, z];
    console.log(`View range updated: x=${x}, y=${y}, w=${w}, h=${h}, z=${z}`);

    // Mark that view range has been updated and trigger fetch if not already downloading
    // Force download since we need tiles for a different area
    this.viewRangeUpdated = true;

    this.checkAndFetchTileBuffer(true); // Force update when view range changes

    return this.viewRange;
  }

  // Check state and fetch tile buffer if needed
  private async checkAndFetchTileBuffer(
    forceUpdate: boolean = false,
  ): Promise<boolean> {
    // If no download is in progress and view range has been updated, fetch new tile buffer
    if (!this.downloadInProgress && this.viewRangeUpdated) {
      return await this.fetchTileBuffer(forceUpdate);
    }
    return false;
  }

  // Register a callback to be called when new tile buffer is ready
  registerTileBufferCallback(callback: TileBufferCallback): boolean {
    if (
      typeof callback === "function" &&
      !this.tileBufferCallbacks.includes(callback)
    ) {
      this.tileBufferCallbacks.push(callback);
      if (this.viewRange) {
        callback(
          this.viewRange[0],
          this.viewRange[1],
          this.viewRange[2],
          this.viewRange[3],
          this.viewRange[4],
          this.bufferSizePower,
          this.tileBuffer,
        );
      }
      return true;
    }
    return false;
  }

  // Remove a previously registered callback
  unregisterTileBufferCallback(callback: TileBufferCallback): boolean {
    const index = this.tileBufferCallbacks.indexOf(callback);
    if (index !== -1) {
      this.tileBufferCallbacks.splice(index, 1);
      return true;
    }
    return false;
  }

  // Notify all registered callbacks with tile range and buffer
  private notifyTileBufferReady(
    x: number,
    y: number,
    w: number,
    h: number,
    z: number,
    bufferSizePower: number,
    tileBuffer: TileBuffer | null,
  ): void {
    for (const callback of this.tileBufferCallbacks) {
      try {
        callback(x, y, w, h, z, bufferSizePower, tileBuffer);
      } catch (error) {
        console.error("Error in tile buffer callback:", error);
      }
    }
  }

  private async fetchTileBuffer(
    forceUpdate: boolean = false,
  ): Promise<boolean> {
    if (!this.viewRange) return false;

    this.viewRangeUpdated = false;
    this.downloadInProgress = true;

    const [x, y, w, h, z] = this.viewRange;

    const requestParams: TileBufferRequestParams = {
      x: x,
      y: y,
      z: z,
      width: w,
      height: h,
      buffer_size_power: this.bufferSizePower,
    };

    if (!forceUpdate && this.currentVersion) {
      requestParams.cached_version = this.currentVersion;
    }

    let tileBufferUpdated = false;
    const startTime = performance.now();

    try {
      const url = buildUrl(this.cgiEndpoint, "tile_range", requestParams);
      const rawResponse = await fetch(url, { cache: "no-cache" });

      if (rawResponse.headers.get("X-Not-Modified") === "true") {
        return false;
      }
      if (!rawResponse.ok) {
        throw new Error(
          `Request failed: ${rawResponse.status} ${rawResponse.statusText}`,
        );
      }

      const newVersion = rawResponse.headers.get("X-Tile-Version");
      if (newVersion) {
        this.currentVersion = newVersion;
        console.log(`Updated tile buffer version to: ${newVersion}`);
      }

      const buffer = await rawResponse.arrayBuffer();
      const bytes = new Uint8Array(buffer);

      // An empty body on a 200 response means "not modified" — Android
      // WebView rejects real 304 status codes, so the Dart interceptor
      // returns 200 with an empty body instead.
      if (bytes.length === 0) {
        return false;
      }

      const endTime = performance.now();
      const duration = Math.round(endTime - startTime);
      window.dispatchEvent(
        new CustomEvent("tileDownloadTiming", {
          detail: {
            duration: duration,
            timestamp: endTime,
            url: url,
            status: 200,
          },
        }),
      );

      // TODO: the tileBuffer wasm deserialization can take up to 2000ms in dev mode, and 30ms in prod mode.
      // Consider moving this into a web worker so that it won't block the main thread.
      // TODO: remove this number
      const LEVEL0_EXP = 9; // is it reasonable?
      this.tileBuffer = TileBuffer.new_from_tile_range_response(
        LEVEL0_EXP,
        bytes,
      );

      console.log(`Tile buffer fetched and deserialized successfully`);

      this.notifyTileBufferReady(
        x,
        y,
        w,
        h,
        z,
        this.bufferSizePower,
        this.tileBuffer,
      );

      tileBufferUpdated = true;
    } catch (error) {
      console.error("Error fetching or deserializing tile buffer:", error);
    } finally {
      this.downloadInProgress = false;

      if (this.viewRangeUpdated) {
        console.log(
          "View range was updated during download, fetching new tile buffer",
        );
        this.checkAndFetchTileBuffer(true);
      }
    }

    return tileBufferUpdated;
  }
}
