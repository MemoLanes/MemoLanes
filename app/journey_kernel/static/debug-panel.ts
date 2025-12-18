/**
 * Debug Panel for Map Overlay
 * Shows when debug=true is in URL hash
 * 
 * This module now uses ReactiveParams for parameter updates.
 * When the rendering mode dropdown changes, it simply sets params.renderMode,
 * and the registered hooks automatically handle layer switching.
 */

import type { LayerConfig, AvailableLayers, ReactiveParams } from "./params";

// Interface for URL hash parameters
interface UrlHashParams {
  [key: string]: string | null;
}

// Interface for tile download timing event detail
interface TileDownloadTimingDetail {
  duration: number;
}

// Type for Leaflet-like map object
interface MapInstance {
  on(event: string, callback: () => void): void;
  getZoom(): number;
  getCenter(): { lng: number; lat: number };
  getBounds(): {
    getNorthEast(): { lng: number; lat: number };
    getSouthWest(): { lng: number; lat: number };
  } | null;
}

export class DebugPanel {
  private map: MapInstance;
  private params: ReactiveParams;
  private panel: HTMLDivElement | null;
  private visible: boolean;
  private availableLayers: AvailableLayers;

  // Framerate monitoring
  private fps: number;
  private frameCount: number;
  private lastTime: number;
  private fpsHistory: number[];
  private maxHistorySize: number;
  private animationId: number | null;
  private fpsCanvas: HTMLCanvasElement | null;
  private fpsCtx: CanvasRenderingContext2D | null;

  // Network timing monitoring
  private lastNetworkDelay: number;
  private networkDelayHistory: number[];
  private maxNetworkHistorySize: number;
  private networkCanvas: HTMLCanvasElement | null;
  private networkCtx: CanvasRenderingContext2D | null;

  constructor(map: MapInstance, params: ReactiveParams, availableLayers: AvailableLayers = {}) {
    this.map = map;
    this.params = params;
    this.panel = null;
    this.visible = false;
    this.availableLayers = availableLayers;

    // Framerate monitoring
    this.fps = 0;
    this.frameCount = 0;
    this.lastTime = performance.now();
    this.fpsHistory = [];
    this.maxHistorySize = 60; // Keep 60 FPS readings for the graph
    this.animationId = null;
    this.fpsCanvas = null;
    this.fpsCtx = null;

    // Network timing monitoring
    this.lastNetworkDelay = 0;
    this.networkDelayHistory = [];
    this.maxNetworkHistorySize = 30; // Keep 30 network timing readings
    this.networkCanvas = null;
    this.networkCtx = null;
  }

  initialize(): void {
    // Create panel element
    this.panel = document.createElement("div");
    this.panel.className = "debug-panel";
    this.panel.style.display = "none";

    // Build rendering mode options based on available layers
    const renderingOptions: string = Object.entries(this.availableLayers)
      .map(([key, layer]: [string, LayerConfig]) => {
        return `<option value="${key}" title="${layer.description}">${layer.name}</option>`;
      })
      .join("");

    // Add content to panel
    this.panel.innerHTML = `
      <div style="font-weight: bold; margin-bottom: 10px; display: flex; justify-content: space-between;">
        <span>Debug Panel</span>
        <button id="close-debug">×</button>
      </div>
      
      <div class="separator"></div>
      
      <div style="margin-bottom: 10px;">
        <label for="rendering-mode" title="Controls how map data is rendered on screen">Rendering Mode:</label>
        <select id="rendering-mode">
          ${renderingOptions || '<option value="canvas">Canvas</option>'}
        </select>
      </div>
      
      <div class="separator"></div>
      
      <div style="margin-bottom: 10px;">
        <div style="font-weight: bold; margin-bottom: 5px;">Performance</div>
        <div style="font-family: monospace; font-size: 12px; margin-bottom: 8px;">
          <div>FPS: <span id="fps-display" style="color: #4CAF50;">-</span></div>
          <div>Network: <span id="network-delay-display" style="color: #2196F3;">-</span> ms</div>
        </div>
        <div style="font-size: 10px; margin-bottom: 4px; color: rgba(255, 255, 255, 0.7);">FPS</div>
        <canvas id="fps-graph" width="200" height="50"></canvas>
        <div style="font-size: 10px; margin: 4px 0; color: rgba(255, 255, 255, 0.7);">Network Delay</div>
        <canvas id="network-graph" width="200" height="50"></canvas>
      </div>
      
      <div class="separator"></div>
      
      <div style="margin-bottom: 10px;">
        <div style="font-weight: bold; margin-bottom: 5px;">Map Viewpoint</div>
        <div id="viewpoint-info" style="font-family: monospace; font-size: 12px;">
          <div>Zoom: <span id="zoom-level">-</span></div>
          <div>Center: <span id="center-coords">-</span></div>
          <div>Bounds: <span id="bounds-coords">-</span></div>
        </div>
      </div>
    `;

    // Add panel to document
    document.body.appendChild(this.panel);

    // Set up event listeners
    this._setupEventListeners();

    // Set up FPS monitoring
    this._setupFpsMonitoring();

    // Check if debug mode is enabled in URL
    this._checkDebugStatus();

    // Register hook to sync dropdown when renderMode changes externally
    this._setupParamsHook();
  }

