/**
 * MapController - Centralized map management
 *
 * This module encapsulates all map-centric logic:
 * - Map instance creation and configuration
 * - Journey layer management (switching between rendering modes)
 * - JourneyTileProvider management
 * - ReactiveParams hooks for map-related properties
 * - Map style management and retry logic
 */

import * as maplibregl from "maplibre-gl";
import type {
  Map as MaplibreMap,
  MapContextEvent,
  RequestTransformFunction,
  ResourceType,
} from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import {
  isMapboxURL,
  transformMapboxUrl,
} from "maplibregl-mapbox-request-transformer";
import {
  AVAILABLE_LAYERS,
  type ReactiveParams,
  type ProjectionType,
} from "./params";
import { JourneyTileProvider } from "./journey-tile-provider";
import { getFogStyle } from "./fog-style";
import { detectMapLocale, type MapLocale } from "./map-locale";
import { transformStyleWithProjection } from "./utils";
import { JOURNEY_LAYER_ID } from "./layers/journey-layer-interface";
import type { JourneyLayer } from "./layers/journey-layer-interface";
import { MAX_MAP_ZOOM } from "./layer-config";

const DATA_POLL_INTERVAL_MS = 1_000;

interface WebGLContextMessageChannel {
  postMessage: (message: string) => void;
}

declare global {
  interface Window {
    onWebGLContextEvent?: WebGLContextMessageChannel;
  }
}

maplibregl.setWorkerUrl(
  new URL("./maplibre-gl-worker.js", window.location.href).toString(),
);

/**
 * Configuration options for MapController
 */
export interface MapControllerConfig {
  /** Container element ID for the map */
  containerId: string;
  /** ReactiveParams instance with validated parameters */
  params: ReactiveParams;
  /**
   * Disable Mapdata automatic render loop.
   * Default: false
   */
  DisableAutoRefresh?: boolean;
}

/**
 * MapController manages the MapLibre map instance and related components
 */
export class MapController {
  private map: MaplibreMap;
  private params: ReactiveParams;
  private readonly mapLocale: MapLocale;
  private DisableAutoRefresh: boolean;
  private currentJourneyLayer: JourneyLayer | null = null;
  private journeyTileProvider: JourneyTileProvider | null = null;
  private styleRetryIntervalId: ReturnType<typeof setInterval> | null = null;
  private pollIntervalId: ReturnType<typeof setInterval> | null = null;
  private webGLContextLost = false;
  private webGLContextLossCount = 0;
  private webGLContextLostAt: number | null = null;

  constructor(config: MapControllerConfig) {
    this.params = config.params;
    this.mapLocale = detectMapLocale();
    this.DisableAutoRefresh = config.DisableAutoRefresh ?? false;

    // Build transform request function for Mapbox styles
    const transformRequest = this.buildTransformRequest();

    // Create the map instance
    this.map = new maplibregl.Map({
      container: config.containerId,
      center: [this.params.lng, this.params.lat],
      zoom: this.params.zoom,
      bounds: this.params.initialBounds
        ? [
            [this.params.initialBounds.west, this.params.initialBounds.south],
            [this.params.initialBounds.east, this.params.initialBounds.north],
          ]
        : undefined,
      fitBoundsOptions: this.params.initialBounds
        ? {
            padding: this.params.initialBoundsPadding,
            maxZoom: MAX_MAP_ZOOM,
            duration: 0,
          }
        : undefined,
      maxZoom: MAX_MAP_ZOOM,
      style: {
        version: 8,
        sources: {},
        layers: [
          {
            id: "background",
            type: "background",
            paint: {
              "background-color": "#e8e4df", // Light beige background contrasting black universe
            },
          },
        ],
        projection: { type: this.params.projection },
      },
      // TODO: maplibre brings more canvas settings, we may fine tune them later
      canvasContextAttributes: {
        antialias: true,
      },
      transformRequest,
      pitchWithRotate: false,
      touchPitch: false,
      attributionControl: false,
    });

    // Disable rotation controls
    this.map.dragRotate.disable();
    this.map.touchZoomRotate.disableRotation();
    this.setupWebGLContextMonitoring();
  }

  private readonly handleWebGLContextLost = (event: MapContextEvent): void => {
    // Ignore duplicate notifications for the same loss. The native side
    // should issue only one WebView reload attempt.
    if (this.webGLContextLostAt !== null) return;

    this.webGLContextLossCount++;
    this.webGLContextLostAt = Date.now();
    this.webGLContextLost = true;
    // MapLibre destroys the old style (and invokes custom-layer onRemove)
    // before emitting this event. Do not call remove() on that dead context.
    this.currentJourneyLayer = null;
    this.reportWebGLContextEvent("lost", event.originalEvent.statusMessage);
  };

