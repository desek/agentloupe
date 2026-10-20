// @agents-index The empty state and the error state blocks, each one message card in place of the charts.

import { el } from './dom.el.js';

/** The empty state: no record in the response. */
export function empty(doc) {
  const c = el(doc, 'section', 'card full state');
  c.append(
    el(doc, 'p', 'figure-small', 'No days published yet'),
  );
  const hint = el(doc, 'p', 'text', 'Days appear after the nightly publish, or after ');
  hint.append(el(doc, 'code', '', 'loupe aggregate --date today'), '.');
  c.append(hint);
  return c;
}

/** The error state with the status or error name, the 400 message when present, and a retry button. */
export function error(doc, err, retry) {
  const c = el(doc, 'section', 'card full state');
  const what = typeof err.status === 'number' ? `HTTP ${err.status}` : String(err.status || err.name || 'Error');
  c.append(el(doc, 'p', 'figure-small', `Data not available (${what})`));
  if (err.detail) c.append(el(doc, 'p', 'text', err.detail));
  const b = el(doc, 'button', 'button', 'Try again');
  b.type = 'button';
  b.addEventListener('click', retry);
  c.append(b);
  return c;
}
