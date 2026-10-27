import type { Map } from "maplibre-gl";

export const BASEMAP_REQUEST_ID_METADATA_KEY = "memolanes:basemap-request-id";

/** Loaded sources include attribution from TileJSON, absent in a style URL. */
export function collectMapAttributions(
  map: Map,
): { requestId: number; entries: string[] } | undefined {
  const style = map.getStyle();
  const requestId = (style?.metadata as Record<string, unknown> | undefined)?.[
    BASEMAP_REQUEST_ID_METADATA_KEY
  ];
  if (typeof requestId !== "number") return;
  const entries = new Set<string>();
  for (const id of Object.keys(style.sources)) {
    const attribution = map.getSource(id)?.attribution;
    if (attribution?.trim()) entries.add(attribution.trim());
  }
  return { requestId, entries: [...entries] };
}

function escapeMarkdown(text: string): string {
  return text.replace(/[\\`*_{}\[\]<>!#|]/g, "\\$&");
}

/** Keep provider links while never forwarding executable markup to Flutter. */
export function attributionToMarkdown(html: string): string {
  const template = document.createElement("template");
  template.innerHTML = html;
  const visit = (node: Node): string => {
    if (node.nodeType === Node.TEXT_NODE)
      return escapeMarkdown(node.textContent || "");
    if (node.nodeType !== Node.ELEMENT_NODE) return "";
    const element = node as Element;
    if (["SCRIPT", "STYLE", "IFRAME", "OBJECT"].includes(element.tagName))
      return "";
    if (element.tagName === "BR") return "\n";
    if (element.tagName === "IMG")
      return escapeMarkdown(element.getAttribute("alt") || "");
    const text = [...element.childNodes].map(visit).join("");
    if (element.tagName === "A") {
      try {
        const url = new URL(element.getAttribute("href") || "");
        if (url.protocol === "https:" || url.protocol === "http:") {
          const href = url.href.replace(/\(/g, "%28").replace(/\)/g, "%29");
          return `[${text}](${href})`;
        }
      } catch {
        /* Keep the label of unsupported links as plain text. */
      }
    }
    return text;
  };
  return [...template.content.childNodes].map(visit).join("").trim();
}
