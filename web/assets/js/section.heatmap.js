// @agents-index The heatmap section of the last 12 months, with a legend of the five levels; the range does not change it.

import { card, el } from './dom.el.js';
import { heatmap } from './chart.heatmap.js';

export function render(doc, cells) {
  const c = card(doc, 'Token usage, last 12 months', 'card full');
  const legend = el(doc, 'div', 'heat-legend');
  legend.append(el(doc, 'span', '', 'Less'));
  for (let i = 0; i < 5; i++) legend.append(el(doc, 'span', `swatch h${i}`));
  legend.append(el(doc, 'span', '', 'More'));
  c.append(heatmap(doc, cells), legend);
  return c;
}
