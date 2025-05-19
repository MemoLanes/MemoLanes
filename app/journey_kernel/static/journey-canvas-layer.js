import { lngLatToTileXY, tileXYToLngLat } from './utils.js';

export class JourneyCanvasLayer {
    constructor(map, journeyTileProvider) {
        this.map = map;
        this.journeyTileProvider = journeyTileProvider;
        this.journeyTileProvider.registerUpdateCallback(this.handleProviderUpdate.bind(this));
        
        // Set the tile extension to "imagedata"
        this.journeyTileProvider.setTileExtension("imagedata");

        this.canvas = document.createElement("canvas");
        this.ctx = this.canvas.getContext("2d");
        this.currentTileRange = [0, 0, 0, 0];
        this.currentZoom = -1;
    }

    initialize() {
        this.map.addSource("main-canvas-source", this.getSourceConfig());
        this.map.addLayer({
            id: "main-canvas-layer",
            source: "main-canvas-source",
            type: "raster",
            paint: {
                "raster-fade-duration": 0,
            },
        });
        this.render();
    }

    getSourceConfig() {
        return {
            type: "canvas",
            canvas: this.canvas,
            animate: false,
            coordinates: [
                [0, 0], [0, 0],
                [0, 0], [0, 0]
            ],
        };
    }

    render(forceRender = false) {
        const zoom = Math.floor(this.map.getZoom());
        const bounds = this.map.getBounds();

        const [leftInit, topInit] = lngLatToTileXY(
            bounds.getNorthWest().toArray(),
            zoom
        );
        const [rightInit, bottomInit] = lngLatToTileXY(
            bounds.getSouthEast().toArray(),
            zoom
        );

        const left = Math.floor(leftInit);
        const top = Math.floor(topInit);
        const right = Math.ceil(rightInit);
        const bottom = Math.ceil(bottomInit);

        const tileRange = [left, top, right, bottom];

        if (forceRender || !this.arraysEqual(this.currentTileRange, tileRange) || this.currentZoom !== zoom) {
            console.log(`Rendering tiles for zoom ${zoom}, range: `, tileRange);
            this.currentTileRange = tileRange;
            this.currentZoom = zoom;

            this.canvas.width = 256 * (right - left + 1);
            this.canvas.height = 256 * (bottom - top + 1);

            this.renderTileRange(tileRange, zoom);
        }
    }

    renderTileRange(tileRange, zoom) {
        const [left, top, right, bottom] = tileRange;

        if (left > right || top > bottom) {
            console.error(`Invalid tile range: left=${left}, right=${right}, top=${top}, bottom=${bottom}`);
            return;
        }

        // Save the tile range and zoom to the tile provider
        this.journeyTileProvider.setSubscribedRange(tileRange, zoom);

        const n = Math.pow(2, zoom);
        this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
        const renderedTiles = new Set();

        for (let x = left; x <= right; x++) {
            for (let y = top; y <= bottom; y++) {
                if (y < 0 || y >= n) continue;

                let xNorm = ((x % n) + n) % n;
                const tileKey = `${xNorm},${y}`;
                if (renderedTiles.has(tileKey)) continue;
                renderedTiles.add(tileKey);

                const dx = (x - left) * 256;
                const dy = (y - top) * 256;
                
                const imageData = this.renderTile(xNorm, y, zoom);
                if (imageData) {
                    this.ctx.putImageData(imageData, dx, dy);
                    
                    let xPos = x + n;
                    while (xPos <= right) {
                        this.ctx.putImageData(imageData, (xPos - left) * 256, dy);
                        xPos += n;
                    }
                    
                    xPos = x - n;
                    while (xPos >= left) {
                        this.ctx.putImageData(imageData, (xPos - left) * 256, dy);
                        xPos -= n;
                    }
                }
            }
        }

        const nw = tileXYToLngLat([left, top], zoom);
        const ne = tileXYToLngLat([right + 1, top], zoom);
        const se = tileXYToLngLat([right + 1, bottom + 1], zoom);
        const sw = tileXYToLngLat([left, bottom + 1], zoom);

        const mainCanvasSource = this.map.getSource("main-canvas-source");
        mainCanvasSource?.setCoordinates([nw, ne, se, sw]);
        mainCanvasSource?.play();
        mainCanvasSource?.pause();
    }

    renderTile(x, y, z) {
        try {
            const imageBufferRaw = this.journeyTileProvider.getTileData(x, y, z);
            const uint8Array = new Uint8ClampedArray(imageBufferRaw);
            return new ImageData(uint8Array, 256, 256);
        } catch (error) {
            console.error(`Failed to render tile ${x},${y},${z}:`, error);
            return null;
        }
    }

    // TODO: maybe we should unify this interface with renderTile
    // Draw a specific tile directly to canvas at the given position
    drawTileToCanvas(x, y, z, canvasX, canvasY) {
        const imageData = this.renderTile(x, y, z);
        if (!imageData) return false;
        
        this.ctx.putImageData(imageData, canvasX, canvasY);
        return true;
    }

    // Helper methods
    arraysEqual(a, b) {
        return Array.isArray(a) &&
            Array.isArray(b) &&
            a.length === b.length &&
            a.every((val, index) => val === b[index]);
    }

    handleProviderUpdate(tileKey) {
        if (tileKey) {
            // Only a specific tile was updated
            const [z, x, y] = tileKey.split('/').map(Number);
            if (z === this.currentZoom) {
                const [left, top, right, bottom] = this.currentTileRange;
                if (x >= left && x <= right && y >= top && y <= bottom) {
                    // Redraw only the specific tile if it's in the visible range
                    const dx = (x - left) * 256;
                    const dy = (y - top) * 256;
                    
                    const tileRedrawn = this.drawTileToCanvas(x, y, z, dx, dy);
                    
                    if (tileRedrawn) {
                        // Draw wrapped tiles if needed
                        const n = Math.pow(2, z);
                        let xPos = x + n;
                        while (xPos <= right) {
                            this.drawTileToCanvas(x, y, z, (xPos - left) * 256, dy);
                            xPos += n;
                        }
                        
                        xPos = x - n;
                        while (xPos >= left) {
                            this.drawTileToCanvas(x, y, z, (xPos - left) * 256, dy);
                            xPos -= n;
                        }
                        
                        // Refresh the canvas source
                        this.map.getSource("main-canvas-source")?.play();
                        this.map.getSource("main-canvas-source")?.pause();
                        return; // Skip full render as we've handled just this tile
                    }
                }
            }
            // Fall back to full render if we couldn't do a partial update
            this.render(true);
        } else {
            // Full update needed
            this.currentTileRange = [-1, -1, -1, -1];
            this.render();
        }
    }

    remove() {
        if (this.map.getLayer("main-canvas-layer")) {
            this.map.removeLayer("main-canvas-layer");
        }
        
        if (this.map.getSource("main-canvas-source")) {
            this.map.removeSource("main-canvas-source");
        }
        
        if (this.journeyTileProvider) {
            this.journeyTileProvider.unregisterUpdateCallback(this.handleProviderUpdate.bind(this));
        }
    }
} 