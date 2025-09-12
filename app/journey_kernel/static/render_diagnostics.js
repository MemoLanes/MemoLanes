// === COORDINATION SETUP ===
{
  window.SETUP_PENDING = false;
  window.EXTERNAL_PARAMS = {};

  const hash = window.location.hash.slice(1);
  if (hash) {
    // default cgiEndpoint (only available if hash parameters are provided)
    window.EXTERNAL_PARAMS.cgi_endpoint = ".";

    const params = new URLSearchParams(hash);

    // Scan all hash parameters and store them in EXTERNAL_PARAMS after successful decoding
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

import { MultiRequest } from "./multirequest.js";

let isRunning = false;
let intervalId = null;
let requestCounter = 0;
let endpointCounter = 0;
let httpEndpoint = null;

// === GLOBAL INSTANCES ===
let httpRequester = null;
let flutterRequester = null;

function trySetup() {
  // Check if we have any endpoint configuration
  const hasCgiEndpoint = window.EXTERNAL_PARAMS.cgi_endpoint;
  const hasHttpEndpoint =
    window.EXTERNAL_PARAMS.http_endpoint || window.EXTERNAL_PARAMS.httpEndpoint;
  const hasFlutterSetup = window.SETUP_PENDING;

  if (!hasCgiEndpoint && !hasHttpEndpoint && !hasFlutterSetup) {
    // No configuration available, wait for either hash params or Flutter injection
    return;
  }

  console.log("Initializing test with:", window.EXTERNAL_PARAMS);

  let httpEndpointUrl = null;
  let flutterEndpointUrl = null;

  // Determine HTTP endpoint from hash params or Flutter injection
  if (window.EXTERNAL_PARAMS.http_endpoint) {
    // Use dedicated http_endpoint parameter
    httpEndpointUrl = window.EXTERNAL_PARAMS.http_endpoint;
  } else if (window.EXTERNAL_PARAMS.httpEndpoint) {
    // Fallback to legacy Flutter-injected httpEndpoint
    httpEndpointUrl = window.EXTERNAL_PARAMS.httpEndpoint;
  }

  // Determine Flutter endpoint from cgi_endpoint or flutter_channel
  if (window.EXTERNAL_PARAMS.flutter_channel) {
    // Use explicit flutter_channel parameter (new preferred method)
    flutterEndpointUrl = `flutter://${window.EXTERNAL_PARAMS.flutter_channel}`;
  } else if (window.EXTERNAL_PARAMS.cgi_endpoint) {
    if (window.EXTERNAL_PARAMS.cgi_endpoint === "flutter") {
      // Legacy Flutter mode - use channel directly
      const flutterChannel =
        window.EXTERNAL_PARAMS.flutter_channel || "RenderDiagnosticsChannel";
      flutterEndpointUrl = `flutter://${flutterChannel}`;
    } else {
      // Use cgi_endpoint as HTTP endpoint if no separate http_endpoint provided
      if (!httpEndpointUrl) {
        httpEndpointUrl = window.EXTERNAL_PARAMS.cgi_endpoint;
      }
      // Also use default Flutter endpoint
      const flutterChannel =
        window.EXTERNAL_PARAMS.flutter_channel || "RenderDiagnosticsChannel";
      flutterEndpointUrl = `flutter://${flutterChannel}`;
    }
  } else {
    // Default Flutter setup if no specific configuration
    const flutterChannel =
      window.EXTERNAL_PARAMS.flutter_channel || "RenderDiagnosticsChannel";
    flutterEndpointUrl = `flutter://${flutterChannel}`;
  }

  // Create MultiRequest instances
  if (httpEndpointUrl) {
    httpRequester = new MultiRequest(httpEndpointUrl);
    httpEndpoint = httpEndpointUrl; // For backwards compatibility
  }

  if (flutterEndpointUrl) {
    flutterRequester = new MultiRequest(flutterEndpointUrl);
  }

  // Update UI
  const statusDiv = document.getElementById("status");
  statusDiv.className = "status ready";

  const httpStatus = httpRequester ? httpEndpointUrl : "Not configured";
  const flutterChannel = flutterRequester
    ? flutterRequester.channelName
    : "Not configured";
  statusDiv.textContent = `Ready! HTTP: ${httpStatus}, IPC: ${flutterChannel}`;

  // Enable start button if we have at least one endpoint
  if (httpRequester || flutterRequester) {
    document.getElementById("startBtn").disabled = false;
  }

  // Log configuration
  if (httpRequester) {
    log(`HTTP endpoint ready: ${httpEndpointUrl}`);
    log(`HTTP status: ${JSON.stringify(httpRequester.getStatus())}`);
  } else {
    log("HTTP endpoint: Not configured");
  }

  if (flutterRequester) {
    log(
      `Flutter IPC ready: ${flutterRequester.channelName} (available: ${flutterRequester.getStatus().channelAvailable})`,
    );
    log(`Flutter status: ${JSON.stringify(flutterRequester.getStatus())}`);
  } else {
    log("Flutter IPC: Not configured");
  }

  // Log hash parameters that were used
  if (Object.keys(window.EXTERNAL_PARAMS).length > 0) {
    log(`Configuration: ${JSON.stringify(window.EXTERNAL_PARAMS)}`);
  }
}

function log(message) {
  const logDiv = document.getElementById("log");
  const timestamp = new Date().toLocaleTimeString();
  logDiv.innerHTML += `[${timestamp}] ${message}<br>`;
  logDiv.scrollTop = logDiv.scrollHeight;
}

// === IPC ERROR HANDLER (for backwards compatibility) ===
window.handleIpcError = function (error) {
  log(`IPC ERROR: ${error}`);
};

// === REQUEST FUNCTIONS ===
// Simplify the getCurrentEndpoint function since we only have two types now
function getCurrentEndpoint() {
  const sizeSelect = document.getElementById("sizeSelect");
  const size = sizeSelect ? sizeSelect.value : "1048576";

  const groupIndex = Math.floor(endpointCounter / 5) % 2;
  return {
    path: groupIndex === 0 ? "random_data" : "ipc",
    size: parseInt(size),
    isIpc: groupIndex === 1, // Every other 5 requests use IPC
  };
}

// Update the makeRequest function to use MultiRequest instances
async function makeRequest() {
  if (!httpRequester && !flutterRequester) {
    log("ERROR: No MultiRequest instances available!");
    return;
  }

  const requestId = ++requestCounter;
  const { path, size, isIpc } = getCurrentEndpoint();
  endpointCounter++;

  const startTime = performance.now();

  if (isIpc) {
    if (!flutterRequester) {
      log(
        `Request #${requestId}: IPC Channel - SKIPPED - Flutter requester not configured`,
      );
      return;
    }

    log(`Request #${requestId}: IPC Channel - ${size} bytes - Starting...`);

    try {
      // Use the flutter requester with random_data query and size payload
      const response = await flutterRequester.fetch("random_data", {
        size: size,
      });
      const endTime = performance.now();
      const duration = Math.round(endTime - startTime);
      const actualSize = response.data?.size || size;
      log(
        `Request #${requestId}: IPC Channel - SUCCESS - ${duration}ms total - ${actualSize} bytes`,
      );
    } catch (error) {
      const endTime = performance.now();
      const duration = Math.round(endTime - startTime);
      log(
        `Request #${requestId}: IPC Channel - ERROR - ${error.message} - ${duration}ms`,
      );
    }
  } else {
    if (!httpRequester) {
      log(
        `Request #${requestId}: HTTP (${path}) - SKIPPED - HTTP requester not configured`,
      );
      return;
    }

    // HTTP request using MultiRequest
    log(`Request #${requestId}: HTTP (${path}) - ${size} bytes - Starting...`);

    try {
      // Use the unified JSON API endpoint with POST request
      // The path 'random_data' maps directly to the RandomData payload in Rust
      const response = await httpRequester.fetch("random_data", { size: size });

      const endTime = performance.now();
      const duration = Math.round(endTime - startTime);

      if (response.success) {
        // Response is already a JS object, access data directly
        const actualSize = response.data?.size || size;
        log(
          `Request #${requestId}: HTTP (${path}) - SUCCESS - ${duration}ms - ${actualSize} bytes`,
        );
      } else {
        log(
          `Request #${requestId}: HTTP (${path}) - ERROR - ${response.error || "Unknown error"} - ${duration}ms`,
        );
      }
    } catch (error) {
      const endTime = performance.now();
      const duration = Math.round(endTime - startTime);
      log(
        `Request #${requestId}: HTTP (${path}) - ERROR - ${error.message} - ${duration}ms`,
      );
    }
  }
}

function startTest() {
  if (isRunning || (!httpRequester && !flutterRequester)) return;

  isRunning = true;
  document.getElementById("startBtn").disabled = true;
  document.getElementById("stopBtn").disabled = false;

  const configuredEndpoints = [];
  if (httpRequester) configuredEndpoints.push("HTTP (random_data)");
  if (flutterRequester) configuredEndpoints.push("IPC (Flutter API)");

  log(
    `Test started - alternating between ${configuredEndpoints.join(" and ")} every 5 requests`,
  );

  if (httpRequester) {
    log(`HTTP Status: ${JSON.stringify(httpRequester.getStatus())}`);
  } else {
    log("HTTP: Not configured - will skip HTTP requests");
  }

  if (flutterRequester) {
    log(`Flutter Status: ${JSON.stringify(flutterRequester.getStatus())}`);
  } else {
    log("Flutter IPC: Not configured - will skip IPC requests");
  }

  makeRequest();
  intervalId = setInterval(makeRequest, 1000);
}

function stopTest() {
  if (!isRunning) return;

  isRunning = false;
  document.getElementById("startBtn").disabled = false;
  document.getElementById("stopBtn").disabled = true;

  if (intervalId) {
    clearInterval(intervalId);
    intervalId = null;
  }

  // Clear any pending requests
  if (httpRequester) httpRequester.clearPending();
  if (flutterRequester) flutterRequester.clearPending();

  log("Test stopped - all pending requests cleared");
}

function clearLog() {
  document.getElementById("log").innerHTML = "";
  requestCounter = 0;
  endpointCounter = 0;

  if (httpRequester || flutterRequester) {
    const httpStatus = httpRequester
      ? httpRequester.cgiEndpoint
      : "Not configured";
    const ipcStatus = flutterRequester
      ? flutterRequester.getStatus().channelAvailable
        ? "Available"
        : "Not Available"
      : "Not configured";

    log(`Log cleared - HTTP: ${httpStatus}, IPC: ${ipcStatus}`);

    if (httpRequester) {
      log(`HTTP Status: ${JSON.stringify(httpRequester.getStatus())}`);
    }

    if (flutterRequester) {
      log(`Flutter Status: ${JSON.stringify(flutterRequester.getStatus())}`);
    }
  } else {
    log("Log cleared - No endpoints configured");
  }
}

// Expose functions to window object to prevent webpack from renaming them
window.trySetup = trySetup;
window.startTest = startTest;
window.stopTest = stopTest;
window.clearLog = clearLog;

// Listen for hash changes (similar to index.js)
window.addEventListener("hashchange", () => {
  console.log("Hash changed, re-parsing parameters...");

  // Re-parse hash parameters
  const hash = window.location.hash.slice(1);
  window.EXTERNAL_PARAMS = {};

  if (hash) {
    window.EXTERNAL_PARAMS.cgi_endpoint = ".";
    const params = new URLSearchParams(hash);

    for (const [key, value] of params.entries()) {
      if (value) {
        try {
          const decodedValue = decodeURIComponent(value);
          window.EXTERNAL_PARAMS[key] = decodedValue;
        } catch (error) {
          console.warn(`Failed to decode parameter '${key}': ${error.message}`);
        }
      }
    }
  }

  // Reinitialize with new parameters
  trySetup();
});

// Initialize when DOM is ready
document.addEventListener("DOMContentLoaded", function () {
  // Try to initialize immediately (hash params are already parsed)
  trySetup();

  // Also mark as ready for Flutter injection (backwards compatibility)
  window.SETUP_PENDING = true;

  window.addEventListener("beforeunload", stopTest);

  if (window.EXTERNAL_PARAMS.cgi_endpoint) {
    log(
      `Page loaded with hash parameters: ${JSON.stringify(window.EXTERNAL_PARAMS)}`,
    );
  } else {
    log("Page loaded - waiting for hash parameters or Flutter injection...");
  }
});
