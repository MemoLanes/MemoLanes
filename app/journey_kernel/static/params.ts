/**
 * Frontend parameters management
 * Handles parsing and validation of parameters from URL hash or external sources
 */

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

// Validated and typed parameters ready for use
export class ValidatedParams {
  readonly cgiEndpoint: string;
  readonly journeyId: string;
  readonly mapStyle: string;
  readonly accessKey: string | null;
  readonly lng: number;
  readonly lat: number;
  readonly zoom: number;
  readonly renderMode: string;
  readonly requiresMapboxToken: boolean;

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
    this.cgiEndpoint = cgiEndpoint;
    this.journeyId = journeyId;
    this.mapStyle = mapStyle;
    this.accessKey = accessKey;
    this.lng = lng;
    this.lat = lat;
    this.zoom = zoom;
    this.renderMode = renderMode;
    this.requiresMapboxToken = requiresMapboxToken;
  }
}

// Validation error with optional detail message
export interface ValidationError {
  type: "error";
  message: string;
  detail?: string;
}

// Result type for validation
export type ValidationResult =
  | { type: "success"; params: ValidatedParams }
  | ValidationError;

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
 * @param defaultMapStyle Default map style to use if not specified
 * @param defaultRenderMode Default rendering mode if not specified
 * @returns ValidationResult with either validated params or error
 */
export function parseAndValidateParams(
  externalParams: ExternalParams,
  availableRenderModes: { [key: string]: any },
  defaultMapStyle: string = "https://tiles.openfreemap.org/styles/liberty",
  defaultRenderMode: string = "canvas",
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
  const mapStyle = externalParams.map_style || defaultMapStyle;

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
  let renderMode = defaultRenderMode;
  if (
    externalParams.render &&
    availableRenderModes[externalParams.render]
  ) {
    renderMode = externalParams.render;
  } else if (
    externalParams.render &&
    !availableRenderModes[externalParams.render]
  ) {
    console.warn(
      `Rendering mode '${externalParams.render}' not available, using ${defaultRenderMode} instead.`,
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

  // Create and return validated parameters
  const validatedParams = new ValidatedParams(
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
    params: validatedParams,
  };
}

