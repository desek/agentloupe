// @agents-index The headline: token velocity of the range in the short form, its unit, the range in words, and a sparkline.

import { el } from './dom.el.js';
import { short } from './format.number.js';
import { words } from './range.select.js';
import { line } from './chart.line.js';

/** The headline block; the sparkline is left out when the view has no record. */
export function render(doc, view) {
  const s = el(doc, 'section', 'headline');
  s.setAttribute('aria-label', 'Token velocity');
  const text = el(doc, 'div', 'headline-text');
  const fig = el(doc, 'div', 'headline-figure', short(view.velocity));
  fig.id = 'headline-figure';
  text.append(fig, el(doc, 'div', 'headline-unit', 'tokens per day'), el(doc, 'div', 'headline-range', words(view.key)));
  s.append(text);
  if (!view.empty) s.append(line(doc, view.tokens, 'text', 'Tokens per day in the range', 'spark'));
  return s;
}
