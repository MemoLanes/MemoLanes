/**
 * Frontend parameters management
 * Handles parsing and validation of parameters from URL hash or external sources
 */

import { JourneyCanvasLayer } from "./layers/journey-canvas-layer";
import type { JourneyLayerConstructor } from "./layers/journey-layer-interface";

// Default values for parameters
const DEFAULT_MAP_STYLE = "https://tiles.openfreemap.org/styles/liberty";
const DEFAULT_RENDER_MODE = "canvas";

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

/** Map of layer key to LayerConfig */
export type AvailableLayers = { [key: string]: LayerConfig };

/**
 * Available rendering layers.
 *
 * To add a new layer:
 * 1. Create a class that implements the JourneyLayer interface
 * 2. Import it at the top of this file
 * 3. Add a new entry below with a unique key
 *
 * Example:
 *   myNewLayer: {
 *     name: "My New Layer",
 *     layerClass: MyNewLayerClass,
 *     bufferSizePower: 9,
 *     description: "Description of my new layer",
 *   },
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
  journey_id?: string;
  access_key?: string;
  lng?: string;
  lat?: string;
  zoom?: string;
  render?: string;
  map_style?: string;
  [key: string]: string | undefined; // Allow additional parameters
}

// ============================================================================
// Reactive Parameters System
// ============================================================================

/**
 * Callback type for property change hooks
 * @param newValue - The new value after change
 * @param oldValue - The previous value before change
 */
export type PropertyChangeCallback<T> = (newValue: T, oldValue: T) => void;

/**
 * Mutable property names that support hooks
 * These are the properties that can be changed at runtime and trigger callbacks
 */
export type MutablePropertyName = 'renderMode' | 'journeyId';

/**
 * ReactiveParams - A reactive parameters class with hook support
 * 
 * This class wraps validated parameters and provides:
 * - Getters for all parameters
 * - Setters for mutable parameters that trigger registered hooks
 * - on() method to register callbacks for property changes
 * - set() method for generic property updates
 * 
 * Usage Example:
 * ```typescript
 * const params = new ReactiveParams(...);
 * 
 * // Register a hook for renderMode changes
 * const unsubscribe = params.on('renderMode', (newMode, oldMode) => {
 *   console.log(`Render mode changed from ${oldMode} to ${newMode}`);
 *   switchRenderingLayer(map);
 * });
 * 
 * // Later, when renderMode changes, the hook is automatically called
 * params.renderMode = 'gl';
 * 
 * // Unsubscribe when no longer needed
 * unsubscribe();
 * ```
 */
export class ReactiveParams {
  // Private storage for parameters
  private _cgiEndpoint: string;
  private _journeyId: string;
  private _mapStyle: string;
  private _accessKey: string | null;
  private _lng: number;
  private _lat: number;
  private _zoom: number;
  private _renderMode: string;
  private _requiresMapboxToken: boolean;

  // Hooks storage - map of property name to set of callbacks
  // Using Set to allow multiple hooks per property and easy removal
  private hooks: Map<MutablePropertyName, Set<PropertyChangeCallback<any>>> = new Map();

  constructor(
    cgiEndpoint: string,
    journeyId: string,
    mapStyle: string,
    accessKey: string | null,
    lng: number,
    lat: number,
    zoom: number,
    renderMode: string,
    requiresMapboxToken: boolean,
  ) {
    this._cgiEndpoint = cgiEndpoint;
    this._journeyId = journeyId;
    this._mapStyle = mapStyle;
    this._accessKey = accessKey;
    this._lng = lng;
    this._lat = lat;
    this._zoom = zoom;
    this._renderMode = renderMode;
    this._requiresMapboxToken = requiresMapboxToken;
  }

  // ============================================================================
  // Hook System
  // ============================================================================

  /**
   * Register a callback to be called when a property changes
   * 
   * @param property - The property name to watch ('renderMode' or 'journeyId')
   * @param callback - Function called with (newValue, oldValue) when property changes
   * @returns Unsubscribe function - call it to remove the hook
   * 
   * Note: Hooks are only called when the value actually changes (oldValue !== newValue)
   */
  on<K extends MutablePropertyName>(
    property: K,
    callback: PropertyChangeCallback<K extends 'renderMode' ? string : string>,
  ): () => void {
    // Initialize the Set for this property if it doesn't exist
    if (!this.hooks.has(property)) {
      this.hooks.set(property, new Set());
    }

    // Add the callback to the Set
    this.hooks.get(property)!.add(callback);

    // Return an unsubscribe function
    // This pattern is common in reactive systems (like RxJS, MobX, etc.)
    return () => {
      this.hooks.get(property)?.delete(callback);
    };
  }

  /**
   * Trigger all registered hooks for a property
   * Called internally when a property value changes
   */
  private triggerHooks<T>(property: MutablePropertyName, newValue: T, oldValue: T): void {
    const callbacks = this.hooks.get(property);
    if (!callbacks) return;

    for (const callback of callbacks) {
      try {
        callback(newValue, oldValue);
      } catch (error) {
        console.error(`Error in ${property} hook callback:`, error);
      }
    }
  }

  // ============================================================================
  // Generic Setter
  // ============================================================================

  /**
   * Generic method to set a mutable property by name
   * This is useful when the property name is dynamic (e.g., from Flutter bridge)
   * 
   * @param key - The property name ('renderMode' or 'journeyId')
   * @param value - The new value to set
   * @returns true if the value was changed, false if it was the same
   */
  set(key: MutablePropertyName, value: string): boolean {
    switch (key) {
      case 'renderMode':
        if (this._renderMode === value) return false;
        const oldRenderMode = this._renderMode;
        this._renderMode = value;
        this.triggerHooks('renderMode', value, oldRenderMode);
        return true;
      
      case 'journeyId':
        if (this._journeyId === value) return false;
        const oldJourneyId = this._journeyId;
        this._journeyId = value;
        this.triggerHooks('journeyId', value, oldJourneyId);
        return true;
      
      default:
        console.warn(`Unknown mutable property: ${key}`);
        return false;
    }
  }

