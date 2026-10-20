// @agents-index Draws a daily series as an inline SVG area and line on its own vertical scale, with no axes.

import { svg } from './dom.el.js';

const W = 600;
const H = 120;

/** An SVG line chart of `values`; `color` is the class suffix (input, output, text), `label` the accessible name. */
export function line(doc, values, color, label, cls = 'chart') {
  const max = Math.max(1, ...values);
  const step = values.length > 1 ? W / (values.length - 1) : 0;
  const pts = values.map((v, i) => `${(i * step).toFixed(1)},${(H - (v / max) * H).toFixed(1)}`);
  const root = svg(doc, 'svg', { viewBox: `0 0 ${W} ${H}`, preserveAspectRatio: 'none', role: 'img', 'aria-label': label, class: cls });
  root.append(
    svg(doc, 'polygon', { points: `0,${H} ${pts.join(' ')} ${W},${H}`, class: `area c-${color}` }),
    svg(doc, 'polyline', { points: pts.join(' '), class: `line c-${color}` }),
  );
  return root;
}
