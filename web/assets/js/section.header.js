// @agents-index Fills the header: the update time and the pressed state of the range buttons; the title comes from the server.

import { updated } from './format.date.js';

/** Sets the update text and marks the button of `key` with aria-pressed true. */
export function render(doc, updatedAt, key) {
  doc.getElementById('updated').textContent = updated(updatedAt);
  for (const b of doc.querySelectorAll('#ranges button')) {
    b.setAttribute('aria-pressed', String(b.dataset.range === key));
  }
}
