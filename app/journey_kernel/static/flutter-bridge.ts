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
    setDeleteMode?: (enabled: boolean) => void;
    onTrackSelected?: FlutterMessageChannel;
    onSelectionBox?: FlutterMessageChannel;
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
  private deleteMode: boolean = false;
  private startPoint: maplibregl.Point | null = null;
  private startLngLat: maplibregl.LngLat | null = null;
  private boxElement: HTMLDivElement | null = null;
  private startMarker: HTMLDivElement | null = null;

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
    // Ensure selection overlays (absolute positioned) are relative to the map container.
    const container = this.map.getContainer();
    if (window.getComputedStyle(container).position === "static") {
      container.style.position = "relative";
    }

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

    this.map.on("click", (e) => {
      if (this.deleteMode) {
        const { lng, lat } = e.lngLat;
        console.log(`Clicked at ${lng}, ${lat} in delete mode`);
        if (window.onTrackSelected) {
          window.onTrackSelected.postMessage(JSON.stringify({ lng, lat }));
        }
      }
    });

    // Box selection logic (support both mouse + touch; WebView on mobile uses touch)
    const startSelectionBox = (e: maplibregl.MapMouseEvent | maplibregl.MapTouchEvent) => {
      if (!this.deleteMode) return;

      const originalEvent = (e as any).originalEvent as any;
      if (originalEvent?.shiftKey) return; // Allow normal box zoom with shift
      if (originalEvent?.touches && originalEvent.touches.length > 1) return; // ignore pinch zoom
      (e as any).preventDefault?.();

      this.startPoint = e.point;
      this.startLngLat = e.lngLat;
      this.map.dragPan.disable();

      // Create start marker
      this.startMarker = document.createElement("div");
      this.startMarker.style.position = "absolute";
      this.startMarker.style.width = "10px";
      this.startMarker.style.height = "10px";
      this.startMarker.style.backgroundColor = "red";
      this.startMarker.style.borderRadius = "50%";
      this.startMarker.style.transform = "translate(-50%, -50%)";
      this.startMarker.style.pointerEvents = "none";
      this.startMarker.style.zIndex = "10000";
      this.startMarker.style.left = this.startPoint.x + "px";
      this.startMarker.style.top = this.startPoint.y + "px";
      container.appendChild(this.startMarker);

      this.boxElement = document.createElement("div");
      this.boxElement.classList.add("box-selection");
      this.boxElement.style.position = "absolute";
      this.boxElement.style.border = "2px dashed red";
      this.boxElement.style.backgroundColor = "rgba(255, 0, 0, 0.2)";
      this.boxElement.style.zIndex = "9999";
      this.boxElement.style.pointerEvents = "none";
      this.boxElement.style.left = this.startPoint.x + "px";
      this.boxElement.style.top = this.startPoint.y + "px";
      this.boxElement.style.width = "0px";
      this.boxElement.style.height = "0px";
      container.appendChild(this.boxElement);
    };

    const moveSelectionBox = (e: maplibregl.MapMouseEvent | maplibregl.MapTouchEvent) => {
      if (!this.deleteMode || !this.startPoint || !this.boxElement) return;

      const originalEvent = (e as any).originalEvent as any;
      if (originalEvent?.touches && originalEvent.touches.length > 1) return; // ignore pinch
      (e as any).preventDefault?.();

      const currentPoint = e.point;
      const minX = Math.min(this.startPoint.x, currentPoint.x);
      const maxX = Math.max(this.startPoint.x, currentPoint.x);
      const minY = Math.min(this.startPoint.y, currentPoint.y);
      const maxY = Math.max(this.startPoint.y, currentPoint.y);

      this.boxElement.style.left = minX + "px";
      this.boxElement.style.top = minY + "px";
      this.boxElement.style.width = maxX - minX + "px";
      this.boxElement.style.height = maxY - minY + "px";
    };

    const endSelectionBox = (e: maplibregl.MapMouseEvent | maplibregl.MapTouchEvent) => {
      if (!this.deleteMode || !this.startPoint || !this.boxElement) return;

      const startLngLat = this.startLngLat;
      const endLngLat = (e as any).lngLat ?? this.map.unproject(e.point);

      // Only trigger if box has some size (avoid triggering on simple clicks)
      if (
        Math.abs(e.point.x - this.startPoint.x) > 5 ||
        Math.abs(e.point.y - this.startPoint.y) > 5
      ) {
        if (window.onSelectionBox && startLngLat) {
          window.onSelectionBox.postMessage(
            JSON.stringify({
              startLat: startLngLat.lat,
              startLng: startLngLat.lng,
              endLat: endLngLat.lat,
              endLng: endLngLat.lng,
            }),
          );
        }
      }

      this.boxElement.remove();
      this.boxElement = null;
      if (this.startMarker) {
        this.startMarker.remove();
        this.startMarker = null;
      }
      this.startPoint = null;
      this.startLngLat = null;
      // Keep dragPan disabled if we are still in delete mode
      if (!this.deleteMode) {
        this.map.dragPan.enable();
      }
    };

    this.map.on("mousedown", startSelectionBox);
    this.map.on("touchstart", startSelectionBox);
    this.map.on("mousemove", moveSelectionBox);
    this.map.on("touchmove", moveSelectionBox);
    this.map.on("mouseup", endSelectionBox);
    this.map.on("touchend", endSelectionBox);
    this.map.on("touchcancel", endSelectionBox);
  }

  /**
   * Setup all window methods that Flutter can call
   */
  setupFlutterCallableMethods(): void {
    window.setDeleteMode = (enabled: boolean) => {
      this.deleteMode = enabled;
      console.log(`Delete mode set to ${enabled}`);
      // Change cursor to indicate delete mode
      this.map.getCanvas().style.cursor = enabled ? "crosshair" : "";
      
      if (enabled) {
        this.map.dragPan.disable();
      } else {
        this.map.dragPan.enable();
        
        // Clean up any active selection
        if (this.boxElement) {
            this.boxElement.remove();
            this.boxElement = null;
        }
        if (this.startMarker) {
            this.startMarker.remove();
            this.startMarker = null;
        }
        this.startPoint = null;
        this.startLngLat = null;
      }
    };

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
