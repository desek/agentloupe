// @agents-index The pull request card: legend with opened and merged sums, the weekly bar chart, and the lines footer line.

import { card, el } from './dom.el.js';
import { grouped } from './format.number.js';
import { label } from './format.date.js';
import { bars } from './chart.bars.js';
import { state } from './section.github.js';
import { linesParts } from './format.lines.js';

/** The footer line: lines added and lines removed, each with its label, in the note style. */
function footer(doc, totals) {
  const p = el(doc, 'p', 'note pr-footer');
  for (const part of linesParts(totals.linesAdded, totals.linesRemoved)) {
    const s = el(doc, 'span', 'pr-lines');
    s.append(el(doc, 'span', part.cls, part.value), el(doc, 'span', '', ` ${part.label}`));
    p.append(s);
  }
  return p;
}

/** The wide card; with GitHub not connected it holds only the state note. */
export function render(doc, view) {
  const c = card(doc, 'Pull requests', 'card wide');
  if (state(doc, c, view.github)) return c;
  const legend = el(doc, 'div', 'pr-legend');
  for (const [name, v, sw] of [['opened', view.totals.prsOpened, 'sw-input'], ['merged', view.totals.prsMerged, 'sw-positive']]) {
    const g = el(doc, 'div', 'legend');
    g.append(el(doc, 'span', `swatch ${sw}`), el(doc, 'span', 'text', name), el(doc, 'span', 'figure-small', grouped(v)));
    legend.append(g);
  }
  const axis = el(doc, 'div', 'axis');
  axis.append(el(doc, 'span', '', label(view.from)), el(doc, 'span', '', 'Weekly'), el(doc, 'span', '', label(view.to)));
  c.append(legend, bars(doc, view.weeks), axis, footer(doc, view.totals));
  return c;
}