  /**
   * Setup hook to sync the dropdown when renderMode changes from outside
   * (e.g., from Flutter or URL hash change)
   */
  private _setupParamsHook(): void {
    this.params.on('renderMode', (newMode, _oldMode) => {
      this._syncRenderingModeDropdown(newMode);
    });
  }

  /**
   * Sync the rendering mode dropdown to match the current params value
   */
  private _syncRenderingModeDropdown(renderingMode: string): void {
    const renderingModeSelect = document.getElementById(
      "rendering-mode",
    ) as HTMLSelectElement | null;
    
    if (renderingModeSelect && this.availableLayers[renderingMode]) {
      renderingModeSelect.value = renderingMode;
    }
  }

  private _setupEventListeners(): void {
    // Close button
    const closeButton = document.getElementById("close-debug");
    if (closeButton) {
      closeButton.addEventListener("click", () => {
        this.hide();
        this._updateUrlHash({ debug: "false" });
      });
    }

    // Rendering mode direct change handler
    // Now simply sets params.renderMode - the hook system handles the rest
    const renderingModeSelect = document.getElementById("rendering-mode");
    if (renderingModeSelect) {
      renderingModeSelect.addEventListener("change", (e: Event) => {
        const target = e.target as HTMLSelectElement;
        const renderingMode = target.value;
        
        // Update URL hash
        this._updateUrlHash({ render: renderingMode });
        
        // Simply set the renderMode on params
        // The ReactiveParams hook system automatically triggers switchRenderingLayer()
        if (this.availableLayers[renderingMode]) {
          this.params.renderMode = renderingMode;
        }
      });
    }

    // Listen for map movement to update viewpoint info
    this.map.on("moveend", () => {
      this._updateViewpointInfo();
    });

    // Also listen for zoom changes
    this.map.on("zoomend", () => {
      this._updateViewpointInfo();
    });
  }

  private _updateViewpointInfo(): void {
    if (!this.visible) return;

    const zoom: number = this.map.getZoom();
    const center = this.map.getCenter();
    const bounds = this.map.getBounds();

    const zoomElement = document.getElementById("zoom-level");
    if (zoomElement) {
      zoomElement.textContent = zoom.toFixed(2);
    }

    const centerElement = document.getElementById("center-coords");
    if (centerElement) {
      centerElement.textContent = `${center.lng.toFixed(5)}, ${center.lat.toFixed(5)}`;
    }

    if (bounds) {
      const ne = bounds.getNorthEast();
      const sw = bounds.getSouthWest();
      const boundsElement = document.getElementById("bounds-coords");
      if (boundsElement) {
        boundsElement.textContent = `SW: ${sw.lng.toFixed(5)}, ${sw.lat.toFixed(5)} | NE: ${ne.lng.toFixed(5)}, ${ne.lat.toFixed(5)}`;
      }
    }
  }

