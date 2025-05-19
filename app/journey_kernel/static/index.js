import { JourneyCanvasLayer } from './journey-canvas-layer.js';
import { JourneyTileProvider } from './journey-tile-provider.js';
import { DebugPanel } from './debug-panel.js';
import mapboxgl from 'mapbox-gl';
import 'mapbox-gl/dist/mapbox-gl.css';
import './debug-panel.css';

let currentJourneyLayer;  // Store reference to current layer
let currentJourneyId;
let currentJourneyTileProvider;
let pollingInterval;      // Store reference to polling interval
let locationMarker = null;

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

    // Default frontEndRendering to true
    let frontEndRendering = true;

    if (hash) {
        const params = new URLSearchParams(hash);
        currentJourneyId = params.get('journey_id');
        const lng = parseFloat(params.get('lng'));
        const lat = parseFloat(params.get('lat'));
        const zoom = parseFloat(params.get('zoom'));
        // Parse frontEndRendering parameter, default to true
        const frontEndRenderingParam = params.get('frontEndRendering');
        if (frontEndRenderingParam !== null) {
            frontEndRendering = frontEndRenderingParam.toLowerCase() === 'true';
        }

        console.log(`journey_id: ${currentJourneyId}, frontEndRendering: ${frontEndRendering}, lng: ${lng}, lat: ${lat}, zoom: ${zoom}`);

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

    // TODO: start loading the initial data earlier.

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

        currentJourneyTileProvider = new JourneyTileProvider(map, currentJourneyId, frontEndRendering);
        if (frontEndRendering) {
            await currentJourneyTileProvider.pollForJourneyUpdates(true);
        }

        // Create and store journey layer
        currentJourneyLayer = new JourneyCanvasLayer(map, currentJourneyTileProvider);

        currentJourneyLayer.initialize();

        // TODO: only use for custom gl layer
        // map.addLayer(currentJourneyLayer);

        map.on("move", () => currentJourneyLayer.render());
        map.on("moveend", () => currentJourneyLayer.render());

        // Set up polling for updates
        pollingInterval = setInterval(() => currentJourneyTileProvider.pollForJourneyUpdates(false), 1000);

        // Create and initialize the debug panel
        const debugPanel = new DebugPanel(map);
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

    window.addEventListener('hashchange', () => currentJourneyTileProvider.pollForJourneyUpdates(true));

    // Add method to window object to trigger manual update
    window.triggerJourneyUpdate = function () {
        return currentJourneyTileProvider.pollForJourneyUpdates(false);
    };
}

// Start initialization
initializeMap().catch(console.error);