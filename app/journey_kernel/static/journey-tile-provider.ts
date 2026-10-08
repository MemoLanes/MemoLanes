import type { TileBuffer } from "../pkg/journey_kernel.js";
import {
  tileRangeContains,
  tileRangeIntersect,
  type TileRange,
} from "./layers/tile-range";
import type * as maplibregl from "maplibre-gl";
import {
  AVAILABLE_LAYERS,
  type LayerConfig,
  type ReactiveParams,
} from "./params";
import { settleGenerationAdvance } from "./generation-policy";
import {
  resolveZoomingOutInteractionState,
  shouldDeferDataViewportUpdate,
} from "./viewport-interaction-policy";
import { getMapWorldSize } from "./layers/utils";
import { getTileRequestRange } from "./layers/tile-request-policy";
import {
  MainThreadTileBufferBackend,
  type TileBufferBackend,
  type TileBufferSource,
} from "./tile-buffer-backend";
export type { TileBufferSource } from "./tile-buffer-backend";
import {
  advanceTileBufferDemand,
  canCommitActiveTileBufferRequest,
  createTileBufferRequestState,
  revokeActiveTileBufferRequest,
  settleTileBufferRequest,
  shouldPreserveActiveTileBufferRequest,
  startTileBufferRequest,
  type ActiveRequestDisposition,
  type TileBufferDemand,
  type TileBufferRequestState,
} from "./tile-buffer-request-state";

const WORLD_SIZE_CHANGE_EPSILON = 1e-6;

/**
 * A deserialized tile buffer together with the range it was fetched for.
 */
