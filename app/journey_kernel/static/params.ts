/**
 * Frontend parameters management
 * Handles parsing and creation of ReactiveParams from URL hash or external sources
 *
 * This module provides:
 * 1. Parameter parsing from URL hash
 * 2. ReactiveParams - a Proxy-based reactive object with hook system
 * 3. createReactiveParams function to build params from external input
 */

import { JourneyCanvasLayer } from "./layers/journey-canvas-layer";
import type { JourneyLayerConstructor } from "./layers/journey-layer-interface";

// Default values for parameters
const DEFAULT_MAP_STYLE = "https://tiles.openfreemap.org/styles/liberty";
const DEFAULT_RENDER_MODE = "canvas";

/** Valid projection types for the map */
export type ProjectionType = "mercator" | "globe";

// ============================================================================
// Layer Configuration
// ============================================================================

/**
 * Configuration for a rendering layer.
 * Add new layer implementations by creating a new LayerConfig entry.
 */
export interface LayerConfig {
  /** Display name for the layer */
  name: string;
  /** The layer class constructor, must implement JourneyLayer interface */
  layerClass: JourneyLayerConstructor;
  /** Power of 2 for tile buffer size (e.g., 8 = 256px, 10 = 1024px) */
  bufferSizePower: number;
  /** Human-readable description */
  description: string;
}

/**
 * Available rendering layers.
 *
 * To add a new layer:
 * 1. Create a class that implements the JourneyLayer interface
 * 2. Import it at the top of this file
 * 3. Add a new entry below with a unique key
 */
export const AVAILABLE_LAYERS: { [key: string]: LayerConfig } = {
  canvas: {
    name: "Canvas",
    layerClass: JourneyCanvasLayer,
    bufferSizePower: 8,
    description: "Uses Canvas API for rendering",
  },
};

// ============================================================================
// External Parameters Interface
// ============================================================================

// Raw external parameters (from URL hash or Flutter)
export interface ExternalParams {
  cgi_endpoint?: string;
  access_key?: string;
  lng?: string;
  lat?: string;
  zoom?: string;
  render?: string;
  map_style?: string;
  fog_density?: string;
  projection?: string;
  debug?: string;
  [key: string]: string | undefined;
}

// ============================================================================
// Reactive Parameters System (Proxy-based)
// ============================================================================

/** Callback type for property change hooks */
export type PropertyChangeCallback<T> = (newValue: T, oldValue: T) => void;

/** Mutable property names that support hooks */
export type MutablePropertyName = "renderMode" | "fogDensity" | "projection";

/** Internal data structure for ReactiveParams */
interface ParamsData {
  // Readonly properties
  cgiEndpoint: string;
  mapStyle: string;
  accessKey: string | null;
  lng: number;
  lat: number;
  zoom: number;
  requiresMapboxToken: boolean;
  debug: boolean;
  // Mutable properties
  renderMode: string;
  fogDensity: number;
  projection: ProjectionType;
}

/**
 * ReactiveParams - A Proxy-based reactive parameters object
 *
 * Properties can be accessed and set directly. Setting mutable properties
 * (renderMode, fogDensity, projection) triggers registered hooks.
 *
 * Usage:
 * ```typescript
 * const params = createReactiveParams(externalParams);
 *
 * // Register a hook
 * const unsubscribe = params.on('renderMode', (newMode, oldMode) => {
 *   console.log(`Changed from ${oldMode} to ${newMode}`);
 * });
 *
 * // Setting triggers the hook automatically
 * params.renderMode = 'gl';
 *
 * // Unsubscribe when done
 * unsubscribe();
 * ```
 */
export interface ReactiveParams extends ParamsData {
  /**
   * Register a callback for property changes
   * @param property - Property name to watch
   * @param callback - Called with (newValue, oldValue) when property changes
   * @returns Unsubscribe function
   */
  on<K extends MutablePropertyName>(
    property: K,
    callback: PropertyChangeCallback<
      K extends "fogDensity"
        ? number
        : K extends "projection"
          ? ProjectionType
          : string
    >,
  ): () => void;
}

/** Set of properties that trigger hooks when changed */
const MUTABLE_PROPERTIES = new Set<MutablePropertyName>([
  "renderMode",
  "fogDensity",
  "projection",
]);

/**
 * Create a ReactiveParams proxy object
 * @param data - Initial parameter values
 * @returns Proxy-wrapped reactive params object
 */
