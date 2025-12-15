import { MultiRequest } from "./multi-requests";

// Type definitions
interface BrowserInfo {
  name: string;
  version: string;
  os: string;
}

interface WasmFeatures {
  referenceTypes: boolean;
  simd: boolean;
  threads: boolean;
  bulkMemory: boolean;
  multiValue: boolean;
}

interface PrecisionFormat {
  rangeMin: number;
  rangeMax: number;
  precision: number;
}

interface PrecisionFormats {
  vertexShader: {
    highpFloat: PrecisionFormat | null;
    mediumpFloat: PrecisionFormat | null;
    lowpFloat: PrecisionFormat | null;
    highpInt: PrecisionFormat | null;
    mediumpInt: PrecisionFormat | null;
    lowpInt: PrecisionFormat | null;
  };
  fragmentShader: {
    highpFloat: PrecisionFormat | null;
    mediumpFloat: PrecisionFormat | null;
    lowpFloat: PrecisionFormat | null;
    highpInt: PrecisionFormat | null;
    mediumpInt: PrecisionFormat | null;
    lowpInt: PrecisionFormat | null;
  };
}

// Extend Window interface
declare global {
  interface Window {
    SETUP_PENDING: boolean;
    EXTERNAL_PARAMS: {
      [key: string]: any;
      cgi_endpoint?: string;
    };
    trySetup?: () => Promise<void>;
    startTest?: () => void;
    stopTest?: () => void;
    clearLog?: () => void;
    handleIpcError?: (error: string) => void;
  }
}

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
          console.warn(
            `Failed to decode parameter '${key}': ${(error as Error).message}`,
          );
          // Skip this parameter if decoding fails
        }
      }
    }
  }
}

// Global state variables
let isRunning: boolean = false;
let intervalId: number | null = null;
let requestCounter: number = 0;

// === GLOBAL INSTANCE ===
let requester: MultiRequest | null = null;

async function trySetup(): Promise<void> {
  // Check if we have any endpoint configuration
  const hasCgiEndpoint = window.EXTERNAL_PARAMS.cgi_endpoint;
  const hasFlutterSetup = window.SETUP_PENDING;

  if (!hasCgiEndpoint && !hasFlutterSetup) {
    // No configuration available, wait for either hash params or Flutter injection
    return;
  }

  console.log("Initializing test with:", window.EXTERNAL_PARAMS);

  // Use cgi_endpoint directly (can be http:// or flutter://)
  const cgiEndpoint = window.EXTERNAL_PARAMS.cgi_endpoint;
  
  if (!cgiEndpoint) {
    log("No cgi_endpoint configured");
    return;
  }

  // Create MultiRequest instance
  requester = new MultiRequest(cgiEndpoint);
  const status = requester.getStatus();

  // Update UI
  const statusDiv = document.getElementById("status") as HTMLDivElement;
  statusDiv.className = "status ready";

  const endpointType = status.isFlutterMode ? "Flutter IPC" : "HTTP";
  statusDiv.textContent = `Ready! Endpoint: ${cgiEndpoint} (${endpointType})`;

  // Enable start button
  (document.getElementById("startBtn") as HTMLButtonElement).disabled = false;

  // Log configuration
  log(`Endpoint ready: ${cgiEndpoint}`);
  log(`Type: ${endpointType}`);
  log(`Status: ${JSON.stringify(status)}`);

  // Log hash parameters that were used
  if (Object.keys(window.EXTERNAL_PARAMS).length > 0) {
    log(`Configuration: ${JSON.stringify(window.EXTERNAL_PARAMS)}`);
  }
}

function log(message: string): void {
  const logDiv = document.getElementById("log") as HTMLDivElement;
  const timestamp = new Date().toLocaleTimeString();
  logDiv.innerHTML += `[${timestamp}] ${message}<br>`;
  logDiv.scrollTop = logDiv.scrollHeight;
}

// === IPC ERROR HANDLER (for backwards compatibility) ===
window.handleIpcError = function (error: string): void {
  log(`IPC ERROR: ${error}`);
};

