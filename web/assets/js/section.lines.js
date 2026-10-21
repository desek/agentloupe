// @agents-index The lines section: two labelled figures, lines added and lines removed, over the selected range; no net value.

import { card, el } from './dom.el.js';
import { grouped } from './format.number.js';
import { state } from './section.github.js';

const signed = (v) => (v < 0 ? `−${grouped(-v)}` : grouped(v));

export function render(doc, view) {
  const c = card(doc, 'Lines changed in the range');
  if (state(doc, c, view.github)) return c;
  const { linesAdded: a, linesRemoved: r } = view.totals;
  const pair = el(doc, 'div', 'pair');
  for (const [v, cls, sw, name] of [[a, 'c-added', 'sw-positive', 'lines added'], [r, 'c-removed', 'sw-output', 'lines removed']]) {
    const g = el(doc, 'div', 'stat');
    const label = el(doc, 'span', 'legend');
    label.append(el(doc, 'span', `swatch ${sw}`), el(doc, 'span', 'text', name));
    g.append(el(doc, 'span', `figure-small ${cls}`, signed(v)), label);
    pair.append(g);
  }
  c.append(pair);
  return c;
}
