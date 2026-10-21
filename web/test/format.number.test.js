// @agents-index Unit tests of the number formats: short form, thin-space groups, and US dollars with two decimals.
import test from 'node:test';
import assert from 'node:assert/strict';
import { short, grouped, usd, percent } from '../assets/js/format.number.js';

test('short_and_grouped', () => {
  assert.equal(short(1283200), '1.3M');
  assert.equal(short(12345), '12.3k');
  assert.equal(short(950), '950');
  assert.equal(short(999960), '1.0M');
  assert.equal(grouped(12345), '12 345');
  assert.equal(usd(0.0049), '$0.00');
  assert.equal(usd(1234.5), '$1 234.50');
});

test('percent_one_decimal', () => {
  assert.equal(percent(0.425), '42.5%');
  assert.equal(percent(0), '0.0%');
  assert.equal(percent(1), '100.0%');
  assert.equal(percent(1 / 3), '33.3%');
});
