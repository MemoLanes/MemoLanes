import { JourneyCanvasLayer } from "./layers/journey-canvas-layer";
import { JourneyTileProvider } from "./journey-tile-provider";
import { DebugPanel } from "./debug-panel";
import init from "../pkg/index.js";
import maplibregl from "maplibre-gl";
import type {
  Map as MaplibreMap,
  Marker,
  RequestTransformFunction,
  ResourceType,
} from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import {
  isMapboxURL,
  transformMapboxUrl,
} from "maplibregl-mapbox-request-transformer";
import { parseUrlHash, parseAndValidateParams } from "./params";
import { FlutterBridge, notifyFlutterReady } from "./flutter-bridge";
import { ensurePlatformCompatibility } from "./platform";
import { transformStyle, displayPageMessage } from "./utils";
import type {
  JourneyLayer,
  JourneyLayerConstructor,
} from "./layers/journey-layer-interface";

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

// Type definitions for rendering layers
interface LayerConfig {
  name: string;
  layerClass: JourneyLayerConstructor;
  bufferSizePower: number;
  description: string;
}

interface AvailableLayers {
  canvas: LayerConfig;
  [key: string]: LayerConfig;
}

// Available rendering layers
const AVAILABLE_LAYERS: AvailableLayers = {
  canvas: {
    name: "Canvas",
    layerClass: JourneyCanvasLayer,
    bufferSizePower: 8,
    description: "Uses Canvas API for rendering",
  }
};

// Global state variables
let currentJourneyLayer: JourneyLayer | null = null; // Store reference to current layer
let currentJourneyTileProvider: JourneyTileProvider;
let locationMarker: Marker | null = null;

/**
 * Function to switch between rendering layers
 * @param map - MapLibre map instance
 * @param params - Validated params object containing render mode
 * @returns The newly created journey layer instance
 */
function switchRenderingLayer(map: MaplibreMap, params: any): JourneyLayer {
  let renderingMode = params.renderMode;
  
  if (!AVAILABLE_LAYERS[renderingMode]) {
    console.warn(
      `Rendering mode '${renderingMode}' not available, using canvas instead.`,
    );
    renderingMode = "canvas";
    params.renderMode = renderingMode; // Update params with fallback
  }

  // Clean up existing layer if present
  if (currentJourneyLayer) {
    currentJourneyLayer.remove();
  }

  // Create new layer instance
  const LayerClass = AVAILABLE_LAYERS[renderingMode].layerClass;
  const bufferSizePower = AVAILABLE_LAYERS[renderingMode].bufferSizePower;

  currentJourneyTileProvider.setBufferSizePower(bufferSizePower);
  currentJourneyLayer = new LayerClass(map, currentJourneyTileProvider);
  currentJourneyLayer.initialize();
  
  return currentJourneyLayer;
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

  // Validate and parse parameters
  const validationResult = parseAndValidateParams(
    window.EXTERNAL_PARAMS,
    AVAILABLE_LAYERS,
  );

  // Handle validation errors
  if (validationResult.type === "error") {
    if (validationResult.detail === "cgi_endpoint parameter is required") {
      // No hash param and no default endpoint, stop setting up and waiting for next setup
      return;
    }

    // Display error message
    displayPageMessage(validationResult.message, validationResult.detail);
    notifyFlutterReady();
    return;
  }

  // Extract validated parameters
  const params = validationResult.params;

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
      layers: [],
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
    // Create a DOM element for the marker
    const el = document.createElement("div");
    el.className = "location-marker";

    // Create the marker but don't add it to the map yet
    locationMarker = new maplibregl.Marker({
      element: el,
    });

    currentJourneyTileProvider = new JourneyTileProvider(
      map,
      params,
      AVAILABLE_LAYERS[params.renderMode].bufferSizePower,
    );

    await currentJourneyTileProvider.pollForJourneyUpdates(true);
    console.log("initial tile buffer loaded");

    // Create and initialize journey layer with selected rendering mode
    currentJourneyLayer = switchRenderingLayer(map, params);
    map.on("styledata", () => {
      console.log("styledata event received");
      const orderedLayerIds = map.getLayersOrder();
      const customIndex = orderedLayerIds.indexOf("memolanes-journey-layer");
      if (customIndex === -1) {
        currentJourneyLayer = switchRenderingLayer(map, params);
      } else if (
        customIndex !== -1 &&
        customIndex !== orderedLayerIds.length - 1
      ) {
        console.log(
          "memolanes-journey-layer is not the most front one, move it to the front",
        );
        map.moveLayer("memolanes-journey-layer");
      }
    });

    // Set up polling for updates
    setInterval(
      () => currentJourneyTileProvider.pollForJourneyUpdates(false),
      1000,
    );

    // Create and initialize the debug panel
    const debugPanel = new DebugPanel(map, AVAILABLE_LAYERS);
    debugPanel.initialize();
    debugPanel.listenForHashChanges();

    // Initialize Flutter bridge
    const flutterBridge = new FlutterBridge({
      map,
      locationMarker: locationMarker!,
      journeyTileProvider: currentJourneyTileProvider,
      switchRenderingLayerFn: (map: any) => switchRenderingLayer(map, params),
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

  // Listen for hash changes
  window.addEventListener("hashchange", () => {
    const hash = window.location.hash.slice(1);
    const params = new URLSearchParams(hash);

    // Check if journey ID has changed
    const newJourneyId = params.get("journey_id");
    if (newJourneyId !== currentJourneyId && newJourneyId !== null) {
      currentJourneyId = newJourneyId;
      // TODO: fix this.
      // @ts-ignore - accessing private property for compatibility, should be refactored to use a public setter
      currentJourneyTileProvider.journeyId = currentJourneyId;
      currentJourneyTileProvider.pollForJourneyUpdates(true);
    }

    // Check if rendering mode has changed
    const newRenderMode = params.get("render");
    if (
      newRenderMode &&
      newRenderMode !== currentRenderingMode &&
      AVAILABLE_LAYERS[newRenderMode]
    ) {
      switchRenderingLayer(map, newRenderMode);
    }
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
