import { JourneyBitmap } from '../pkg';
import { LRUCache } from './lru-cache';
import { tileXYZToKey } from './utils';

export class JourneyTileProvider {
    constructor(map, journeyId, frontEndRendering = true) {
        this.map = map;
        this.journeyId = journeyId;
        this.frontEndRendering = frontEndRendering;
        this.onUpdateCallbacks = []; // Array to store update callbacks
        this.tileCache = new LRUCache(200); // LRU cache with a capacity of 200 tiles
        this.currentVersion = null; // Store the current version
        this.subscribed_range = null; // Store the current subscribed tile range and zoom
        this.tileExtension = "imagedata"; // Default tile extension
        
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

    getTileData(x, y, z) {
        if (this.frontEndRendering) {
            return this.journeyBitmap.get_tile_data(BigInt(x), BigInt(y), z, this.tileExtension);
        } else {
            // Server-side rendering
            const tileKey = tileXYZToKey(x, y, z);
            
            // Check cache first
            if (this.tileCache.has(tileKey)) {
                return this.tileCache.get(tileKey);
            }
            
            // Return blank tile immediately
            const blankTile = new Uint8ClampedArray(this.blankTileData);
            
            // Fetch from server asynchronously
            this.fetchTileFromServer(x, y, z);
            
            return blankTile;
        }
    }

    async fetchTileFromServer(x, y, z, disableCache = false) {
        const tileKey = tileXYZToKey(x, y, z);
        try {
            const tilePath = getJourneyTileFilePathWithId(this.journeyId, x, y, z, this.tileExtension);
            const response = await fetch(tilePath, {
                cache: disableCache ? 'no-cache' : 'force-cache' // Disable browser cache if requested
            });
            
            if (!response.ok) throw new Error(`Failed to fetch tile: ${response.status}`);
            
            // Get binary image data directly as ArrayBuffer
            const arrayBuffer = await response.arrayBuffer();
            const imageData = new Uint8ClampedArray(arrayBuffer);
            
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

    // Save the current tile range and zoom level that is being displayed
    setSubscribedRange(tileRange, zoom) {
        this.subscribed_range = {
            tileRange: [...tileRange], // Make a copy of the array
            zoom: zoom
        };
    }

    // Fetch all tiles within the current subscribed range
    fetchTilesForSubscribedRange() {
        if (!this.subscribed_range) {
            console.warn('No subscribed range available to fetch tiles for');
            return;
        }

        const { tileRange, zoom } = this.subscribed_range;
        const [left, top, right, bottom] = tileRange;

        console.log(`Fetching tiles for subscribed range: zoom=${zoom}, range=${tileRange}`);

        // Fetch all tiles in the range
        for (let x = left; x <= right; x++) {
            for (let y = top; y <= bottom; y++) {
                const tileKey = tileXYZToKey(x, y, zoom);
                // Remove from cache to force refetch
                this.tileCache.remove(tileKey);
                // Fetch the tile with cache disabled
                this.fetchTileFromServer(x, y, zoom, true);
            }
        }
    }

    // typically two use cases: if the original page detect a data change, then no cache (forceUpdate = true)
    // if it is just a periodic update or normal check, then use cache (forceUpdate = false)
    async pollForJourneyUpdates(forceUpdate = false) {
        if (this.frontEndRendering) {
            // Front-end rendering: fetch the full journey bitmap
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

                this.journeyBitmap = journeyBitmap;
                this.notifyUpdates();

                // Try to fetch provisioned camera location
                this.fetchCameraOptions();
            } catch (error) {
                console.error('Error while fetching journey data:', error);
                return null;
            }
        } else {
            // Server-side rendering: use HEAD request to check if journey data has changed
            try {
                const filePath = getJourneyFilePathWithId(this.journeyId);
                console.log(`Checking for updates with HEAD request: ${filePath}`);
                
                const fetchOptions = {
                    method: 'HEAD',
                    headers: {}
                };
                
                if (!forceUpdate && this.currentVersion) {
                    fetchOptions.headers['If-None-Match'] = this.currentVersion;
                }
                
                const response = await fetch(filePath, fetchOptions);
                
                // Store the new version if available
                const newVersion = response.headers.get('ETag');
                const isChanged = newVersion && newVersion !== this.currentVersion;
                
                if (newVersion) {
                    this.currentVersion = newVersion;
                    console.log(`Updated version to: ${newVersion}`);
                }
                
                if (response.status === 304) {
                    console.log('Journey data has not changed');
                    return null;
                }
                
                if (response.status === 404) {
                    console.error(`Journey not found: ${filePath}`);
                    return null;
                }
                
                if (!response.ok) {
                    console.error(`Failed to check for updates: ${response.status} ${response.statusText}`);
                    return null;
                }
                
                if (isChanged || response.status === 200) {
                    console.log('Journey data has changed, updating tiles');
                    
                    // Fetch tiles for the current subscribed range
                    if (this.subscribed_range) {
                        this.fetchTilesForSubscribedRange();
                    } else {
                        console.warn('No subscribed range available, cannot update tiles');
                    }
                    
                    // Notify listeners that data has changed
                    this.notifyUpdates();
                    
                    // Fetch camera options if needed
                    this.fetchCameraOptions();
                } else {
                    console.log('Journey data has not changed');
                }
            } catch (error) {
                console.error('Error while checking for journey updates:', error);
                return null;
            }
        }
    }
    
    // Helper method to fetch camera options
    async fetchCameraOptions() {
        try {
            const cameraResponse = await fetch(`${getJourneyCameraOptionPathWithId(this.journeyId)}`);
            if (cameraResponse.ok) {
                const cameraData = await cameraResponse.json();
                const cameraOptions = {
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
    }

    // Setter method to update frontEndRendering mode
    setFrontEndRendering(enabled) {
        if (this.frontEndRendering === enabled) {
            // No change, return early
            return false;
        }
        
        console.log(`Switching rendering mode: frontEndRendering=${enabled}`);
        this.frontEndRendering = enabled;
        
        // Clear tile cache when switching modes
        this.tileCache.clear();
        
        // If switching to front-end rendering, fetch the full journey bitmap
        if (enabled) {
            // Trigger a full refresh
            this.pollForJourneyUpdates(true);
        } else {
            // For server-side rendering, fetch tiles for the current view
            if (this.subscribed_range) {
                this.fetchTilesForSubscribedRange();
            }
        }
        
        // Notify all listeners that the rendering mode has changed
        this.notifyUpdates();
        
        return true; // Indicate that the mode was changed
    }

    // Setter method to update the tile extension
    setTileExtension(extension) {
        if (typeof extension === 'string' && extension.length > 0) {
            this.tileExtension = extension;
            return true;
        }
        return false;
    }
}

function getJourneyFilePathWithId(journeyId) {
    return journeyId ? `journey/${journeyId}/journey_bitmap.bin` : `journey_bitmap.bin`;
}

function getJourneyTileFilePathWithId(journeyId, x, y, z, extension = "imagedata") {
    return `journey/${journeyId}/tiles/${z}/${x}/${y}.${extension}`;
}

function getJourneyCameraOptionPathWithId(journeyId) {
    return `journey/${journeyId}/provisioned_camera_option`;
}
