import { JourneyTileProvider } from "./journey-tile-provider";
import { DebugPanel } from "./debug-panel";
import init from "../pkg/index.js";
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
  parseUrlHash,
  createReactiveParams,
  AVAILABLE_LAYERS,
  ReactiveParams,
} from "./params";
import { FlutterBridge, notifyFlutterReady } from "./flutter-bridge";
import { ensurePlatformCompatibility } from "./platform";
import { transformStyle, displayPageMessage } from "./utils";
import { JOURNEY_LAYER_ID } from "./layers/journey-layer-interface";
import type { JourneyLayer } from "./layers/journey-layer-interface";

import "./debug-panel.css";

// Extend Window interface for custom properties
declare global {
  interface Window {
    SETUP_PENDING: boolean;
    EXTERNAL_PARAMS: {
      [key: string]: any;
      cgi_endpoint?: string;
    };
    trySetup?: () => Promise<void>;
  }
}

// Initialize window properties
window.SETUP_PENDING = false;
window.EXTERNAL_PARAMS = {};

// Global state variables
let currentJourneyLayer: JourneyLayer | null = null; // Store reference to current layer
let currentJourneyTileProvider: JourneyTileProvider;

/**
 * Function to switch between rendering layers
 * This function handles the actual layer switching logic.
 * It is called automatically when params.renderMode changes (via hook).
 *
 * @param map - MapLibre map instance
 * @param params - ReactiveParams instance containing render mode
 * @returns The newly created journey layer instance
 */
function switchRenderingLayer(
  map: MaplibreMap,
  params: ReactiveParams,
): JourneyLayer {
  let renderingMode = params.renderMode;

  if (!AVAILABLE_LAYERS[renderingMode]) {
    console.warn(
      `Rendering mode '${renderingMode}' not available, using canvas instead.`,
    );
    renderingMode = "canvas";
    // Note: We don't update params.renderMode here to avoid recursive hook calls
    // The fallback is just for this rendering operation
  }

  // Clean up existing layer if present
  if (currentJourneyLayer) {
    currentJourneyLayer.remove();
  }

  // Create new layer instance
  // Note: bufferSizePower is automatically updated by JourneyTileProvider
  // when it receives the renderMode change via its own hook
  const LayerClass = AVAILABLE_LAYERS[renderingMode].layerClass;
  currentJourneyLayer = new LayerClass(map, currentJourneyTileProvider);
  currentJourneyLayer.initialize();

  return currentJourneyLayer;
}

/**
 * Register hooks on ReactiveParams to handle property changes
 * This is the central place where we wire up reactive behaviors for the map/layer.
 *
 * Note: JourneyTileProvider registers its own hooks for renderMode (bufferSizePower)
 * and journeyId (pollForJourneyUpdates) internally.
 *
 * @param map - MapLibre map instance
 * @param params - ReactiveParams instance to register hooks on
 */
function registerParamsHooks(map: MaplibreMap, params: ReactiveParams): void {
  // Hook for renderMode changes
  // When renderMode changes, automatically switch the rendering layer
  params.on("renderMode", (newMode, oldMode) => {
    console.log(
      `[ReactiveParams] renderMode changed: ${oldMode} -> ${newMode}`,
    );
    switchRenderingLayer(map, params);
  });
}

/**
 * Try to setup and initialize the map with given parameters
 */
