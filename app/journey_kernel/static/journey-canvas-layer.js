import { tileXYToLngLat } from "./utils.js";

export class JourneyCanvasLayer {
  constructor(
    map,
    journeyTileProvider,
    bgColor = [0.0, 0.0, 0.0, 0.5],
    fgColor = [1.0, 1.0, 1.0, 0.0],
  ) {
    this.map = map;
    this.journeyTileProvider = journeyTileProvider;

    let r = Math.round(bgColor[0] * 255);
    let g = Math.round(bgColor[1] * 255);
    let b = Math.round(bgColor[2] * 255);
    let a = bgColor[3];
    this.bgColor = `rgba(${r}, ${g}, ${b}, ${a})`;

    r = Math.round(fgColor[0] * 255);
    g = Math.round(fgColor[1] * 255);
    b = Math.round(fgColor[2] * 255);
    a = fgColor[3];
    this.fgColor = `rgba(${r}, ${g}, ${b}, ${a})`;

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

    this._repaintCallback = (x, y, w, h, z, bufferSizePower, tileBuffer) => {
      this.redrawCanvas(x, y, w, h, z, bufferSizePower, tileBuffer);
    };
    this.journeyTileProvider.registerTileBufferCallback(this._repaintCallback);
  }

  getSourceConfig() {
    return {
      type: "canvas",
      canvas: this.canvas,
      animate: false,
      coordinates: [
        [0, 0.01],
        [0.01, 0.01],
        [0.01, 0],
        [0, 0],
      ],
    };
  }

  redrawCanvas(x, y, w, h, z, bufferSizePower, tileBuffer) {
    if (!tileBuffer) {
      return;
    }

    console.log(`redrawing canvas ${x}, ${y}, ${w}, ${h}, ${z}`);
    const [left, top, right, bottom] = [x, y, x + w, y + h];

    const tileSize = Math.pow(2, bufferSizePower);
    this.canvas.width = tileSize * w;
    this.canvas.height = tileSize * h;

    const n = Math.pow(2, z);
    // Initialize the canvas with a semi-transparent grey background
    this.ctx.fillStyle = this.bgColor;
    this.ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);

    for (let x = left; x < right; x++) {
      for (let y = top; y < bottom; y++) {
        if (y < 0 || y >= n) continue;

        let xNorm = ((x % n) + n) % n;

        const dx = (x - left) * tileSize;
        const dy = (y - top) * tileSize;

        // Get pixels coordinates from journeyTileProvider
        const pixelCoords = tileBuffer.get_tile_pixels(
          BigInt(xNorm),
          BigInt(y),
          z,
          bufferSizePower,
        );

        if (pixelCoords && pixelCoords.length > 0) {
          // Draw each point from the pixel coordinates
          this.ctx.fillStyle = this.fgColor;

          // Process pairs of coordinates (x,y)
          for (let i = 0; i < pixelCoords.length; i += 2) {
            const pointX = dx + pixelCoords[i];
            const pointY = dy + pixelCoords[i + 1];
            // Clear the pixel first to remove the background
            this.ctx.clearRect(pointX, pointY, 1, 1);
            // Then draw with the foreground color
            this.ctx.fillRect(pointX, pointY, 1, 1);
          }
        }
      }
    }

    // This is a workaround for a maplibre 5.7.3 bug (or feature).
    //  for a map view of multi-worldview (map wrap arounds and lng may be out of -180 - 180 range),
    //  it has a strict limit that the centor of the canvas fall into the half-open [-180, 180) range,
    //  or equivalently, the centor's mercator coordinate x must fall in [0, 1) range.
    //  but for our codes, in border case, the centor's mercator coordinate x may be 1.
    //  so we multiply both left and right x by 0.999999 to make it fall into the [0, 1) range.
    // More info can be found at the calling stack referenced below,
    //  https://github.com/maplibre/maplibre-gl-js/blob/8895e414984a6348a1260ed986a0d2d7753367a8/src/source/image_source.ts#L228
    //  https://github.com/maplibre/maplibre-gl-js/blob/8895e414984a6348a1260ed986a0d2d7753367a8/src/source/image_source.ts#L350
    //  https://github.com/maplibre/maplibre-gl-js/blob/08fce0cfbf28f4da2cde60025588a8cb9323c9fe/src/source/tile_id.ts#L23
    const almost = (x) => x * 0.999999;

    const nw = tileXYToLngLat([almost(left), top], z);
    const ne = tileXYToLngLat([almost(right), top], z);
    const se = tileXYToLngLat([almost(right), bottom], z);
    const sw = tileXYToLngLat([almost(left), bottom], z);

    const mainCanvasSource = this.map.getSource("main-canvas-source");
    mainCanvasSource?.setCoordinates([nw, ne, se, sw]);
    mainCanvasSource?.play();
    mainCanvasSource?.pause();
  }

  remove() {
    if (this.map.getLayer("main-canvas-layer")) {
      this.map.removeLayer("main-canvas-layer");
    }

    if (this.map.getSource("main-canvas-source")) {
      this.map.removeSource("main-canvas-source");
    }

    if (this.journeyTileProvider) {
      this.journeyTileProvider.unregisterTileBufferCallback(
        this._repaintCallback,
      );
    }
  }
}
