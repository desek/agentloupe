// @agents-index Entry module of the dashboard: loads the days, keeps the range in the URL fragment, and draws every section.

import { createClient } from './api.days.js';
import { localDay } from './format.date.js';
import { fromFragment, toFragment } from './range.select.js';
import { build, heatmap } from './view.build.js';
import * as header from './section.header.js';
import * as headline from './section.headline.js';
import * as cost from './section.cost.js';
import * as prs from './section.prs.js';
import * as lines from './section.lines.js';
import * as tokens from './section.tokens.js';
import * as heat from './section.heatmap.js';
import * as log from './section.log.js';
import * as footer from './section.footer.js';
import * as states from './section.state.js';
import { el } from './dom.el.js';

const doc = document;
const main = doc.getElementById('main');
const client = createClient((u, o) => fetch(u, o), localDay(new Date()));
let resp = null;
let heatNode = null;

/** Draws every section of the selected range; the heatmap is drawn once and kept. */
function draw() {
  const key = fromFragment(location.hash);
  header.render(doc, resp.updated_at, key);
  const view = build(resp, key);
  if (view.empty) {
    main.replaceChildren(headline.render(doc, view), states.empty(doc));
    return Promise.resolve();
  }
  const totals = cost.render(doc, view, client.subscription(key, resp.to));
  const pair = el(doc, 'div', 'row');
  pair.append(prs.render(doc, view), lines.render(doc, view), tokens.render(doc, view));
  heatNode = heatNode || heat.render(doc, heatmap(resp));
  main.replaceChildren(headline.render(doc, view), totals.node, pair, heatNode, log.render(doc, view));
  return totals.done;
}

/** Loads the data and draws; marks the time of the first full draw on the root element for the render measurement. */
async function start() {
  try {
    resp = await client.load();
  } catch (err) {
    main.replaceChildren(states.error(doc, err, start));
    return;
  }
  await draw();
  doc.documentElement.dataset.drawMs = performance.now().toFixed(1);
}

for (const b of doc.querySelectorAll('#ranges button')) {
  b.addEventListener('click', () => { location.hash = toFragment(b.dataset.range); });
}
addEventListener('hashchange', () => { if (resp) draw(); });
footer.render(doc);
start();
