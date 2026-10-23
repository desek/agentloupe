// @agents-index The rolling chart of the live section: tokens per 10-second bucket over the last 10 minutes, as inline SVG.

import { svg } from './dom.el.js';

export const BUCKET = 10000;
export const COUNT = 60;

const start = (now) => Math.floor(now / BUCKET) * BUCKET;

/** Adds `v` tokens at `now` and drops each bucket older than 10 minutes; returns the new list. */
export function add(buckets, now, v) {
  const t = start(now);
  const out = buckets.filter((b) => b.t > t - COUNT * BUCKET);
  const last = out[out.length - 1];
  if (last && last.t === t) last.v += v;
  else out.push({ t, v });
  return out;
}

/** The 60 values of the window that ends at `now`, oldest first; a bucket with no event is 0. */
export function series(buckets, now) {
  const t = start(now);
  const at = new Map(buckets.map((b) => [b.t, b.v]));
  return Array.from({ length: COUNT }, (_, i) => at.get(t - (COUNT - 1 - i) * BUCKET) || 0);
}

/** One bar per bucket in a 600 by 80 view box; the colours come from the design tokens. */
export function render(doc, values) {
  const max = Math.max(1, ...values);
  const s = svg(doc, 'svg', { viewBox: '0 0 600 80', preserveAspectRatio: 'none', class: 'live-chart', role: 'img', 'aria-label': 'Tokens per 10 seconds, last 10 minutes' });
  s.append(svg(doc, 'line', { x1: 0, y1: 79.5, x2: 600, y2: 79.5, stroke: 'var(--color-rule)' }));
  values.forEach((v, i) => {
    if (!v) return;
    const h = Math.max(1, (v / max) * 78);
    s.append(svg(doc, 'rect', { x: i * 10 + 1, y: 79 - h, width: 8, height: h, fill: 'var(--color-input)', rx: 1 }));
  });
  return s;
}