  // ============================================================================
  // Property Getters and Setters
  // ============================================================================

  // Readonly properties - only getters
  get cgiEndpoint(): string {
    return this._cgiEndpoint;
  }

  get mapStyle(): string {
    return this._mapStyle;
  }

  get accessKey(): string | null {
    return this._accessKey;
  }

  get lng(): number {
    return this._lng;
  }

  get lat(): number {
    return this._lat;
  }

  get zoom(): number {
    return this._zoom;
  }

  get requiresMapboxToken(): boolean {
    return this._requiresMapboxToken;
  }

  // Mutable properties - getters and setters with hook triggers
  
  /**
   * Rendering mode (e.g., 'canvas', 'gl')
   * Setting this property triggers registered 'renderMode' hooks
   */
  get renderMode(): string {
    return this._renderMode;
  }

  set renderMode(value: string) {
    if (this._renderMode === value) return;
    const oldValue = this._renderMode;
    this._renderMode = value;
    this.triggerHooks('renderMode', value, oldValue);
  }

  /**
   * Journey ID for the current session
   * Setting this property triggers registered 'journeyId' hooks
   */
  get journeyId(): string {
    return this._journeyId;
  }

  set journeyId(value: string) {
    if (this._journeyId === value) return;
    const oldValue = this._journeyId;
    this._journeyId = value;
    this.triggerHooks('journeyId', value, oldValue);
  }
}

// ============================================================================
// Backward Compatibility Alias
// ============================================================================

/**
 * @deprecated Use ReactiveParams instead
 * This alias is kept for backward compatibility during migration
 */
export type ValidatedParams = ReactiveParams;

// Validation error with optional detail message
export interface ValidationError {
  type: "error";
  message: string;
  detail?: string;
}

// Result type for validation
export type ValidationResult =
  | { type: "success"; params: ReactiveParams }
  | ValidationError;

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

  // Scan all hash parameters and store them after successful decoding
  // Supported parameters for endpoint configuration:
  // - cgi_endpoint: HTTP endpoint URL, "flutter://<channel>" for IPC mode, or "flutter" for legacy IPC
  // Other parameters: journey_id, access_key, lng, lat, zoom, render, etc.
  for (const [key, value] of params.entries()) {
    if (value) {
      try {
        const decodedValue = decodeURIComponent(value);
        externalParams[key] = decodedValue;
      } catch (error) {
        console.warn(
          `Failed to decode parameter '${key}': ${(error as Error).message}`,
        );
        // Skip this parameter if decoding fails
      }
    }
  }

  return externalParams;
}

/**
 * Parse and validate external parameters
 * @param externalParams Raw external parameters from URL hash or Flutter
 * @param availableRenderModes Map of available rendering modes
 * @returns ValidationResult with either validated params or error
 */
export function parseAndValidateParams(
  externalParams: ExternalParams,
  availableRenderModes: { [key: string]: any },
): ValidationResult {
  // Check if cgi_endpoint is provided
  if (!externalParams.cgi_endpoint) {
    return {
      type: "error",
      message: "No endpoint configuration",
      detail: "cgi_endpoint parameter is required",
    };
  }

  // Check if journey_id is provided
  if (!externalParams.journey_id) {
    return {
      type: "error",
      message: "Journey ID not provided",
      detail: "journey_id parameter is required",
    };
  }

  const journeyId = externalParams.journey_id;
  const cgiEndpoint = externalParams.cgi_endpoint;

  // Determine map style
  const mapStyle = externalParams.map_style || DEFAULT_MAP_STYLE;

  // Check if mapbox style requires access token
  const requiresMapboxToken =
    typeof mapStyle === "string" && mapStyle.startsWith("mapbox://");

  // Validate access key for mapbox styles
  let accessKey: string | null = null;
  if (requiresMapboxToken) {
    if (!externalParams.access_key) {
      return {
        type: "error",
        message: "TOKEN not provided",
        detail: "access_key is required for Mapbox styles",
      };
    }
    accessKey = externalParams.access_key;
  }

  // Parse and validate rendering mode
  let renderMode = DEFAULT_RENDER_MODE;
  if (externalParams.render && availableRenderModes[externalParams.render]) {
    renderMode = externalParams.render;
  } else if (
    externalParams.render &&
    !availableRenderModes[externalParams.render]
  ) {
    console.warn(
      `Rendering mode '${externalParams.render}' not available, using ${DEFAULT_RENDER_MODE} instead.`,
    );
  }

  // Parse coordinates and zoom with fallbacks
  const lng = externalParams.lng
    ? isNaN(parseFloat(externalParams.lng))
      ? 0
      : parseFloat(externalParams.lng)
    : 0;

  const lat = externalParams.lat
    ? isNaN(parseFloat(externalParams.lat))
      ? 0
      : parseFloat(externalParams.lat)
    : 0;

  const zoom = externalParams.zoom
    ? isNaN(parseFloat(externalParams.zoom))
      ? 2
      : parseFloat(externalParams.zoom)
    : 2;

  // Create and return ReactiveParams instance
  const reactiveParams = new ReactiveParams(
    cgiEndpoint,
    journeyId,
    mapStyle,
    accessKey,
    lng,
    lat,
    zoom,
    renderMode,
    requiresMapboxToken,
  );

  return {
    type: "success",
    params: reactiveParams,
  };
}
