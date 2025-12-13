{
  window.SETUP_PENDING = false;
  window.EXTERNAL_PARAMS = {};
}

import { JourneyCanvasLayer } from "./layers/JourneyCanvasLayer";
import { JourneyTileProvider } from "./JourneyTileProvider";
import { DebugPanel } from "./DebugPanel";
import init from "../pkg/index.js";
import maplibregl from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import {
  isMapboxURL,
  transformMapboxUrl,
  transformMapboxStyle,
} from "maplibregl-mapbox-request-transformer";
import { disableMagnifierIfIOS } from "./utils";
import { parseUrlHash, parseAndValidateParams } from "./params";

import "./debug-panel.css";

// Available rendering layers
const AVAILABLE_LAYERS = {
  canvas: {
    name: "Canvas",
    layerClass: JourneyCanvasLayer,
    bufferSizePower: 8,
    description: "Uses Canvas API for rendering",
  },
};

let currentJourneyLayer; // Store reference to current layer
let currentJourneyId;
let currentJourneyTileProvider;
let pollingInterval; // Store reference to polling interval
let locationMarker = null;
let currentRenderingMode = "canvas"; // Default rendering mode
// TODO: This default is only used for testing with `rust/examples`, which is
// a little weird.
let currentMapStyle = "https://tiles.openfreemap.org/styles/liberty";
let transformRequest = (url, resourceType) => {
  return { url };
};

// Function to switch between rendering layers
function switchRenderingLayer(map, renderingMode) {
  if (!AVAILABLE_LAYERS[renderingMode]) {
    console.warn(
      `Rendering mode '${renderingMode}' not available, using canvas instead.`,
    );
    renderingMode = "canvas";
  }

  // Clean up existing layer if present
  if (currentJourneyLayer) {
    currentJourneyLayer.remove && currentJourneyLayer.remove();
  }

  // Create new layer instance
  const LayerClass = AVAILABLE_LAYERS[renderingMode].layerClass;
  const bufferSizePower = AVAILABLE_LAYERS[renderingMode].bufferSizePower;

  currentJourneyTileProvider.setBufferSizePower(bufferSizePower);
  currentJourneyLayer = new LayerClass(map, currentJourneyTileProvider);
  currentJourneyLayer.initialize();

  currentRenderingMode = renderingMode;
  return currentJourneyLayer;
}

function tryNotifyFlutterReady() {
  if (window.readyForDisplay) {
    window.readyForDisplay.postMessage("");
  }
}

// Function to check WebView version compatibility for Android and iOS
function checkWebViewVersion() {
  const ua = navigator.userAgent;

  // Check if it's Android
  const isAndroid = /Android/i.test(ua);
  if (isAndroid) {
    // Extract Chrome version (WebView uses Chrome version)
    const chromeMatch = ua.match(/Chrome\/(\d+)\.(\d+)\.(\d+)/);
    if (!chromeMatch) {
      return { compatible: true }; // Can't determine version, allow to proceed
    }

    const majorVersion = parseInt(chromeMatch[1], 10);

    // Check if version is greater or equal to 96
    if (majorVersion <= 95) {
      return {
        compatible: false,
        message: "The system's WebView version is not compatible",
        detail:
          "Please update your Android System WebView to version 96 or higher.",
      };
    }

    return { compatible: true };
  }

  // Check if it's iOS
  const isIOS = /iPhone|iPad|iPod/i.test(ua);
  if (isIOS) {
    // Extract iOS version (format: "OS 15_1" or "OS 15_1_0")
    const iosMatch = ua.match(/OS (\d+)_(\d+)(?:_(\d+))?/);
    if (!iosMatch) {
      return { compatible: true }; // Can't determine version, allow to proceed
    }

    const majorVersion = parseInt(iosMatch[1], 10);
    const minorVersion = parseInt(iosMatch[2], 10);

    // Check if version is greater than or equal to 15.1
    if (majorVersion < 15 || (majorVersion === 15 && minorVersion < 1)) {
      return {
        compatible: false,
        message: "The system's iOS version is not compatible",
        detail: "Please update your iOS to version 15.1 or higher.",
      };
    }

    return { compatible: true };
  }

  // Not Android or iOS, no check needed
  return { compatible: true };
}

