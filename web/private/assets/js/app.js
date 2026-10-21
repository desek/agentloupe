// @agents-index Entry module of the private page: loads the report for the range and group, and draws the table, chart, and sessions.

import { createClient } from './api.report.js';
import { fromOf } from './range.select.js';
import { tagKeys, fill } from './group.select.js';
import * as table from './table.groups.js';
import * as chart from './chart.tokens.js';
import * as list from './list.sessions.js';
import { localDay } from '/assets/js/format.date.js';
import { el } from '/assets/js/dom.el.js';

const doc = document;
const client = createClient((u, o) => fetch(u, o));
const today = localDay(new Date());
const state = { range: '3M', group: 'token', picked: null, keys: [] };
const $ = (id) => doc.getElementById(id);

/** A 401 means the cookie ended: go to the login form. */
function fail(err) {
  if (err.status === 401) { location.assign('/private/login'); return; }
  $('status').textContent = `The private API failed: ${err.message}`;
}

function pick(row, byToken) {
  state.picked = row.group;
  for (const tr of doc.querySelectorAll('#groups tr.pick')) tr.setAttribute('aria-selected', String(tr.dataset.group === row.group));
  $('chart-label').textContent = `Tokens per day, ${row.group}`;
  $('chart').replaceChildren(chart.render(doc, row.days));
  if (!byToken) return;
  $('sessions-label').textContent = `Sessions, ${row.group}`;
  const from = fromOf(state.range, today);
  client.sessions(row.group, from).then((p) => {
    $('sessions').replaceChildren(list.render(doc, p, (c) => client.sessions(row.group, from, c)));
  }, fail);
}

async function load() {
  const byToken = state.group === 'token';
  const tagKey = byToken ? null : state.group.slice(4);
  let resp;
  try {
    resp = await client.report(fromOf(state.range, today), tagKey);
  } catch (e) { fail(e); return; }
  if (byToken) state.keys = tagKeys(resp.rows);
  fill(doc, $('group'), state.keys, state.group);
  $('status').textContent = `${resp.from} to ${resp.to}`;
  $('groups').replaceChildren(table.render(doc, resp.rows, state.picked, (r) => pick(r, byToken), byToken));
  const row = resp.rows.find((r) => r.group === state.picked) || resp.rows[0];
  if (row) pick(row, byToken);
  else { $('chart').replaceChildren(el(doc, 'p', 'note', 'No call in this range.')); }
}

for (const b of doc.querySelectorAll('#ranges button')) {
  b.addEventListener('click', () => {
    state.range = b.dataset.range;
    for (const o of doc.querySelectorAll('#ranges button')) o.setAttribute('aria-pressed', String(o === b));
    load();
  });
}
$('group').addEventListener('change', (e) => {
  state.group = e.target.value;
  state.picked = null;
  if (state.group !== 'token') $('sessions').replaceChildren(el(doc, 'p', 'note', 'The session list is per proxy token.'));
  load();
});
load();
