{
  window.SETUP_PENDING = false;
  window.EXTERNAL_PARAMS = {};

  const hash = window.location.hash.slice(1);
  if (hash) {
    // default cgiEndpoint (only available if hash parameters are provided)
    window.EXTERNAL_PARAMS.cgi_endpoint = ".";

    const params = new URLSearchParams(hash);

    // Scan all hash parameters and store them in EXTERNAL_PARAMS after successful decoding
    // Supported parameters for endpoint configuration:
    // - cgi_endpoint: HTTP endpoint URL, "flutter://<channel>" for IPC mode, or "flutter" for legacy IPC
    // - http_endpoint: Explicit HTTP endpoint (alternative to cgi_endpoint)
    // Other parameters: journey_id, access_key, lng, lat, zoom, render, etc.
    for (const [key, value] of params.entries()) {
      if (value) {
        try {
          const decodedValue = decodeURIComponent(value);
          window.EXTERNAL_PARAMS[key] = decodedValue;
        } catch (error) {
          console.warn(`Failed to decode parameter '${key}': ${error.message}`);
          // Skip this parameter if decoding fails
        }
      }
    }
  }
}

import { JourneyCanvasLayer } from "./journey-canvas-layer.js";
import { JourneyTileProvider } from "./journey-tile-provider.js";
import { DebugPanel } from "./debug-panel.js";
import init from "../pkg/index.js";
import maplibregl from 'maplibre-gl';
import 'maplibre-gl/dist/maplibre-gl.css';
import {
  isMapboxURL,
  transformMapboxUrl,
  transformMapboxStyle
} from 'maplibregl-mapbox-request-transformer'

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
let currentMapStyle = "https://tiles.openfreemap.org/styles/liberty";
let transformRequest = (url, resourceType) => {
  return { url }
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

async function trySetup() {
  console.log(`parse external params`, window.EXTERNAL_PARAMS);
  if (!window.EXTERNAL_PARAMS.cgi_endpoint) {
    // no hash param and no default endpoint, stop setting up and waiting for next setup
    return;
  }

  if (window.EXTERNAL_PARAMS.map_style) {
    currentMapStyle = window.EXTERNAL_PARAMS.map_style;

    if (typeof currentMapStyle === "string" && currentMapStyle.startsWith("mapbox://")) {
      if (window.EXTERNAL_PARAMS.access_key) {
        transformRequest = (url, resourceType) => {
          if (isMapboxURL(url)) {
            return transformMapboxUrl(url, resourceType, window.EXTERNAL_PARAMS.access_key)
          }
          return { url }
        }
      } else {
        document.body.innerHTML = `<div style="padding: 20px; font-family: Arial, sans-serif; color: red;"><h1>TOKEN not provided</h1></div>`;
        return;
      }
    }
  }

  // Check if journey_id is provided
  if (!window.EXTERNAL_PARAMS.journey_id) {
    document.body.innerHTML = `<div style="padding: 20px; font-family: Arial, sans-serif; color: red;"><h1>Journey ID not provided</h1></div>`;
    return;
  }

  // Get journey ID from EXTERNAL_PARAMS
  currentJourneyId = window.EXTERNAL_PARAMS.journey_id;

  // Get rendering mode from EXTERNAL_PARAMS
  if (
    window.EXTERNAL_PARAMS.render &&
    AVAILABLE_LAYERS[window.EXTERNAL_PARAMS.render]
  ) {
    currentRenderingMode = window.EXTERNAL_PARAMS.render;
  }

  // Parse coordinates and zoom from EXTERNAL_PARAMS with fallbacks
  const lng = window.EXTERNAL_PARAMS.lng
    ? isNaN(parseFloat(window.EXTERNAL_PARAMS.lng))
      ? 0
      : parseFloat(window.EXTERNAL_PARAMS.lng)
    : 0;
  const lat = window.EXTERNAL_PARAMS.lat
    ? isNaN(parseFloat(window.EXTERNAL_PARAMS.lat))
      ? 0
      : parseFloat(window.EXTERNAL_PARAMS.lat)
    : 0;
  const zoom = window.EXTERNAL_PARAMS.zoom
    ? isNaN(parseFloat(window.EXTERNAL_PARAMS.zoom))
      ? 2
      : parseFloat(window.EXTERNAL_PARAMS.zoom)
    : 2;

  console.log(
    `journey_id: ${currentJourneyId}, render: ${currentRenderingMode}, lng: ${lng}, lat: ${lat}, zoom: ${zoom}`,
  );
  console.log(
    "EXTERNAL_PARAMS for endpoint configuration:",
    window.EXTERNAL_PARAMS,
  );

  const map = new maplibregl.Map({
    container: "map",
    center: [lng, lat],
    zoom: zoom,
    maxZoom: 14,
    // TODO: maplibre brings more canvas settings, we may fine tune them later
    canvasContextAttributes: {
      antialias: true,
    },
    transformRequest,
    pitchWithRotate: false,
    touchPitch: false,
    attributionControl: {
      compact: false
    },
  });

    // For V12 Styles you'll also need to add
    map.setStyle(currentMapStyle, {
        transformStyle: transformMapboxStyle
    })

  map.dragRotate.disable();
  map.touchZoomRotate.disableRotation();

  map.on("style.load", async (e) => {
    // Create a DOM element for the marker
    const el = document.createElement("div");
    el.className = "location-marker";

    // Create the marker but don't add it to the map yet
    // locationMarker = new maplibregl.Marker(el);
    locationMarker = new maplibregl.Marker({
      element: el
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
      if (window.readyForDisplay) {
        window.readyForDisplay.postMessage("");
      }
    }, 200);
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

// Ensure WASM module is initialized before using its exports downstream
init()
  .then(() => {
    console.log("WASM module initialized");

    trySetup().catch(console.error);

    window.SETUP_PENDING = true;
  })
  .catch(console.error);
