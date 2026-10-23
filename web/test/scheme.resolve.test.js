// @agents-index Unit tests of the color scheme rule: each row of the rule table, in the module and in the scheme script, and the storage error fall-back.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import { KEY, readStored, resolve, systemOf } from '../assets/js/scheme.resolve.js';

// The rule table of dashboard.md (Color scheme): stored value, system preference, scheme, pressed button.
// `error` stands for a storage that throws; null as the system stands for no preference.
const TABLE = [
  [null, 'light', 'light', 'system'],
  [null, 'dark', 'dark', 'system'],
  [null, null, 'dark', 'system'],
  ['light', 'dark', 'light', 'light'],
  ['dark', 'light', 'dark', 'dark'],
  ['sepia', 'light', 'light', 'system'],
  ['error', 'light', 'light', 'system'],
];

/** A fake storage that holds `value`, or throws on each access when `value` is `error`. */
function storage(value) {
  return {
    getItem(k) {
      if (value === 'error') throw new Error('SecurityError');
      return k === KEY ? value : null;
    },
  };
}

/** A fake matchMedia for a preference, or undefined for a browser with no matchMedia. */
function matchMedia(system) {
  if (system === null) return undefined;
  return (q) => ({ matches: q === '(prefers-color-scheme: light)' && system === 'light' });
}

const boot = readFileSync(new URL('../assets/js/scheme.boot.js', import.meta.url), 'utf8');

/** Runs the scheme script with fakes and returns the data-scheme it set on the root element. */
function runBoot(stored, system) {
  const attrs = {};
  const win = { matchMedia: matchMedia(system) };
  Object.defineProperty(win, 'localStorage', { get: () => storage(stored) });
  const doc = { documentElement: { setAttribute: (k, v) => { attrs[k] = v; } } };
  vm.runInNewContext(boot, { document: doc, window: win });
  return attrs['data-scheme'];
}

test('resolve_table', () => {
  for (const [stored, system, scheme, button] of TABLE) {
    const mql = matchMedia(system)?.('(prefers-color-scheme: light)') ?? null;
    const got = resolve(readStored(storage(stored)), systemOf(mql));
    assert.deepEqual(got, { scheme, state: button }, `${stored} ${system}`);
    assert.equal(runBoot(stored, system), scheme, `scheme script: ${stored} ${system}`);
  }
});

test('storage_error_falls_back', () => {
  assert.equal(readStored(storage('error')), null);
  assert.equal(readStored(undefined), null);
  assert.doesNotThrow(() => runBoot('error', 'light'));
  assert.equal(runBoot('error', 'light'), 'light');
});

test('boot_is_classic_script', () => {
  assert.doesNotMatch(boot, /^\s*(import|export)\b/m);
});
