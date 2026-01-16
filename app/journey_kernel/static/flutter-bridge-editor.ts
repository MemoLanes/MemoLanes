import maplibregl from "maplibre-gl";
import { FlutterBridge } from "./flutter-bridge";

// Declare window extensions for Flutter channels (editor only)
declare global {
  interface Window {
    setDeleteMode?: (enabled: boolean) => void;
    setDrawMode?: (enabled: boolean) => void;
    onSelectionBox?: { postMessage: (message: string) => void };
    onDrawPath?: { postMessage: (message: string) => void };
  }
}

export interface FlutterBridgeEditorConfig {
  flutterBridge: FlutterBridge;
}

export class FlutterBridgeEditor {
  private map: maplibregl.Map;
  private deleteMode: boolean = false;
  private drawMode: boolean = false;
  private startPoint: maplibregl.Point | null = null;
  private startLngLat: maplibregl.LngLat | null = null;
  private boxElement: HTMLDivElement | null = null;
  private startMarker: HTMLDivElement | null = null;

  // Freehand draw state
  private drawPoints: maplibregl.LngLat[] = [];
  private drawSourceId = "_flutter_draw_path";
  private drawLayerId = "_flutter_draw_path_layer";

  constructor(config: FlutterBridgeEditorConfig) {
    this.map = config.flutterBridge.getMap();
  }

  initialize(): void {
    this.setupEditorEventListeners();
    this.setupEditorCallableMethods();
  }

  private isMultiTouch(event: any): boolean {
    return Boolean(event?.touches && event.touches.length > 1);
  }

  private preventDefault(event: any): void {
    event?.preventDefault?.();
  }

