// @agents-index Fills the header: the pressed state of the range buttons; the wordmark comes from the server.

/** Marks the button of `key` with aria-pressed true and the others false. */
export function render(doc, key) {
  for (const b of doc.querySelectorAll('#ranges button')) {
    b.setAttribute('aria-pressed', String(b.dataset.range === key));
  }
}
