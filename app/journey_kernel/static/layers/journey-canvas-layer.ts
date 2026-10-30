import {
  createCanvasTrackColors,
  drawCanvasTileRange,
  type CanvasRaster,
} from "./canvas-raster";
import { tileXYToLngLat } from "./utils";
import type * as maplibregl from "maplibre-gl";
import type { CanvasSource, CanvasSourceSpecification } from "maplibre-gl";
import type { TileBuffer } from "../../pkg/journey_kernel.js";
import type { JourneyTileProvider } from "../journey-tile-provider";
import { DEFAULT_FOG_RGBA } from "../fog-style";
import { JOURNEY_LAYER_ID } from "./journey-layer-interface";
import type { JourneyLayer, RGBAColor } from "./journey-layer-interface";
import { CanvasGlobeLayer } from "./canvas-globe-layer";

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
  private globeLayer: CanvasGlobeLayer;
  private readonly trackColors: Uint32Array;
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private raster?: CanvasRaster;
  private _repaintCallback?: TileBufferCallback;
  private canvasPrepared = false;

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
    this.globeLayer = new CanvasGlobeLayer(
      map,
      `${layerId}-canvas-globe`,
      layerId,
      bgColor,
      fgColor,
      journeyTileProvider,
    );

    this.trackColors = createCanvasTrackColors(bgColor, fgColor);

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
        "raster-opacity-transition": { duration: 0, delay: 0 },
      },
    });

    // One Canvas globe mesh keeps all three charts joined at the seams.
    this.map.addLayer(this.globeLayer);

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

  isReadyForDisplay(): boolean {
    return (
      this.globeLayer.isReadyForDisplay() &&
      (this.journeyTileProvider.getViewRange() === null ||
        (this.canvasPrepared &&
          !!this.map.getSource(this.sourceId) &&
          this.map.isSourceLoaded(this.sourceId)))
    );
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

    this.raster = drawCanvasTileRange(
      this.canvas,
      this.ctx,
      this.trackColors,
      this.raster,
      [x, y, w, h, z],
      bufferSizePower,
      tileBuffer,
      w === 2 ** z,
    );

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
    this.canvasPrepared = !!mainCanvasSource;
    this.globeLayer.setMercator(this.canvas, [x, y, w, h, z]);
  }

  remove(): void {
    this.canvasPrepared = false;
    if (this.map.getLayer(this.layerId)) {
      this.map.removeLayer(this.layerId);
    }

    if (this.map.getLayer(this.globeLayer.id)) {
      this.map.removeLayer(this.globeLayer.id);
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
