// @agents-index The live section under the headline: the connection dot, five figures of the current day, the paused state, and the sparkline.

import { el } from './dom.el.js';
import { grouped, short, usd } from './format.number.js';
import { apply, empty } from './live.state.js';
import { add, render, series } from './live.chart.js';
import { start, url } from './live.socket.js';

const FIGS = [
  ['inFlight', 'In flight', grouped],
  ['calls', 'Calls', grouped],
  ['tokens', 'Tokens', short],
  ['rate', 'Tokens per minute', short],
  ['cost', 'API-equivalent cost*', usd],
];

/** The tokens of the last minute: the sum of the last six 10-second buckets of `values`. */
export const perMinute = (values) => values.slice(-6).reduce((a, v) => a + v, 0);

/** Builds the section once and starts the socket; the node keeps its state across redraws. */
export function mount(doc, win) {
  const s = el(doc, 'section', 'card full live');
  s.setAttribute('aria-label', 'Today, live');
  const head = el(doc, 'div', 'live-head');
  const note = el(doc, 'span', 'note live-note', 'Connecting');
  note.setAttribute('role', 'status');
  head.append(el(doc, 'span', 'live-dot'), el(doc, 'h2', 'label', 'Today'), note);
  const grid = el(doc, 'div', 'live-figs');
  const out = {};
  for (const [k, label, fmt] of FIGS) {
    const f = el(doc, 'div', 'live-fig');
    out[k] = el(doc, 'div', 'live-value', fmt(0));
    f.append(out[k], el(doc, 'div', 'note', label));
    grid.append(f);
  }
  const box = el(doc, 'div', 'live-box');
  s.append(head, grid, box);
  let st = empty();
  let buckets = [];
  const draw = () => {
    const values = series(buckets, Date.now());
    const figs = { ...st, rate: perMinute(values) };
    for (const [k, , fmt] of FIGS) out[k].textContent = fmt(figs[k]);
    box.replaceChildren(render(doc, values));
  };
  draw();
  win.setInterval(draw, 10000);
  start({
    doc,
    url: url(win.location),
    open: (u) => new win.WebSocket(u),
    timer: (f, ms) => win.setTimeout(f, ms),
    clear: (t) => win.clearTimeout(t),
    frame: (f) => {
      const r = apply(st, f);
      st = r.state;
      if (r.added) buckets = add(buckets, Date.now(), r.added);
      s.classList.remove('live-paused');
      note.textContent = '';
      draw();
    },
    paused: () => { s.classList.add('live-paused'); note.textContent = 'Paused, reconnecting'; },
    hide: () => { s.hidden = true; },
  });
  return s;
}
