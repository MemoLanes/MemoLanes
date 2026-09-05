import { JourneyCanvasLayer } from "./layers/journey-canvas-layer";
import type { JourneyLayerConstructor } from "./layers/journey-layer-interface";
import type { TileRequestPolicy } from "./layers/tile-request-policy";
import type { TileBufferBackend } from "./tile-buffer-backend";

/** Map limits and renderer registration are kept together. */
export const MAX_MAP_ZOOM = 14;

export interface LayerConfig {
  name: string;
  layerClass: JourneyLayerConstructor;
  bufferSizePower: number;
  tileRequestPolicy: TileRequestPolicy;
  description: string;
  /** The callback requests fresh data after a backend loses its decoded source. */
  createTileBufferBackend?: (onInvalidated: () => void) => TileBufferBackend;
}

export const AVAILABLE_LAYERS: Record<string, LayerConfig> = {
  canvas: {
    name: "Canvas",
    layerClass: JourneyCanvasLayer,
    bufferSizePower: 8,
    tileRequestPolicy: { maxSourceZoom: MAX_MAP_ZOOM },
    description: "Uses Canvas API for rendering",
  },
};