  private clearSelectionOverlay(): void {
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

  private ensureDrawLayer(): void {
    const srcAny = (this.map.getSource(this.drawSourceId) as any) ?? null;
    if (!srcAny) {
      this.map.addSource(this.drawSourceId, {
        type: "geojson",
        data: {
          type: "Feature",
          geometry: { type: "LineString", coordinates: [] },
        },
      } as any);
    }
    if (!this.map.getLayer(this.drawLayerId)) {
      this.map.addLayer({
        id: this.drawLayerId,
        type: "line",
        source: this.drawSourceId,
        layout: {
          "line-join": "round",
          "line-cap": "round",
        },
        paint: {
          "line-color": "#B6E13D",
          "line-width": 4,
          "line-opacity": 0.9,
        },
      } as any);
    }
  }

  private updateDrawLayer(): void {
    const src = this.map.getSource(this.drawSourceId) as any;
    if (!src) return;
    const coords = this.drawPoints.map((p) => [p.lng, p.lat]);
    src.setData({
      type: "Feature",
      geometry: { type: "LineString", coordinates: coords },
    });
  }

  private clearDrawLayer(): void {
    this.drawPoints = [];
    this.updateDrawLayer();
  }

  private simplifyDrawPoints(
    points: maplibregl.LngLat[],
    minPixelDistance: number,
  ): maplibregl.LngLat[] {
    if (points.length <= 2) return points;

    const simplified: maplibregl.LngLat[] = [points[0]];
    let lastProjected = this.map.project(points[0]);

    for (let i = 1; i < points.length - 1; i++) {
      const projected = this.map.project(points[i]);
      const dx = projected.x - lastProjected.x;
      const dy = projected.y - lastProjected.y;
      if (Math.hypot(dx, dy) >= minPixelDistance) {
        simplified.push(points[i]);
        lastProjected = projected;
      }
    }

    // Always keep the last point to preserve the end of the stroke.
    if (simplified[simplified.length - 1] !== points[points.length - 1]) {
      simplified.push(points[points.length - 1]);
    }

    return simplified;
  }

  private setupEditorEventListeners(): void {
    // Ensure selection overlays (absolute positioned) are relative to the map container.
    const container = this.map.getContainer();
    if (window.getComputedStyle(container).position === "static") {
      container.style.position = "relative";
    }

    // Box selection logic (support both mouse + touch; WebView on mobile uses touch)
    const startSelectionBox = (
      e: maplibregl.MapMouseEvent | maplibregl.MapTouchEvent,
    ) => {
      if (!this.deleteMode || this.drawMode) return;

      const originalEvent = (e as any).originalEvent as any;
      if (originalEvent?.shiftKey) return; // Allow normal box zoom with shift
      if (this.isMultiTouch(originalEvent)) {
        // Multi-touch should be handled by map (pinch zoom), not edit.
        this.clearSelectionOverlay();
        return;
      }
      this.preventDefault(originalEvent);

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

    const moveSelectionBox = (
      e: maplibregl.MapMouseEvent | maplibregl.MapTouchEvent,
    ) => {
      if (!this.deleteMode || this.drawMode || !this.startPoint || !this.boxElement) return;

      const originalEvent = (e as any).originalEvent as any;
      if (this.isMultiTouch(originalEvent)) {
        // Cancel selection when user starts pinch zoom.
        this.clearSelectionOverlay();
        return;
      }
      this.preventDefault(originalEvent);

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

    const endSelectionBox = (
      e: maplibregl.MapMouseEvent | maplibregl.MapTouchEvent,
    ) => {
      if (!this.deleteMode || this.drawMode || !this.startPoint || !this.boxElement) return;

      const originalEvent = (e as any).originalEvent as any;
      if (this.isMultiTouch(originalEvent)) {
        this.clearSelectionOverlay();
        return;
      }

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

      this.clearSelectionOverlay();
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

    // Freehand draw logic (mouse + touch)
    const startDraw = (e: maplibregl.MapMouseEvent | maplibregl.MapTouchEvent) => {
      if (!this.drawMode || this.deleteMode) return;

      const originalEvent = (e as any).originalEvent as any;
      if (originalEvent?.shiftKey) return;
      if (this.isMultiTouch(originalEvent)) {
        // Let pinch zoom work without starting a draw.
        this.clearDrawLayer();
        return;
      }
      this.preventDefault(originalEvent);

      this.ensureDrawLayer();
      this.clearDrawLayer();

      const lngLat = (e as any).lngLat ?? this.map.unproject(e.point);
      this.drawPoints = [lngLat];
      this.updateDrawLayer();

      this.map.dragPan.disable();
    };

    const moveDraw = (e: maplibregl.MapMouseEvent | maplibregl.MapTouchEvent) => {
      if (!this.drawMode || this.deleteMode) return;
      const originalEvent = (e as any).originalEvent as any;
      if (this.isMultiTouch(originalEvent)) {
        // Cancel drawing when user starts pinch zoom.
        this.clearDrawLayer();
        return;
      }
      this.preventDefault(originalEvent);

      if (this.drawPoints.length === 0) return;
      const lngLat = (e as any).lngLat ?? this.map.unproject(e.point);
      const last = this.drawPoints[this.drawPoints.length - 1];

      // Simple sampling guard: only add if moved a bit
      const eps = 1e-6;
      if (Math.abs(lngLat.lng - last.lng) < eps && Math.abs(lngLat.lat - last.lat) < eps) {
        return;
      }

      this.drawPoints.push(lngLat);
      this.updateDrawLayer();
    };

    const endDraw = (e: maplibregl.MapMouseEvent | maplibregl.MapTouchEvent) => {
      if (!this.drawMode || this.deleteMode) return;
      const originalEvent = (e as any).originalEvent as any;
      if (this.isMultiTouch(originalEvent)) {
        this.clearDrawLayer();
        return;
      }
      this.preventDefault(originalEvent);

      // Add final point
      if (this.drawPoints.length > 0) {
        const lngLat = (e as any).lngLat ?? this.map.unproject(e.point);
        this.drawPoints.push(lngLat);
      }

      const zoom = this.map.getZoom();
      const densityBoost = Math.min(
        6,
        Math.max(0, (this.drawPoints.length - 120) / 120),
      );
      const zoomFactor = Math.max(0, 10 - zoom) * 0.6;
      const minPixelDistance = Math.min(14, 2 + densityBoost * 2 + zoomFactor);

      const finalPoints = this.simplifyDrawPoints(
        this.drawPoints,
        minPixelDistance,
      );

      if (finalPoints.length >= 2) {
        if (window.onDrawPath) {
          window.onDrawPath.postMessage(
            JSON.stringify({
              points: finalPoints.map((p) => ({ lat: p.lat, lng: p.lng })),
            }),
          );
        }
      }

      // Clear the temporary overlay; the real track will be rendered by Rust after update.
      this.clearDrawLayer();

      if (!this.drawMode) {
        this.map.dragPan.enable();
      }
    };

    this.map.on("mousedown", startDraw);
    this.map.on("touchstart", startDraw);
    this.map.on("mousemove", moveDraw);
    this.map.on("touchmove", moveDraw);
    this.map.on("mouseup", endDraw);
    this.map.on("touchend", endDraw);
    this.map.on("touchcancel", endDraw);
  }

  private setupEditorCallableMethods(): void {
    window.setDeleteMode = (enabled: boolean) => {
      this.deleteMode = enabled;
      console.log(`Delete mode set to ${enabled}`);
      // Change cursor to indicate delete mode
      this.map.getCanvas().style.cursor = enabled ? "crosshair" : "";

      // Ensure pinch zoom remains available in edit modes.
      this.map.touchZoomRotate.enable();

      // Delete and draw modes are mutually exclusive.
      if (enabled) {
        this.drawMode = false;
      }

      if (enabled) {
        this.map.dragPan.disable();
      } else {
        this.map.dragPan.enable();
        this.clearSelectionOverlay();
      }
    };

    window.setDrawMode = (enabled: boolean) => {
      this.drawMode = enabled;
      console.log(`Draw mode set to ${enabled}`);
      // Change cursor to indicate draw mode
      this.map.getCanvas().style.cursor = enabled ? "crosshair" : "";

      // Ensure pinch zoom remains available in edit modes.
      this.map.touchZoomRotate.enable();

      // Draw and delete modes are mutually exclusive.
      if (enabled) {
        this.deleteMode = false;
      }

      if (enabled) {
        // Ensure any active selection UI is removed.
        this.clearSelectionOverlay();
        this.map.dragPan.disable();
      } else {
        this.map.dragPan.enable();
        // Clear temporary draw overlay.
        this.clearDrawLayer();
      }
    };
  }
}