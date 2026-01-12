/**
 * Flutter Bridge Module
 * Manages all communication between WebView and Flutter
 *
 * This module now uses ReactiveParams for parameter updates.
 * When properties like renderMode or journeyId are set on params,
 * the registered hooks in index.ts automatically handle the side effects
 * (e.g., switching layers, refreshing tile data).
 */

import maplibregl from "maplibre-gl";
import type { ReactiveParams } from "./params";

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
    updateJourneyId?: (newJourneyId: string) => boolean;
  }
}

export interface FlutterBridgeConfig {
  map: maplibregl.Map;
  params: ReactiveParams;
}

export class FlutterBridge {
  private map: maplibregl.Map;
  private locationMarker: maplibregl.Marker;
  private params: ReactiveParams;

  constructor(config: FlutterBridgeConfig) {
    this.map = config.map;
    this.params = config.params;

    // Create location marker element
    const el = document.createElement("div");
    el.className = "location-marker";

    // Create the marker (not added to map until updateLocationMarker is called)
    this.locationMarker = new maplibregl.Marker({
      element: el,
    });
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
    window.updateLocationMarker = (() => {
    let isFlying = false;
    return (
      lng: number,
      lat: number,
      show: boolean = true,
      flyto: boolean = false
    ) => {
      if (show) {
        this.locationMarker.setLngLat([lng, lat]).addTo(this.map);

        if (flyto && !isFlying) {
          const currentZoom = this.map.getZoom();
          isFlying = true;

          this.map.flyTo({
            center: [lng, lat],
            zoom: currentZoom < 14 ? 16 : currentZoom,
            essential: true,
          });

          const onMoveEnd = () => {
            isFlying = false;
            this.map.off('moveend', onMoveEnd);
          };
          this.map.on('moveend', onMoveEnd);
        }
      } else {
        this.locationMarker.remove();
      }
    };
  })();

    /**
     * Update journey ID
     *
     * This method now simply sets params.journeyId.
     * The ReactiveParams hook system automatically triggers pollForJourneyUpdates()
     * when the value changes.
     *
     * @param newJourneyId - The new journey ID
     * @returns true if the journey ID was changed, false if empty or already set
     */
    window.updateJourneyId = (newJourneyId: string): boolean => {
      if (!newJourneyId) {
        console.warn("updateJourneyId: journey ID cannot be empty");
        return false;
      }

      if (newJourneyId === this.params.journeyId) {
        console.log(
          `updateJourneyId: journey ID is already set to '${newJourneyId}'`,
        );
        return false;
      }

      console.log(
        `updateJourneyId: switching from '${this.params.journeyId}' to '${newJourneyId}'`,
      );

      // Simply set the journeyId - the hook handles pollForJourneyUpdates
      this.params.journeyId = newJourneyId;

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
