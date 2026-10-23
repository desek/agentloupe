// @agents-index Reads one pixel of a PNG screenshot with node:zlib only: 8-bit RGB or RGBA, no interlace, every row filter.
import { inflateSync } from 'node:zlib';

/** The [r, g, b] of the pixel at (x, y) of a base64 PNG. */
export function pixel(base64, x, y) {
  const buf = Buffer.from(base64, 'base64');
  let pos = 8;
  let width = 0; let type = 0; const idat = [];
  while (pos < buf.length) {
    const len = buf.readUInt32BE(pos);
    const kind = buf.toString('ascii', pos + 4, pos + 8);
    const data = buf.subarray(pos + 8, pos + 8 + len);
    if (kind === 'IHDR') {
      width = data.readUInt32BE(0);
      if (data[8] !== 8 || data[12] !== 0) throw new Error('only 8-bit non-interlaced PNG');
      type = data[9];
    } else if (kind === 'IDAT') idat.push(data);
    pos += 12 + len;
  }
  const bpp = type === 6 ? 4 : type === 2 ? 3 : 0;
  if (!bpp) throw new Error(`PNG color type ${type} is not RGB or RGBA`);
  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * bpp;
  let prev = Buffer.alloc(stride);
  let row = null;
  // Each row depends on the row above it, so unfilter every row down to y.
  for (let r = 0; r <= y; r++) {
    const f = raw[r * (stride + 1)];
    const src = raw.subarray(r * (stride + 1) + 1, (r + 1) * (stride + 1));
    row = Buffer.alloc(stride);
    for (let i = 0; i < stride; i++) {
      const a = i >= bpp ? row[i - bpp] : 0;
      const b = prev[i];
      const c = i >= bpp ? prev[i - bpp] : 0;
      let v = src[i];
      if (f === 1) v += a;
      else if (f === 2) v += b;
      else if (f === 3) v += (a + b) >> 1;
      else if (f === 4) {
        const p = a + b - c; const pa = Math.abs(p - a); const pb = Math.abs(p - b); const pc = Math.abs(p - c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      row[i] = v & 255;
    }
    prev = row;
  }
  return [row[x * bpp], row[x * bpp + 1], row[x * bpp + 2]];
}
