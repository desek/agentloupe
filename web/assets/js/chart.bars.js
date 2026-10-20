// @agents-index Draws opened and merged pull requests per ISO week as an inline SVG bar chart with two bars per week.

import { svg } from './dom.el.js';
import { grouped } from './format.number.js';

const H = 150;

/** An SVG bar chart of `weeks` ([{week, opened, merged}]); each group has an opened and a merged bar. */
export function bars(doc, weeks) {
  const W = Math.max(1, weeks.length) * 30;
  const max = Math.max(1, ...weeks.map((w) => Math.max(w.opened, w.merged)));
  const root = svg(doc, 'svg', { viewBox: `0 0 ${W} ${H}`, preserveAspectRatio: 'none', role: 'img', 'aria-label': 'Pull requests per week, opened and merged', class: 'chart bars' });
  weeks.forEach((w, i) => {
    const pairs = [['opened', w.opened, 'input'], ['merged', w.merged, 'positive']];
    pairs.forEach(([name, v, color], j) => {
      const h = (v / max) * H;
      const r = svg(doc, 'rect', { x: i * 30 + 3 + j * 12, y: H - h, width: 11, height: h, rx: 2, class: `c-${color}` });
      const t = svg(doc, 'title');
      t.textContent = `${w.week}: ${grouped(v)} ${name}`;
      r.append(t);
      root.append(r);
    });
  });
  return root;
}
