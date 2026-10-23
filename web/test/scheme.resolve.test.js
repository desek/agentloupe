// @agents-index Unit tests of the color scheme rule: each row of the rule table, in the module and in the scheme script.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import { resolve } from '../assets/js/scheme.resolve.js';

const boot = readFileSync(new URL('../assets/js/scheme.boot.js', import.meta.url), 'utf8');

/** Runs the scheme script; `stored` 'error' makes the storage throw, `system` null removes matchMedia. */
function runBoot(stored, system) {
  const root = { dataset: {} };
  const window = { get localStorage() { if (stored === 'error') throw new Error('blocked'); return { getItem: () => stored }; } };
  if (system) window.matchMedia = (q) => ({ matches: q.includes('light') && system === 'light' });
  vm.runInNewContext(boot, { document: { documentElement: root }, window });
  return root.dataset.scheme;
}

// The rule table of dashboard.md (Color scheme): stored, system, scheme, pressed button.
const TABLE = [[null, 'light', 'light', 'system'], [null, 'dark', 'dark', 'system'], [null, null, 'dark', 'system'],
  ['light', 'dark', 'light', 'light'], ['dark', 'light', 'dark', 'dark'], ['sepia', 'light', 'light', 'system']];

test('resolve_table', () => {
  for (const [stored, system, scheme, state] of TABLE) {
    assert.deepEqual(resolve(stored, system), { scheme, state });
    assert.equal(runBoot(stored, system), scheme);
  }
});

test('storage_error_falls_back', () => {
  assert.equal(runBoot('error', 'light'), 'light');
});
