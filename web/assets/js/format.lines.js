// @agents-index Pure format of the lines footer line of the pull request card: labelled added and removed figures, no net value.

import { grouped } from './format.number.js';

/** An integer with thin-space groups; a negative value takes the minus sign U+2212. */
export const signed = (v) => (v < 0 ? `−${grouped(-v)}` : grouped(v));

/** The two parts of the footer line, in order, each with its text, label, and color class. */
export function linesParts(added, removed) {
  return [
    { value: signed(added), label: 'lines added', cls: 'c-added' },
    { value: signed(removed), label: 'lines removed', cls: 'c-removed' },
  ];
}