  private readonly handleWebGLContextRestored = (
    event: MapContextEvent,
  ): void => {
    const elapsedMs =
      this.webGLContextLostAt === null
        ? undefined
        : Date.now() - this.webGLContextLostAt;

    this.webGLContextLostAt = null;
    this.webGLContextLost = false;

    // MapLibre fires this after creating the replacement painter, but before
    // its asynchronously restored style is guaranteed to accept custom
    // layers. The styledata handler remains the single rebuild point.
    this.currentJourneyLayer = null;

    // This is useful for browser diagnostics. The Flutter app reloads the
    // complete WebView immediately on context loss because MapLibre cannot
    // restore custom layers reliably.
    this.map.resize();
    this.map.triggerRepaint();
    this.reportWebGLContextEvent(
      "restored",
      event.originalEvent.statusMessage,
      elapsedMs,
    );
  };

  /**
   * Observe MapLibre's public context events. Flutter uses the lost event to
   * reload the complete WebView; restored remains useful in browser tooling
   * and when no Flutter channel is installed.
   */
  private setupWebGLContextMonitoring(): void {
    this.map.on("webglcontextlost", this.handleWebGLContextLost);
    this.map.on("webglcontextrestored", this.handleWebGLContextRestored);
  }

  private reportWebGLContextEvent(
    state: "lost" | "restored",
    statusMessage: string,
    elapsedMs?: number,
  ): void {
    const center = this.map.getCenter();
    const canvas = this.map.getCanvas();
    const payload = {
      state,
      timestamp: new Date().toISOString(),
      lossCount: this.webGLContextLossCount,
      elapsedMs,
      statusMessage: statusMessage || undefined,
      renderMode: this.params.renderMode,
      view: {
        lng: Number(center.lng.toFixed(6)),
        lat: Number(center.lat.toFixed(6)),
        zoom: Number(this.map.getZoom().toFixed(2)),
      },
      canvas: {
        width: canvas.width,
        height: canvas.height,
        clientWidth: canvas.clientWidth,
        clientHeight: canvas.clientHeight,
        devicePixelRatio: window.devicePixelRatio || 1,
      },
    };
    const message = JSON.stringify(payload);

    if (state === "lost") {
      console.warn(`[MapController] WebGL context lost: ${message}`);
    } else {
      console.log(`[MapController] WebGL context restored: ${message}`);
    }

    try {
      window.onWebGLContextEvent?.postMessage(message);
    } catch (error) {
      console.warn(
        "[MapController] Failed to report WebGL context event to Flutter:",
        error,
      );
    }
  }

  /**
   * Build the transform request function for Mapbox URL transformation
   */
  private buildTransformRequest(): RequestTransformFunction {
    if (this.params.requiresMapboxToken && this.params.accessKey) {
      return (url: string, resourceType?: ResourceType) => {
        if (isMapboxURL(url)) {
          // transformMapboxUrl expects ResourceType to be string, safe to cast
          return transformMapboxUrl(
            url,
            resourceType as any,
            this.params.accessKey!,
          );
        }
        return { url };
      };
    }

    return (url: string, _resourceType?: ResourceType) => {
      return { url };
    };
  }

  /**
   * Initialize the map controller
   * This sets up the tile provider, layers, and event handlers
   *
   * @returns Promise that resolves when initialization is complete
   */
  async initialize(): Promise<void> {
    return new Promise((resolve) => {
      this.map.on("load", async () => {
        // Create JourneyTileProvider (it registers its own hooks for renderMode)
        this.journeyTileProvider = new JourneyTileProvider(
          this.map,
          this.params,
        );

        // Initial tile buffer load
        await this.journeyTileProvider.waitForTileBufferUpdate();
        console.log("initial tile buffer loaded");

        // Register hooks for reactive property changes
        this.registerParamsHooks();

        // Create and initialize journey layer with selected rendering mode
        this.currentJourneyLayer = this.switchRenderingLayer();

        // Handle style changes to maintain journey layer position
        this.setupStyleDataHandler();

        // Set up polling for tile updates
        if (!this.DisableAutoRefresh) {
          this.pollIntervalId = setInterval(
            () => this.journeyTileProvider?.pollForJourneyUpdates(false),
            DATA_POLL_INTERVAL_MS,
          );
        }

        // Apply the actual map style (deferred until journey layer is added)
        this.applyMapStyle();

        // Set up retry logic for failed style loads
        this.setupStyleRetryLogic();

        // Workaround: WebView may report stale GL surface dimensions
        // when the app starts, causing the canvas to
        // initialize at the wrong size. This blocks until the canvas matches
        // the container, keeping the Flutter overlay visible during the fix.
        await this.ensureCorrectCanvasDimensions();

        resolve();
      });
    });
  }