async function trySetup() {
  // Parse URL hash if EXTERNAL_PARAMS is empty
  if (Object.keys(window.EXTERNAL_PARAMS).length === 0) {
    window.EXTERNAL_PARAMS = parseUrlHash();
  }

  console.log(`parse external params`, window.EXTERNAL_PARAMS);

  // Validate and parse parameters
  const validationResult = parseAndValidateParams(
    window.EXTERNAL_PARAMS,
    AVAILABLE_LAYERS,
    currentMapStyle,
    currentRenderingMode,
  );

  // Handle validation errors
  if (validationResult.type === "error") {
    if (validationResult.detail === "cgi_endpoint parameter is required") {
      // No hash param and no default endpoint, stop setting up and waiting for next setup
      return;
    }

    // Display error message
    document.body.innerHTML = `<div style="padding: 20px; font-family: Arial, sans-serif; color: red;"><h1>${validationResult.message}</h1>${validationResult.detail ? `<p>${validationResult.detail}</p>` : ""}</div>`;
    tryNotifyFlutterReady();
    return;
  }

  // Extract validated parameters
  const params = validationResult.params;
  currentJourneyId = params.journeyId;
  currentRenderingMode = params.renderMode;
  currentMapStyle = params.mapStyle;

  // Configure transform request for Mapbox styles
  if (params.requiresMapboxToken && params.accessKey) {
    transformRequest = (url, resourceType) => {
      if (isMapboxURL(url)) {
        return transformMapboxUrl(url, resourceType, params.accessKey);
      }
      return { url };
    };
  }

  console.log(
    `journey_id: ${currentJourneyId}, render: ${currentRenderingMode}, lng: ${params.lng}, lat: ${params.lat}, zoom: ${params.zoom}`,
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

  map.on("load", async (e) => {
    // Create a DOM element for the marker
    const el = document.createElement("div");
    el.className = "location-marker";

    // Create the marker but don't add it to the map yet
    // locationMarker = new maplibregl.Marker(el);
    locationMarker = new maplibregl.Marker({
      element: el,
    });

    // Add method to window object to update marker position
    window.updateLocationMarker = function (
      lng,
      lat,
      show = true,
      flyto = false,
    ) {
      if (show) {
        locationMarker.setLngLat([lng, lat]).addTo(map);
        if (flyto) {
          const currentZoom = map.getZoom();
          map.flyTo({
            center: [lng, lat],
            zoom: currentZoom < 14 ? 16 : currentZoom,
            essential: true,
          });
        }
      } else {
        locationMarker.remove();
      }
    };

    currentJourneyTileProvider = new JourneyTileProvider(
      map,
      currentJourneyId,
      AVAILABLE_LAYERS[currentRenderingMode].bufferSizePower,
    );

    await currentJourneyTileProvider.pollForJourneyUpdates(true);
    console.log("initial tile buffer loaded");

    // Create and initialize journey layer with selected rendering mode
    currentJourneyLayer = switchRenderingLayer(map, currentRenderingMode);
    map.on("styledata", () => {
      console.log("styledata event received");
      const orderedLayerIds = map.getLayersOrder();
      const customIndex = orderedLayerIds.indexOf("memolanes-journey-layer");
      if (customIndex === -1) {
        currentJourneyLayer = switchRenderingLayer(map, currentRenderingMode);
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
    pollingInterval = setInterval(
      () => currentJourneyTileProvider.pollForJourneyUpdates(false),
      1000,
    );

    // Create and initialize the debug panel
    const debugPanel = new DebugPanel(map, AVAILABLE_LAYERS);
    debugPanel.initialize();
    debugPanel.listenForHashChanges();

    // give the map a little time to render before notifying Flutter
    setTimeout(() => {
      tryNotifyFlutterReady();
    }, 200);

    // defer the map style initialization after memolanes layer added.
    map.setStyle(currentMapStyle, {
      transformStyle: transformMapboxStyle,
    });

    // In case mapbox completely fails to load (i.e. app running on mainland China
    // iPhone does not have network access by default)
    setInterval(() => {
      const layerCount = map.getLayersOrder().length;
      if (layerCount <= 1) {
        console.log("Re-attempt to load map style");
        map.setStyle(currentMapStyle, {
          transformStyle: transformMapboxStyle,
        });
      }
    }, 8 * 1000);
  });

  // Replace the simple movestart listener with dragstart
  map.on("dragstart", () => {
    // Only notify Flutter when user drags the map
    if (window.onMapMoved) {
      window.onMapMoved.postMessage("");
    }
  });

  // Listen for zoom changes
  map.on("zoomstart", (event) => {
    let fromUser = event.originalEvent && event.originalEvent.type !== "resize";
    if (fromUser && window.onMapMoved) {
      window.onMapMoved.postMessage("");
    }
  });

  // Add method to window object to get current map view
  window.getCurrentMapView = function () {
    const center = map.getCenter();
    return JSON.stringify({
      lng: center.lng,
      lat: center.lat,
      zoom: map.getZoom(),
    });
  };

  // Listen for hash changes
  window.addEventListener("hashchange", () => {
    const hash = window.location.hash.slice(1);
    const params = new URLSearchParams(hash);

    // Check if journey ID has changed
    const newJourneyId = params.get("journey_id");
    if (newJourneyId !== currentJourneyId && newJourneyId !== null) {
      currentJourneyId = newJourneyId;
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

  // Add method to window object to trigger manual update
  window.triggerJourneyUpdate = function () {
    return currentJourneyTileProvider.pollForJourneyUpdates(false);
  };

  // Add method to switch rendering layers
  window.switchRenderingLayer = function (renderingMode) {
    return switchRenderingLayer(map, renderingMode);
  };

  // Add method to update journey ID
  window.updateJourneyId = function (newJourneyId) {
    if (!newJourneyId) {
      console.warn("updateJourneyId: journey ID cannot be empty");
      return false;
    }

    if (newJourneyId === currentJourneyId) {
      console.log(
        `updateJourneyId: journey ID is already set to '${newJourneyId}'`,
      );
      return false;
    }

    console.log(
      `updateJourneyId: switching from '${currentJourneyId}' to '${newJourneyId}'`,
    );

    // Update the current journey ID
    currentJourneyId = newJourneyId;

    // Update the tile provider's journey ID
    if (currentJourneyTileProvider) {
      currentJourneyTileProvider.journeyId = currentJourneyId;
      // Force update to fetch data for the new journey
      currentJourneyTileProvider.pollForJourneyUpdates(true);
    }

    return true;
  };
}

window.trySetup = trySetup;

// Check WebView version compatibility for Android and iOS (before wasm initialization)
const versionCheck = checkWebViewVersion();
if (!versionCheck.compatible) {
  document.body.innerHTML = `<div style="padding: 20px; font-family: Arial, sans-serif; color: red;"><h1>${versionCheck.message}</h1><p>${versionCheck.detail}</p></div>`;
  tryNotifyFlutterReady();
  throw new Error("Incompatible WebView version - stopping JS execution.");
}

disableMagnifierIfIOS();

// Ensure WASM module is initialized before using its exports downstream
init()
  .then(() => {
    console.log("WASM module initialized");

    trySetup().catch(console.error);

    window.SETUP_PENDING = true;
  })
  .catch(console.error);