// === REQUEST FUNCTIONS ===
function getRequestSize(): number {
  const sizeSelect = document.getElementById(
    "sizeSelect",
  ) as HTMLSelectElement | null;
  const size = sizeSelect ? sizeSelect.value : "1048576";
  return parseInt(size);
}

// Make a request to the configured endpoint
async function makeRequest(): Promise<void> {
  if (!requester) {
    log("ERROR: No requester available!");
    return;
  }

  const requestId = ++requestCounter;
  const size = getRequestSize();
  const status = requester.getStatus();
  const endpointType = status.isFlutterMode ? "Flutter IPC" : "HTTP";

  const startTime = performance.now();
  log(`Request #${requestId}: ${endpointType} - ${size} bytes - Starting...`);

  try {
    // Use the unified fetch method (works for both HTTP and Flutter IPC)
    const response = await requester.fetch("random_data", { size: size });

    const endTime = performance.now();
    const duration = Math.round(endTime - startTime);

    if (response.success) {
      // Response is already a JS object, access data directly
      const actualSize = response.data?.size || size;
      log(
        `Request #${requestId}: ${endpointType} - SUCCESS - ${duration}ms - ${actualSize} bytes`,
      );
    } else {
      log(
        `Request #${requestId}: ${endpointType} - ERROR - ${response.error || "Unknown error"} - ${duration}ms`,
      );
    }
  } catch (error) {
    const endTime = performance.now();
    const duration = Math.round(endTime - startTime);
    log(
      `Request #${requestId}: ${endpointType} - ERROR - ${(error as Error).message} - ${duration}ms`,
    );
  }
}

function startTest(): void {
  if (isRunning || !requester) return;

  isRunning = true;
  (document.getElementById("startBtn") as HTMLButtonElement).disabled = true;
  (document.getElementById("stopBtn") as HTMLButtonElement).disabled = false;

  const status = requester.getStatus();
  const endpointType = status.isFlutterMode ? "Flutter IPC" : "HTTP";

  log(`Test started - Endpoint: ${status.endpoint} (${endpointType})`);
  log(`Status: ${JSON.stringify(status)}`);

  makeRequest();
  intervalId = setInterval(makeRequest, 1000) as any;
}

function stopTest(): void {
  if (!isRunning) return;

  isRunning = false;
  (document.getElementById("startBtn") as HTMLButtonElement).disabled = false;
  (document.getElementById("stopBtn") as HTMLButtonElement).disabled = true;

  if (intervalId) {
    clearInterval(intervalId);
    intervalId = null;
  }

  // Clear any pending requests
  if (requester) requester.clearPending();

  log("Test stopped - all pending requests cleared");
}

function clearLog(): void {
  (document.getElementById("log") as HTMLDivElement).innerHTML = "";
  requestCounter = 0;

  if (requester) {
    const status = requester.getStatus();
    const endpointType = status.isFlutterMode ? "Flutter IPC" : "HTTP";
    
    log(`Log cleared - Endpoint: ${status.endpoint} (${endpointType})`);
    log(`Status: ${JSON.stringify(status)}`);
  } else {
    log("Log cleared - No endpoint configured");
  }
}

