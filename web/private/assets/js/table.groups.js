// @agents-index The table of the report groups; a click on a row selects that group.

import { el } from '/assets/js/dom.el.js';
import { grouped } from '/assets/js/format.number.js';

const COLS = [
  ['group', 'Group', false], ['total_tokens', 'Tokens', true], ['input_tokens', 'Input', true],
  ['output_tokens', 'Output', true], ['calls', 'Calls', true], ['sessions', 'Sessions', true],
  ['api_equivalent_cost_usd', 'API-equivalent USD', true], ['unpriced_calls', 'Unpriced', true],
];

/** A table of `rows`; `selected` is the group name of the marked row, `onPick(row)` runs on a click. */
export function render(doc, rows, selected, onPick, byToken) {
  const t = el(doc, 'table', 'table');
  const head = el(doc, 'tr');
  const cols = byToken ? [...COLS, ['status', 'Status', false]] : COLS;
  for (const [, name, num] of cols) head.append(el(doc, 'th', num ? 'num' : '', name));
  t.append(head);
  for (const r of rows) {
    const tr = el(doc, 'tr', 'pick');
    tr.dataset.group = r.group;
    tr.setAttribute('aria-selected', String(r.group === selected));
    for (const [key, , num] of cols) {
      const v = r[key];
      const text = num && typeof v === 'number' ? grouped(v) : v ?? '';
      tr.append(el(doc, 'td', num ? 'num' : '', text));
    }
    tr.addEventListener('click', () => onPick(r));
    t.append(tr);
  }
  return rows.length ? t : el(doc, 'p', 'note', 'No calls in this range.');
}
