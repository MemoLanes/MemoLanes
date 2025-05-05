import { JourneyBitmap } from '../pkg';

export class JourneyTileProvider {
    constructor(map, journeyId, frontEndRendering = true) {
        this.map = map;
        this.journeyId = journeyId;
        this.frontEndRendering = frontEndRendering;
        this.onUpdateCallbacks = []; // Array to store update callbacks

        if (this.frontEndRendering) {
            this.pollForJourneyUpdates(true);
        } else {
            // TODO: server-side rendering
        }
    }
    
    // TODO: make it support server-side rendering.
    get_tile_image(x, y, z) {
        if (this.frontEndRendering) {
            return this.journeyBitmap.get_tile_image(BigInt(x), BigInt(y), z);
        } else {
            // TODO: render the tile image on the server side.
            return null;
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
            callback(this.journeyBitmap);
        }
    }

    // typically two use cases: if the original page detect a data change, then no cache (forceUpdate = true)
    // if it is just a periodic update or normal check, then use cache (forceUpdate = false)
    async pollForJourneyUpdates(forceUpdate = false) {
        const filePath = getJourneyFilePathWithId(this.journeyId);
        
        console.log(`Fetching ${filePath}`);

        const useIfNoneMatch = ! forceUpdate;
        const fetchOptions = {
            headers: useIfNoneMatch ? { 'If-None-Match': '*' } : {}
        };

        try {
            const response = await fetch(`${filePath}`, fetchOptions);

            // If server returns 304 Not Modified, return null
            if (response.status === 304) {
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
