// @agents-index The lines section: lines added, lines removed, and the net value over the selected range.

import { card, el } from './dom.el.js';
import { grouped } from './format.number.js';
import { state } from './section.github.js';

export function render(doc, view) {
  const c = card(doc, 'Lines changed in the range');
  if (state(doc, c, view.github)) return c;
  const { linesAdded: a, linesRemoved: r } = view.totals;
  for (const [sign, v, sw, name] of [['+', a, 'sw-positive', 'added'], ['-', r, 'sw-output', 'removed']]) {
    const g = el(doc, 'div', 'legend');
    g.append(el(doc, 'span', `swatch ${sw}`), el(doc, 'span', 'figure-small', `${sign}${grouped(v)}`), el(doc, 'span', 'text', name));
    c.append(g);
  }
  const net = a - r;
  c.append(el(doc, 'p', 'text', `Net ${net >= 0 ? '+' : ''}${grouped(net)} lines`));
  return c;
}