// === BROWSER CAPABILITIES DETECTION ===
function detectBrowserCapabilities(): void {
  const capabilitiesDiv = document.getElementById(
    "capabilitiesInfo",
  ) as HTMLDivElement;
  const precisionTableDiv = document.getElementById(
    "precisionTable",
  ) as HTMLDivElement;

  const info: string[] = [];

  // Browser information
  const ua = navigator.userAgent;
  const browserInfo = parseBrowserInfo(ua);

  info.push(
    `<strong>Browser:</strong> ${browserInfo.name} ${browserInfo.version}`,
  );
  info.push(`<strong>OS:</strong> ${browserInfo.os}`);
  info.push(`<strong>Platform:</strong> ${navigator.platform}`);

  // Screen information
  info.push(
    `<strong>Screen:</strong> ${window.screen.width}×${window.screen.height} (Device Pixel Ratio: ${window.devicePixelRatio || 1})`,
  );

  // Detect WebAssembly support
  const wasmSupported = typeof WebAssembly === "object";
  info.push(
    `<strong>WebAssembly:</strong> ${wasmSupported ? "✓ Supported" : "✗ Not Supported"}`,
  );

  // Detect WebAssembly features
  if (wasmSupported) {
    const wasmFeatures = detectWasmFeatures();
    info.push(`<strong>WASM Features:</strong>`);
    info.push(
      `&nbsp;&nbsp;• Reference Types (externref): ${wasmFeatures.referenceTypes ? "✓" : "✗"}`,
    );
    info.push(`&nbsp;&nbsp;• SIMD: ${wasmFeatures.simd ? "✓" : "✗"}`);
    info.push(`&nbsp;&nbsp;• Threads: ${wasmFeatures.threads ? "✓" : "✗"}`);
    info.push(
      `&nbsp;&nbsp;• Bulk Memory: ${wasmFeatures.bulkMemory ? "✓" : "✗"}`,
    );
    info.push(
      `&nbsp;&nbsp;• Multi-value: ${wasmFeatures.multiValue ? "✓" : "✗"}`,
    );
  }

  // Detect WebGL version
  const canvas = document.createElement("canvas");
  let gl2: WebGL2RenderingContext | null = null;
  let gl1: WebGLRenderingContext | null = null;

  try {
    gl2 = canvas.getContext("webgl2");
  } catch (e) {
    // WebGL2 not supported
  }

  try {
    gl1 =
      canvas.getContext("webgl") ||
      (canvas.getContext("experimental-webgl") as WebGLRenderingContext);
  } catch (e) {
    // WebGL not supported
  }

  if (gl2) {
    info.push(`<strong>WebGL:</strong> ✓ WebGL 2.0 Supported`);
    const debugInfo = gl2.getExtension("WEBGL_debug_renderer_info");
    if (debugInfo) {
      const vendor = gl2.getParameter(debugInfo.UNMASKED_VENDOR_WEBGL);
      const renderer = gl2.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL);
      info.push(`<strong>GPU Vendor:</strong> ${vendor}`);
      info.push(`<strong>GPU Renderer:</strong> ${renderer}`);
    }
  } else if (gl1) {
    info.push(`<strong>WebGL:</strong> ✓ WebGL 1.0 Only`);
  } else {
    info.push(`<strong>WebGL:</strong> ✗ Not Supported`);
  }

  // User Agent (collapsed by default, can be expanded)
  info.push(
    `<details style="margin-top: 8px;"><summary style="cursor: pointer; font-weight: bold;">User Agent (click to expand)</summary><code style="word-break: break-all; font-size: 10px;">${ua}</code></details>`,
  );

  capabilitiesDiv.innerHTML = info.join("<br>");

  // Generate precision table
  const gl = gl2 || gl1;
  if (gl) {
    generatePrecisionTable(gl, precisionTableDiv);
  }
}

