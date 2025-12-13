/**
 * Flutter Bridge Module
 * Manages all communication between WebView and Flutter
 */

import type maplibregl from "maplibre-gl";

// Type definitions for Flutter message channels
interface FlutterMessageChannel {
  postMessage: (message: string) => void;
}

// Declare window extensions for Flutter channels
declare global {
  interface Window {
    readyForDisplay?: FlutterMessageChannel;
    onMapMoved?: FlutterMessageChannel;
    trySetup?: () => Promise<void>;
    updateLocationMarker?: (
      lng: number,
      lat: number,
      show?: boolean,
      flyto?: boolean,
    ) => void;
    getCurrentMapView?: () => string;
    triggerJourneyUpdate?: () => Promise<void>;
    switchRenderingLayer?: (renderingMode: string) => any;
    updateJourneyId?: (newJourneyId: string) => boolean;
  }
}

export interface FlutterBridgeConfig {
  map: maplibregl.Map;
  locationMarker: maplibregl.Marker;
  journeyTileProvider: any; // JourneyTileProvider type
  switchRenderingLayerFn: (map: any, renderingMode: string) => any;
  getCurrentJourneyId: () => string;
  setCurrentJourneyId: (id: string) => void;
}

export class FlutterBridge {
  private map: maplibregl.Map;
  private locationMarker: maplibregl.Marker;
  private journeyTileProvider: any;
  private switchRenderingLayerFn: (map: any, renderingMode: string) => any;
  private getCurrentJourneyId: () => string;
  private setCurrentJourneyId: (id: string) => void;

  constructor(config: FlutterBridgeConfig) {
    this.map = config.map;
    this.locationMarker = config.locationMarker;
    this.journeyTileProvider = config.journeyTileProvider;
    this.switchRenderingLayerFn = config.switchRenderingLayerFn;
    this.getCurrentJourneyId = config.getCurrentJourneyId;
    this.setCurrentJourneyId = config.setCurrentJourneyId;
  }

  /**
   * Notify Flutter that WebView is ready for display
   */
  notifyReady(): void {
    if (window.readyForDisplay) {
      window.readyForDisplay.postMessage("");
    }
  }

  /**
   * Notify Flutter that the map has been moved by user
   */
  notifyMapMoved(): void {
    if (window.onMapMoved) {
      window.onMapMoved.postMessage("");
    }
  }

  /**
   * Setup all map event listeners that notify Flutter
   */
  setupMapEventListeners(): void {
    // Notify Flutter when user drags the map
    this.map.on("dragstart", () => {
      this.notifyMapMoved();
    });

    // Notify Flutter when user zooms the map
    this.map.on("zoomstart", (event) => {
      const fromUser =
        event.originalEvent && event.originalEvent.type !== "resize";
      if (fromUser) {
        this.notifyMapMoved();
      }
    });
  }

  /**
   * Setup all window methods that Flutter can call
   */
  setupFlutterCallableMethods(): void {
    // Update location marker
    window.updateLocationMarker = (
      lng: number,
      lat: number,
      show: boolean = true,
      flyto: boolean = false,
    ) => {
      if (show) {
        this.locationMarker.setLngLat([lng, lat]).addTo(this.map);
        if (flyto) {
          const currentZoom = this.map.getZoom();
          this.map.flyTo({
            center: [lng, lat],
            zoom: currentZoom < 14 ? 16 : currentZoom,
            essential: true,
          });
        }
      } else {
        this.locationMarker.remove();
      }
    };

    // Get current map view
    window.getCurrentMapView = () => {
      const center = this.map.getCenter();
      return JSON.stringify({
        lng: center.lng,
        lat: center.lat,
        zoom: this.map.getZoom(),
      });
    };

    // Trigger journey update
    window.triggerJourneyUpdate = () => {
      return this.journeyTileProvider.pollForJourneyUpdates(false);
    };

    // Switch rendering layer
    window.switchRenderingLayer = (renderingMode: string) => {
      return this.switchRenderingLayerFn(this.map, renderingMode);
    };

    // Update journey ID
    window.updateJourneyId = (newJourneyId: string): boolean => {
      if (!newJourneyId) {
        console.warn("updateJourneyId: journey ID cannot be empty");
        return false;
      }

      const currentJourneyId = this.getCurrentJourneyId();
      if (newJourneyId === currentJourneyId) {
        console.log(
          `updateJourneyId: journey ID is already set to '${newJourneyId}'`,
        );
        return false;
      }

      console.log(
        `updateJourneyId: switching from '${currentJourneyId}' to '${newJourneyId}'`,
      );

      // Update the current journey ID
      this.setCurrentJourneyId(newJourneyId);

      // Update the tile provider's journey ID
      if (this.journeyTileProvider) {
        this.journeyTileProvider.journeyId = newJourneyId;
        // Force update to fetch data for the new journey
        this.journeyTileProvider.pollForJourneyUpdates(true);
      }

      return true;
    };
  }

  /**
   * Initialize all Flutter bridge functionality
   * Call this after map is loaded and all dependencies are ready
   */
  initialize(): void {
    this.setupMapEventListeners();
    this.setupFlutterCallableMethods();
  }
}

/**
 * Helper function to notify Flutter that WebView is ready
 * Can be called before FlutterBridge is initialized
 */
export function notifyFlutterReady(): void {
  if (window.readyForDisplay) {
    window.readyForDisplay.postMessage("");
  }
}