export interface LoadedTileBuffer {
  /** Opaque identity of the decoded buffer used for async queries. */
  source: TileBufferSource;
  /**
   * Monotonically increasing frontend data generation. Viewport-only buffer
   * replacements retain the same generation; renderer-version changes and
   * explicit data refreshes advance it.
   */
  generation: number;
  // Unlike `viewRange`, this does not advance while a replacement buffer is
  // still downloading, so it always describes `source` itself.
  range: TileRange;
  bufferSizePower: number;
}

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
  private viewRange: TileRange | null; // Store the current viewport tile range [x, y, w, h, z]
  // TODO: evaluate whether we need to make this public (also the bufferSizePower)
  tileBuffer: TileBuffer | null; // Store the tile buffer data
  private loadedTileBuffer: LoadedTileBuffer | null;
  private dataGeneration: number;
  private generationAdvancePending: boolean;
  private tileBufferUpdatePromise: Promise<boolean> | null;
  private tileBufferRequestState: TileBufferRequestState;
  private readonly tileBufferBackends = new Map<string, TileBufferBackend>();
  private nextTileBufferSourceId: number;
  bufferSizePower: number;
  private isGlobeProjection: boolean; // Flag indicating if globe projection is used
  private tileBufferCallbacks: TileBufferCallback[]; // Array to store tile buffer update callbacks
  private cgiEndpoint: string;
  private viewportUpdateDeferred: boolean;
  private lastObservedWorldSize: number;
  private zoomingOutInProgress: boolean;
  private disposed: boolean;
  private readonly unsubscribeRenderMode: () => void;
  private readonly handleMapMove = (): void => {
    this.tryUpdateViewRange();
  };
  private readonly handleMapMoveEnd = (): void => {
    this.tryUpdateViewRange();
  };

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
    this.loadedTileBuffer = null;
    this.dataGeneration = 0;
    this.generationAdvancePending = false;
    this.tileBufferUpdatePromise = null;
    this.tileBufferRequestState = createTileBufferRequestState();
    this.nextTileBufferSourceId = 1;

    this.bufferSizePower = this.getBufferSizePowerFromRenderMode(
      params.renderMode,
    );

    // TODO: better handling of globe projection
    this.isGlobeProjection = isGlobeProjection;

    this.tileBufferCallbacks = [];

    this.cgiEndpoint = window.EXTERNAL_PARAMS.cgi_endpoint || ".";
    this.viewportUpdateDeferred = false;
    this.lastObservedWorldSize = getMapWorldSize(this.map);
    this.zoomingOutInProgress = false;
    this.disposed = false;
    console.log(`JourneyTileProvider: endpoint: ${this.cgiEndpoint}`);

    this.unsubscribeRenderMode = this.params.on("renderMode", (newMode) => {
      const newBufferSizePower = this.getBufferSizePowerFromRenderMode(newMode);
      this.setBufferSizePower(newBufferSizePower);
      // TODO: should we also refresh the tile buffer?
    });

    this.map.on("move", this.handleMapMove);
    this.map.on("moveend", this.handleMapMoveEnd);
    this.tryUpdateViewRange();
  }

  private getBufferSizePowerFromRenderMode(renderMode: string): number {
    return this.getLayerConfig(renderMode).bufferSizePower;
  }

  private getLayerConfig(renderMode: string): LayerConfig {
    return AVAILABLE_LAYERS[renderMode] ?? AVAILABLE_LAYERS.canvas;
  }

  // typically two use cases: if the original page detect a data change, then no cache (forceUpdate = true)
  // if it is just a periodic update or normal check, then use cache (forceUpdate = false)
  async pollForJourneyUpdates(
    forceUpdate: boolean = false,
  ): Promise<boolean | null> {
    if (this.disposed) return null;
    // Periodic checks can wait for the next tick. Advancing demand while a
    // request is decoding or downloading would supersede useful work every
    // second on slow devices. Explicit refreshes must still queue a follow-up.
    if (!forceUpdate && this.tileBufferUpdatePromise) return false;
    try {
      // console.log("Checking for journey updates via tile buffer");

      // Force update view range and fetch tile buffer
      // A forced poll is an explicit data/proxy refresh. Keep this separate
      // from viewport-triggered forced downloads, which do not represent a
      // newer data generation.
      if (forceUpdate) {
        this.generationAdvancePending = true;
      }
      this.queueTileBufferUpdate();
      const tileBufferUpdated = await this.checkAndFetchTileBuffer(forceUpdate);

      return tileBufferUpdated;
    } catch (error) {
      console.error("Error while checking for journey updates:", error);
      return null;
    }
  }

  /** Wait until the current viewport's tile buffer has finished updating. */
  async waitForTileBufferUpdate(): Promise<void> {
    await this.checkAndFetchTileBuffer(true);
  }

  setBufferSizePower(bufferSizePower: number): void {
    if (this.disposed || this.bufferSizePower === bufferSizePower) {
      return;
    }

    console.log(
      `Switching buffer size power: ${this.bufferSizePower} -> ${bufferSizePower}`,
    );
    this.bufferSizePower = bufferSizePower;
    this.pollForJourneyUpdates(true);
  }

  // Try to update the current viewport tile range, only if it has changed
  tryUpdateViewRange(): TileRange | null {
    if (this.disposed) return null;

    // Observe every move frame, including frames whose integer tile range is
    // unchanged. That lets us distinguish a continuing zoom-out from a pan
    // when the range eventually expands.
    const currentWorldSize = getMapWorldSize(this.map);
    this.zoomingOutInProgress = resolveZoomingOutInteractionState(
      this.map.isZooming(),
      this.zoomingOutInProgress,
      this.lastObservedWorldSize,
      currentWorldSize,
      WORLD_SIZE_CHANGE_EPSILON,
    );
    this.lastObservedWorldSize = currentWorldSize;

    const currentViewRange = getTileRequestRange(
      this.map,
      this.isGlobeProjection,
      this.bufferSizePower,
      this.getLayerConfig(this.params.renderMode).tileRequestPolicy,
    );
    const [x, y, w, h, z] = currentViewRange;

    const rangeUnchanged =
      this.viewRange &&
      this.viewRange[0] === x &&
      this.viewRange[1] === y &&
      this.viewRange[2] === w &&
      this.viewRange[3] === h &&
      this.viewRange[4] === z;
    if (rangeUnchanged && !this.viewportUpdateDeferred) {
      const requestState = this.tileBufferRequestState;
      if (
        !this.map.isMoving() &&
        requestState.active !== null &&
        requestState.requestQueued &&
        requestState.active.requestId < requestState.desiredRequestId
      ) {
        this.tileBufferRequestState =
          revokeActiveTileBufferRequest(requestState);
      }
      return this.viewRange;
    }

    if (!rangeUnchanged) {
      this.viewRange = [x, y, w, h, z];
    }
    const viewRange = this.viewRange;
    if (!viewRange) {
      return null;
    }

    // Pan gestures can keep the loaded buffer. Zooming in to a finer tile
    // zoom, or consuming the buffered guard band, has to fetch before moveend
    // so tracks do not stay at the gesture-start resolution or expose holes.
    const loaded = this.loadedTileBuffer;
    const loadedCoversDesiredRange =
      loaded !== null && tileRangeContains(loaded.range, viewRange);
    const loadedTileZoomIsSufficient =
      loaded !== null && loaded.range[4] >= viewRange[4];
    if (
      shouldDeferDataViewportUpdate(
        this.map.isMoving(),
        loaded !== null,
        loadedCoversDesiredRange,
        loadedTileZoomIsSufficient,
      )
    ) {
      if (!this.viewportUpdateDeferred && this.tileBufferRequestState.active) {
        this.tileBufferRequestState = advanceTileBufferDemand(
          this.tileBufferRequestState,
          { queueRequest: false, activeRequest: "supersede" },
        );
      }
      this.viewportUpdateDeferred = true;
      return this.viewRange;
    }

    this.viewportUpdateDeferred = false;
    console.log(`View range updated: x=${x}, y=${y}, w=${w}, h=${h}, z=${z}`);

    // Mark that view range has been updated and trigger fetch if not already downloading
    // Force download since we need tiles for a different area
    // During a continuous zoom-out, each successively larger viewport is
    // useful. Let the single in-flight request commit so coverage grows while
    // the gesture is still active, and coalesce later changes into one latest
    // follow-up request. Other viewport/data changes reject obsolete results
    // when they arrive.
    const preserveInFlightRequest = shouldPreserveActiveTileBufferRequest(
      this.tileBufferRequestState,
      this.getTileBufferDemand(viewRange),
    );
    this.queueTileBufferUpdate(
      preserveInFlightRequest ? "preserve" : "supersede",
    );

    if (!this.tileBufferUpdatePromise) {
      void this.checkAndFetchTileBuffer(true); // Force update when view range changes
    }

    return this.viewRange;
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.unsubscribeRenderMode();
    this.map.off("move", this.handleMapMove);
    this.map.off("moveend", this.handleMapMoveEnd);
    this.tileBufferCallbacks.length = 0;
    this.tileBufferRequestState = {
      ...this.tileBufferRequestState,
      requestQueued: false,
    };
    if (this.tileBufferRequestState.active) {
      this.tileBufferRequestState = advanceTileBufferDemand(
        this.tileBufferRequestState,
        { queueRequest: false, activeRequest: "supersede" },
      );
    }
    this.releaseTileBufferSource(this.loadedTileBuffer?.source ?? null);
    this.loadedTileBuffer = null;
    this.tileBuffer = null;
    for (const backend of this.tileBufferBackends.values()) backend.dispose();
    this.tileBufferBackends.clear();
  }

  // Check state and fetch tile buffer if needed
  private checkAndFetchTileBuffer(
    forceUpdate: boolean = false,
  ): Promise<boolean> {
    if (this.disposed) return Promise.resolve(false);
    if (this.tileBufferUpdatePromise) {
      return this.tileBufferUpdatePromise;
    }

    const pendingUpdate = this.fetchPendingTileBufferUpdates(
      forceUpdate,
    ).finally(() => {
      if (this.tileBufferUpdatePromise === pendingUpdate) {
        this.tileBufferUpdatePromise = null;
      }
    });
    this.tileBufferUpdatePromise = pendingUpdate;
    return pendingUpdate;
  }

  private async fetchPendingTileBufferUpdates(
    forceUpdate: boolean,
  ): Promise<boolean> {
    let tileBufferUpdated = false;

    while (!this.disposed && this.tileBufferRequestState.requestQueued) {
      if (await this.fetchTileBuffer(forceUpdate)) {
        tileBufferUpdated = true;
      }
      // A viewport change queued during the download must bypass version caching.
      forceUpdate = true;
    }

    return tileBufferUpdated;
  }

  /** The loaded tile buffer and the range it covers, if any. */
  getLoadedTileBuffer(): LoadedTileBuffer | null {
    return this.loadedTileBuffer;
  }

  // Register a callback to be called when new tile buffer is ready
  registerTileBufferCallback(callback: TileBufferCallback): boolean {
    if (
      typeof callback !== "function" ||
      this.tileBufferCallbacks.includes(callback)
    ) {
      return false;
    }
    this.tileBufferCallbacks.push(callback);
    this.invokeTileBufferCallback(callback);
    return true;
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

  // Notify all registered callbacks that a new tile buffer is available
  private notifyTileBufferReady(): void {
    for (const callback of this.tileBufferCallbacks) {
      this.invokeTileBufferCallback(callback);
    }
  }

  private invokeTileBufferCallback(callback: TileBufferCallback): void {
    const loaded = this.loadedTileBuffer;
    if (!loaded) {
      return;
    }
    const source = loaded.source;
    try {
      callback(
        ...loaded.range,
        loaded.bufferSizePower,
        source.mainThreadBuffer,
      );
    } catch (error) {
      console.error("Error in tile buffer callback:", error);
    }
  }

  private async fetchTileBuffer(
    forceUpdate: boolean = false,
  ): Promise<boolean> {
    if (this.disposed || !this.viewRange) return false;

    const [x, y, w, h, z] = this.viewRange;
    const requestRange: TileRange = [x, y, w, h, z];
    const startedRequest = startTileBufferRequest(
      this.tileBufferRequestState,
      requestRange,
      this.bufferSizePower,
    );
    this.tileBufferRequestState = startedRequest.state;
    const activeRequest = startedRequest.active;
    const requestId = activeRequest.requestId;
    // Capture this at request start. If an explicit refresh arrives while the
    // request is in flight, the queued follow-up request consumes it instead.
    const forceGenerationAdvance = this.generationAdvancePending;
    this.generationAdvancePending = false;

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
    const rejectObsoleteResponse = (): void => {
      this.generationAdvancePending = settleGenerationAdvance(
        this.generationAdvancePending,
        forceGenerationAdvance,
        "failed",
      );
    };
    try {
      const url = buildUrl(this.cgiEndpoint, "tile_range", requestParams);
      const rawResponse = await fetch(url, { cache: "no-cache" });
      if (this.disposed) return false;

      if (!rawResponse.ok) {
        throw new Error(
          `Request failed: ${rawResponse.status} ${rawResponse.statusText}`,
        );
      }
      if (!this.canCommitTileBufferRequest(requestId)) {
        rejectObsoleteResponse();
        return false;
      }

      if (rawResponse.headers.get("X-Not-Modified") === "true") {
        this.generationAdvancePending = settleGenerationAdvance(
          this.generationAdvancePending,
          forceGenerationAdvance,
          "unchanged",
        );
        return false;
      }

      const newVersion = rawResponse.headers.get("X-Tile-Version");
      const rendererVersionChanged =
        newVersion !== null && newVersion !== this.currentVersion;

      const buffer = await rawResponse.arrayBuffer();
      if (this.disposed) return false;
      if (!this.canCommitTileBufferRequest(requestId)) {
        rejectObsoleteResponse();
        return false;
      }
      const bytes = new Uint8Array(buffer);
      const responseByteLength = bytes.byteLength;

      // An empty body on a 200 response means "not modified" — Android
      // WebView rejects real 304 status codes, so the Dart interceptor
      // returns 200 with an empty body instead.
      if (responseByteLength === 0) {
        this.generationAdvancePending = settleGenerationAdvance(
          this.generationAdvancePending,
          forceGenerationAdvance,
          "unchanged",
        );
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

      const previousSource = this.loadedTileBuffer?.source ?? null;
      const source = await this.decodeTileBuffer(buffer);
      if (this.disposed) {
        this.releaseTileBufferSource(source);
        return false;
      }
      if (!this.canCommitTileBufferRequest(requestId)) {
        this.releaseTileBufferSource(source);
        rejectObsoleteResponse();
        return false;
      }

      this.generationAdvancePending = settleGenerationAdvance(
        this.generationAdvancePending,
        forceGenerationAdvance,
        "committed",
      );

      // Commit generation metadata only after the staging buffer has been
      // deserialized successfully.
      if (
        this.dataGeneration === 0 ||
        rendererVersionChanged ||
        forceGenerationAdvance
      ) {
        this.dataGeneration += 1;
      }
      if (newVersion) {
        this.currentVersion = newVersion;
        console.log(`Updated tile buffer version to: ${newVersion}`);
      }

      this.releaseTileBufferSource(previousSource);
      this.tileBuffer = source.mainThreadBuffer;
      this.loadedTileBuffer = {
        source,
        generation: this.dataGeneration,
        range: requestRange,
        bufferSizePower: activeRequest.bufferSizePower,
      };

      console.log(`Tile buffer fetched and deserialized successfully`);

      this.notifyTileBufferReady();

      tileBufferUpdated = true;
    } catch (error) {
      if (this.disposed) return false;
      this.generationAdvancePending = settleGenerationAdvance(
        this.generationAdvancePending,
        forceGenerationAdvance,
        "failed",
      );
      console.error("Error fetching or deserializing tile buffer:", error);
    } finally {
      this.tileBufferRequestState = settleTileBufferRequest(
        this.tileBufferRequestState,
        activeRequest.requestId,
      );
    }

    return tileBufferUpdated;
  }

  private decodeTileBuffer(buffer: ArrayBuffer): Promise<TileBufferSource> {
    const mode = this.params.renderMode;
    let backend = this.tileBufferBackends.get(mode);
    if (!backend) {
      const factory = this.getLayerConfig(mode).createTileBufferBackend;
      backend = factory
        ? factory(() => {
            if (this.disposed) return;
            this.queueTileBufferUpdate();
            void this.checkAndFetchTileBuffer(true);
          })
        : new MainThreadTileBufferBackend();
      this.tileBufferBackends.set(mode, backend);
    }
    return backend.decode(this.nextTileBufferSourceId++, buffer);
  }

  private releaseTileBufferSource(source: TileBufferSource | null): void {
    try {
      source?.release();
    } catch (error) {
      console.warn("Unable to release a tile buffer source", error);
    }
  }

  private getTileBufferDemand(viewRange: TileRange): TileBufferDemand {
    const active = this.tileBufferRequestState.active;
    return {
      mapMoving: this.map.isMoving(),
      zoomingOut: this.zoomingOutInProgress,
      activeRequestMatchesResolution:
        active !== null &&
        active.range[4] === viewRange[4] &&
        active.bufferSizePower === this.bufferSizePower,
      activeRequestCoversDesiredRange:
        active !== null && tileRangeContains(active.range, viewRange),
      activeRequestOverlapsDesiredRange:
        active !== null && tileRangeIntersect(active.range, viewRange) !== null,
    };
  }

  private canCommitTileBufferRequest(requestId: number): boolean {
    return (
      this.viewRange !== null &&
      canCommitActiveTileBufferRequest(
        this.tileBufferRequestState,
        requestId,
        this.getTileBufferDemand(this.viewRange),
      )
    );
  }

  private queueTileBufferUpdate(
    activeRequest: ActiveRequestDisposition = "supersede",
  ): void {
    this.tileBufferRequestState = advanceTileBufferDemand(
      this.tileBufferRequestState,
      {
        queueRequest: true,
        activeRequest,
      },
    );
  }
}
