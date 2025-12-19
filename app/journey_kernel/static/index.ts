/**
 * Main Entry Point - Application Initialization and Module Coordination
 *
 * This module serves as the application's entry point and coordinator:
 * - WASM module initialization
 * - Platform compatibility checks
 * - Parameter parsing and validation
 * - Module instantiation and wiring (MapController, DebugPanel, FlutterBridge)
 * - Window exports for Flutter communication
 *
 * Map-centric logic has been moved to MapController for better separation of concerns.
 */

import { DebugPanel } from "./debug-panel";
import init from "../pkg/index.js";
import { parseUrlHash, createReactiveParams, ReactiveParams } from "./params";
import { FlutterBridge, notifyFlutterReady } from "./flutter-bridge";
import { ensurePlatformCompatibility } from "./platform";
import { displayPageMessage } from "./utils";
import { MapController } from "./map-controller";

import "./debug-panel.css";

// ============================================================================
// Window Interface Extensions
// ============================================================================

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

// Initialize window properties for external communication
window.SETUP_PENDING = false;
window.EXTERNAL_PARAMS = {};

// ============================================================================
// Main Setup Function
// ============================================================================

/**
 * Main setup function that initializes all application components
 * This function orchestrates the initialization of:
 * 1. Parameter parsing and validation
 * 2. MapController (handles map, layers, tile provider)
 * 3. DebugPanel (optional, when debug mode enabled)
 * 4. FlutterBridge (handles Flutter-WebView communication)
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

  console.log(
    `journey_id: ${params.journeyId}, render: ${params.renderMode}, lng: ${params.lng}, lat: ${params.lat}, zoom: ${params.zoom}`,
  );
  console.log(
    "EXTERNAL_PARAMS for endpoint configuration:",
    window.EXTERNAL_PARAMS,
  );

  // Create and initialize MapController
  // MapController handles: map instance, tile provider, layers, style management
  const mapController = new MapController({
    containerId: "map",
    params,
  });

  await mapController.initialize();
  console.log("MapController initialized");

  // Initialize DebugPanel (only when debug mode is enabled)
  if (params.debug) {
    const debugPanel = new DebugPanel(mapController.getMap(), params);
    debugPanel.initialize();
  }

  // Initialize FlutterBridge for Flutter-WebView communication
  // FlutterBridge manages location marker and window methods for Flutter
  const flutterBridge = new FlutterBridge({
    map: mapController.getMap(),
    params,
  });
  flutterBridge.initialize();

  // Notify Flutter that the map is ready (with small delay for rendering)
  setTimeout(() => {
    notifyFlutterReady();
  }, 200);
}

// Export trySetup to window for Flutter to call
window.trySetup = trySetup;

// ============================================================================
// Application Bootstrap
// ============================================================================

// Check platform compatibility before initializing
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

// Initialize WASM module and start setup
init()
  .then(() => {
    console.log("WASM module initialized");

    trySetup().catch(console.error);

    window.SETUP_PENDING = true;
  })
  .catch(console.error);
