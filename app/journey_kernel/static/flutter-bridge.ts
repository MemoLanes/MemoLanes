/**
 * Flutter Bridge Module
 * Manages all communication between WebView and Flutter
 *
 * This module now uses ReactiveParams for parameter updates.
 * When properties like renderMode are set on params,
 * the registered hooks in index.ts automatically handle the side effects
 * (e.g., switching layers, refreshing tile data).
 */

import * as maplibregl from "maplibre-gl";
import { MapController, type MapAppearance } from "./map-controller";
import { LocationFollowController } from "./location-follow-controller";
import { MAX_MAP_ZOOM } from "./layer-config";
import { observeSingleMapTap } from "./single-map-tap";
import {
  createAchievementRegionOverlay,
  type AchievementRegionFeature,
} from "./achievement-region-overlay";

interface MapBounds {
  west: number;
  south: number;
  east: number;
  north: number;
}

interface MapPadding {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

interface MapView {
  lng: number;
  lat: number;
  zoom: number;
}

interface AchievementRegionCommand {
  document: number;
  restore?: boolean;
  owner?: number;
  generation: number;
  revision: number;
  featureGeoJson: string | null;
  bounds: MapBounds | null;
  padding: MapPadding;
  view: MapView | null;
}

// Type definitions for Flutter message channels
interface FlutterMessageChannel {
  postMessage: (message: string) => void;
}

// Declare window extensions for Flutter channels
declare global {
  interface Window {
    readyForDisplay?: FlutterMessageChannel;
    readyForCommands?: FlutterMessageChannel;
    onMapMoved?: FlutterMessageChannel;
    onMapTap?: FlutterMessageChannel;
    setMapTapEnabled?: (
      document: number,
      lease: number,
      enabled: boolean,
    ) => void;
    applyMapAppearance?: (document: number, appearance: MapAppearance) => void;
    onMapViewChanged?: FlutterMessageChannel;
    onMapZoomChanged?: FlutterMessageChannel;
    onMapAttributionChanged?: FlutterMessageChannel;
    trySetup?: () => Promise<void>;
    updateLocationMarker?: (
      lng: number,
      lat: number,
      show?: boolean,
      flyto?: boolean,
    ) => void;
    getCurrentMapView?: () => string;
    captureMapScreenshot?: () => Promise<string>;
    getMapAttribution?: () => string | null;
    refreshMapData?: () => Promise<boolean | null>;
    flyToBounds?: (bounds: MapBounds, padding: MapPadding) => boolean;
    flyToView?: (view: MapView) => void;
    setAchievementRegion?: (feature: AchievementRegionFeature | null) => void;
    applyAchievementRegionCommand?: (
      command: AchievementRegionCommand,
    ) => false | { document: number; owner: number; generation: number };
  }
}

export class FlutterBridge {
  private mapController: MapController;
  private map: maplibregl.Map;
  private locationMarker: maplibregl.Marker;
  private cameraOwner = 0;
  private suppressViewPush = false;
  private locationFollowController: LocationFollowController;

