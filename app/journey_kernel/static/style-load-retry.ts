import type { ErrorEvent, Map, MapSourceDataEvent } from "maplibre-gl";

export const PLACEHOLDER_STYLE_METADATA_KEY = "memolanes:placeholder";

/** A one-shot guard for a custom basemap load, independent of fog loading. */
export class BasemapLoadGuard {
  private readonly map: Pick<Map, "getStyle" | "on" | "off">;
  private readonly fallback: () => void;
  private readonly sourceIds: ReadonlySet<string>;
  private readonly timer: ReturnType<typeof setTimeout>;
  private settled = false;
  private disposed = false;
  private styleApplying = false;
  private sourceFailed = false;
  private styleLoaded = false;
  private tileRequested = false;

  constructor(
    map: Pick<Map, "getStyle" | "on" | "off">,
    fallback: () => void,
    sourceIds: ReadonlySet<string>,
  ) {
    this.map = map;
    this.fallback = fallback;
    this.sourceIds = sourceIds;
    this.timer = setTimeout(() => this.fail(), 20_000);
    map.on("error", this.onError);
    map.on("style.load", this.onStyleLoad);
    map.on("sourcedata", this.onSourceData);
    map.on("sourcedataloading", this.onSourceLoading);
    map.on("idle", this.onIdle);
    map.on("remove", this.dispose);
    // GPU recovery belongs to the existing WebView reload path.
    map.on("webglcontextlost", this.dispose);
  }

  /** Start SDK error/event tracking only when this load's JSON is applied. */
  watchStyle(): void {
    this.styleApplying = true;
  }

  private hasLoadedStyle(): boolean {
    const style = this.map.getStyle();
    return (
      this.styleLoaded &&
      !!style &&
      (style.metadata as Record<string, unknown> | undefined)?.[
        PLACEHOLDER_STYLE_METADATA_KEY
      ] !== true
    );
  }

  private readonly onError = (
    event: ErrorEvent & { sourceId?: string },
  ): void => {
    if (this.styleApplying && !this.styleLoaded && !event.sourceId) {
      this.fail(); // Validation of the style object failed.
    } else if (
      this.styleLoaded &&
      event.sourceId &&
      this.sourceIds.has(event.sourceId)
    ) {
      this.sourceFailed = true;
    }
    // Sprite/font errors on a loaded style do not by themselves cause fallback.
  };

  private readonly onStyleLoad = (): void => {
    if (this.styleApplying) this.styleLoaded = true;
  };

  private readonly onSourceData = (event: MapSourceDataEvent): void => {
    if (
      !this.hasLoadedStyle() ||
      !this.sourceIds.has(event.sourceId) ||
      event.tile?.state !== "loaded"
    ) {
      return;
    }
    this.tileRequested = true;
    if (
      this.map.getStyle()?.sources[event.sourceId]?.type === "vector" &&
      !event.tile.latestFeatureIndex
    ) {
      // MapLibre marks vector 404s as loaded without parsed data. Even a
      // successful vector tile with zero features has a feature index.
      return;
    }
    // Some tiles may fail at coverage edges. One usable tile proves that the
    // basemap works, so later partial failures must not replace it.
    this.finish();
  };

  private readonly onSourceLoading = (event: MapSourceDataEvent): void => {
    if (
      this.hasLoadedStyle() &&
      this.sourceIds.has(event.sourceId) &&
      event.tile
    ) {
      this.tileRequested = true;
    }
  };

  private readonly onIdle = (): void => {
    if (!this.hasLoadedStyle()) return;
    // MapLibre suppresses tile 404 errors. If it requested tiles but reached
    // idle without one usable tile, the entire initial viewport failed.
    if (this.sourceFailed || this.tileRequested) this.fail();
    else this.finish(); // Also accepts valid background/GeoJSON-only styles.
  };

  fail(): void {
    if (this.settled) return;
    this.finish();
    // Leave MapLibre's event dispatch before replacing its current style.
    queueMicrotask(() => {
      if (!this.disposed) this.fallback();
    });
  }

  private finish(): void {
    this.settled = true;
    clearTimeout(this.timer);
    this.map.off("error", this.onError);
    this.map.off("style.load", this.onStyleLoad);
    this.map.off("sourcedata", this.onSourceData);
    this.map.off("sourcedataloading", this.onSourceLoading);
    this.map.off("idle", this.onIdle);
    this.map.off("remove", this.dispose);
    this.map.off("webglcontextlost", this.dispose);
  }

  readonly dispose = (): void => {
    this.disposed = true;
    this.finish();
  };
}
