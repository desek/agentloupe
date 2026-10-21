// @agents-index Unit tests of the live chart: 60 buckets of 10 seconds with the oldest dropped, and inline SVG with design token colours.
import test from 'node:test';
import assert from 'node:assert/strict';
import { add, render, series } from '../assets/js/live.chart.js';
import { fakeDoc } from './live.fake.js';

test('rolling_buckets_10_minutes', () => {
  let b = [];
  const t0 = 1_000_000_000_000;
  for (let s = 0; s < 720; s += 5) b = add(b, t0 + s * 1000, 1);
  const end = t0 + 719 * 1000;
  assert.ok(b.length <= 60);
  const v = series(b, end);
  assert.equal(v.length, 60);
  assert.equal(v.reduce((a, x) => a + x, 0), 120);
  assert.ok(b.every((x) => x.t > end - 600000));
});

test('chart_is_inline_svg_with_tokens', () => {
  const doc = fakeDoc();
  render(doc, [0, 3, 5, ...Array(57).fill(1)]);
  assert.equal(doc.made.filter((n) => n.tag === 'svg').length, 1);
  for (const n of doc.made) {
    for (const k of ['fill', 'stroke']) if (n.attrs[k]) assert.match(n.attrs[k], /^var\(--[a-z0-9-]+\)$/);
  }
});
