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

import maplibregl from "maplibre-gl";
import type {
  Map as MaplibreMap,
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
import { transformStyleWithProjection } from "./utils";
import { JOURNEY_LAYER_ID } from "./layers/journey-layer-interface";
import type { JourneyLayer } from "./layers/journey-layer-interface";

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
  disableAutoRender?: boolean;
}

/**
 * MapController manages the MapLibre map instance and related components
 */
export class MapController {
  private map: MaplibreMap;
  private params: ReactiveParams;
  private disableAutoRender: boolean;
  private currentJourneyLayer: JourneyLayer | null = null;
  private journeyTileProvider: JourneyTileProvider | null = null;
  private styleRetryIntervalId: ReturnType<typeof setInterval> | null = null;
  private pollIntervalId: ReturnType<typeof setInterval> | null = null;

  constructor(config: MapControllerConfig) {
    this.params = config.params;
    this.disableAutoRender = config.disableAutoRender ?? false;

    // Build transform request function for Mapbox styles
    const transformRequest = this.buildTransformRequest();

    // Create the map instance
    this.map = new maplibregl.Map({
      container: config.containerId,
      center: [this.params.lng, this.params.lat],
      zoom: this.params.zoom,
      maxZoom: 14,
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
        await this.journeyTileProvider.pollForJourneyUpdates(true);
        console.log("initial tile buffer loaded");

        // Register hooks for reactive property changes
        this.registerParamsHooks();

        // Create and initialize journey layer with selected rendering mode
        this.currentJourneyLayer = this.switchRenderingLayer();

        // Handle style changes to maintain journey layer position
        this.setupStyleDataHandler();

        // Set up polling for tile updates
        if (!this.disableAutoRender) {
          this.pollIntervalId = setInterval(
            () => this.journeyTileProvider?.pollForJourneyUpdates(false),
            1000,
          );
        }

        // Apply the actual map style (deferred until journey layer is added)
        this.applyMapStyle();

        // Set up retry logic for failed style loads
        this.setupStyleRetryLogic();

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
   * Get the JourneyTileProvider instance
   */
  getTileProvider(): JourneyTileProvider | null {
    return this.journeyTileProvider;
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
  private switchRenderingLayer(): JourneyLayer {
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

    // Create new layer instance
    const LayerClass = AVAILABLE_LAYERS[renderingMode].layerClass;
    // Use fogDensity as the alpha value for bgColor
    const bgColor: [number, number, number, number] = [
      0.0,
      0.0,
      0.0,
      this.params.fogDensity,
    ];

    const newLayer = new LayerClass(
      this.map,
      this.journeyTileProvider!,
      undefined, // use default layerId
      bgColor,
    );
    newLayer.initialize();

    this.currentJourneyLayer = newLayer;
    return newLayer;
  }

  /**
   * Register hooks on ReactiveParams to handle property changes
   * These hooks automatically respond to changes in renderMode, fogDensity, and projection
   */
  private registerParamsHooks(): void {
    // Hook for renderMode changes - switch rendering layer
    this.params.on("renderMode", (newMode, oldMode) => {
      console.log(
        `[MapController] renderMode changed: ${oldMode} -> ${newMode}`,
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
        ),
    });
  }

  /**
   * Set up retry logic for failed style loads
   * This handles cases where network access fails (e.g., mainland China iPhones)
   */
  private setupStyleRetryLogic(): void {
    this.styleRetryIntervalId = setInterval(() => {
      const layerCount = this.map.getLayersOrder().length;
      if (layerCount <= 1) {
        console.log("Re-attempting to load map style");
        this.applyMapStyle();
      }
    }, 8 * 1000);
  }

  /**
   * Clean up resources when the controller is destroyed
   */
  destroy(): void {
    if (this.styleRetryIntervalId) {
      clearInterval(this.styleRetryIntervalId);
      this.styleRetryIntervalId = null;
    }
    if (this.pollIntervalId) {
      clearInterval(this.pollIntervalId);
      this.pollIntervalId = null;
    }
    if (this.currentJourneyLayer) {
      this.currentJourneyLayer.remove();
      this.currentJourneyLayer = null;
    }
    this.map.remove();
  }
}
