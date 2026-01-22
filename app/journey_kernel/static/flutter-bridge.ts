/**
 * Flutter Bridge Module
 * Manages all communication between WebView and Flutter
 *
 * This module now uses ReactiveParams for parameter updates.
 * When properties like renderMode are set on params,
 * the registered hooks in index.ts automatically handle the side effects
 * (e.g., switching layers, refreshing tile data).
 */

import maplibregl from "maplibre-gl";
import { MapController } from "./map-controller";

// Type definitions for Flutter message channels
interface FlutterMessageChannel {
  postMessage: (message: string) => void;
}

// Declare window extensions for Flutter channels
declare global {
  interface Window {
    readyForDisplay?: FlutterMessageChannel;
    onMapMoved?: FlutterMessageChannel;
    onMapViewChanged?: FlutterMessageChannel;
    onMapZoomChanged?: FlutterMessageChannel;
    trySetup?: () => Promise<void>;
    updateLocationMarker?: (
      lng: number,
      lat: number,
      show?: boolean,
      flyto?: boolean,
    ) => void;
    getCurrentMapView?: () => string;
    refreshMapData?: () => Promise<boolean | null>;
  }
}

export class FlutterBridge {
  private mapController: MapController;
  private map: maplibregl.Map;
  private locationMarker: maplibregl.Marker;

  constructor(mapController: MapController) {
    this.mapController = mapController;
    this.map = mapController.getMap();

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
  * Get the underlying map instance
  */
  getMap(): maplibregl.Map {
    return this.map;
  }

  /**
   * Notify Flutter that the map view has changed
   */
  notifyMapViewChanged = (() => {
    let lastPushedMapView: string | undefined;
    let lastPushTime = 0;

    const THROTTLE_MS = 1_000;

    return () => {
      if (!window.onMapViewChanged) return;

      const now = Date.now();
      if (now - lastPushTime < THROTTLE_MS) {
        return;
      }

      const center = this.map.getCenter();
      const payload = {
        lng: Number(center.lng.toFixed(6)),
        lat: Number(center.lat.toFixed(6)),
        zoom: Number(this.map.getZoom().toFixed(2)),
        bearing: Number(this.map.getBearing().toFixed(2)),
        pitch: Number(this.map.getPitch().toFixed(2)),
      };

      const json = JSON.stringify(payload);

      if (json === lastPushedMapView) return;

      window.onMapViewChanged.postMessage(json);
      lastPushedMapView = json;
      lastPushTime = now;
    };
  })();

  /**
 * Notify Flutter when the map zoom integer changes
 */
  notifyMapZoomChanged = (() => {
    let lastZoom: number | undefined;
    let lastPushTime = 0;

    const THROTTLE_MS = 50;

    return () => {
      const messageHandler = window.onMapZoomChanged;
      if (!messageHandler?.postMessage) return;

      const now = Date.now();
      if (now - lastPushTime < THROTTLE_MS) return;

      const zoom = Math.trunc(this.map.getZoom());

      if (zoom === lastZoom) return;

      messageHandler.postMessage(zoom.toString());

      lastZoom = zoom;
      lastPushTime = now;
    };
  })();

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

    // Notify Flutter when map view changed
    this.map.on("idle", () => {
      this.notifyMapViewChanged();
    });

    // Notify Flutter when map finished loading
    this.map.on("load", () => {
      this.notifyMapZoomChanged();
    });

    // Notify Flutter when zoom level changes
    this.map.on("zoom", () => {
      this.notifyMapZoomChanged();
    });
  }

  /**
   * Setup all window methods that Flutter can call
   */
  setupFlutterCallableMethods(): void {
    // Update location marker
    window.updateLocationMarker = (() => {
      let isFlying = false;
      const onMoveEnd = () => {
        isFlying = false;
      };
      this.map.on("moveend", onMoveEnd);
      return (
        lng: number,
        lat: number,
        show: boolean = true,
        flyto: boolean = false,
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
          }
        } else {
          this.locationMarker.remove();
        }
      };
    })();

    // Get current map view
    window.getCurrentMapView = () => {
      const center = this.map.getCenter();
      return JSON.stringify({
        lng: center.lng,
        lat: center.lat,
        zoom: this.map.getZoom(),
      });
    };

    // Refresh map data - allows Flutter to trigger a data refresh
    window.refreshMapData = () => this.mapController.refreshMapData();
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
