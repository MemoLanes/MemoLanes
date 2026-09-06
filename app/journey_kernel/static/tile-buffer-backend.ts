import { TileBuffer } from "../pkg/journey_kernel.js";

/** One decoded response. Renderer-specific backends may extend this handle. */
export interface TileBufferSource {
  readonly id: number;
  /** Available for Canvas callbacks; null when another backend owns the data. */
  readonly mainThreadBuffer: TileBuffer | null;
  /** Release this response exactly once; repeated calls must be harmless. */
  release(): void;
}

/** Owns decoding facilities, while the provider owns individual sources. */
export interface TileBufferBackend {
  decode(sourceId: number, bytes: ArrayBuffer): Promise<TileBufferSource>;
  /** Pending decodes must reject or return a releasable source after disposal. */
  dispose(): void;
}

export class MainThreadTileBufferBackend implements TileBufferBackend {
  async decode(id: number, bytes: ArrayBuffer): Promise<TileBufferSource> {
    const buffer = TileBuffer.new_from_tile_range_response(
      new Uint8Array(bytes),
    );
    let released = false;
    return {
      id,
      mainThreadBuffer: buffer,
      release() {
        if (released) return;
        released = true;
        buffer.free();
      },
    };
  }

  dispose(): void {
    // Each source owns its allocation and is released by the provider.
  }
}
