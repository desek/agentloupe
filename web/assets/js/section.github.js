// @agents-index Shared state text of the GitHub sections: not connected replaces the figures, partial adds a note.

import { el } from './dom.el.js';

/** Appends the state note to `c`; returns true when the section must show no figures and no chart. */
export function state(doc, c, github) {
  if (github === 'not_connected') {
    c.append(el(doc, 'p', 'text', 'GitHub not connected'));
    return true;
  }
  if (github === 'partial') c.append(el(doc, 'p', 'note', 'Partial GitHub data'));
  return false;
}