  /**
   * Get the underlying MapLibre map instance
   */
  getMap(): MaplibreMap {
    return this.map;
  }

  /**
   * Get the ReactiveParams instance
   */
  getParams(): ReactiveParams {
    return this.params;
  }

  /**
   * Get the JourneyTileProvider instance
   */
  getTileProvider(): JourneyTileProvider | null {
    return this.journeyTileProvider;
  }

  /**
   * Disable periodic journey-data polling.
   *
   * This can be called before initialization to prevent the polling timer from
   * being created, or afterwards to stop an existing timer.
   */
  disableAutoRefresh(): void {
    this.DisableAutoRefresh = true;
    this.clearAutoRefreshInterval();
  }

  /**
   * Refresh map data by forcing a tile buffer update
   * This is called when the underlying data has changed (e.g., new journey data imported)
   * @returns Promise<boolean | null> - true if data was updated, false if no change, null on error
   */
  async refreshMapData(): Promise<boolean | null> {
    if (!this.journeyTileProvider) {
      console.warn(
        "[MapController] Cannot refresh: tile provider not initialized",
      );
      return null;
    }
    console.log("[MapController] Refreshing map data");
    return await this.journeyTileProvider.pollForJourneyUpdates(true);
  }

  /**
   * Switch between rendering layers based on current params.renderMode
   * This handles cleanup of the old layer and creation of a new one
   *
   * @returns The newly created journey layer instance
   */
  private switchRenderingLayer(): JourneyLayer | null {
    // MapLibre restores its style before it creates the replacement painter.
    // During that window, styledata and reactive hooks must not initialize a
    // custom layer against the context that has just been lost.
    if (this.webGLContextLost) {
      this.currentJourneyLayer = null;
      return null;
    }
    if (!this.journeyTileProvider || !this.hasUsableStyle()) {
      return null;
    }

    let renderingMode = this.params.renderMode;

    if (!AVAILABLE_LAYERS[renderingMode]) {
      console.warn(
        `Rendering mode '${renderingMode}' not available, using canvas instead.`,
      );
      renderingMode = "canvas";
      // Note: We don't update params.renderMode here to avoid recursive hook calls
    }

    // Clean up existing layer if present
    if (this.currentJourneyLayer) {
      this.currentJourneyLayer.remove();
    }

    // Resolve a private copy of the selected palette. Density is a runtime
    // override (debug panel), so update the RGBA value directly before passing
    // it to the layer; the canonical palette remains unchanged.
    const LayerClass = AVAILABLE_LAYERS[renderingMode].layerClass;
    const fogStyle = getFogStyle(this.params.fogStyle);
    fogStyle.rgba[3] = this.params.fogDensity;

    const newLayer = new LayerClass(
      this.map,
      this.journeyTileProvider!,
      undefined, // use default layerId
      fogStyle.rgba,
    );
    newLayer.initialize();

    this.currentJourneyLayer = newLayer;
    return newLayer;
  }

  /**
   * Register hooks on ReactiveParams to handle property changes
   * These hooks automatically respond to changes in rendering and map properties.
   */
  private registerParamsHooks(): void {
    // Hook for renderMode changes - switch rendering layer
    this.params.on("renderMode", (newMode, oldMode) => {
      console.log(
        `[MapController] renderMode changed: ${oldMode} -> ${newMode}`,
      );
      this.switchRenderingLayer();
    });

    // Recreate the layer when switching between light and dark fog palettes.
    this.params.on("fogStyle", (newStyle, oldStyle) => {
      console.log(
        `[MapController] fogStyle changed: ${oldStyle} -> ${newStyle}`,
      );
      this.switchRenderingLayer();
    });

    // Hook for fogDensity changes - recreate layer with new bgColor alpha
    this.params.on("fogDensity", (newDensity, oldDensity) => {
      console.log(
        `[MapController] fogDensity changed: ${oldDensity} -> ${newDensity}`,
      );
      this.switchRenderingLayer();
    });

    // Hook for projection changes - update map style with new projection
    this.params.on("projection", (newProjection, oldProjection) => {
      console.log(
        `[MapController] projection changed: ${oldProjection} -> ${newProjection}`,
      );
      this.map.setStyle(this.params.mapStyle, {
        transformStyle: (previousStyle: any, nextStyle: any) =>
          transformStyleWithProjection(
            previousStyle,
            nextStyle,
            newProjection as ProjectionType,
            this.mapLocale,
          ),
      });
    });
  }

