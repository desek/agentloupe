// @agents-index The footer: the covered sources, and the asterisk sentence with the LiteLLM link and the short price commit.

import { el } from './dom.el.js';

/** The asterisk sentence from the template in index.html, where the server put the short price commit. */
function priceNote(doc) {
  const t = doc.getElementById('price-note');
  return t && t.content ? t.content.cloneNode(true) : el(doc, 'p', 'footnote', '* LiteLLM list prices, not money paid.');
}

export function render(doc) {
  const f = doc.getElementById('footer');
  f.replaceChildren(el(doc, 'p', '', 'Covers Claude Code and Pi. claude.ai chats are not covered.'), priceNote(doc));
}
