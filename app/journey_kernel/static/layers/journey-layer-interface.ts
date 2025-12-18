import type { Map } from "maplibre-gl";
import type { JourneyTileProvider } from "../journey-tile-provider";

/**
 * RGBA color tuple: [red, green, blue, alpha]
 * Values are in range [0, 1]
 */
export type RGBAColor = [number, number, number, number];

/**
 * Common interface for all journey rendering layers.
 * Both Canvas-based and WebGL-based layers should implement this interface.
 */
export interface JourneyLayer {
  /**
   * Initialize the layer and add it to the map.
   * This method should be called after the layer is constructed.
   */
  initialize(): void;

  /**
   * Remove the layer from the map and clean up resources.
   */
  remove(): void;
}

/**
 * Constructor signature for journey layer classes.
 * This allows the layer class to be used as a factory.
 */
export interface JourneyLayerConstructor {
  new (
    map: Map,
    journeyTileProvider: JourneyTileProvider,
    bgColor?: RGBAColor,
    fgColor?: RGBAColor,
  ): JourneyLayer;
}

