// @agents-index The daily log: one row per record of the range, newest first, with the summary put in as text only.

import { card, el } from './dom.el.js';
import { label } from './format.date.js';
import { grouped } from './format.number.js';

/** The log card of `view.log`; a null summary shows "No summary". */
export function render(doc, view) {
  const c = card(doc, 'Daily log', 'card full');
  const list = el(doc, 'ul', 'log');
  for (const r of view.log) {
    const row = el(doc, 'li', 'log-row');
    row.append(
      el(doc, 'span', 'log-date', label(r.day)),
      el(doc, 'span', 'log-summary', r.summary ?? 'No summary'),
      el(doc, 'span', 'log-num', `${grouped(r.prs)} PRs`),
      el(doc, 'span', 'log-num', `${grouped(r.tokens)} tokens`),
    );
    list.append(row);
  }
  c.append(list);
  return c;
}
