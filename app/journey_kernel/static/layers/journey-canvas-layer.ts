import { tileXYToLngLat } from "./utils";
import type * as maplibregl from "maplibre-gl";
import type { CanvasSource, CanvasSourceSpecification } from "maplibre-gl";
import type { TileBuffer } from "../../pkg/journey_kernel.js";
import type { JourneyTileProvider } from "../journey-tile-provider";
import { DEFAULT_FOG_RGBA } from "../fog-style";
import { JOURNEY_LAYER_ID } from "./journey-layer-interface";
import type { JourneyLayer, RGBAColor } from "./journey-layer-interface";
import { CanvasPoleFoggyLayer } from "./canvas-pole-foggy-layer";

/**
 * Tile buffer callback function type
 */
type TileBufferCallback = (
  x: number,
  y: number,
  w: number,
  h: number,
  z: number,
  bufferSizePower: number,
  tileBuffer: TileBuffer | null,
) => void;

/**
 * JourneyCanvasLayer is a Canvas-based journey layer that renders tracks
 * using the HTML Canvas 2D API.
 *
 * Implements JourneyLayer for unified layer management.
 */
export class JourneyCanvasLayer implements JourneyLayer {
  private map: maplibregl.Map;
  private journeyTileProvider: JourneyTileProvider;
  private layerId: string;
  private sourceId: string;
  private poleLayer: CanvasPoleFoggyLayer;
  private readonly trackColors: Uint32Array;
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private raster?: {
    image: ImageData;
    pixels: Uint32Array;
    coverage: Uint8Array;
  };
  private _repaintCallback?: TileBufferCallback;

  constructor(
    map: maplibregl.Map,
    journeyTileProvider: JourneyTileProvider,
    layerId: string = JOURNEY_LAYER_ID,
    bgColor: RGBAColor = [...DEFAULT_FOG_RGBA],
    fgColor: RGBAColor = [1.0, 1.0, 1.0, 0.0],
  ) {
    this.map = map;
    this.journeyTileProvider = journeyTileProvider;
    this.layerId = layerId;
    this.sourceId = `${layerId}-canvas-source`;
    this.poleLayer = new CanvasPoleFoggyLayer(
      map,
      `${layerId}-canvas-poles`,
      bgColor,
    );

    // A narrow feather around the original solid pixels softens stair steps
    // without blurring away thin tracks. Mix premultiplied colors so both
    // transparent tracks and custom translucent foregrounds keep their color.
    const colors = new Uint8ClampedArray(256 * 4);
    for (let index = 0; index < 256; index++) {
      const coverage = index / 255;
      const bgAlpha = bgColor[3] * (1 - coverage);
      const fgAlpha = fgColor[3] * coverage;
      const alpha = bgAlpha + fgAlpha;
      for (let channel = 0; channel < 3; channel++) {
        colors[index * 4 + channel] = alpha
          ? (Math.round(bgColor[channel] * 255) * bgAlpha +
              Math.round(fgColor[channel] * 255) * fgAlpha) /
            alpha
          : 0;
      }
      colors[index * 4 + 3] = alpha * 255;
    }
    this.trackColors = new Uint32Array(colors.buffer);

    this.canvas = document.createElement("canvas");
    const ctx = this.canvas.getContext("2d");
    if (!ctx) {
      throw new Error("Failed to get 2D context from canvas");
    }
    this.ctx = ctx;
  }

  initialize(): void {
    this.map.addSource(this.sourceId, this.getSourceConfig() as any);
    this.map.addLayer({
      id: this.layerId,
      source: this.sourceId,
      type: "raster",
      paint: {
        "raster-fade-duration": 0,
      },
    });

    // Canvas/Image sources deliberately disable MapLibre's globe pole mesh.
    // The Canvas-owned pole mesh meets the texture exactly at the Mercator
    // edge, avoiding a translucent overlap band.
    this.map.addLayer(this.poleLayer);

    this._repaintCallback = (
      x: number,
      y: number,
      w: number,
      h: number,
      z: number,
      bufferSizePower: number,
      tileBuffer: TileBuffer | null,
    ): void => {
      this.redrawCanvas(x, y, w, h, z, bufferSizePower, tileBuffer);
    };
    this.journeyTileProvider.registerTileBufferCallback(this._repaintCallback);
  }

