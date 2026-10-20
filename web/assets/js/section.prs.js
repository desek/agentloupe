// @agents-index The pull request section: opened and merged sums of the range and the bar chart per ISO week.

import { card, el } from './dom.el.js';
import { grouped } from './format.number.js';
import { label } from './format.date.js';
import { bars } from './chart.bars.js';
import { state } from './section.github.js';

export function render(doc, view) {
  const c = card(doc, 'Pull requests');
  if (state(doc, c, view.github)) return c;
  const pair = el(doc, 'div', 'pair');
  for (const [name, v, sw] of [['opened', view.totals.prsOpened, 'sw-input'], ['merged', view.totals.prsMerged, 'sw-positive']]) {
    const g = el(doc, 'div', 'legend');
    g.append(el(doc, 'span', `swatch ${sw}`), el(doc, 'span', 'figure-small', grouped(v)), el(doc, 'span', 'text', name));
    pair.append(g);
  }
  const axis = el(doc, 'div', 'axis');
  axis.append(el(doc, 'span', '', label(view.from)), el(doc, 'span', '', 'Weekly'), el(doc, 'span', '', label(view.to)));
  c.append(pair, bars(doc, view.weeks), axis);
  return c;
}
