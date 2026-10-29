/** A real source bitmap and its OR-reduced pyramid for WASM/GPU regressions. */
export function encodeTileRangeResponse([x, y, w, h, z], cells, exponent = 10) {
  const side = 2 ** exponent;
  const tiles = new Map();
  for (const [cx, cy] of cells) {
    const tx = Math.floor(cx / side) - x,
      ty = Math.floor(cy / side) - y;
    if (tx < 0 || tx >= w || ty < 0 || ty >= h) continue;
    const index = ty * w + tx;
    if (!tiles.has(index)) tiles.set(index, []);
    tiles.get(index).push([cx % side, cy % side]);
  }
  const header = new Uint8Array(20);
  const view = new DataView(header.buffer);
  header[0] = exponent;
  header[1] = z;
  view.setInt32(4, x, true);
  view.setInt32(8, y, true);
  view.setUint16(12, w, true);
  view.setUint16(14, h, true);
  view.setUint16(16, w * h, true);
  view.setUint16(18, tiles.size, true);
  const presence = new Uint8Array(Math.ceil((w * h) / 8));
  const chunks = [header, presence];
  for (let index = 0; index < w * h; index++) {
    const points = tiles.get(index);
    if (!points) continue;
    presence[index >> 3] |= 1 << (index % 8);
    chunks.push(new Uint8Array([exponent + 1, 0]));
    for (let level = 0; level <= exponent; level++) {
      const levelSide = side >> level;
      const bits = new Uint8Array(Math.ceil((levelSide * levelSide) / 8));
      for (const [px, py] of points) {
        const bit = (py >> level) * levelSide + (px >> level);
        bits[bit >> 3] |= 1 << (bit % 8);
      }
      const length = new Uint8Array(4);
      new DataView(length.buffer).setUint32(0, levelSide * levelSide, true);
      chunks.push(length, bits);
    }
  }
  const bytes = new Uint8Array(
    chunks.reduce((sum, chunk) => sum + chunk.length, 0),
  );
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.length;
  }
  return bytes;
}
