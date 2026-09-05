import type { Map as MaplibreMap } from "maplibre-gl";

interface LocationFollowOptions {
  minUpdateIntervalMs?: number;
  animationDurationMs?: number;
  minMovementPixels?: number;
}

/**
 * Presentation-only, latest-value camera following.
 *
 * GPS persistence happens before this controller is called. While an animation
 * is active, newer fixes replace the pending target instead of extending the
 * current animation indefinitely.
 *
 * Long-distance locate uses MapLibre's distance-based `flyTo` duration. A fixed
 * short duration makes city-scale flights look like a teleport. Nearby GPS
 * tracking uses `easeTo` so the camera pans at the current zoom instead of
 * flying out and back in.
 */
export class LocationFollowController {
  private readonly map: MaplibreMap;
  private pendingCenter: [number, number] | null = null;
  private following = false;
  private ownAnimationActive = false;
  private lastAnimationStartedAt = Number.NEGATIVE_INFINITY;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private readonly minUpdateIntervalMs: number;
  private readonly animationDurationMs: number;
  private readonly minMovementPixels: number;

  private readonly onMoveEnd = () => {
    this.ownAnimationActive = false;
    this.pump();
  };

  constructor(map: MaplibreMap, options: LocationFollowOptions = {}) {
    this.map = map;
    this.minUpdateIntervalMs = options.minUpdateIntervalMs ?? 400;
    this.animationDurationMs = options.animationDurationMs ?? 900;
    this.minMovementPixels = options.minMovementPixels ?? 1.5;
    this.map.on("moveend", this.onMoveEnd);
  }

  update(lng: number, lat: number, following: boolean): void {
    if (!following) {
      this.cancel();
      return;
    }
    this.following = true;
    this.pendingCenter = [lng, lat];
    this.pump();
  }

  /** Stop presentation following without affecting location collection. */
  cancel(): void {
    const stopOwnAnimation = this.ownAnimationActive;
    this.following = false;
    this.pendingCenter = null;
    this.ownAnimationActive = false;
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    if (stopOwnAnimation) {
      this.map.stop();
    }
  }

  dispose(): void {
    this.cancel();
    this.map.off("moveend", this.onMoveEnd);
  }

  private pump(): void {
    if (
      !this.following ||
      this.pendingCenter === null ||
      this.ownAnimationActive ||
      this.map.isMoving()
    ) {
      return;
    }

    const waitMs = Math.max(
      0,
      this.lastAnimationStartedAt +
        this.minUpdateIntervalMs -
        performance.now(),
    );
    if (waitMs > 0) {
      if (this.timer === null) {
        this.timer = setTimeout(() => {
          this.timer = null;
          this.pump();
        }, waitMs);
      }
      return;
    }

    const center = this.pendingCenter;
    this.pendingCenter = null;
    const currentZoom = this.map.getZoom();
    const screenDistance = this.screenDistancePx(center);
    if (currentZoom >= 11 && screenDistance < this.minMovementPixels) {
      return;
    }

    this.ownAnimationActive = true;
    this.lastAnimationStartedAt = performance.now();

    const targetZoom = currentZoom < 11 ? 14 : currentZoom;
    if (currentZoom < 11 || screenDistance >= this.longFlightThresholdPx()) {
      // Let MapLibre derive duration from distance so locate-me stays a flight.
      this.map.flyTo({
        center,
        zoom: targetZoom,
        essential: true,
      });
      return;
    }

    this.map.easeTo({
      center,
      duration: this.animationDurationMs,
      easing: easeOutQuad,
      essential: true,
    });
  }

  private screenDistancePx(center: [number, number]): number {
    const target = this.map.project(center);
    const canvas = this.map.getCanvas();
    const dx = target.x - canvas.clientWidth / 2;
    const dy = target.y - canvas.clientHeight / 2;
    return Math.hypot(dx, dy);
  }

  private longFlightThresholdPx(): number {
    const canvas = this.map.getCanvas();
    const shortest = Math.min(canvas.clientWidth, canvas.clientHeight);
    return Math.max(120, shortest * 0.35);
  }
}

function easeOutQuad(value: number): number {
  return 1 - (1 - value) * (1 - value);
}
