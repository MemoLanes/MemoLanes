export class JourneyCanvasLayer {
    constructor(map, journeyTileProvider) {
        this.map = map;
        this.journeyTileProvider = journeyTileProvider;
        this.journeyTileProvider.registerUpdateCallback(this.handleProviderUpdate.bind(this));

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

        const [leftInit, topInit] = this.lngLatToTileXY(
            bounds.getNorthWest().toArray(),
            zoom
        );
        const [rightInit, bottomInit] = this.lngLatToTileXY(
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

                const imageData = this.renderTile(xNorm, y, zoom);
                if (imageData) {
                    let xPos = x;
                    while (xPos <= right) {
                        const dx = (xPos - left) * 256;
                        const dy = (y - top) * 256;
                        this.ctx.putImageData(imageData, dx, dy);
                        xPos += n;
                    }
                    xPos = x - n;
                    while (xPos >= left) {
                        const dx = (xPos - left) * 256;
                        const dy = (y - top) * 256;
                        this.ctx.putImageData(imageData, dx, dy);
                        xPos -= n;
                    }
                }
            }
        }

        const nw = this.tileXYToLngLat([left, top], zoom);
        const ne = this.tileXYToLngLat([right + 1, top], zoom);
        const se = this.tileXYToLngLat([right + 1, bottom + 1], zoom);
        const sw = this.tileXYToLngLat([left, bottom + 1], zoom);

        const mainCanvasSource = this.map.getSource("main-canvas-source");
        mainCanvasSource?.setCoordinates([nw, ne, se, sw]);
        mainCanvasSource?.play();
        mainCanvasSource?.pause();
    }

    renderTile(x, y, z) {
        try {
            const imageBufferRaw = this.journeyTileProvider.get_tile_image(x, y, z);
            const uint8Array = new Uint8ClampedArray(imageBufferRaw);
            const imageData = new ImageData(uint8Array, 256, 256);

            const tempCanvas = document.createElement('canvas');
            tempCanvas.width = 256;
            tempCanvas.height = 256;
            const tempCtx = tempCanvas.getContext('2d');

            tempCtx.putImageData(imageData, 0, 0);

            return tempCtx.getImageData(0, 0, 256, 256);
        } catch (error) {
            console.error(`Failed to render tile ${x},${y},${z}:`, error);
            return null;
        }
    }

    // Helper methods
    lngLatToTileXY([lng, lat], zoom) {
        const n = Math.pow(2, zoom);
        const x = Math.floor((lng + 180) / 360 * n);
        const latRad = lat * Math.PI / 180;
        const y = Math.floor((1 - Math.log(Math.tan(latRad) + 1 / Math.cos(latRad)) / Math.PI) / 2 * n);
        return [x, y];
    }

    tileXYToLngLat([x, y], zoom) {
        const n = Math.pow(2, zoom);
        const lng = x / n * 360 - 180;
        const latRad = Math.atan(Math.sinh(Math.PI * (1 - 2 * y / n)));
        const lat = latRad * 180 / Math.PI;
        return [lng, lat];
    }

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
                    // TODO: only redraw if the tile is in the visible range
                    // this.renderTile(x, y, z);
                    this.render(true);
                    this.map.getSource("main-canvas-source")?.play();
                    this.map.getSource("main-canvas-source")?.pause();
                }
            }
        } else {
            // Full update needed
            this.currentTileRange = [-1, -1, -1, -1];
            this.render();
        }
    }

    onRemove() {
        if (this.journeyTileProvider) {
            this.journeyTileProvider.unregisterUpdateCallback(this.handleProviderUpdate.bind(this));
        }
    }
} 