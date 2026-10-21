// @agents-index The totals row: the tokens card with the share bar, the API-equivalent cost card with its footnote, and the subscription card.

import { card, el } from './dom.el.js';
import { grouped, percent, short, usd } from './format.number.js';
import { line } from './chart.line.js';
import { bar } from './chart.shares.js';
import { shares } from './stats.shares.js';
import { empty } from './section.state.js';

/** The tokens card: the total, the bar, and one labelled line per kind; the empty state when the total is zero. */
function tokens(doc, totals) {
  const tok = card(doc, 'Tokens in the range');
  // Fresh input is input_total less the cache kinds, so the four counts always sum to the total.
  const { input, output, cacheRead, cacheWrite } = totals;
  const s = shares({ input: input - cacheRead - cacheWrite, cacheRead, cacheWrite, output });
  if (!s) {
    tok.append(...empty(doc).childNodes);
    return tok;
  }
  tok.append(el(doc, 'div', 'figure', grouped(s.total)), bar(doc, s.kinds));
  const list = el(doc, 'ul', 'shares');
  for (const k of s.kinds) {
    const li = el(doc, 'li', 'legend');
    li.append(el(doc, 'span', `swatch seg-${k.key}`), el(doc, 'span', '', k.label), el(doc, 'span', 'note', `${short(k.count)} · ${percent(k.share)}`));
    list.append(li);
  }
  tok.append(list);
  return tok;
}

/** The footnote from the template in index.html, where the server put the short price commit. */
function footnote(doc) {
  const t = doc.getElementById && doc.getElementById('cost-footnote');
  return t && t.content ? t.content.cloneNode(true) : el(doc, 'p', 'note', '* Prices from LiteLLM model_prices_and_context_window.json.');
}

/** The totals row; `subscription` is a promise of the subscription_cost_usd of the range. */
export function render(doc, view, subscription) {
  const row = el(doc, 'div', 'row');
  row.setAttribute('aria-label', 'Totals');

  const cost = card(doc, 'Cost (API-equivalent)*');
  const fig = el(doc, 'div', 'figure', usd(view.totals.cost));
  fig.id = 'totals-figure';
  cost.append(fig, el(doc, 'p', 'text', 'List price of the tokens, not money paid.'));
  if (view.totals.unpriced > 0) cost.append(el(doc, 'p', 'note', `${grouped(view.totals.unpriced)} calls not priced`));
  cost.append(line(doc, view.cost, 'output', 'Cost per day in US dollars'), footnote(doc));

  const sub = card(doc, 'Subscription cost');
  const value = el(doc, 'div', 'figure', '...');
  sub.append(value, el(doc, 'p', 'text', 'Money paid for subscriptions, pro-rated per day.'));
  const done = Promise.resolve(subscription).then(
    (v) => { value.textContent = v === 0 ? 'No subscription configured' : usd(v); if (v === 0) value.className = 'text'; },
    () => { value.textContent = 'Not available'; value.className = 'text'; },
  );

  row.append(tokens(doc, view.totals), cost, sub);
  return { node: row, done };
}
