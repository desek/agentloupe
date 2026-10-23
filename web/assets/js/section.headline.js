// @agents-index The headline: an eyebrow with the unit and the range, the token velocity in the short form, and a sparkline at the figure height.

import { el } from './dom.el.js';
import { short } from './format.number.js';
import { line } from './chart.line.js';

/** The eyebrow text: the unit and the range key, as `Tokens per day · 3M avg`. */
export const eyebrow = (key) => `Tokens per day · ${key} avg`;

/** The headline block; the sparkline is left out when the view has no record. */
export function render(doc, view) {
  const s = el(doc, 'section', 'headline');
  s.setAttribute('aria-label', 'Token velocity');
  const row = el(doc, 'div', 'headline-row');
  const fig = el(doc, 'div', 'headline-figure', short(view.velocity));
  fig.id = 'headline-figure';
  row.append(fig);
  if (!view.empty) row.append(line(doc, view.tokens, 'text', 'Tokens per day in the range', 'spark'));
  s.append(el(doc, 'div', 'eyebrow', eyebrow(view.key)), row);
  return s;
}
