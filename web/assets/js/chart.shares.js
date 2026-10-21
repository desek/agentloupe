// @agents-index Draws the token shares of a range as one stacked horizontal bar in inline SVG, with a text alternative.

import { svg } from './dom.el.js';
import { percent } from './format.number.js';

/** The bar of `kinds` from stats.shares.js; a kind with a zero count has no segment. */
export function bar(doc, kinds) {
  const alt = kinds.map((k) => `${k.label} ${percent(k.share)}`).join(', ');
  const root = svg(doc, 'svg', { viewBox: '0 0 100 10', preserveAspectRatio: 'none', role: 'img', 'aria-label': `Token shares: ${alt}`, class: 'share-bar' });
  for (const k of kinds) {
    if (k.count === 0) continue;
    root.append(svg(doc, 'rect', { x: k.start * 100, y: 0, width: k.share * 100, height: 10, class: `seg-${k.key}` }));
  }
  return root;
}
