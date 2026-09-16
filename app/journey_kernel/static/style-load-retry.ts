import type { Map } from "maplibre-gl";

export const PLACEHOLDER_STYLE_METADATA_KEY = "memolanes:placeholder";
// Assume a style load completes within 8 seconds.
const STYLE_RETRY_INTERVAL_MS = 8_000;

/** Retry the initial basemap until it replaces the placeholder style. */
export class StyleLoadRetry {
  private readonly timer: ReturnType<typeof setInterval>;

  constructor(
    map: Pick<Map, "getStyle">,
    retry: () => void,
    canRetry: () => boolean,
  ) {
    this.timer = setInterval(() => {
      if (!canRetry()) return;
      const style = map.getStyle();
      if (!style) return;
      const metadata = style.metadata as Record<string, unknown> | undefined;
      if (metadata?.[PLACEHOLDER_STYLE_METADATA_KEY] !== true) {
        this.dispose();
        return;
      }
      retry();
    }, STYLE_RETRY_INTERVAL_MS);
  }

  dispose(): void {
    clearInterval(this.timer);
  }
}
