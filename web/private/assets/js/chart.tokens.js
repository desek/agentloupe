// @agents-index The bar chart of the total tokens per day of one group, as inline SVG.

import { el, svg } from '/assets/js/dom.el.js';
import { short } from '/assets/js/format.number.js';

/** The bars of `days` (entries with date and total_tokens); the height scales to the largest day. */
export function render(doc, days) {
  if (!days || !days.length) return el(doc, 'p', 'note', 'No day with a call.');
  const max = Math.max(...days.map((d) => d.total_tokens), 1);
  const w = 1000;
  const h = 150;
  const bw = w / days.length;
  const s = svg(doc, 'svg', { class: 'chart bars', viewBox: `0 0 ${w} ${h}`, preserveAspectRatio: 'none', role: 'img', 'aria-label': 'Tokens per day' });
  days.forEach((d, i) => {
    const bh = (d.total_tokens / max) * h;
    const r = svg(doc, 'rect', { class: 'bar', x: i * bw, y: h - bh, width: Math.max(bw - 1, 1), height: bh });
    r.append(svg(doc, 'title'));
    r.firstChild.textContent = `${d.date}: ${short(d.total_tokens)} tokens`;
    s.append(r);
  });
  const wrap = el(doc, 'div');
  const axis = el(doc, 'div', 'axis');
  axis.append(el(doc, 'span', '', days[0].date), el(doc, 'span', '', days[days.length - 1].date));
  wrap.append(s, axis);
  return wrap;
}
