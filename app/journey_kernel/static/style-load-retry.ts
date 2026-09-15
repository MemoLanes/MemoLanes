import type { ErrorEvent, Map } from "maplibre-gl";

/** Retry failed style downloads without confusing overlay layers with a basemap. */
export class StyleLoadRetry {
  private readonly map: Pick<Map, "on" | "off">;
  private readonly timer: ReturnType<typeof setInterval>;
  private requestUrl: string | null = null;
  private failed = false;

  private readonly onError = (event: ErrorEvent): void => {
    // Registering an error listener disables MapLibre's default console output.
    console.error(event.error);
    // MapLibre attaches the request URL to HTTP and network errors. Source,
    // tile, sprite and glyph failures must not reload the entire style.
    const error = event.error as Error & { url?: string };
    if (
      this.requestUrl !== null &&
      typeof error.url === "string" &&
      new URL(error.url, window.location.href).href === this.requestUrl
    ) {
      this.failed = true;
    }
  };

  constructor(
    map: Pick<Map, "on" | "off">,
    retry: () => void,
    canRetry: () => boolean,
  ) {
    this.map = map;
    this.map.on("error", this.onError);
    this.timer = setInterval(() => {
      // Do not interrupt a slow request merely because eight seconds elapsed.
      if (!this.failed || !canRetry()) return;
      this.failed = false;
      retry();
    }, 8_000);
  }

  /** Called by transformRequest, after Mapbox URL/token conversion. */
  requestStarted(url: string): void {
    this.requestUrl = new URL(url, window.location.href).href;
    this.failed = false;
  }

  /** Called by transformStyle once the requested JSON has been downloaded. */
  responseReceived(): void {
    this.requestUrl = null;
    this.failed = false;
  }

  dispose(): void {
    clearInterval(this.timer);
    this.map.off("error", this.onError);
    this.responseReceived();
  }
}
