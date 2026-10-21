// @agents-index Builds the view of a range from the API response: daily points, sums, velocity, ISO weeks, GitHub state, log, heatmap.

import { addDays, isoWeek } from './format.date.js';
import { RANGES } from './range.select.js';
import { levels } from './stats.quartiles.js';

const GITHUB = ['prs_opened', 'prs_merged', 'lines_added', 'lines_removed'];

const num = (v) => (typeof v === 'number' && Number.isFinite(v) ? v : 0);
const sum = (a) => a.reduce((s, v) => s + v, 0);
const tokens = (r) => (r ? num(r.input_total) + num(r.output) : 0);

/** The `n` days that end on `to`, oldest first. */
export function days(to, n) {
  return Array.from({ length: n }, (_, i) => addDays(to, i - n + 1));
}

/** One point per day of the window; a day with no record has rec null and counts as zero. */
export function points(records, to, n) {
  const map = new Map(records.map((r) => [r.day, r]));
  return days(to, n).map((day) => ({ day, rec: map.get(day) || null }));
}

/** not_connected when every record has null in all GitHub fields, partial when some do, else connected. */
export function githubState(records) {
  if (records.length === 0) return 'not_connected';
  const nulls = records.map((r) => GITHUB.every((f) => r[f] === null || r[f] === undefined));
  if (nulls.every(Boolean)) return 'not_connected';
  const some = records.some((r) => GITHUB.some((f) => r[f] === null || r[f] === undefined));
  return some ? 'partial' : 'connected';
}

/** Opened and merged pull requests per ISO week, Monday to Sunday, oldest first. */
export function weeks(pts) {
  const out = [];
  for (const p of pts) {
    const week = isoWeek(p.day);
    if (out.length === 0 || out[out.length - 1].week !== week) out.push({ week, start: p.day, opened: 0, merged: 0 });
    const w = out[out.length - 1];
    w.opened += num(p.rec?.prs_opened);
    w.merged += num(p.rec?.prs_merged);
  }
  return out;
}

/** The view of range `key` of the response `resp` ({to, days}). */
export function build(resp, key) {
  const n = RANGES[key];
  const pts = points(resp.days, resp.to, n);
  const series = (f) => pts.map((p) => num(p.rec?.[f]));
  const tok = pts.map((p) => tokens(p.rec));
  const s = (f) => sum(series(f));
  return {
    key,
    n,
    from: pts[0].day,
    to: resp.to,
    empty: resp.days.length === 0,
    points: pts.map((p) => p.day),
    tokens: tok,
    input: series('input_total'),
    output: series('output'),
    cost: series('cost_usd'),
    velocity: Math.round(sum(tok) / n),
    totals: {
      tokens: sum(tok),
      input: s('input_total'),
      output: s('output'),
      cacheRead: s('cache_read'),
      cacheWrite: s('cache_write'),
      cost: s('cost_usd'),
      unpriced: s('unpriced_calls'),
      prsOpened: s('prs_opened'),
      prsMerged: s('prs_merged'),
      linesAdded: s('lines_added'),
      linesRemoved: s('lines_removed'),
    },
    github: githubState(resp.days),
    weeks: weeks(pts),
    log: pts.filter((p) => p.rec).reverse().map((p) => ({
      day: p.day,
      summary: p.rec.summary ?? null,
      prs: typeof p.rec.prs_opened === 'number' ? p.rec.prs_opened : null,
      tokens: tokens(p.rec),
    })),
  };
}

/** The cells of the last 365 days that end on `to`, with value, level, week column, and weekday row. */
export function heatmap(resp) {
  const pts = points(resp.days, resp.to, 365);
  const values = pts.map((p) => tokens(p.rec));
  const lv = levels(values);
  const first = (new Date(`${pts[0].day}T00:00:00Z`).getUTCDay() + 6) % 7;
  return pts.map((p, i) => ({
    day: p.day,
    value: values[i],
    level: lv[i],
    col: Math.floor((i + first) / 7),
    row: (i + first) % 7,
  }));
}
