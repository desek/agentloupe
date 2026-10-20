// @agents-index The tokens per day section: one line chart for input and one for output, each on its own scale, with the sums.

import { card, el } from './dom.el.js';
import { grouped, short } from './format.number.js';
import { line } from './chart.line.js';

export function render(doc, view) {
  const c = card(doc, 'Tokens per day', 'card wide');
  for (const [name, values, total, color] of [['Input', view.input, view.totals.input, 'input'], ['Output', view.output, view.totals.output, 'output']]) {
    const head = el(doc, 'div', 'axis');
    const g = el(doc, 'span', 'legend');
    g.append(el(doc, 'span', `swatch sw-${color}`), el(doc, 'span', '', `${name} ${grouped(total)}`));
    head.append(g, el(doc, 'span', '', `peak ${short(Math.max(0, ...values))}/day`));
    c.append(head, line(doc, values, color, `${name} tokens per day`));
  }
  return c;
}
