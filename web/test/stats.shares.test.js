// @agents-index Unit tests of the token share module: counts sum to the total, zero kinds, zero total, and the rounding range.
import test from 'node:test';
import assert from 'node:assert/strict';
import { shares } from '../assets/js/stats.shares.js';
import { percent } from '../assets/js/format.number.js';

const printed = (s) => s.kinds.reduce((a, k) => a + Number(percent(k.share).slice(0, -1)), 0);

test('counts_sum_to_total_and_segments_fill_bar', () => {
  const s = shares({ input: 1, cacheRead: 1, cacheWrite: 1, output: 0 });
  assert.equal(s.total, 3);
  assert.equal(s.kinds.reduce((a, k) => a + k.count, 0), 3);
  const last = s.kinds[s.kinds.length - 1];
  assert.ok(Math.abs(last.start + last.share - 1) < 1e-12);
  const p = printed(s);
  assert.ok(p >= 99.8 && p <= 100.2, String(p));
});

test('zero_kind_is_zero_percent', () => {
  const s = shares({ input: 5, cacheRead: 0, cacheWrite: 0, output: 5 });
  assert.equal(percent(s.kinds[1].share), '0.0%');
  assert.equal(s.kinds[1].count, 0);
});

test('zero_total_is_empty', () => {
  assert.equal(shares({ input: 0, cacheRead: 0, cacheWrite: 0, output: 0 }), null);
});

test('rounding_stays_in_range', () => {
  for (let i = 1; i < 500; i++) {
    const s = shares({ input: i, cacheRead: i * 7 + 3, cacheWrite: 1000 - i, output: i * i });
    const p = printed(s);
    assert.ok(p >= 99.8 && p <= 100.2, `${i}: ${p}`);
  }
});