  getSourceConfig(): CanvasSourceSpecification {
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

  redrawCanvas(
    x_raw: number,
    y: number,
    w_raw: number,
    h: number,
    z: number,
    bufferSizePower: number,
    tileBuffer: TileBuffer | null,
  ): void {
    if (!tileBuffer) {
      return;
    }

    let x = x_raw;
    let w = w_raw;

    // when the viewpoint takes up multiple worlds, maplibre tends to render the canvas once for each worlds.
    // therefore, we limit the tile range to be at most a world width.
    if (w > 1 << z) {
      x = 0;
      w = 1 << z;
    }
    console.log(
      `redrawing canvas ${x_raw}, ${y}, ${w_raw}, ${h}, ${z}, adjusted x: ${x}, w: ${w}`,
    );

    const [left, top, right, bottom] = [x, y, x + w, y + h];

    const tileSize = Math.pow(2, bufferSizePower);

    const n = Math.pow(2, z);
    const width = tileSize * w;
    const height = tileSize * h;
    if (this.canvas.width !== width) this.canvas.width = width;
    if (this.canvas.height !== height) this.canvas.height = height;
    if (
      this.raster?.image.width !== width ||
      this.raster.image.height !== height
    ) {
      const image = this.ctx.createImageData(width, height);
      this.raster = {
        image,
        pixels: new Uint32Array(image.data.buffer),
        coverage: new Uint8Array(width * height),
      };
    }
    // Panning usually keeps the same dimensions. Reuse only the current
    // raster, avoiding repeated allocations without retaining old viewports.
    const { image, pixels, coverage } = this.raster;
    pixels.fill(this.trackColors[0]);
    coverage.fill(0);
    const addCoverage = (offset: number, weight: number): void => {
      if (coverage[offset] !== 255) {
        // Keep partially revealed cells distinct from solid track cores.
        const level = Math.min(254, coverage[offset] + weight);
        coverage[offset] = level;
        pixels[offset] = this.trackColors[level];
      }
    };

    for (let x = left; x < right; x++) {
      for (let y = top; y < bottom; y++) {
        if (y < 0 || y >= n) continue;

        const dx = (x - left) * tileSize;
        const dy = (y - top) * tileSize;

        // Get pixels coordinates from journeyTileProvider
        // TODO: we need smooth transition when x range jump in the center of the Pacific Ocean.
        const pixelCoords = tileBuffer.get_tile_pixels(
          x,
          y,
          z,
          bufferSizePower,
        );

        if (pixelCoords && pixelCoords.length > 0) {
          // Process pairs of coordinates (x,y)
          for (let i = 0; i < pixelCoords.length; i += 2) {
            const pointX = dx + pixelCoords[i];
            const pointY = dy + pixelCoords[i + 1];
            const center = pointY * width + pointX;
            if (coverage[center] === 255) continue;
            coverage[center] = 255;
            pixels[center] = this.trackColors[255];
            // A small, fixed 3x3 kernel fills diagonal notches more evenly
            // than independent halos. Preserve solid cores and count each
            // source pixel once, so duplicates and draw order have no effect.
            // Canvas-space neighbors also keep internal tile seams invisible.
            // Most points are interior: use fixed offsets to avoid checking
            // canvas bounds and world wrapping for each of eight neighbors.
            if (
              pointX > 0 &&
              pointX < width - 1 &&
              pointY > 0 &&
              pointY < height - 1
            ) {
              addCoverage(center - width - 1, 10);
              addCoverage(center - width, 46);
              addCoverage(center - width + 1, 10);
              addCoverage(center - 1, 46);
              addCoverage(center + 1, 46);
              addCoverage(center + width - 1, 10);
              addCoverage(center + width, 46);
              addCoverage(center + width + 1, 10);
              continue;
            }
            for (let oy = -1; oy <= 1; oy++) {
              const py = pointY + oy;
              if (py < 0 || py >= height) continue;
              for (let ox = -1; ox <= 1; ox++) {
                if (ox === 0 && oy === 0) continue;
                let px = pointX + ox;
                if (w === n) px = (px + width) % width;
                else if (px < 0 || px >= width) continue;
                // About 18% at edge neighbors and 4% at corners.
                addCoverage(py * width + px, ox !== 0 && oy !== 0 ? 10 : 46);
              }
            }
          }
        }
      }
    }

    // One upload at the existing resolution avoids per-point Canvas calls
    // and does not increase the texture size sent to MapLibre.
    this.ctx.putImageData(image, 0, 0);

    // This is a workaround for a maplibre 5.7.3 bug (or feature).
    //  for a map view of multi-worldview (map wrap arounds and lng may be out of -180 - 180 range),
    //  it has a strict limit that the center of the canvas falls into the half-open [-180, 180) range,
    //  or equivalently, the center's mercator coordinate x must fall in [0, 1) range.
    //  but for our code, in a border case, the center's mercator coordinate x may be 1.
    //  so we multiply both left and right x by a number that is close to 1 but smaller than 1
    //  to make it fall into the [0, 1) range.
    // More info can be found at the calling stack referenced below,
    //  https://github.com/maplibre/maplibre-gl-js/blob/8895e414984a6348a1260ed986a0d2d7753367a8/src/source/image_source.ts#L228
    //  https://github.com/maplibre/maplibre-gl-js/blob/8895e414984a6348a1260ed986a0d2d7753367a8/src/source/image_source.ts#L350
    //  https://github.com/maplibre/maplibre-gl-js/blob/08fce0cfbf28f4da2cde60025588a8cb9323c9fe/src/source/tile_id.ts#L23
    const almost = (x: number): number => x * (1 - 10 * Number.EPSILON);
    const nw = tileXYToLngLat(almost(left), top, z);
    const ne = tileXYToLngLat(almost(right), top, z);
    const se = tileXYToLngLat(almost(right), bottom, z);
    const sw = tileXYToLngLat(almost(left), bottom, z);

    const mainCanvasSource = this.map.getSource(this.sourceId) as
      CanvasSource | undefined;
    mainCanvasSource?.setCoordinates([
      [nw.lng, nw.lat],
      [ne.lng, ne.lat],
      [se.lng, se.lat],
      [sw.lng, sw.lat],
    ]);
    mainCanvasSource?.play();
    mainCanvasSource?.pause();
  }

  remove(): void {
    if (this.map.getLayer(this.layerId)) {
      this.map.removeLayer(this.layerId);
    }

    if (this.map.getLayer(this.poleLayer.id)) {
      this.map.removeLayer(this.poleLayer.id);
    }

    if (this.map.getSource(this.sourceId)) {
      this.map.removeSource(this.sourceId);
    }

    if (this.journeyTileProvider && this._repaintCallback) {
      this.journeyTileProvider.unregisterTileBufferCallback(
        this._repaintCallback,
      );
    }
    this.raster = undefined;
  }
}
