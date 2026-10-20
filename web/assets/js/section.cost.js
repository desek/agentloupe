// @agents-index The totals row: total tokens, the API-equivalent cost card with its daily chart, and the subscription cost card.

import { card, el } from './dom.el.js';
import { grouped, usd } from './format.number.js';
import { line } from './chart.line.js';

/** The totals row; `subscription` is a promise of the subscription_cost_usd of the range. */
export function render(doc, view, subscription) {
  const row = el(doc, 'div', 'row');
  row.setAttribute('aria-label', 'Totals');

  const tok = card(doc, 'Tokens in the range');
  tok.append(el(doc, 'div', 'figure', grouped(view.totals.tokens)));
  tok.append(el(doc, 'p', 'text', `${grouped(view.totals.input)} input, ${grouped(view.totals.output)} output`));

  const cost = card(doc, 'Cost (API-equivalent)');
  const fig = el(doc, 'div', 'figure', usd(view.totals.cost));
  fig.id = 'totals-figure';
  cost.append(fig, el(doc, 'p', 'text', 'List price of the tokens, not money paid.'));
  if (view.totals.unpriced > 0) cost.append(el(doc, 'p', 'note', `${grouped(view.totals.unpriced)} calls not priced`));
  cost.append(line(doc, view.cost, 'output', 'Cost per day in US dollars'));

  const sub = card(doc, 'Subscription cost');
  const value = el(doc, 'div', 'figure', '...');
  sub.append(value, el(doc, 'p', 'text', 'Money paid for subscriptions, pro-rated per day.'));
  const done = Promise.resolve(subscription).then(
    (v) => { value.textContent = v === 0 ? 'No subscription configured' : usd(v); if (v === 0) value.className = 'text'; },
    () => { value.textContent = 'Not available'; value.className = 'text'; },
  );

  row.append(tok, cost, sub);
  return { node: row, done };
}