function createReactiveProxy(data: ParamsData): ReactiveParams {
  // Store hooks: Map<propertyName, Set<callback>>
  const hooks = new Map<
    MutablePropertyName,
    Set<PropertyChangeCallback<any>>
  >();

  const handler: ProxyHandler<ParamsData> = {
    get(target, prop: string) {
      // Handle the 'on' method
      if (prop === "on") {
        return (
          property: MutablePropertyName,
          callback: PropertyChangeCallback<any>,
        ) => {
          if (!hooks.has(property)) {
            hooks.set(property, new Set());
          }
          hooks.get(property)!.add(callback);
          // Return unsubscribe function
          return () => hooks.get(property)?.delete(callback);
        };
      }
      return target[prop as keyof ParamsData];
    },

    set(target, prop: string, value: any) {
      const key = prop as keyof ParamsData;
      const oldValue = target[key];

      // Apply value transformations for specific properties
      let newValue = value;
      if (prop === "fogDensity") {
        // Clamp fogDensity between 0 and 1
        newValue = Math.max(0, Math.min(1, value));
      } else if (prop === "projection") {
        // Normalize projection value
        newValue = value === "mercator" ? "mercator" : "globe";
      }

      // Skip if value unchanged
      if (oldValue === newValue) return true;

      // Update value
      (target as any)[key] = newValue;

      // Trigger hooks for mutable properties
      if (MUTABLE_PROPERTIES.has(prop as MutablePropertyName)) {
        const callbacks = hooks.get(prop as MutablePropertyName);
        if (callbacks) {
          for (const callback of callbacks) {
            try {
              callback(newValue, oldValue);
            } catch (error) {
              console.error(`Error in ${prop} hook callback:`, error);
            }
          }
        }
      }

      return true;
    },
  };

  return new Proxy(data, handler) as unknown as ReactiveParams;
}

// ============================================================================
// Parameter Parsing Functions
// ============================================================================

/**
 * Parse URL hash into ExternalParams object
 * @returns Parsed parameters or empty object if no hash
 */
export function parseUrlHash(): ExternalParams {
  const externalParams: ExternalParams = {};
  const hash = window.location.hash.slice(1);

  if (!hash) {
    return externalParams;
  }

  // Set default cgi_endpoint only when hash parameters are provided
  externalParams.cgi_endpoint = ".";

  const params = new URLSearchParams(hash);

  for (const [key, value] of params.entries()) {
    if (value) {
      try {
        externalParams[key] = decodeURIComponent(value);
      } catch (error) {
        console.warn(
          `Failed to decode parameter '${key}': ${(error as Error).message}`,
        );
      }
    }
  }

  return externalParams;
}

/**
 * Create a ReactiveParams instance from external parameters
 *
 * @param externalParams - Raw external parameters from URL hash or Flutter
 * @returns ReactiveParams instance, or null if cgi_endpoint is not yet available
 * @throws Error if required parameters are missing or invalid
 */
export function createReactiveParams(
  externalParams: ExternalParams,
): ReactiveParams | null {
  // Return null if cgi_endpoint not provided - wait for next setup
  if (!externalParams.cgi_endpoint) {
    return null;
  }

  // Determine map style
  const mapStyle = externalParams.map_style || DEFAULT_MAP_STYLE;
  const requiresMapboxToken = mapStyle.startsWith("mapbox://");

  // Validate access key for mapbox styles
  if (requiresMapboxToken && !externalParams.access_key) {
    throw new Error(
      "Mapbox access token not provided. access_key is required for Mapbox styles.",
    );
  }

  // Parse and validate rendering mode
  let renderMode = DEFAULT_RENDER_MODE;
  if (externalParams.render) {
    if (AVAILABLE_LAYERS[externalParams.render]) {
      renderMode = externalParams.render;
    } else {
      console.warn(
        `Rendering mode '${externalParams.render}' not available, using ${DEFAULT_RENDER_MODE} instead.`,
      );
    }
  }

  // Helper to parse number with fallback
  const parseNum = (val: string | undefined, fallback: number): number => {
    if (!val) return fallback;
    const parsed = parseFloat(val);
    return isNaN(parsed) ? fallback : parsed;
  };

  // Create the reactive proxy
  return createReactiveProxy({
    cgiEndpoint: externalParams.cgi_endpoint,
    mapStyle,
    accessKey: requiresMapboxToken ? externalParams.access_key! : null,
    lng: parseNum(externalParams.lng, 0),
    lat: parseNum(externalParams.lat, 0),
    zoom: parseNum(externalParams.zoom, 2),
    renderMode,
    requiresMapboxToken,
    fogDensity: Math.max(
      0,
      Math.min(1, parseNum(externalParams.fog_density, 0.5)),
    ),
    projection: externalParams.projection === "mercator" ? "mercator" : "globe",
    debug: externalParams.debug === "true",
  });
}
