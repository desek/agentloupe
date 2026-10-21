// @agents-index The session list of one proxy token, with a button that loads the next page.

import { el } from '/assets/js/dom.el.js';
import { grouped } from '/assets/js/format.number.js';

/** One list item per session row. */
export function items(doc, rows) {
  return rows.map((r) => {
    const li = el(doc, 'li');
    li.append(
      el(doc, 'span', 'num', r.started_at.slice(0, 16).replace('T', ' ')),
      el(doc, 'span', '', r.agent_label),
      el(doc, 'span', '', r.summary || ''),
      el(doc, 'span', 'num', `${grouped(r.total_tokens)} tokens`),
      el(doc, 'span', 'num', `${r.api_equivalent_cost_usd} USD`),
    );
    return li;
  });
}

/** The list of the first page; `more(cursor)` returns the next page, and the button adds it. */
export function render(doc, page, more) {
  const wrap = el(doc, 'div');
  const ul = el(doc, 'ul', 'sessions');
  ul.append(...items(doc, page.rows));
  wrap.append(page.rows.length ? ul : el(doc, 'p', 'note', 'No session in this range.'));
  let next = page.next;
  if (next) {
    const b = el(doc, 'button', 'button', 'More');
    b.type = 'button';
    b.addEventListener('click', async () => {
      const p = await more(next);
      ul.append(...items(doc, p.rows));
      next = p.next;
      if (!next) b.remove();
    });
    wrap.append(b);
  }
  return wrap;
}
