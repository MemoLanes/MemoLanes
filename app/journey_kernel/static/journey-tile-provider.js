import { JourneyBitmap } from '../pkg';
import { LRUCache } from './lru-cache';

export class JourneyTileProvider {
    constructor(map, journeyId, frontEndRendering = true) {
        this.map = map;
        this.journeyId = journeyId;
        this.frontEndRendering = frontEndRendering;
        this.onUpdateCallbacks = []; // Array to store update callbacks
        this.tileCache = new LRUCache(200); // LRU cache with a capacity of 200 tiles
        this.currentVersion = null; // Store the current version
        
        // Create blank tile image data once
        const blankCanvas = document.createElement('canvas');
        blankCanvas.width = 256;
        blankCanvas.height = 256;
        const ctx = blankCanvas.getContext('2d');
        // TODO: remove these hard-coded values
        ctx.fillStyle = 'rgba(0,0,0,0.5)'; // Foggy
        ctx.fillRect(0, 0, 256, 256);
        this.blankTileData = ctx.getImageData(0, 0, 256, 256).data;

        if (this.frontEndRendering) {
            // this.pollForJourneyUpdates(true);
        } else {
            // TODO: server-side rendering
        }
    }
    
    // TODO: make it support server-side rendering.
    get_tile_image(x, y, z) {
        if (this.frontEndRendering) {
            return this.journeyBitmap.get_tile_image(BigInt(x), BigInt(y), z);
        } else {
            // Server-side rendering
            const tileKey = `${z}/${x}/${y}`;
            
            // Check cache first
            if (this.tileCache.has(tileKey)) {
                return this.tileCache.get(tileKey);
            }
            
            // Return blank tile immediately
            const blankTile = new Uint8ClampedArray(this.blankTileData);
            
            // Fetch from server asynchronously
            this.fetchTileFromServer(x, y, z, tileKey);
            
            return blankTile;
        }
    }

    async fetchTileFromServer(x, y, z, tileKey) {
        try {
            const tilePath = getJourneyTileFilePathWithId(this.journeyId, x, y, z);
            const response = await fetch(tilePath, {
                cache: 'force-cache' // Use browser's cache
            });
            
            if (!response.ok) throw new Error(`Failed to fetch tile: ${response.status}`);
            
            const blob = await response.blob();
            const bitmap = await createImageBitmap(blob);
            
            // Draw the bitmap to a canvas to get image data
            const canvas = document.createElement('canvas');
            canvas.width = 256;
            canvas.height = 256;
            const ctx = canvas.getContext('2d');
            ctx.drawImage(bitmap, 0, 0);
            const imageData = ctx.getImageData(0, 0, 256, 256).data;
            
            // Store in cache
            this.tileCache.set(tileKey, imageData);
            
            // Notify that we need to redraw
            this.notifyTileUpdate(tileKey);
        } catch (error) {
            console.error('Error fetching tile:', error);
        }
    }

    // Add a method to notify when a specific tile is updated
    notifyTileUpdate(tileKey) {
        for (const callback of this.onUpdateCallbacks) {
            callback(tileKey);
        }
    }

    // Register a callback to be called when journey data updates
    registerUpdateCallback(callback) {
        this.onUpdateCallbacks.push(callback);
    }
    
    // Remove a previously registered callback
    unregisterUpdateCallback(callback) {
        this.onUpdateCallbacks = this.onUpdateCallbacks.filter(cb => cb !== callback);
    }
    
    // Notify all registered callbacks
    notifyUpdates() {
        for (const callback of this.onUpdateCallbacks) {
            callback();
        }
    }

    // typically two use cases: if the original page detect a data change, then no cache (forceUpdate = true)
    // if it is just a periodic update or normal check, then use cache (forceUpdate = false)
    async pollForJourneyUpdates(forceUpdate = false) {
        const filePath = getJourneyFilePathWithId(this.journeyId);
        
        console.log(`Fetching ${filePath}`);

        // Use version tracking unless force update is requested
        const fetchOptions = {
            headers: {}
        };
        
        if (!forceUpdate && this.currentVersion) {
            fetchOptions.headers['If-None-Match'] = this.currentVersion;
        }

        try {
            const response = await fetch(`${filePath}`, fetchOptions);

            // Store the new version if available
            const newVersion = response.headers.get('ETag');
            if (newVersion) {
                this.currentVersion = newVersion;
                console.log(`Updated version to: ${newVersion}`);
            }

            // If server returns 304 Not Modified, return null
            if (response.status === 304) {
                console.log('Content not modified');
                return null;
            }
            
            // If server returns 404 Not Found
            if (response.status === 404) {
                console.error(`Journey not found: ${filePath}`);
                return null;
            }
            
            if (!response.ok) {
                console.error(`Failed to fetch journey: ${response.status} ${response.statusText}`);
                return null;
            }

            const arrayBuffer = await response.arrayBuffer();
            const journeyBitmap = JourneyBitmap.from_bytes(new Uint8Array(arrayBuffer));
            console.log(`Loaded ${filePath}`);

            // TODO: only do this when front-end rendering is enabled.
            this.journeyBitmap = journeyBitmap;
            this.notifyUpdates();

            // Try to fetch provisioned camera location
            let cameraOptions = null;
            try {
                const cameraResponse = await fetch(`${getJourneyCameraOptionPathWithId(this.journeyId)}`);
                if (cameraResponse.ok) {
                    const cameraData = await cameraResponse.json();
                    cameraOptions = {
                        center: [cameraData.lng, cameraData.lat],
                        zoom: cameraData.zoom
                    };
                    console.log('Using provisioned camera location:', cameraData);
                    // TODO: if it is initial, set locations directly rather than flyTo (no animation)
                    this.map.flyTo(cameraOptions);
                }
            } catch (error) {
                console.log('No provisioned camera location available:', error);
            }
        } catch (error) {
            console.error('Error while fetching journey data:', error);
            return null;
        }
    } 
}

function getJourneyFilePathWithId(journeyId) {
    return journeyId ? `journey/${journeyId}/journey_bitmap.bin` : `journey_bitmap.bin`;
}

function getJourneyTileFilePathWithId(journeyId, x, y, z) {
    return `journey/${journeyId}/tiles/${z}/${x}/${y}.png`;
}

function getJourneyCameraOptionPathWithId(journeyId) {
    return `journey/${journeyId}/provisioned_camera_option`;
}
