import { TileBuffer } from "../pkg";
import { getViewportTileRange } from "./layers/utils";
import { MultiRequest } from "./multi-requests";
import type maplibregl from "maplibre-gl";
import type { ValidatedParams } from "./params";

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
  id: string;
  x: number;
  y: number;
  z: number;
  width: number;
  height: number;
  buffer_size_power: number;
  cached_version?: string;
}

/**
 * Tile buffer response data structure
 */
interface TileBufferResponseData {
  status?: number;
  headers?: {
    version?: string;
  };
  body?: string; // base64 encoded
}

/**
 * Tile buffer response structure
 */
interface TileBufferResponse {
  success: boolean;
  error?: string;
  data?: TileBufferResponseData;
  requestId?: string;
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

export class JourneyTileProvider {
  private map: maplibregl.Map;
  private params: ValidatedParams;
  private currentVersion: string | null; // Store the current version
  private viewRange: ViewRange | null; // Store the current viewport tile range [x, y, w, h, z]
  private tileBuffer: TileBuffer | null; // Store the tile buffer data
  private viewRangeUpdated: boolean; // Flag indicating view range has been updated
  private downloadInProgress: boolean; // Flag indicating download is in progress
  private bufferSizePower: number;
  private isGlobeProjection: boolean; // Flag indicating if globe projection is used
  private tileBufferCallbacks: TileBufferCallback[]; // Array to store tile buffer update callbacks
  private multiRequest: MultiRequest;

  constructor(
    map: maplibregl.Map,
    params: ValidatedParams,
    bufferSizePower: number = 8,
    isGlobeProjection: boolean = false,
  ) {
    this.map = map;
    this.params = params;
    this.currentVersion = null; // Store the current version
    this.viewRange = null; // Store the current viewport tile range [x, y, w, h, z]
    this.tileBuffer = null; // Store the tile buffer data
    this.viewRangeUpdated = false; // Flag indicating view range has been updated
    this.downloadInProgress = false; // Flag indicating download is in progress
    this.bufferSizePower = bufferSizePower;
    // TODO: better handling of globe projection
    this.isGlobeProjection = isGlobeProjection;

    this.tileBufferCallbacks = []; // Array to store tile buffer update callbacks

    // Initialize MultiRequest instance based on endpoint configuration
    this.multiRequest = this.initializeMultiRequest();

    this.map.on("move", () => this.tryUpdateViewRange());
    this.map.on("moveend", () => this.tryUpdateViewRange());
    // Initial update
    this.tryUpdateViewRange();
  }

  // Initialize MultiRequest instance based on endpoint configuration
  private initializeMultiRequest(): MultiRequest {
    // Use cgi_endpoint if provided, otherwise default to current directory
    const endpointUrl = window.EXTERNAL_PARAMS.cgi_endpoint || ".";

    console.log(
      `JourneyTileProvider: Initializing MultiRequest with endpoint: ${endpointUrl}`,
    );
    return new MultiRequest(endpointUrl);
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

  // Fetch tile buffer for current view range
  private async fetchTileBuffer(
    forceUpdate: boolean = false,
  ): Promise<boolean> {
    if (!this.viewRange) return false;

    // Reset update flag and set download flag
    this.viewRangeUpdated = false;
    this.downloadInProgress = true;

    const [x, y, w, h, z] = this.viewRange;

    // Create request parameters for MultiRequest
    const requestParams: TileBufferRequestParams = {
      id: this.params.journeyId,
      x: x,
      y: y,
      z: z,
      width: w,
      height: h,
      buffer_size_power: this.bufferSizePower,
    };

    // Add cached version if available and not forcing update
    if (!forceUpdate && this.currentVersion) {
      requestParams.cached_version = this.currentVersion;
    }

    // console.log(
    //   `Fetching tile buffer via MultiRequest with params:`,
    //   requestParams,
    // );

    let tileBufferUpdated = false;
    const startTime = performance.now();

    try {
      // Make the request - response is now a direct JS object
      const response = (await this.multiRequest.fetch(
        "tile_range",
        requestParams,
      )) as TileBufferResponse;

      // Check success status directly
      if (!response.success) {
        throw new Error(response.error || "Request failed");
      }

      // Handle 304 status in response data
      if (response.data && response.data.status === 304) {
        // console.log("Tile buffer has not changed (304 Not Modified)");
        return false;
      }

      // Emit timing data for successful downloads (not 304)
      // Build a representative URL for logging purposes
      const endTime = performance.now();
      const duration = Math.round(endTime - startTime);
      const logUrl = this.buildLogUrl(requestParams);
      window.dispatchEvent(
        new CustomEvent("tileDownloadTiming", {
          detail: {
            duration: duration,
            timestamp: endTime,
            url: logUrl,
            status: response.data?.status || 200,
            requestId: response.requestId || "unknown",
          },
        }),
      );

      // Update version from response data headers
      const newVersion = response.data?.headers?.version;
      if (newVersion) {
        this.currentVersion = newVersion;
        console.log(`Updated tile buffer version to: ${newVersion}`);
      }

      // Get the binary data
      // response.data.body is base64 encoded, so decode it
      if (!response.data?.body) {
        throw new Error("No body in response data");
      }

      const bytes = Uint8Array.from(atob(response.data.body), (c) =>
        c.charCodeAt(0),
      );

      // TODO: the tileBuffer wasm deserialization can take up to 2000ms in dev mode, and 30ms in prod mode.
      // consider move this into web worker so that it won't block the main thread.
      // Deserialize into a TileBuffer object using the WebAssembly module
      this.tileBuffer = TileBuffer.from_bytes(bytes);

      console.log(
        `Tile buffer fetched and deserialized successfully via ${this.multiRequest.getStatus().isFlutterMode ? "Flutter IPC" : "HTTP"}`,
      );

      // Notify all registered callbacks that a new tile buffer is ready
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
      // Reset download flag
      this.downloadInProgress = false;

      // Check if view range was updated during download
      // If so, start another download
      if (this.viewRangeUpdated) {
        console.log(
          "View range was updated during download, fetching new tile buffer",
        );
        this.checkAndFetchTileBuffer(true);
      }
    }

    return tileBufferUpdated;
  }

  // Helper method to build URL for logging purposes
  private buildLogUrl(params: TileBufferRequestParams): string {
    const endpoint = (this.multiRequest as any).cgiEndpoint;
    if (endpoint && endpoint.startsWith("flutter://")) {
      return `${endpoint}/tile_range`;
    }

    const urlParams = new URLSearchParams(params as any).toString();
    return `${endpoint}/tile_range?${urlParams}`;
  }
}
