/**
 * Debug Panel for Map Overlay
 * Shows when debug=true is in URL hash
 * Controls caching and rendering modes
 */

export class DebugPanel {
  constructor(map) {
    this.map = map;
    this.panel = null;
    this.visible = false;
  }

  initialize() {
    // Create panel element
    this.panel = document.createElement('div');
    this.panel.className = 'debug-panel';
    this.panel.style.display = 'none';

    // Add content to panel
    this.panel.innerHTML = `
      <div style="font-weight: bold; margin-bottom: 10px; display: flex; justify-content: space-between;">
        <span>Debug Panel</span>
        <button id="close-debug">×</button>
      </div>
      
      <div style="margin-bottom: 10px;">
        <label for="caching-mode">Caching Mode:</label>
        <select id="caching-mode">
          <option value="auto">Auto</option>
          <option value="performance">Performance</option>
          <option value="light">Light</option>
        </select>
      </div>
      
      <div class="separator"></div>
      
      <div style="margin-bottom: 10px;">
        <label for="rendering-mode">Rendering Mode:</label>
        <select id="rendering-mode">
          <option value="auto">Auto</option>
          <option value="canvas">Canvas</option>
        </select>
      </div>
      
      <button id="apply-settings">Apply Settings</button>
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

    // Apply settings button
    document.getElementById('apply-settings').addEventListener('click', () => {
      const cachingMode = document.getElementById('caching-mode').value;
      const renderingMode = document.getElementById('rendering-mode').value;
      
      this._updateUrlHash({
        cache: cachingMode,
        render: renderingMode
      });
    });
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
      urlParams.set(key, params[key]);
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
      const renderingMode = urlParams.get('render') || 'auto';
      
      document.getElementById('caching-mode').value = cachingMode;
      document.getElementById('rendering-mode').value = renderingMode;
    } else {
      this.hide();
    }
  }

  show() {
    this.panel.style.display = 'block';
    this.visible = true;
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