async function trySetup(): Promise<void> {
  // Parse URL hash if EXTERNAL_PARAMS is empty
  if (Object.keys(window.EXTERNAL_PARAMS).length === 0) {
    window.EXTERNAL_PARAMS = parseUrlHash();
  }

  console.log(`parse external params`, window.EXTERNAL_PARAMS);

  // Create ReactiveParams from external parameters
  // Returns null if cgi_endpoint is not available yet (waiting for Flutter)
  // Throws error for other validation failures
  let params: ReactiveParams;
  try {
    const result = createReactiveParams(window.EXTERNAL_PARAMS);
    if (result === null) {
      // No cgi_endpoint yet, stop setting up and wait for next setup call
      return;
    }
    params = result;
  } catch (error) {
    // Display error message for validation failures
    const errorMessage = error instanceof Error ? error.message : String(error);
    displayPageMessage("Configuration Error", errorMessage);
    notifyFlutterReady();
    return;
  }

  let transformRequest: RequestTransformFunction = (
    url: string,
    _resourceType?: ResourceType,
  ) => {
    return { url };
  };

  // Configure transform request for Mapbox styles
  if (params.requiresMapboxToken && params.accessKey) {
    transformRequest = (url: string, resourceType?: ResourceType) => {
      if (isMapboxURL(url)) {
        // transformMapboxUrl expects ResourceType to be string, safe to cast
        return transformMapboxUrl(url, resourceType as any, params.accessKey!);
      }
      return { url };
    };
  }

  console.log(
    `journey_id: ${params.journeyId}, render: ${params.renderMode}, lng: ${params.lng}, lat: ${params.lat}, zoom: ${params.zoom}`,
  );
  console.log(
    "EXTERNAL_PARAMS for endpoint configuration:",
    window.EXTERNAL_PARAMS,
  );

  const map = new maplibregl.Map({
    container: "map",
    center: [params.lng, params.lat],
    zoom: params.zoom,
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
      // TODO: use the projection mode passed in params later.
      projection: { type: "globe" },
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

  map.dragRotate.disable();
  map.touchZoomRotate.disableRotation();

  map.on("load", async () => {
    // JourneyTileProvider automatically gets bufferSizePower from params.renderMode
    // and updates it when renderMode changes via its own hook
    currentJourneyTileProvider = new JourneyTileProvider(map, params);

    await currentJourneyTileProvider.pollForJourneyUpdates(true);
    console.log("initial tile buffer loaded");

    // Register hooks BEFORE creating the initial layer
    // This ensures all future changes are automatically handled
    registerParamsHooks(map, params);

    // Create and initialize journey layer with selected rendering mode
    // Note: This is the initial setup, subsequent changes go through the hook
    currentJourneyLayer = switchRenderingLayer(map, params);

    map.on("styledata", (_) => {
      console.log("styledata event received");
      const orderedLayerIds = map.getLayersOrder();

      // after style reset, the previous layer may have different lifecycles:
      // 1. for custom layers following the style spec, they will be erased so
      //    we need to add them back.
      // 2. for custom layers following the CustomLayerInterface, they will be kept
      //    in the bottom of the layer stack so we need to move them to the top.

      // console.log("orderedLayerIds:", orderedLayerIds);
      const customIndex = orderedLayerIds.indexOf(JOURNEY_LAYER_ID);
      if (customIndex === -1) {
        console.log(`${JOURNEY_LAYER_ID} not found, add it into the map`);
        currentJourneyLayer = switchRenderingLayer(map, params);
      } else if (
        customIndex !== -1 &&
        customIndex !== orderedLayerIds.length - 1
      ) {
        console.log(
          `${JOURNEY_LAYER_ID} is not the most front one, move it to the front`,
        );
        map.moveLayer(JOURNEY_LAYER_ID);
      }
    });

    // Set up polling for updates
    setInterval(
      () => currentJourneyTileProvider.pollForJourneyUpdates(false),
      1000,
    );

    // Create and initialize the debug panel
    const debugPanel = new DebugPanel(map, params);
    debugPanel.initialize();

    // Initialize Flutter bridge
    // FlutterBridge manages its own locationMarker internally
    // and uses params for reactive property updates
    const flutterBridge = new FlutterBridge({
      map,
      params,
    });
    flutterBridge.initialize();

    // give the map a little time to render before notifying Flutter
    setTimeout(() => {
      notifyFlutterReady();
    }, 200);

    // defer the map style initialization after memolanes layer added.
    map.setStyle(params.mapStyle, {
      transformStyle: transformStyle,
    });

    // In case mapbox completely fails to load (i.e. app running on mainland China
    // iPhone does not have network access by default)
    setInterval(() => {
      const layerCount = map.getLayersOrder().length;
      if (layerCount <= 1) {
        console.log("Re-attempt to load map style");
        map.setStyle(params.mapStyle, {
          transformStyle: transformStyle,
        });
      }
    }, 8 * 1000);
  });
}

// Export trySetup to window for Flutter to call
window.trySetup = trySetup;

try {
  ensurePlatformCompatibility();
} catch (error) {
  // Display error message on the webpage
  const errorMessage = error instanceof Error ? error.message : String(error);
  displayPageMessage("Platform Compatibility Error", errorMessage);

  // Notify Flutter even on error so app can handle the error state
  notifyFlutterReady();
  throw error;
}

// Ensure WASM module is initialized before using its exports downstream
init()
  .then(() => {
    console.log("WASM module initialized");

    trySetup().catch(console.error);

    window.SETUP_PENDING = true;
  })
  .catch(console.error);
