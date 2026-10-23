// @agents-index Unit tests of the scheme switch with fakes: checked is dark, the key, the return to the system, the follow of the system, only one key.
import test from 'node:test';
import assert from 'node:assert/strict';
import { mount } from '../assets/js/scheme.control.js';

const keys = new Set();

/** A page with the switch, a recording storage (or one that throws), and a preference a test can change. */
function page(data = {}, light = false, blocked = false) {
  const fns = [];
  const mq = { matches: light, addEventListener: (_, f) => fns.push(f) };
  const op = (f) => (k, v) => { keys.add(k); if (blocked) throw new Error('blocked'); return f(k, v); };
  const localStorage = { getItem: op((k) => data[k] ?? null), setItem: op((k, v) => { data[k] = v; }), removeItem: op((k) => { delete data[k]; }) };
  const sw = { a: {}, setAttribute(k, v) { this.a[k] = v; }, addEventListener(_, f) { this.click = f; } };
  const root = { dataset: {} };
  const p = { data, sw, location: { hash: '#range=6M' }, documentElement: root, getElementById: () => sw };
  p.set = (v) => { mq.matches = v; fns.forEach((f) => f()); };
  p.seen = () => [root.dataset.scheme, sw.a['aria-checked']];
  mount(p, { localStorage, matchMedia: () => mq, location: p.location });
  return p;
}

test('click_stores_the_override_and_a_system_value_removes_it', () => {
  const p = page({ 'dashboard.scheme': 'sepia' });
  assert.deepEqual(p.seen(), ['dark', 'true']);
  p.sw.click();
  assert.deepEqual([p.data['dashboard.scheme'], ...p.seen()], ['light', 'light', 'false']);
  p.sw.click();
  assert.deepEqual([p.data['dashboard.scheme'], ...p.seen()], [undefined, 'dark', 'true']);
  assert.equal(p.location.hash, '#range=6M');
  assert.deepEqual(page({ 'dashboard.scheme': 'dark' }, true).seen(), ['dark', 'true']);
});

test('system_is_followed_only_without_an_override', () => {
  const p = page();
  p.set(true);
  assert.deepEqual(p.seen(), ['light', 'false']);
  p.sw.click();
  assert.deepEqual([p.data['dashboard.scheme'], ...p.seen()], ['dark', 'dark', 'true']);
  p.set(false);
  p.set(true);
  assert.deepEqual(p.seen(), ['dark', 'true']);
  const b = page({}, true, true);
  b.sw.click();
  assert.deepEqual(b.seen(), ['dark', 'true']);
});

test('only_scheme_key_used', () => {
  assert.deepEqual([...keys], ['dashboard.scheme']);
});
