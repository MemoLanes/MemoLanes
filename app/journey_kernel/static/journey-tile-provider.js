import { JourneyBitmap, TileBuffer } from '../pkg';
import { getViewportTileRange } from './utils';

export class JourneyTileProvider {
    constructor(map, journeyId, bufferSizePower = 8) {
        this.map = map;
        this.journeyId = journeyId;
        this.currentVersion = null; // Store the current version
        this.viewRange = null; // Store the current viewport tile range [x, y, w, h, z]
        this.tileBuffer = null; // Store the tile buffer data
        this.viewRangeUpdated = false; // Flag indicating view range has been updated
        this.downloadInProgress = false; // Flag indicating download is in progress
        this.bufferSizePower = bufferSizePower;

        this.tileBufferCallbacks = []; // Array to store tile buffer update callbacks
        
        this.map.on('move', () => this.tryUpdateViewRange());
        this.map.on('moveend', () => this.tryUpdateViewRange());
        // Initial update
        this.tryUpdateViewRange();
    }

    // typically two use cases: if the original page detect a data change, then no cache (forceUpdate = true)
    // if it is just a periodic update or normal check, then use cache (forceUpdate = false)
    async pollForJourneyUpdates(forceUpdate = false) {
        try {
            console.log('Checking for journey updates via tile buffer');
            
            // Force update view range and fetch tile buffer
            this.viewRangeUpdated = true;
            const tileBufferUpdated = await this.checkAndFetchTileBuffer(forceUpdate);
            
            // If tile buffer was updated (new version), also fetch camera options
            if (tileBufferUpdated) {
                console.log('Journey data has changed, fetching camera options');
                await this.fetchCameraOptions();
            }
            
            return tileBufferUpdated;
        } catch (error) {
            console.error('Error while checking for journey updates:', error);
            return null;
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

    setBufferSizePower(bufferSizePower) {
        if (this.bufferSizePower === bufferSizePower) {
            return;
        }

        console.log(`Switching buffer size power: ${this.bufferSizePower} -> ${bufferSizePower}`);
        this.bufferSizePower = bufferSizePower;
        this.pollForJourneyUpdates(true);
    }

    // Try to update the current viewport tile range, only if it has changed
    tryUpdateViewRange() {
        const [x, y, w, h, z] = getViewportTileRange(this.map);
        
        // Skip update if the values haven't changed
        if (this.viewRange && 
            this.viewRange[0] === x && 
            this.viewRange[1] === y && 
            this.viewRange[2] === w && 
            this.viewRange[3] === h && 
            this.viewRange[4] === z) {
            return this.viewRange;
        }
        
        // Update only when values have changed
        this.viewRange = [x, y, w, h, z];
        console.log(`View range updated: x=${x}, y=${y}, w=${w}, h=${h}, z=${z}`);
        
        // Mark that view range has been updated and trigger fetch if not already downloading
        // Force download since we need tiles for a different area
        this.viewRangeUpdated = true;

        this.checkAndFetchTileBuffer(true); // Force update when view range changes
        
        return this.viewRange;
    }
    
    // Check state and fetch tile buffer if needed
    async checkAndFetchTileBuffer(forceUpdate = false) {
        // If no download is in progress and view range has been updated, fetch new tile buffer
        if (!this.downloadInProgress && this.viewRangeUpdated) {
            return await this.fetchTileBuffer(forceUpdate);
        }
        return false;
    }
    
    // Register a callback to be called when new tile buffer is ready
    registerTileBufferCallback(callback) {
        if (typeof callback === 'function' && !this.tileBufferCallbacks.includes(callback)) {
            this.tileBufferCallbacks.push(callback);
            callback(this.viewRange[0], this.viewRange[1], this.viewRange[2], this.viewRange[3], this.viewRange[4], this.bufferSizePower, this.tileBuffer);
            return true;
        }
        return false;
    }
    
    // Remove a previously registered callback
    unregisterTileBufferCallback(callback) {
        const index = this.tileBufferCallbacks.indexOf(callback);
        if (index !== -1) {
            this.tileBufferCallbacks.splice(index, 1);
            return true;
        }
        return false;
    }
    
    // Notify all registered callbacks with tile range and buffer
    notifyTileBufferReady(x, y, w, h, z, bufferSizePower, tileBuffer) {
        for (const callback of this.tileBufferCallbacks) {
            try {
                callback(x, y, w, h, z, bufferSizePower, tileBuffer);
            } catch (error) {
                console.error('Error in tile buffer callback:', error);
            }
        }
    }

    // Fetch tile buffer for current view range
    async fetchTileBuffer(forceUpdate = false) {
        if (!this.viewRange) return false;
        
        // Reset update flag and set download flag
        this.viewRangeUpdated = false;
        this.downloadInProgress = true;
        
        const [x, y, w, h, z] = this.viewRange;
        const tileRangeUrl = getJourneyTileRangePathWithId(this.journeyId, x, y, w, h, z, this.bufferSizePower);
        
        console.log(`Fetching tile buffer from: ${tileRangeUrl}`);
        
        let tileBufferUpdated = false;
        
        try {
            const fetchOptions = {
                headers: {}
            };
            
            // Add ETag header for conditional request if we have a current version and not forcing update
            if (!forceUpdate && this.currentVersion) {
                fetchOptions.headers['If-None-Match'] = this.currentVersion;
            }
            
            // Measure fetch timing
            const fetchStartTime = performance.now();
            const response = await fetch(tileRangeUrl, fetchOptions);
            const fetchEndTime = performance.now();
            const fetchDuration = fetchEndTime - fetchStartTime;
            
            if (response.status === 304) {
                console.log('Tile buffer has not changed (304 Not Modified)');
                return false;
            }
            
            if (!response.ok) {
                throw new Error(`Failed to fetch tile buffer: ${response.status} ${response.statusText}`);
            }
            
            // Emit timing data for successful downloads (not 304)
            window.dispatchEvent(new CustomEvent('tileDownloadTiming', {
                detail: {
                    duration: fetchDuration,
                    timestamp: fetchEndTime,
                    url: tileRangeUrl,
                    status: response.status
                }
            }));
            
            // Update version from ETag header
            const newVersion = response.headers.get('ETag');
            if (newVersion) {
                this.currentVersion = newVersion;
                console.log(`Updated tile buffer version to: ${newVersion}`);
            }
            
            // Get the binary data
            const arrayBuffer = await response.arrayBuffer();
            const bytes = new Uint8Array(arrayBuffer);
            
            // Deserialize into a TileBuffer object using the WebAssembly module
            this.tileBuffer = TileBuffer.from_bytes(bytes);
            
            console.log(`Tile buffer fetched and deserialized successfully`);
            
            // Notify all registered callbacks that a new tile buffer is ready
            this.notifyTileBufferReady(x, y, w, h, z, this.bufferSizePower, this.tileBuffer);
            
            tileBufferUpdated = true;
        } catch (error) {
            console.error('Error fetching or deserializing tile buffer:', error);
        } finally {
            // Reset download flag
            this.downloadInProgress = false;
            
            // Check if view range was updated during download
            // If so, start another download
            if (this.viewRangeUpdated) {
                console.log('View range was updated during download, fetching new tile buffer');
                this.checkAndFetchTileBuffer(true);
            }
        }
        
        return tileBufferUpdated;
    }
}

function getJourneyTileRangePathWithId(journeyId, x, y, w, h, z, bufferSizePower) {
    return `journey/${journeyId}/tile_range?x=${x}&y=${y}&z=${z}&width=${w}&height=${h}&buffer_size_power=${bufferSizePower}`;
}

function getJourneyCameraOptionPathWithId(journeyId) {
    return `journey/${journeyId}/provisioned_camera_option`;
}