// Helper function to detect WebAssembly features
// TODO: WASM feature check currently only works for externref, other checks seems to be wrong.
function detectWasmFeatures(): WasmFeatures {
  const features: WasmFeatures = {
    referenceTypes: false,
    simd: false,
    threads: false,
    bulkMemory: false,
    multiValue: false,
  };

  // Test Reference Types (externref)
  try {
    // This is a minimal WASM module that uses externref
    // (module (func (param externref)))
    const wasmBinary = new Uint8Array([
      0x00, 0x61, 0x73, 0x6d, // magic: \0asm
      0x01, 0x00, 0x00, 0x00, // version: 1
      0x01, 0x05, 0x01, 0x60, 0x01, 0x6f, 0x00, // type section: func(externref)
      0x03, 0x02, 0x01, 0x00, // func section
      0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b, // code section
    ]);
    new WebAssembly.Module(wasmBinary);
    features.referenceTypes = true;
  } catch (e) {
    // Reference types not supported
  }

  // Test SIMD
  try {
    // (module (func (result v128) (v128.const i32x4 0 0 0 0)))
    const wasmBinary = new Uint8Array([
      0x00, 0x61, 0x73, 0x6d, // magic
      0x01, 0x00, 0x00, 0x00, // version
      0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7b, // type: func()->v128
      0x03, 0x02, 0x01, 0x00, // func
      0x0a, 0x0a, 0x01, 0x08, 0x00, 0xfd, 0x0c, 0x00, 0x00, 0x00, 0x00,
      0x0b, // code: v128.const
    ]);
    new WebAssembly.Module(wasmBinary);
    features.simd = true;
  } catch (e) {
    // SIMD not supported
  }

  // Test Threads (SharedArrayBuffer required)
  try {
    features.threads = typeof SharedArrayBuffer !== "undefined";
    if (features.threads) {
      // Additional test: try to create a WASM module with shared memory
      // (module (memory 1 1 shared))
      const wasmBinary = new Uint8Array([
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        0x05, 0x04, 0x01, 0x03, 0x01, 0x01, // memory section: 1 1 shared
      ]);
      new WebAssembly.Module(wasmBinary);
    }
  } catch (e) {
    features.threads = false;
  }

  // Test Bulk Memory Operations
  try {
    // (module (memory 1) (func (memory.fill (i32.const 0) (i32.const 0) (i32.const 0))))
    const wasmBinary = new Uint8Array([
      0x00, 0x61, 0x73, 0x6d, // magic
      0x01, 0x00, 0x00, 0x00, // version
      0x05, 0x03, 0x01, 0x00, 0x01, // memory section
      0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // type section
      0x03, 0x02, 0x01, 0x00, // func section
      0x0a, 0x0e, 0x01, 0x0c, 0x00, 0x41, 0x00, 0x41, 0x00, 0x41, 0x00, 0xfc,
      0x0b, 0x00, 0x0b, // code: memory.fill
    ]);
    new WebAssembly.Module(wasmBinary);
    features.bulkMemory = true;
  } catch (e) {
    // Bulk memory not supported
  }

  // Test Multi-value
  try {
    // (module (func (result i32 i32) (i32.const 0) (i32.const 1)))
    const wasmBinary = new Uint8Array([
      0x00, 0x61, 0x73, 0x6d, // magic
      0x01, 0x00, 0x00, 0x00, // version
      0x01, 0x06, 0x01, 0x60, 0x00, 0x02, 0x7f, 0x7f, // type: func()->(i32,i32)
      0x03, 0x02, 0x01, 0x00, // func
      0x0a, 0x09, 0x01, 0x07, 0x00, 0x41, 0x00, 0x41, 0x01, 0x0b, // code
    ]);
    new WebAssembly.Module(wasmBinary);
    features.multiValue = true;
  } catch (e) {
    // Multi-value not supported
  }

  return features;
}

