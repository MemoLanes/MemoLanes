import { JourneyCanvasLayer } from './journey-canvas-layer.js';
import { JourneyTileProvider } from './journey-tile-provider.js';
import { DebugPanel } from './debug-panel.js';
import mapboxgl from 'mapbox-gl';
import 'mapbox-gl/dist/mapbox-gl.css';
import './debug-panel.css';

// Available rendering layers
const AVAILABLE_LAYERS = {
    'canvas': {
        name: 'Canvas',
        layerClass: JourneyCanvasLayer,
        bufferSizePower: 8,
        description: 'Uses Canvas API for rendering'
    }
};

let currentJourneyLayer;  // Store reference to current layer
let currentJourneyId;
let currentJourneyTileProvider;
let pollingInterval;      // Store reference to polling interval
let locationMarker = null;
let currentRenderingMode = 'canvas'; // Default rendering mode

// Function to switch between rendering layers
function switchRenderingLayer(map, renderingMode) {
    if (!AVAILABLE_LAYERS[renderingMode]) {
        console.warn(`Rendering mode '${renderingMode}' not available, using canvas instead.`);
        renderingMode = 'canvas';
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

async function initializeMap() {
    // Load Mapbox token from .token.json
    const tokenResponse = await fetch('./token.json');
    const tokenData = await tokenResponse.json();
    mapboxgl.accessToken = tokenData['MAPBOX-ACCESS-TOKEN'];

    const hash = window.location.hash.slice(1);

    // Parse hash parameters for initial map view
    let initialView = {
        center: [0, 0],
        zoom: 2
    };

    if (hash) {
        const params = new URLSearchParams(hash);
        currentJourneyId = params.get('journey_id');
        const lng = parseFloat(params.get('lng'));
        const lat = parseFloat(params.get('lat'));
        const zoom = parseFloat(params.get('zoom'));
        
        // Get rendering mode from URL if available
        const urlRenderMode = params.get('render');
        if (urlRenderMode && AVAILABLE_LAYERS[urlRenderMode]) {
            currentRenderingMode = urlRenderMode;
        }
        
        console.log(`journey_id: ${currentJourneyId}, render: ${currentRenderingMode}, lng: ${lng}, lat: ${lat}, zoom: ${zoom}`);

        if (!isNaN(lng) && !isNaN(lat) && !isNaN(zoom)) {
            initialView = {
                center: [lng, lat],
                zoom: zoom
            };
        }
    }

    const map = new mapboxgl.Map({
        container: 'map',
        style: 'mapbox://styles/mapbox/streets-v12',
        center: initialView.center,
        zoom: initialView.zoom,
        maxZoom: 14,
        antialias: true,
        projection: 'mercator',
        pitch: 0,
        pitchWithRotate: false,
        touchPitch: false,
    });
    map.dragRotate.disable();
    map.touchZoomRotate.disableRotation();

    map.on('style.load', async (e) => {
        // Create a DOM element for the marker
        const el = document.createElement('div');
        el.className = 'location-marker';

        // Create the marker but don't add it to the map yet
        locationMarker = new mapboxgl.Marker(el);

        // Add method to window object to update marker position
        window.updateLocationMarker = function (lng, lat, show = true, flyto = false) {
            if (show) {
                locationMarker.setLngLat([lng, lat]).addTo(map);
                if (flyto) {
                    const currentZoom = map.getZoom();
                    map.flyTo({
                        center: [lng, lat],
                        zoom: currentZoom < 14 ? 16 : currentZoom,
                        essential: true
                    });
                }
            } else {
                locationMarker.remove();
            }
        };

        currentJourneyTileProvider = new JourneyTileProvider(map, currentJourneyId, AVAILABLE_LAYERS[currentRenderingMode].bufferSizePower);
        
        await currentJourneyTileProvider.pollForJourneyUpdates(true);
        console.log('initial tile buffer loaded');

        // Create and initialize journey layer with selected rendering mode
        currentJourneyLayer = switchRenderingLayer(map, currentRenderingMode);

        // Set up polling for updates
        pollingInterval = setInterval(() => currentJourneyTileProvider.pollForJourneyUpdates(false), 1000);

        // Create and initialize the debug panel
        const debugPanel = new DebugPanel(map, AVAILABLE_LAYERS);
        debugPanel.initialize();
        debugPanel.listenForHashChanges();

        // give the map a little time to render before notifying Flutter
        setTimeout(() => {
            if (window.readyForDisplay) {
                window.readyForDisplay.postMessage('');
            }
        }, 200);
    });

    // Replace the simple movestart listener with dragstart
    map.on('dragstart', () => {
        // Only notify Flutter when user drags the map
        if (window.onMapMoved) {
            window.onMapMoved.postMessage('');
        }
    });

    // Listen for zoom changes
    map.on('zoomstart', (event) => {
        let fromUser = event.originalEvent && (event.originalEvent.type !== 'resize')
        if (fromUser && window.onMapMoved) {
            window.onMapMoved.postMessage('');
        }
    });

    // Add method to window object to get current map view
    window.getCurrentMapView = function () {
        const center = map.getCenter();
        return JSON.stringify({
            lng: center.lng,
            lat: center.lat,
            zoom: map.getZoom()
        });
    };

    // Listen for hash changes
    window.addEventListener('hashchange', () => {
        const hash = window.location.hash.slice(1);
        const params = new URLSearchParams(hash);
        
        // Check if journey ID has changed
        const newJourneyId = params.get('journey_id');
        if (newJourneyId !== currentJourneyId && newJourneyId !== null) {
            currentJourneyId = newJourneyId;
            currentJourneyTileProvider.journeyId = currentJourneyId;
            currentJourneyTileProvider.pollForJourneyUpdates(true);
        }
        
        // Check if rendering mode has changed
        const newRenderMode = params.get('render');
        if (newRenderMode && newRenderMode !== currentRenderingMode && AVAILABLE_LAYERS[newRenderMode]) {
            switchRenderingLayer(map, newRenderMode);
        }
    });

    // Add method to window object to trigger manual update
    window.triggerJourneyUpdate = function () {
        return currentJourneyTileProvider.pollForJourneyUpdates(false);
    };
    
    // Add method to switch rendering layers
    window.switchRenderingLayer = function(renderingMode) {
        return switchRenderingLayer(map, renderingMode);
    };
}

// Start initialization
initializeMap().catch(console.error);