  private _setupFpsMonitoring(): void {
    // Get canvas elements and contexts
    const fpsCanvas = document.getElementById(
      "fps-graph",
    ) as HTMLCanvasElement | null;
    if (fpsCanvas) {
      this.fpsCanvas = fpsCanvas;
      this.fpsCtx = fpsCanvas.getContext("2d");
    }

    const networkCanvas = document.getElementById(
      "network-graph",
    ) as HTMLCanvasElement | null;
    if (networkCanvas) {
      this.networkCanvas = networkCanvas;
      this.networkCtx = networkCanvas.getContext("2d");
    }

    // Start FPS monitoring loop
    this._startFpsLoop();

    // Set up network timing listener
    this._setupNetworkMonitoring();
  }

  private _startFpsLoop(): void {
    const measureFps = (currentTime: number): void => {
      this.frameCount++;

      // Calculate FPS every second
      if (currentTime - this.lastTime >= 1000) {
        this.fps = Math.round(
          (this.frameCount * 1000) / (currentTime - this.lastTime),
        );
        this.frameCount = 0;
        this.lastTime = currentTime;

        // Add to history
        this.fpsHistory.push(this.fps);
        if (this.fpsHistory.length > this.maxHistorySize) {
          this.fpsHistory.shift();
        }

        // Update display
        this._updateFpsDisplay();
        this._renderFpsGraph();
      }

      this.animationId = requestAnimationFrame(measureFps);
    };

    this.animationId = requestAnimationFrame(measureFps);
  }

  private _stopFpsLoop(): void {
    if (this.animationId) {
      cancelAnimationFrame(this.animationId);
      this.animationId = null;
    }
  }

  private _setupNetworkMonitoring(): void {
    // Listen for network timing events
    window.addEventListener("tileDownloadTiming", (event: Event) => {
      const customEvent = event as CustomEvent<TileDownloadTimingDetail>;
      const { duration } = customEvent.detail;
      this.lastNetworkDelay = Math.round(duration);

      // Add to history
      this.networkDelayHistory.push(this.lastNetworkDelay);
      if (this.networkDelayHistory.length > this.maxNetworkHistorySize) {
        this.networkDelayHistory.shift();
      }

      // Update display
      this._updateNetworkDisplay();
      this._renderNetworkGraph();
    });
  }

  private _updateFpsDisplay(): void {
    if (!this.visible) return;

    const fpsElement = document.getElementById("fps-display");
    if (fpsElement) {
      fpsElement.textContent = this.fps.toString();

      // Color code based on FPS
      if (this.fps >= 50) {
        fpsElement.style.color = "#4CAF50"; // Green
      } else if (this.fps >= 30) {
        fpsElement.style.color = "#FF9800"; // Orange
      } else {
        fpsElement.style.color = "#F44336"; // Red
      }
    }
  }

  private _updateNetworkDisplay(): void {
    if (!this.visible) return;

    const networkElement = document.getElementById("network-delay-display");
    if (networkElement) {
      networkElement.textContent = this.lastNetworkDelay.toString();

      // Color code based on network delay
      if (this.lastNetworkDelay <= 100) {
        networkElement.style.color = "#4CAF50"; // Green - Fast
      } else if (this.lastNetworkDelay <= 500) {
        networkElement.style.color = "#FF9800"; // Orange - Moderate
      } else {
        networkElement.style.color = "#F44336"; // Red - Slow
      }
    }
  }