  constructor(mapController: MapController) {
    this.mapController = mapController;
    const owner = window.EXTERNAL_PARAMS?.achievement_map_owner;
    this.cameraOwner = typeof owner === "number" ? owner : 0;
    this.map = mapController.getMap();

    // Create location marker element
    const el = document.createElement("div");
    el.className = "location-marker";

    // Create the marker (not added to map until updateLocationMarker is called)
    this.locationMarker = new maplibregl.Marker({
      element: el,
    });
    this.locationFollowController = new LocationFollowController(this.map, {
      camera: this.mapController,
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
      window.onMapMoved.postMessage(
        JSON.stringify({
          document: window.EXTERNAL_PARAMS?.achievement_map_document,
          owner: this.cameraOwner,
        }),
      );
    }
  }

  private getMapAttribution(): string | null {
    // MapLibre has already collected and sanitized the visible source credits.
    const credits = this.map
      .getContainer()
      .querySelector(
        ".maplibregl-ctrl-attrib:not(.maplibregl-attrib-empty) .maplibregl-ctrl-attrib-inner",
      );
    if (!credits?.textContent?.trim()) return null;

    // Preserve links for Flutter's Markdown dialog; treat other content as text.
    const escapeText = (text: string) =>
      text.replace(/[!-/:-@\[-`{-~]/g, "\\$&");
    const toMarkdown = (node: Node): string => {
      if (node.nodeType === Node.TEXT_NODE) {
        return escapeText(node.textContent ?? "");
      }
      if (node instanceof HTMLBRElement) return "\n";
      if (node instanceof HTMLAnchorElement) {
        const label = escapeText(node.textContent ?? "");
        if (node.protocol !== "https:" && node.protocol !== "http:") {
          return label;
        }
        const href = node.href
          .replace(/[<>\\\s]/g, encodeURIComponent)
          .replace(/&/g, "&amp;");
        return `[${label}](<${href}>)`;
      }
      return Array.from(node.childNodes, toMarkdown).join("");
    };
    return toMarkdown(credits);
  }

  observeMapAttribution(): void {
    const channel = window.onMapAttributionChanged;
    const control = this.map
      .getContainer()
      .querySelector(".maplibregl-ctrl-attrib");
    if (!channel || !control) return;

    let available = false;
    const update = () => {
      const next = Boolean(this.getMapAttribution()?.trim());
      if (next === available) return;
      available = next;
      channel.postMessage(String(next));
    };
    const observer = new MutationObserver(update);
    observer.observe(control, {
      attributes: true,
      attributeFilter: ["class"],
      childList: true,
      subtree: true,
    });
    this.map.once("remove", () => observer.disconnect());
    update();
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
      if (!window.onMapViewChanged || this.suppressViewPush) return;

      const now = Date.now();
      if (now - lastPushTime < THROTTLE_MS) {
        return;
      }

      const center = this.map.getCenter();
      const payload = {
        document: window.EXTERNAL_PARAMS?.achievement_map_document,
        owner: this.cameraOwner,
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
   * Setup all map event listeners that notify Flutter
   */
  setupMapEventListeners(): void {
    // Notify Flutter when user drags the map
    this.map.on("dragstart", () => {
      this.locationFollowController.cancel();
      this.notifyMapMoved();
    });

    // Notify Flutter when user zooms the map
    this.map.on("zoomstart", (event) => {
      const fromUser =
        event.originalEvent && event.originalEvent.type !== "resize";
      if (fromUser) {
        this.locationFollowController.cancel();
        this.notifyMapMoved();
      }
    });

    // Notify Flutter when map view changed
    this.map.on("idle", () => {
      this.notifyMapViewChanged();
    });
  }

  /**
   * Setup all window methods that Flutter can call
   */
  setupFlutterCallableMethods(): void {
    const documentId = window.EXTERNAL_PARAMS?.achievement_map_document;
    let stopObservingTaps: (() => void) | undefined;
    let tapLease = -1;
    window.setMapTapEnabled = (document, lease, enabled) => {
      if (document !== documentId || lease <= tapLease) return;
      tapLease = lease;
      stopObservingTaps?.();
      stopObservingTaps = enabled
        ? observeSingleMapTap(this.map, () => {
            window.onMapTap?.postMessage(JSON.stringify({ document, lease }));
          })
        : undefined;
    };
    this.map.once("remove", () => stopObservingTaps?.());
    window.applyMapAppearance = (document, appearance) => {
      if (document === documentId)
        this.mapController.applyMapAppearance(appearance);
    };

    let setRegion:
      ReturnType<typeof createAchievementRegionOverlay> | undefined;
    window.setAchievementRegion = (feature) => {
      if (feature !== null)
        setRegion ??= createAchievementRegionOverlay(this.map);
      setRegion?.(feature);
    };

    if (typeof documentId === "number") {
      let latestGeneration = 0;
      let drawnFeature: string | null | undefined = null;
      let fittedRevision: number | undefined;
      let fittedOwner: number | undefined;
      let pendingFit:
        | {
            owner: number;
            revision: number;
            bounds: MapBounds;
            padding: MapPadding;
          }
        | undefined;
      const finishPendingFit = () => {
        const fit = pendingFit;
        if (!fit || !window.flyToBounds!(fit.bounds, fit.padding)) return;
        fittedRevision = fit.revision;
        fittedOwner = fit.owner;
        pendingFit = undefined;
      };
      const retryAfterResize = () => {
        const fit = pendingFit;
        if (!fit) return;
        // MapLibre emits the resize's moveend after resize; it would end the polar lock.
        queueMicrotask(() => {
          if (pendingFit === fit) finishPendingFit();
        });
      };
      this.map.on("resize", retryAfterResize);
      this.map.once("remove", () => {
        pendingFit = undefined;
        this.map.off("resize", retryAfterResize);
      });
      window.applyAchievementRegionCommand = (command) => {
        if (
          command.document !== documentId ||
          (command.owner ?? 0) < this.cameraOwner ||
          command.generation <= latestGeneration
        ) {
          return false;
        }
        // Reject older commands even if applying this one fails partway through.
        latestGeneration = command.generation;
        pendingFit = undefined;
        const owner = command.owner ?? 0;
        if (owner !== this.cameraOwner || command.featureGeoJson === null) {
          this.suppressViewPush = true;
          this.locationFollowController.cancel();
          this.mapController.cancelCamera();
          this.cameraOwner = owner;
          if (!command.restore) this.suppressViewPush = false;
        }
        const { featureGeoJson, revision, bounds, padding, view } = command;
        if (featureGeoJson !== drawnFeature) {
          // A partial drawing failure must not make the next request skip drawing.
          drawnFeature = undefined;
          window.setAchievementRegion!(
            featureGeoJson === null ? null : JSON.parse(featureGeoJson),
          );
          drawnFeature = featureGeoJson;
        }
        if (command.restore) {
          // Publish the new lease only after the home camera is installed.
          // jumpTo avoids an intermediate Explorer-to-home persistence window.
          if (view)
            this.mapController.jumpTo({
              center: [view.lng, view.lat],
              zoom: view.zoom,
            });
          this.suppressViewPush = false;
          return {
            document: documentId,
            owner,
            generation: command.generation,
          };
        }
        // Flutter may retry a request whose fit ran but whose reply was lost.
        if (
          (owner !== fittedOwner || revision !== fittedRevision) &&
          (bounds || view)
        ) {
          if (bounds) {
            // Flutter can publish the new padding before MapLibre sees the resize.
            pendingFit = { owner, revision, bounds, padding };
            finishPendingFit();
          } else if (view) {
            window.flyToView!(view);
            fittedRevision = revision;
            fittedOwner = owner;
          }
        }
        return { document: documentId, owner, generation: command.generation };
      };
    }

    // Update location marker
    window.updateLocationMarker = (
      lng: number,
      lat: number,
      show: boolean = true,
      flyto: boolean = false,
    ) => {
      if (show) {
        this.locationMarker.setLngLat([lng, lat]).addTo(this.map);
        this.locationFollowController.update(lng, lat, flyto);
      } else {
        this.locationFollowController.cancel();
        this.locationMarker.remove();
      }
    };

    // Get current map view
    window.getCurrentMapView = () => {
      const center = this.map.getCenter();
      return JSON.stringify({
        document: window.EXTERNAL_PARAMS?.achievement_map_document,
        owner: this.cameraOwner,
        lng: center.lng,
        lat: center.lat,
        zoom: this.map.getZoom(),
      });
    };

    window.getMapAttribution = () => this.getMapAttribution();

    // Refresh map data - allows Flutter to trigger a data refresh
    window.refreshMapData = () => this.mapController.refreshMapData();
    window.captureMapScreenshot = () => this.mapController.captureScreenshot();

    // Focus a journey within the visible area above its Flutter detail card.
    window.flyToBounds = (bounds, padding) => {
      // MapLibre's initial-bounds path calls fitBounds with the same padding
      // and zoom cap. fitBounds calculates the camera, then calls flyTo.
      return this.mapController.fitBounds(
        [
          [bounds.west, bounds.south],
          [bounds.east, bounds.north],
        ],
        { padding, maxZoom: MAX_MAP_ZOOM, duration: 900, essential: true },
      );
    };

    // Return from journey details to the camera saved before opening them.
    window.flyToView = (view) => {
      this.mapController.flyTo({
        center: [view.lng, view.lat],
        zoom: view.zoom,
        duration: 900,
        essential: true,
      });
    };
  }

  /**
   * Initialize all Flutter bridge functionality
   * Call this after map is loaded and all dependencies are ready
   */
  initialize(): void {
    this.setupMapEventListeners();
    this.setupFlutterCallableMethods();
    this.observeMapAttribution();
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
