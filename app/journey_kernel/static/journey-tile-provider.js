import { JourneyBitmap, TileBuffer } from "../pkg";
import { getViewportTileRange } from "./utils";
import { MultiRequest } from "./multirequest.js";

export class JourneyTileProvider {
  constructor(map, journeyId, bufferSizePower = 8) {
    this.map = map;
    this.journeyId = journeyId;
    this.currentVersion = null; // Store the current version
    this.viewRange = null; // Store the current viewport tile range [x, y, w, h, z]
    this.tileBuffer = null; // Store the tile buffer data
    this.viewRangeUpdated = false; // Flag indicating view range has been updated
    this.downloadInProgress = false; // Flag indicating download is in progress
    this.bufferSizePower = bufferSizePower;

    this.tileBufferCallbacks = []; // Array to store tile buffer update callbacks

    // Initialize MultiRequest instance based on endpoint configuration
    this.multiRequest = this.initializeMultiRequest();

    this.map.on("move", () => this.tryUpdateViewRange());
    this.map.on("moveend", () => this.tryUpdateViewRange());
    // Initial update
    this.tryUpdateViewRange();
  }

  // Initialize MultiRequest instance based on endpoint configuration
  initializeMultiRequest() {
    // Determine endpoint type based on EXTERNAL_PARAMS
    let endpointUrl = null;

    // Check for flutter channel configuration
    if (window.EXTERNAL_PARAMS.flutter_channel) {
      // Use explicit flutter_channel parameter
      endpointUrl = `flutter://${window.EXTERNAL_PARAMS.flutter_channel}`;
    } else if (window.EXTERNAL_PARAMS.cgi_endpoint) {
      if (window.EXTERNAL_PARAMS.cgi_endpoint === "flutter") {
        // Legacy Flutter mode - use default channel
        const flutterChannel = "TileProviderChannel"; // Default channel for tile provider
        endpointUrl = `flutter://${flutterChannel}`;
      } else {
        // HTTP endpoint
        endpointUrl = window.EXTERNAL_PARAMS.cgi_endpoint;
      }
    } else {
      // Fallback to current working directory for HTTP
      endpointUrl = ".";
    }

    console.log(
      `JourneyTileProvider: Initializing MultiRequest with endpoint: ${endpointUrl}`,
    );
    return new MultiRequest(endpointUrl);
  }

  // typically two use cases: if the original page detect a data change, then no cache (forceUpdate = true)
  // if it is just a periodic update or normal check, then use cache (forceUpdate = false)
  async pollForJourneyUpdates(forceUpdate = false) {
    try {
      console.log("Checking for journey updates via tile buffer");

      // Force update view range and fetch tile buffer
      this.viewRangeUpdated = true;
      const tileBufferUpdated = await this.checkAndFetchTileBuffer(forceUpdate);

      return tileBufferUpdated;
    } catch (error) {
      console.error("Error while checking for journey updates:", error);
      return null;
    }
  }

  setBufferSizePower(bufferSizePower) {
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
  tryUpdateViewRange() {
    const [x, y, w, h, z] = getViewportTileRange(this.map);

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
  async checkAndFetchTileBuffer(forceUpdate = false) {
    // If no download is in progress and view range has been updated, fetch new tile buffer
    if (!this.downloadInProgress && this.viewRangeUpdated) {
      return await this.fetchTileBuffer(forceUpdate);
    }
    return false;
  }

  // Register a callback to be called when new tile buffer is ready
  registerTileBufferCallback(callback) {
    if (
      typeof callback === "function" &&
      !this.tileBufferCallbacks.includes(callback)
    ) {
      this.tileBufferCallbacks.push(callback);
      callback(
        this.viewRange[0],
        this.viewRange[1],
        this.viewRange[2],
        this.viewRange[3],
        this.viewRange[4],
        this.bufferSizePower,
        this.tileBuffer,
      );
      return true;
    }
    return false;
  }

  // Remove a previously registered callback
  unregisterTileBufferCallback(callback) {
    const index = this.tileBufferCallbacks.indexOf(callback);
    if (index !== -1) {
      this.tileBufferCallbacks.splice(index, 1);
      return true;
    }
    return false;
  }

  // Notify all registered callbacks with tile range and buffer
  notifyTileBufferReady(x, y, w, h, z, bufferSizePower, tileBuffer) {
    for (const callback of this.tileBufferCallbacks) {
      try {
        callback(x, y, w, h, z, bufferSizePower, tileBuffer);
      } catch (error) {
        console.error("Error in tile buffer callback:", error);
      }
    }
  }

  // Fetch tile buffer for current view range
  async fetchTileBuffer(forceUpdate = false) {
    if (!this.viewRange) return false;

    // Reset update flag and set download flag
    this.viewRangeUpdated = false;
    this.downloadInProgress = true;

    const [x, y, w, h, z] = this.viewRange;

    // Create request parameters for MultiRequest
    const requestParams = {
      id: this.journeyId,
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

    console.log(
      `Fetching tile buffer via MultiRequest with params:`,
      requestParams,
    );

    let tileBufferUpdated = false;
    const startTime = performance.now();

    try {
      // Make the request - response is now a direct JS object
      const response = await this.multiRequest.fetch(
        "tile_range",
        requestParams,
      );

      // Check success status directly
      if (!response.success) {
        throw new Error(response.error || "Request failed");
      }

      // Handle 304 status in response data
      if (response.data && response.data.status === 304) {
        console.log("Tile buffer has not changed (304 Not Modified)");
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
      const bytes = Uint8Array.from(atob(response.data.body), (c) =>
        c.charCodeAt(0),
      );

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
  buildLogUrl(params) {
    const endpoint = this.multiRequest.cgiEndpoint;
    if (endpoint.startsWith("flutter://")) {
      return `${endpoint}/tile_range`;
    }

    const urlParams = new URLSearchParams(params).toString();
    return `${endpoint}/tile_range?${urlParams}`;
  }
}