  /**
   * Set up handler for styledata events to maintain journey layer position
   * After style reset, custom layers may need to be re-added or repositioned
   */
  private setupStyleDataHandler(): void {
    this.map.on("styledata", (_) => {
      console.log("styledata event received");
      if (this.webGLContextLost) {
        return;
      }

      const orderedLayerIds = this.map.getLayersOrder();

      // After style reset, layers may have different lifecycles:
      // 1. Style-spec layers get erased - need to re-add
      // 2. CustomLayerInterface layers stay but move to bottom - need to reorder

      const customIndex = orderedLayerIds.indexOf(JOURNEY_LAYER_ID);
      if (customIndex === -1) {
        console.log(`${JOURNEY_LAYER_ID} not found, adding to map`);
        this.switchRenderingLayer();
      } else if (customIndex !== orderedLayerIds.length - 1) {
        console.log(`${JOURNEY_LAYER_ID} is not frontmost, moving to front`);
        this.map.moveLayer(JOURNEY_LAYER_ID);
      }
    });
  }

  /**
   * Apply the map style with projection transform
   */
  private applyMapStyle(): void {
    this.map.setStyle(this.params.mapStyle, {
      transformStyle: (previousStyle: any, nextStyle: any) =>
        transformStyleWithProjection(
          previousStyle,
          nextStyle,
          this.params.projection,
          this.mapLocale,
        ),
    });
  }

  /**
   * Set up retry logic for failed style loads
   * This handles cases where network access fails (e.g., mainland China iPhones)
   */
  private setupStyleRetryLogic(): void {
    this.styleRetryIntervalId = setInterval(() => {
      if (this.webGLContextLost) {
        return;
      }

      const layerCount = this.map.getLayersOrder().length;
      if (layerCount <= 1) {
        console.log("Re-attempting to load map style");
        this.applyMapStyle();
      }
    }, 8 * 1000);
  }

  /**
   * Workaround for WebView canvas size bug when app starts.
   * The GL surface may report stale dimensions, resulting in
   * a wrongly-sized canvas (e.g. 1100x825 instead of 1080x2400).
   *
   * Uses a rAF loop to compare actual canvas backing-store dimensions
   * against expected (container size * devicePixelRatio). Calls resize()
   * each frame until they match or a max retry count is reached.
   * The returned promise blocks initialize() so the Flutter overlay stays
   * visible during correction — no visual glitch.
   */
  private ensureCorrectCanvasDimensions(): Promise<void> {
    return new Promise((resolve) => {
      const MAX_ATTEMPTS = 60; // ~1s at 60fps
      let attempts = 0;

      const check = () => {
        const container = this.map.getContainer();
        const canvas = this.map.getCanvas();
        const dpr = window.devicePixelRatio || 1;
        const expectedW = Math.round(container.clientWidth * dpr);
        const expectedH = Math.round(container.clientHeight * dpr);

        // ±1px tolerance: MapLibre may floor/truncate the DPR scaling
        const wOk = Math.abs(canvas.width - expectedW) <= 1;
        const hOk = Math.abs(canvas.height - expectedH) <= 1;

        if (!wOk || !hOk) {
          attempts++;
          this.map.resize();

          if (attempts < MAX_ATTEMPTS) {
            requestAnimationFrame(check);
            return;
          }
          console.warn(
            `[MapController] Canvas dimensions still incorrect after ${MAX_ATTEMPTS} attempts: ` +
              `expected ${expectedW}x${expectedH}, got ${canvas.width}x${canvas.height}`,
          );
        } else if (attempts > 0) {
          console.log(
            `[MapController] Canvas dimensions corrected after ${attempts} resize(s)`,
          );
        } else {
          console.log("[MapController] Canvas dimensions are correct");
        }
        resolve();
      };

      requestAnimationFrame(check);
    });
  }

  /**
   * Clean up resources when the controller is destroyed
   */
  destroy(): void {
    this.map.off("webglcontextlost", this.handleWebGLContextLost);
    this.map.off("webglcontextrestored", this.handleWebGLContextRestored);
    if (this.styleRetryIntervalId) {
      clearInterval(this.styleRetryIntervalId);
      this.styleRetryIntervalId = null;
    }
    this.clearAutoRefreshInterval();
    if (this.currentJourneyLayer) {
      this.currentJourneyLayer.remove();
      this.currentJourneyLayer = null;
    }
    this.journeyTileProvider?.dispose();
    this.journeyTileProvider = null;
    this.map.remove();
  }

  private hasUsableStyle(): boolean {
    const style = this.map.getStyle() as
      { layers?: Array<{ id: string }> } | undefined;
    return Array.isArray(style?.layers) && style.layers.length > 0;
  }

  private clearAutoRefreshInterval(): void {
    if (this.pollIntervalId !== null) {
      clearInterval(this.pollIntervalId);
      this.pollIntervalId = null;
    }
  }
}
