// @agents-index Unit tests of the heatmap levels: five levels at the quartiles, and all zeros at level 0.
import test from 'node:test';
import assert from 'node:assert/strict';
import { levels } from '../assets/js/stats.quartiles.js';

test('five_levels', () => {
  assert.deepEqual(levels([0, 1, 2, 3, 4, 5, 6, 7, 8]), [0, 1, 1, 2, 2, 3, 3, 4, 4]);
});

test('all_zero_is_level_zero', () => {
  assert.deepEqual(levels([0, 0, 0]), [0, 0, 0]);
});
