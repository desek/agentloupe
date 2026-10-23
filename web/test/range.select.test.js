// @agents-index Unit tests of the range fragment: a known fragment selects its range, anything else selects 1M.
import test from 'node:test';
import assert from 'node:assert/strict';
import { fromFragment } from '../assets/js/range.select.js';

test('fragment_selects_range', () => {
  assert.equal(fromFragment('#range=3M'), '3M');
  assert.equal(fromFragment('#range=6M'), '6M');
  assert.equal(fromFragment('#range=1Y'), '1Y');
  assert.equal(fromFragment('#x'), '1M');
  assert.equal(fromFragment(''), '1M');
});
