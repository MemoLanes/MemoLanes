import init, { JourneyBitmap } from '../pkg/journey_kernel.js';
import { JourneyCanvasLayer } from './journey-canvas-layer.js';
import mapboxgl from 'mapbox-gl';
import 'mapbox-gl/dist/mapbox-gl.css';

let currentJourneyLayer;  // Store reference to current layer
let pollingInterval;      // Store reference to polling interval
let locationMarker = null;

async function loadJourneyData(filename, useIfNoneMatch = false) {
    console.log(`Fetching ${filename}`);
    const fetchOptions = {
        headers: useIfNoneMatch ? { 'If-None-Match': '*' } : {}
    };

    const response = await fetch(`${filename}`, fetchOptions);

    // If server returns 304 Not Modified, return null
    if (response.status === 304) {
        return null;
    }

    const arrayBuffer = await response.arrayBuffer();
    const journeyBitmap = JourneyBitmap.from_bytes(new Uint8Array(arrayBuffer));
    console.log(`Loaded ${filename}`);

    // Try to fetch provisioned camera location
    let cameraOptions = null;
    try {
        const cameraResponse = await fetch(`${filename}/provisioned_camera_option`);
        if (cameraResponse.ok) {
            const cameraData = await cameraResponse.json();
            cameraOptions = {
                center: [cameraData.lng, cameraData.lat],
                zoom: cameraData.zoom
            };
            console.log('Using provisioned camera location:', cameraData);
        }
    } catch (error) {
        console.log('No provisioned camera location available:', error);
    }

    return { journeyBitmap, cameraOptions };
}

async function pollForUpdates(filename, map) {
    try {
        const result = await loadJourneyData(filename, true);
        if (result) {
            if (result.journeyBitmap) {
                console.log('Update detected, updating journey bitmap');
                currentJourneyLayer.updateJourneyBitmap(result.journeyBitmap);
            }
            if (result.cameraOptions) {
                console.log('Camera update detected, flying to new location');
                map.flyTo(result.cameraOptions);
            }
        }
    } catch (error) {
        console.error('Error polling for updates:', error);
    }
}

async function handleHashChange(map) {
    const hash = window.location.hash.slice(1);
    const filename = hash ? `items/${hash}` : '../journey_bitmap.bin';

    // Clear existing polling interval
    if (pollingInterval) {
        clearInterval(pollingInterval);
    }

    try {
        const result = await loadJourneyData(filename);
        if (result) {
            const { journeyBitmap, cameraOptions } = result;

            // Update existing layer if it exists
            if (currentJourneyLayer) {
                currentJourneyLayer.updateJourneyBitmap(journeyBitmap);
            }

            // Only animate to new camera position if cameraOptions is provided
            if (cameraOptions) {
                map.flyTo(cameraOptions);
            }

            // Set up polling for updates every 1 second
            pollingInterval = setInterval(() => pollForUpdates(filename, map), 1000);
        }
    } catch (error) {
        console.error('Error loading journey data:', error);
    }
}

async function initializeMap() {
    await init();

    // Load Mapbox token from .token.json
    const tokenResponse = await fetch('./token.json');
    const tokenData = await tokenResponse.json();
    mapboxgl.accessToken = tokenData['MAPBOX-ACCESS-TOKEN'];

    const map = new mapboxgl.Map({
        container: 'map',
        style: 'mapbox://styles/mapbox/streets-v12',
        center: [0, 0],
        zoom: 2,
        maxZoom: 14,
        antialias: true,
        projection: 'mercator',
        pitch: 0,
        pitchWithRotate: false,
        touchPitch: false,
    });

    // Wait for the map to be fully loaded before proceeding
    await new Promise(resolve => {
        if (map.loaded()) {
            resolve();
        } else {
            map.once('load', resolve);
        }
    });

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

    // Initial load of journey data
    const hash = window.location.hash.slice(1);
    const filename = hash ? `items/${hash}` : '../journey_bitmap.bin';
    const result = await loadJourneyData(filename);

    if (result) {
        const { journeyBitmap, cameraOptions } = result;

        // Update initial camera position only if cameraOptions is provided
        if (cameraOptions) {
            map.setCenter(cameraOptions.center);
            map.setZoom(cameraOptions.zoom);
        }

        // Create and store journey layer
        currentJourneyLayer = new JourneyCanvasLayer(map, journeyBitmap);

        map.addSource("main-canvas-source", currentJourneyLayer.getSourceConfig());
        map.addLayer({
            id: "main-canvas-layer",
            source: "main-canvas-source",
            type: "raster",
            paint: {
                "raster-fade-duration": 0,
            },
        });
        currentJourneyLayer.render();
        map.addLayer(currentJourneyLayer);

        map.on("move", () => currentJourneyLayer.render());
        map.on("moveend", () => currentJourneyLayer.render());

        // Set up polling for updates
        pollingInterval = setInterval(() => pollForUpdates(filename, map), 1000);
    }

    // Listen for hash changes
    window.addEventListener('hashchange', () => handleHashChange(map));

    // Replace the simple movestart listener with dragstart
    map.on('dragstart', () => {
        // Only notify Flutter when user drags the map
        if (window.onMapMoved) {
            window.onMapMoved.postMessage('');
        }
    });
}

// Start initialization
initializeMap().catch(console.error);