// Helper function to parse browser info from User Agent
function parseBrowserInfo(ua: string): BrowserInfo {
  let browserName = "Unknown";
  let browserVersion = "";
  let os = "Unknown";

  // Detect OS
  if (ua.indexOf("Win") !== -1) os = "Windows";
  else if (ua.indexOf("Mac") !== -1) os = "macOS";
  else if (ua.indexOf("Linux") !== -1) os = "Linux";
  else if (ua.indexOf("Android") !== -1) os = "Android";
  else if (
    ua.indexOf("iOS") !== -1 ||
    ua.indexOf("iPhone") !== -1 ||
    ua.indexOf("iPad") !== -1
  )
    os = "iOS";

  // Detect browser (order matters!)
  if (ua.indexOf("Edg") !== -1) {
    browserName = "Edge";
    const match = ua.match(/Edg\/([0-9.]+)/);
    browserVersion = match ? match[1] : "";
  } else if (ua.indexOf("Chrome") !== -1 && ua.indexOf("Safari") !== -1) {
    browserName = "Chrome";
    const match = ua.match(/Chrome\/([0-9.]+)/);
    browserVersion = match ? match[1] : "";
  } else if (ua.indexOf("Safari") !== -1 && ua.indexOf("Chrome") === -1) {
    browserName = "Safari";
    const match = ua.match(/Version\/([0-9.]+)/);
    browserVersion = match ? match[1] : "";
  } else if (ua.indexOf("Firefox") !== -1) {
    browserName = "Firefox";
    const match = ua.match(/Firefox\/([0-9.]+)/);
    browserVersion = match ? match[1] : "";
  } else if (ua.indexOf("MSIE") !== -1 || ua.indexOf("Trident") !== -1) {
    browserName = "Internet Explorer";
    const match = ua.match(/(?:MSIE |rv:)([0-9.]+)/);
    browserVersion = match ? match[1] : "";
  }

  return { name: browserName, version: browserVersion, os: os };
}