  private _renderFpsGraph(): void {
    if (!this.visible || !this.fpsCtx || !this.fpsCanvas) return;

    const canvas = this.fpsCanvas;
    const ctx = this.fpsCtx;
    const width: number = canvas.width;
    const height: number = canvas.height;

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    if (this.fpsHistory.length < 2) return;

    // Draw grid lines
    ctx.strokeStyle = "#555";
    ctx.lineWidth = 1;

    // Horizontal grid lines (FPS values)
    const gridLines: number[] = [30, 60];
    gridLines.forEach((fps: number) => {
      const y: number = height - (fps / 60) * height;
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(width, y);
      ctx.stroke();
    });

    // Draw FPS line
    ctx.strokeStyle = "#2196F3";
    ctx.lineWidth = 2;
    ctx.beginPath();

    const stepX: number = width / (this.maxHistorySize - 1);

    this.fpsHistory.forEach((fps: number, index: number) => {
      const x: number = index * stepX;
      const y: number = height - Math.min(fps / 60, 1) * height; // Normalize to 60 FPS max

      if (index === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    });

    ctx.stroke();

    // Draw FPS labels
    ctx.fillStyle = "#ccc";
    ctx.font = "8px monospace";
    ctx.textAlign = "left";
    ctx.fillText("60", 2, 10);
    ctx.fillText("30", 2, height - 18);
    ctx.fillText("0", 2, height - 2);
  }

  private _renderNetworkGraph(): void {
    if (!this.visible || !this.networkCtx || !this.networkCanvas) return;

    const canvas = this.networkCanvas;
    const ctx = this.networkCtx;
    const width: number = canvas.width;
    const height: number = canvas.height;

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    if (this.networkDelayHistory.length < 2) return;

    // Calculate max delay for scaling (minimum 1000ms for consistent scale)
    const maxDelay: number = Math.max(
      1000,
      Math.max(...this.networkDelayHistory),
    );

    // Draw grid lines
    ctx.strokeStyle = "#555";
    ctx.lineWidth = 1;

    // Horizontal grid lines (delay values)
    const gridLines: number[] = [500, 1000];
    gridLines.forEach((delay: number) => {
      if (delay <= maxDelay) {
        const y: number = height - (delay / maxDelay) * height;
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(width, y);
        ctx.stroke();
      }
    });

    // Draw network delay line
    ctx.strokeStyle = "#FF9800";
    ctx.lineWidth = 2;
    ctx.beginPath();

    const stepX: number = width / (this.maxNetworkHistorySize - 1);

    this.networkDelayHistory.forEach((delay: number, index: number) => {
      const x: number = index * stepX;
      const y: number = height - (delay / maxDelay) * height;

      if (index === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    });

    ctx.stroke();

    // Draw delay labels
    ctx.fillStyle = "#ccc";
    ctx.font = "8px monospace";
    ctx.textAlign = "left";
    if (maxDelay >= 1000) {
      ctx.fillText("1s", 2, 10);
    }
    if (maxDelay >= 500) {
      ctx.fillText("500ms", 2, height - 18);
    }
    ctx.fillText("0", 2, height - 2);
  }

  private _updateUrlHash(params: UrlHashParams): void {
    const hash: string = window.location.hash.slice(1);
    const urlParams = new URLSearchParams(hash);

    // Update or add provided params
    Object.keys(params).forEach((key: string) => {
      if (params[key] === null) {
        urlParams.delete(key);
      } else {
        urlParams.set(key, params[key] as string);
      }
    });

    // Update URL without reloading page
    window.location.hash = urlParams.toString();
  }

  private _checkDebugStatus(): void {
    const hash: string = window.location.hash.slice(1);
    const urlParams = new URLSearchParams(hash);
    const debugParam: string | null = urlParams.get("debug");

    if (debugParam === "true") {
      this.show();

      // Sync dropdown with current params value
      this._syncRenderingModeDropdown(this.params.renderMode);

      // Update viewpoint info
      this._updateViewpointInfo();
    } else {
      this.hide();
    }
  }

  show(): void {
    if (!this.panel) return;

    this.panel.style.display = "block";
    this.visible = true;
    this._updateViewpointInfo();
    this._updateNetworkDisplay();
    this._renderNetworkGraph();

    // Start FPS monitoring when panel is shown
    if (!this.animationId) {
      this._startFpsLoop();
    }
  }

  hide(): void {
    if (!this.panel) return;

    this.panel.style.display = "none";
    this.visible = false;

    // Stop FPS monitoring when panel is hidden to save resources
    this._stopFpsLoop();
  }

  // Listen for hash changes to show/hide panel
  listenForHashChanges(): void {
    window.addEventListener("hashchange", () => {
      this._checkDebugStatus();
    });
  }

  // Clean up resources
  destroy(): void {
    this._stopFpsLoop();
    if (this.panel && this.panel.parentNode) {
      this.panel.parentNode.removeChild(this.panel);
    }
  }
}
