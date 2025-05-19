/**
 * Debug Panel for Map Overlay
 * Shows when debug=true is in URL hash
 * Controls caching and rendering modes
 */

export class DebugPanel {
  constructor(map, availableLayers = {}) {
    this.map = map;
    this.panel = null;
    this.visible = false;
    this.availableLayers = availableLayers;
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
      
      <div style="margin-bottom: 10px;">
        <label for="caching-mode" title="Controls how journey data is loaded and cached">Caching Mode:</label>
        <select id="caching-mode">
          <option value="auto" title="System decides best caching strategy">Auto</option>
          <option value="performance" title="Frontend rendering: loads full journey data for better performance">Performance</option>
          <option value="light" title="Server-side rendering: loads only visible tiles for lower memory usage">Light</option>
        </select>
        <div class="hint">Performance = Frontend rendering, Light = Server tiles</div>
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

    // Caching mode direct change handler
    document.getElementById('caching-mode').addEventListener('change', (e) => {
      const cachingMode = e.target.value;
      this._updateUrlHash({ cache: cachingMode });
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
      
      // Set initial values from URL params
      const cachingMode = urlParams.get('cache') || 'auto';
      const renderingMode = urlParams.get('render') || 'canvas';
      
      document.getElementById('caching-mode').value = cachingMode;
      
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
  }

  hide() {
    this.panel.style.display = 'none';
    this.visible = false;
  }

  // Listen for hash changes to show/hide panel
  listenForHashChanges() {
    window.addEventListener('hashchange', () => {
      this._checkDebugStatus();
    });
  }
} 