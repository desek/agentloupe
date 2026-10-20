// @agents-index The footer: the covered sources and the statement that claude.ai chats are not covered.

import { el } from './dom.el.js';

export function render(doc) {
  const f = doc.getElementById('footer');
  f.replaceChildren(el(doc, 'p', '', 'Covers Claude Code and Pi. claude.ai chats are not covered.'));
}
