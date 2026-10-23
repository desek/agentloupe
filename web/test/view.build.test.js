// @agents-index Unit tests of the view: missing days as zero, range lengths, ISO weeks, velocity, lines per range, GitHub states.
import test from 'node:test';
import assert from 'node:assert/strict';
import { build, days, weeks, points, githubState } from '../assets/js/view.build.js';
import { addDays } from '../assets/js/format.date.js';

const TO = '2026-10-20';
const rec = (day, f = {}) => ({ day, input_total: 900, output: 10, cost_usd: 0.5, unpriced_calls: 0, prs_opened: null, prs_merged: null, lines_added: null, lines_removed: null, summary: null, ...f });

test('missing_days_are_zero', () => {
  const v = build({ to: TO, days: [rec('2026-10-01'), rec('2026-10-10'), rec(TO)] }, '3M');
  assert.equal(v.tokens.length, 91);
  assert.equal(v.tokens.filter((t) => t === 0).length, 88);
});

test('range_lengths', () => {
  for (const [k, n] of [['1M', 30], ['3M', 91], ['6M', 182], ['1Y', 365]]) {
    const v = build({ to: TO, days: [] }, k);
    assert.equal(v.points.length, n);
    assert.equal(v.points[n - 1], TO);
    assert.equal(v.points[0], addDays(TO, 1 - n));
  }
  assert.equal(days(TO, 91).length, 91);
});

test('week_buckets_iso', () => {
  const w = weeks(points([rec('2026-12-31', { prs_opened: 2, prs_merged: 1 }), rec('2027-01-04', { prs_opened: 1, prs_merged: 0 })], '2027-01-10', 14));
  assert.deepEqual(w.map((x) => x.week), ['2026-W53', '2027-W01']);
  assert.deepEqual(w.map((x) => [x.opened, x.merged]), [[2, 1], [1, 0]]);
  assert.equal(w[0].start, '2026-12-28');
});

test('velocity_counts_missing_days', () => {
  const v = build({ to: TO, days: [rec('2026-09-01'), rec('2026-10-01'), rec(TO)] }, '3M');
  assert.equal(v.velocity, 30);
});

test('lines_follow_range', () => {
  const all = Array.from({ length: 400 }, (_, i) => rec(addDays(TO, -i), { prs_opened: 0, prs_merged: 0, lines_added: 10, lines_removed: 4 }));
  const r = { to: TO, days: all.reverse() };
  const m3 = build(r, '3M').totals;
  const y1 = build(r, '1Y').totals;
  assert.deepEqual([m3.linesAdded, m3.linesRemoved], [910, 364]);
  assert.deepEqual([y1.linesAdded, y1.linesRemoved], [3650, 1460]);
});

test('github_null_states', () => {
  assert.equal(githubState([rec('2026-10-01'), rec('2026-10-02')]), 'not_connected');
  assert.equal(githubState([rec('2026-10-01'), rec('2026-10-02', { prs_opened: 1, prs_merged: 1, lines_added: 3, lines_removed: 1 })]), 'partial');
  assert.equal(githubState([rec('2026-10-02', { prs_opened: 1, prs_merged: 1, lines_added: 3, lines_removed: 1 })]), 'connected');
});
