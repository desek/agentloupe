// @agents-index Draws the heatmap of the last 365 days as inline SVG: one cell per day, columns of weeks from Monday to Sunday.

import { svg } from './dom.el.js';
import { label } from './format.date.js';
import { grouped } from './format.number.js';

const CELL = 11;
const GAP = 3;

/** An SVG grid of `cells` ([{day, value, level, col, row}]); each cell has a title with the date and the token count. */
export function heatmap(doc, cells) {
  const cols = cells.length ? cells[cells.length - 1].col + 1 : 1;
  const W = cols * (CELL + GAP);
  const H = 7 * (CELL + GAP);
  const root = svg(doc, 'svg', { viewBox: `0 0 ${W} ${H}`, role: 'img', 'aria-label': 'Tokens per day, last 12 months', class: 'heat' });
  for (const c of cells) {
    const r = svg(doc, 'rect', { x: c.col * (CELL + GAP), y: c.row * (CELL + GAP), width: CELL, height: CELL, rx: 2, class: `h${c.level}` });
    const t = svg(doc, 'title');
    t.textContent = `${label(c.day)}: ${grouped(c.value)} tokens`;
    r.append(t);
    root.append(r);
  }
  return root;
}
