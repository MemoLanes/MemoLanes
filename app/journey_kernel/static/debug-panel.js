/**
 * Debug Panel for Map Overlay
 * Shows when debug=true is in URL hash
 */

export class DebugPanel {
  constructor(map, availableLayers = {}) {
    this.map = map;
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

  initialize() {
    // Create panel element
    this.panel = document.createElement('div');
    this.panel.className = 'debug-panel';
    this.panel.style.display = 'none';

    // Build rendering mode options based on available layers
    const renderingOptions = Object.entries(this.availableLayers).map(([key, layer]) => {
      return `<option value="${key}" title="${layer.description}">${layer.name}</option>`;
    }).join('');

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
    
    // Set up easter egg
    this._setupEasterEgg();
    
    // Check if debug mode is enabled in URL
    this._checkDebugStatus();
  }

  _setupEventListeners() {
    // Close button
    document.getElementById('close-debug').addEventListener('click', () => {
      this.hide();
      this._updateUrlHash({debug: 'false'});
    });

    // Rendering mode direct change handler
    document.getElementById('rendering-mode').addEventListener('change', (e) => {
      const renderingMode = e.target.value;
      this._updateUrlHash({ render: renderingMode });
      if (window.switchRenderingLayer && this.availableLayers[renderingMode]) {
        window.switchRenderingLayer(renderingMode);
      }
    });

    // Listen for map movement to update viewpoint info
    this.map.on('moveend', () => {
      this._updateViewpointInfo();
    });
    
    // Also listen for zoom changes
    this.map.on('zoomend', () => {
      this._updateViewpointInfo();
    });
  }

  _updateViewpointInfo() {
    if (!this.visible) return;
    
    const zoom = this.map.getZoom();
    const center = this.map.getCenter();
    const bounds = this.map.getBounds();
    
    document.getElementById('zoom-level').textContent = zoom.toFixed(2);
    document.getElementById('center-coords').textContent = 
      `${center.lng.toFixed(5)}, ${center.lat.toFixed(5)}`;
    
    if (bounds) {
      const ne = bounds.getNorthEast();
      const sw = bounds.getSouthWest();
      document.getElementById('bounds-coords').textContent = 
        `SW: ${sw.lng.toFixed(5)}, ${sw.lat.toFixed(5)} | NE: ${ne.lng.toFixed(5)}, ${ne.lat.toFixed(5)}`;
    }
  }

  _setupFpsMonitoring() {
    // Get canvas elements and contexts
    this.fpsCanvas = document.getElementById('fps-graph');
    this.fpsCtx = this.fpsCanvas.getContext('2d');
    
    this.networkCanvas = document.getElementById('network-graph');
    this.networkCtx = this.networkCanvas.getContext('2d');
    
    // Start FPS monitoring loop
    this._startFpsLoop();
    
    // Set up network timing listener
    this._setupNetworkMonitoring();
  }

  _startFpsLoop() {
    const measureFps = (currentTime) => {
      this.frameCount++;
      
      // Calculate FPS every second
      if (currentTime - this.lastTime >= 1000) {
        this.fps = Math.round((this.frameCount * 1000) / (currentTime - this.lastTime));
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

  _stopFpsLoop() {
    if (this.animationId) {
      cancelAnimationFrame(this.animationId);
      this.animationId = null;
    }
  }

  _setupNetworkMonitoring() {
    // Listen for network timing events
    window.addEventListener('tileDownloadTiming', (event) => {
      const { duration } = event.detail;
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

  _updateFpsDisplay() {
    if (!this.visible) return;
    
    const fpsElement = document.getElementById('fps-display');
    if (fpsElement) {
      fpsElement.textContent = this.fps;
      
      // Color code based on FPS
      if (this.fps >= 50) {
        fpsElement.style.color = '#4CAF50'; // Green
      } else if (this.fps >= 30) {
        fpsElement.style.color = '#FF9800'; // Orange
      } else {
        fpsElement.style.color = '#F44336'; // Red
      }
    }
  }

  _updateNetworkDisplay() {
    if (!this.visible) return;
    
    const networkElement = document.getElementById('network-delay-display');
    if (networkElement) {
      networkElement.textContent = this.lastNetworkDelay;
      
      // Color code based on network delay
      if (this.lastNetworkDelay <= 100) {
        networkElement.style.color = '#4CAF50'; // Green - Fast
      } else if (this.lastNetworkDelay <= 500) {
        networkElement.style.color = '#FF9800'; // Orange - Moderate
      } else {
        networkElement.style.color = '#F44336'; // Red - Slow
      }
    }
  }

  _renderFpsGraph() {
    if (!this.visible || !this.fpsCtx) return;
    
    const canvas = this.fpsCanvas;
    const ctx = this.fpsCtx;
    const width = canvas.width;
    const height = canvas.height;
    
    // Clear canvas
    ctx.clearRect(0, 0, width, height);
    
    if (this.fpsHistory.length < 2) return;
    
    // Draw grid lines
    ctx.strokeStyle = '#555';
    ctx.lineWidth = 1;
    
    // Horizontal grid lines (FPS values)
    const gridLines = [30, 60];
    gridLines.forEach(fps => {
      const y = height - (fps / 60) * height;
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(width, y);
      ctx.stroke();
    });
    
    // Draw FPS line
    ctx.strokeStyle = '#2196F3';
    ctx.lineWidth = 2;
    ctx.beginPath();
    
    const stepX = width / (this.maxHistorySize - 1);
    
    this.fpsHistory.forEach((fps, index) => {
      const x = index * stepX;
      const y = height - Math.min(fps / 60, 1) * height; // Normalize to 60 FPS max
      
      if (index === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    });
    
    ctx.stroke();
    
    // Draw FPS labels
    ctx.fillStyle = '#ccc';
    ctx.font = '8px monospace';
    ctx.textAlign = 'left';
    ctx.fillText('60', 2, 10);
    ctx.fillText('30', 2, height - 18);
    ctx.fillText('0', 2, height - 2);
  }

  _renderNetworkGraph() {
    if (!this.visible || !this.networkCtx) return;
    
    const canvas = this.networkCanvas;
    const ctx = this.networkCtx;
    const width = canvas.width;
    const height = canvas.height;
    
    // Clear canvas
    ctx.clearRect(0, 0, width, height);
    
    if (this.networkDelayHistory.length < 2) return;
    
    // Calculate max delay for scaling (minimum 1000ms for consistent scale)
    const maxDelay = Math.max(1000, Math.max(...this.networkDelayHistory));
    
    // Draw grid lines
    ctx.strokeStyle = '#555';
    ctx.lineWidth = 1;
    
    // Horizontal grid lines (delay values)
    const gridLines = [500, 1000];
    gridLines.forEach(delay => {
      if (delay <= maxDelay) {
        const y = height - (delay / maxDelay) * height;
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(width, y);
        ctx.stroke();
      }
    });
    
    // Draw network delay line
    ctx.strokeStyle = '#FF9800';
    ctx.lineWidth = 2;
    ctx.beginPath();
    
    const stepX = width / (this.maxNetworkHistorySize - 1);
    
    this.networkDelayHistory.forEach((delay, index) => {
      const x = index * stepX;
      const y = height - (delay / maxDelay) * height;
      
      if (index === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    });
    
    ctx.stroke();
    
    // Draw delay labels
    ctx.fillStyle = '#ccc';
    ctx.font = '8px monospace';
    ctx.textAlign = 'left';
    if (maxDelay >= 1000) {
      ctx.fillText('1s', 2, 10);
    }
    if (maxDelay >= 500) {
      ctx.fillText('500ms', 2, height - 18);
    }
    ctx.fillText('0', 2, height - 2);
  }

  _setupEasterEgg() {
    let clickCount = 0;
    let lastClickTime = 0;
    let clickTimeout;

    this.map.on('click', (e) => {
      const currentTime = new Date().getTime();
      const clickTimeDelta = currentTime - lastClickTime;
      lastClickTime = currentTime;

      // Reset click count if too much time has passed between clicks
      if (clickTimeDelta > 500) {
        clickCount = 1;
      } else {
        clickCount++;
      }

      // Clear any existing timeout
      if (clickTimeout) {
        clearTimeout(clickTimeout);
      }

      // Set a timeout to reset click count after a delay
      clickTimeout = setTimeout(() => {
        clickCount = 0;
      }, 500);

      // Check for triple click near 0,0
      if (clickCount === 3) {
        // Check if click is within ±1 degree of 0,0
        const { lng, lat } = e.lngLat;
        if (Math.abs(lng) <= 1 && Math.abs(lat) <= 1) {
          // Toggle debug panel
          if (this.visible) {
            this.hide();
            this._updateUrlHash({ debug: 'false' });
          } else {
            this.show();
            this._updateUrlHash({ debug: 'true' });
          }
        }
      }
    });
  }

  _updateUrlHash(params) {
    const hash = window.location.hash.slice(1);
    const urlParams = new URLSearchParams(hash);
    
    // Update or add provided params
    Object.keys(params).forEach(key => {
      if (params[key] === null) {
        urlParams.delete(key);
      } else {
        urlParams.set(key, params[key]);
      }
    });
    
    // Update URL without reloading page
    window.location.hash = urlParams.toString();
  }

  _checkDebugStatus() {
    const hash = window.location.hash.slice(1);
    const urlParams = new URLSearchParams(hash);
    const debugParam = urlParams.get('debug');
    
    if (debugParam === 'true') {
      this.show();
      
      const renderingMode = urlParams.get('render') || 'canvas';
      
      // Only set rendering mode if it's available
      const renderingModeSelect = document.getElementById('rendering-mode');
      if (this.availableLayers[renderingMode] && renderingModeSelect) {
        renderingModeSelect.value = renderingMode;
      }
      
      // Update viewpoint info
      this._updateViewpointInfo();
    } else {
      this.hide();
    }
  }

  show() {
    this.panel.style.display = 'block';
    this.visible = true;
    this._updateViewpointInfo();
    this._updateNetworkDisplay();
    this._renderNetworkGraph();
    
    // Start FPS monitoring when panel is shown
    if (!this.animationId) {
      this._startFpsLoop();
    }
  }

  hide() {
    this.panel.style.display = 'none';
    this.visible = false;
    
    // Stop FPS monitoring when panel is hidden to save resources
    this._stopFpsLoop();
  }

  // Listen for hash changes to show/hide panel
  listenForHashChanges() {
    window.addEventListener('hashchange', () => {
      this._checkDebugStatus();
    });
  }

  // Clean up resources
  destroy() {
    this._stopFpsLoop();
    if (this.panel && this.panel.parentNode) {
      this.panel.parentNode.removeChild(this.panel);
    }
  }
} 