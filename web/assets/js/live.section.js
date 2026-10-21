// @agents-index The live section under the headline: the figures of the current day, the in-flight count, the paused state, and the chart.

import { el } from './dom.el.js';
import { grouped, short, usd } from './format.number.js';
import { apply, empty } from './live.state.js';
import { add, render, series } from './live.chart.js';
import { start, url } from './live.socket.js';

const FIGS = [
  ['calls', 'Calls today', grouped],
  ['tokens', 'Tokens today', short],
  ['segments', 'Open segments', grouped],
  ['cost', 'API-equivalent cost', usd],
];

/** Builds the section once and starts the socket; the node keeps its state across redraws. */
export function mount(doc, win) {
  const s = el(doc, 'section', 'card full live');
  s.setAttribute('aria-label', 'Live');
  const head = el(doc, 'div', 'live-head');
  const flight = el(doc, 'span', 'note');
  const note = el(doc, 'p', 'note live-note', 'Connecting');
  head.append(el(doc, 'h2', 'label', 'Live, today'), flight);
  const grid = el(doc, 'div', 'live-figs');
  const out = {};
  for (const [k, label, fmt] of FIGS) {
    const f = el(doc, 'div', 'live-fig');
    out[k] = el(doc, 'div', 'live-value', fmt(0));
    f.append(out[k], el(doc, 'div', 'note', label));
    grid.append(f);
  }
  const box = el(doc, 'div', 'live-box');
  s.append(head, grid, box, note);
  let st = empty();
  let buckets = [];
  const draw = () => {
    for (const [k, , fmt] of FIGS) out[k].textContent = fmt(st[k]);
    flight.textContent = `${grouped(st.inFlight)} in flight`;
    box.replaceChildren(render(doc, series(buckets, Date.now())));
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
      note.textContent = 'Live';
      draw();
    },
    paused: () => { s.classList.add('live-paused'); note.textContent = 'Live feed paused, reconnecting'; },
    hide: () => { s.hidden = true; },
  });
  return s;
}