function generatePrecisionTable(
  gl: WebGLRenderingContext | WebGL2RenderingContext,
  container: HTMLDivElement,
): void {
  // Get precision formats for both float and int
  const precisionFormats: PrecisionFormats = {
    vertexShader: {
      highpFloat: gl.getShaderPrecisionFormat(gl.VERTEX_SHADER, gl.HIGH_FLOAT),
      mediumpFloat: gl.getShaderPrecisionFormat(
        gl.VERTEX_SHADER,
        gl.MEDIUM_FLOAT,
      ),
      lowpFloat: gl.getShaderPrecisionFormat(gl.VERTEX_SHADER, gl.LOW_FLOAT),
      highpInt: gl.getShaderPrecisionFormat(gl.VERTEX_SHADER, gl.HIGH_INT),
      mediumpInt: gl.getShaderPrecisionFormat(gl.VERTEX_SHADER, gl.MEDIUM_INT),
      lowpInt: gl.getShaderPrecisionFormat(gl.VERTEX_SHADER, gl.LOW_INT),
    },
    fragmentShader: {
      highpFloat: gl.getShaderPrecisionFormat(
        gl.FRAGMENT_SHADER,
        gl.HIGH_FLOAT,
      ),
      mediumpFloat: gl.getShaderPrecisionFormat(
        gl.FRAGMENT_SHADER,
        gl.MEDIUM_FLOAT,
      ),
      lowpFloat: gl.getShaderPrecisionFormat(gl.FRAGMENT_SHADER, gl.LOW_FLOAT),
      highpInt: gl.getShaderPrecisionFormat(gl.FRAGMENT_SHADER, gl.HIGH_INT),
      mediumpInt: gl.getShaderPrecisionFormat(
        gl.FRAGMENT_SHADER,
        gl.MEDIUM_INT,
      ),
      lowpInt: gl.getShaderPrecisionFormat(gl.FRAGMENT_SHADER, gl.LOW_INT),
    },
  };

  // Helper function to calculate bits (from reference code)
  function calculateBits(format: PrecisionFormat | null): number {
    if (!format || (format.rangeMin === 0 && format.rangeMax === 0)) {
      return 0;
    }
    const isInt = format.precision === 0;
    return isInt
      ? format.rangeMin + 1
      : format.precision + Math.log2(format.rangeMin + 1) + 2;
  }

  // Helper function to format precision info - show raw integer values
  function formatPrecision(precision: PrecisionFormat | null): string {
    if (!precision || (precision.rangeMin === 0 && precision.rangeMax === 0)) {
      return "not supported";
    }
    return `rangeMin: ${precision.rangeMin}, rangeMax: ${precision.rangeMax}, precision: ${precision.precision}`;
  }

  // Helper function to format bits
  function formatBits(precision: PrecisionFormat | null): string {
    if (!precision || (precision.rangeMin === 0 && precision.rangeMax === 0)) {
      return "-";
    }
    return Math.round(calculateBits(precision)).toString();
  }

  // Helper to create table rows
  function createRow(
    label: string,
    vsFormat: PrecisionFormat | null,
    fsFormat: PrecisionFormat | null,
  ): string {
    return `
      <tr>
        <td style="border: 1px solid #dee2e6; padding: 8px; font-weight: bold;">${label}</td>
        <td style="border: 1px solid #dee2e6; padding: 8px; text-align: center;">${formatBits(vsFormat)}</td>
        <td style="border: 1px solid #dee2e6; padding: 8px; font-family: monospace; font-size: 11px;">${formatPrecision(vsFormat)}</td>
        <td style="border: 1px solid #dee2e6; padding: 8px; text-align: center;">${formatBits(fsFormat)}</td>
        <td style="border: 1px solid #dee2e6; padding: 8px; font-family: monospace; font-size: 11px;">${formatPrecision(fsFormat)}</td>
      </tr>
    `;
  }

  // Create table HTML
  const tableHTML = `
    <h5 style="margin: 10px 0 5px 0;">WebGL Shader Precision (GLSL)</h5>
    <table style="width: 100%; border-collapse: collapse; font-size: 12px;">
      <thead>
        <tr style="background: #e9ecef;">
          <th style="border: 1px solid #dee2e6; padding: 8px; text-align: left;">Type</th>
          <th colspan="2" style="border: 1px solid #dee2e6; padding: 8px; text-align: center;">Vertex Shader</th>
          <th colspan="2" style="border: 1px solid #dee2e6; padding: 8px; text-align: center;">Fragment Shader</th>
        </tr>
        <tr style="background: #f8f9fa;">
          <th style="border: 1px solid #dee2e6; padding: 8px;"></th>
          <th style="border: 1px solid #dee2e6; padding: 8px; text-align: center;">bits</th>
          <th style="border: 1px solid #dee2e6; padding: 8px; text-align: center;">precision</th>
          <th style="border: 1px solid #dee2e6; padding: 8px; text-align: center;">bits</th>
          <th style="border: 1px solid #dee2e6; padding: 8px; text-align: center;">precision</th>
        </tr>
      </thead>
      <tbody>
        ${createRow("highp float", precisionFormats.vertexShader.highpFloat, precisionFormats.fragmentShader.highpFloat)}
        ${createRow("mediump float", precisionFormats.vertexShader.mediumpFloat, precisionFormats.fragmentShader.mediumpFloat)}
        ${createRow("lowp float", precisionFormats.vertexShader.lowpFloat, precisionFormats.fragmentShader.lowpFloat)}
        ${createRow("highp int", precisionFormats.vertexShader.highpInt, precisionFormats.fragmentShader.highpInt)}
        ${createRow("mediump int", precisionFormats.vertexShader.mediumpInt, precisionFormats.fragmentShader.mediumpInt)}
        ${createRow("lowp int", precisionFormats.vertexShader.lowpInt, precisionFormats.fragmentShader.lowpInt)}
      </tbody>
    </table>
    <p style="font-size: 11px; color: #666; margin-top: 8px;">
      Note: rangeMin/rangeMax represent the log2 of the absolute value of min/max representable values. 
      Precision represents the number of bits of precision. Bits is calculated as: for int types = rangeMin + 1, for float types = precision + log2(rangeMin + 1) + 2.
    </p>
  `;

  container.innerHTML = tableHTML;
}

// Expose functions to window object to prevent webpack from renaming them
window.trySetup = trySetup;
window.startTest = startTest;
window.stopTest = stopTest;
window.clearLog = clearLog;

// Listen for hash changes (similar to index.ts)
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
          console.warn(
            `Failed to decode parameter '${key}': ${(error as Error).message}`,
          );
        }
      }
    }
  }

  // Reinitialize with new parameters
  trySetup();
});

// Initialize when DOM is ready
document.addEventListener("DOMContentLoaded", function () {
  // Detect and display browser capabilities first
  detectBrowserCapabilities();

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
