import type { TileBuffer } from "../../pkg/journey_kernel";
import type { TileRange } from "./tile-range";
import type { RGBAColor } from "./journey-layer-interface";

export interface CanvasRaster {
  image: ImageData;
  pixels: Uint32Array;
  coverage: Uint8Array;
}

export function createCanvasTrackColors(
  bgColor: RGBAColor,
  fgColor: RGBAColor,
): Uint32Array {
  const colors = new Uint8ClampedArray(256 * 4);
  for (let index = 0; index < 256; index++) {
    const coverage = index / 255;
    const bgAlpha = bgColor[3] * (1 - coverage);
    const fgAlpha = fgColor[3] * coverage;
    const alpha = bgAlpha + fgAlpha;
    for (let channel = 0; channel < 3; channel++) {
      colors[index * 4 + channel] = alpha
        ? (Math.round(bgColor[channel] * 255) * bgAlpha +
            Math.round(fgColor[channel] * 255) * fgAlpha) /
          alpha
        : 0;
    }
    colors[index * 4 + 3] = alpha * 255;
  }
  return new Uint32Array(colors.buffer);
}

/** Rasterize source pixels with the existing Canvas feather, with optional
 * Mercator wrapping. Polar chart edges stay finite. */
export function drawCanvasTileRange(
  canvas: HTMLCanvasElement,
  ctx: CanvasRenderingContext2D,
  trackColors: Uint32Array,
  raster: CanvasRaster | undefined,
  [left, top, w, h, z]: TileRange,
  bufferSizePower: number,
  tileBuffer: Pick<TileBuffer, "get_tile_pixels">,
  wrapX = false,
): CanvasRaster {
  const tileSize = 2 ** bufferSizePower,
    n = 2 ** z;
  const width = tileSize * w,
    height = tileSize * h;
  const right = left + w,
    bottom = top + h;
  if (canvas.width !== width) canvas.width = width;
  if (canvas.height !== height) canvas.height = height;
  if (raster?.image.width !== width || raster.image.height !== height) {
    const image = ctx.createImageData(width, height);
    raster = {
      image,
      pixels: new Uint32Array(image.data.buffer),
      coverage: new Uint8Array(width * height),
    };
  }
  // Panning usually keeps the same dimensions. Reuse only the current
  // raster, avoiding repeated allocations without retaining old viewports.
  const { image, pixels, coverage } = raster!;
  pixels.fill(trackColors[0]);
  coverage.fill(0);
  const addCoverage = (offset: number, weight: number): void => {
    if (coverage[offset] !== 255) {
      // Keep partially revealed cells distinct from solid track cores.
      const level = Math.min(254, coverage[offset] + weight);
      coverage[offset] = level;
      pixels[offset] = trackColors[level];
    }
  };

  for (let x = left; x < right; x++) {
    for (let y = top; y < bottom; y++) {
      if (y < 0 || y >= n) continue;

      const dx = (x - left) * tileSize;
      const dy = (y - top) * tileSize;

      // Get pixels coordinates from journeyTileProvider
      // TODO: we need smooth transition when x range jump in the center of the Pacific Ocean.
      const pixelCoords = tileBuffer.get_tile_pixels(x, y, z, bufferSizePower);

      if (pixelCoords && pixelCoords.length > 0) {
        // Process pairs of coordinates (x,y)
        for (let i = 0; i < pixelCoords.length; i += 2) {
          const pointX = dx + pixelCoords[i];
          const pointY = dy + pixelCoords[i + 1];
          const center = pointY * width + pointX;
          if (coverage[center] === 255) continue;
          coverage[center] = 255;
          pixels[center] = trackColors[255];
          // A small, fixed 3x3 kernel fills diagonal notches more evenly
          // than independent halos. Preserve solid cores and count each
          // source pixel once, so duplicates and draw order have no effect.
          // Canvas-space neighbors also keep internal tile seams invisible.
          // Most points are interior: use fixed offsets to avoid checking
          // canvas bounds and world wrapping for each of eight neighbors.
          if (
            pointX > 0 &&
            pointX < width - 1 &&
            pointY > 0 &&
            pointY < height - 1
          ) {
            addCoverage(center - width - 1, 10);
            addCoverage(center - width, 46);
            addCoverage(center - width + 1, 10);
            addCoverage(center - 1, 46);
            addCoverage(center + 1, 46);
            addCoverage(center + width - 1, 10);
            addCoverage(center + width, 46);
            addCoverage(center + width + 1, 10);
            continue;
          }
          for (let oy = -1; oy <= 1; oy++) {
            const py = pointY + oy;
            if (py < 0 || py >= height) continue;
            for (let ox = -1; ox <= 1; ox++) {
              if (ox === 0 && oy === 0) continue;
              let px = pointX + ox;
              if (wrapX) px = (px + width) % width;
              else if (px < 0 || px >= width) continue;
              // About 18% at edge neighbors and 4% at corners.
              addCoverage(py * width + px, ox !== 0 && oy !== 0 ? 10 : 46);
            }
          }
        }
      }
    }
  }

  // One upload at the existing resolution avoids per-point Canvas calls
  // and does not increase the texture size sent to MapLibre.
  ctx.putImageData(image, 0, 0);

  return raster!;
}
