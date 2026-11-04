import {
  createCanvasTrackColors,
  drawCanvasTileRange,
  type CanvasRaster,
} from "./canvas-raster";
import type * as maplibregl from "maplibre-gl";
import type { TileBuffer } from "../../pkg/journey_kernel.js";
import type { JourneyTileProvider } from "../journey-tile-provider";
import { DEFAULT_FOG_RGBA } from "../fog-style";
import { JOURNEY_LAYER_ID } from "./journey-layer-interface";
import type { JourneyLayer, RGBAColor } from "./journey-layer-interface";
import { CanvasMeshLayer } from "./canvas-mesh-layer";

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
  private meshLayer: CanvasMeshLayer;
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
    this.meshLayer = new CanvasMeshLayer(
      map,
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
    this.map.addLayer(this.meshLayer);

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

  isReadyForDisplay(): boolean {
    return (
      !!this.map.getLayer(this.layerId) &&
      this.meshLayer.isReadyForDisplay() &&
      (this.journeyTileProvider.getViewRange() === null || this.canvasPrepared)
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

    // Repeated worlds share one Canvas image; the mesh wraps its UV coordinates.
    if (w > 1 << z) {
      x = 0;
      w = 1 << z;
    }
    console.log(
      `redrawing canvas ${x_raw}, ${y}, ${w_raw}, ${h}, ${z}, adjusted x: ${x}, w: ${w}`,
    );

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

    this.canvasPrepared = true;
    this.meshLayer.setMercator(this.canvas, [x, y, w, h, z]);
  }

  remove(): void {
    this.canvasPrepared = false;
    if (this.map.getLayer(this.layerId)) {
      this.map.removeLayer(this.layerId);
    }

    if (this.journeyTileProvider && this._repaintCallback) {
      this.journeyTileProvider.unregisterTileBufferCallback(
        this._repaintCallback,
      );
    }
    this.raster = undefined;
  }
}
