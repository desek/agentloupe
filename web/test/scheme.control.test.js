// @agents-index Unit tests of the scheme control with fakes: the key, the pressed button, the follow of the system, only one key.
import test from 'node:test';
import assert from 'node:assert/strict';
import { mount } from '../assets/js/scheme.control.js';

const keys = new Set();

/** A page with the three buttons, a recording storage (or one that throws), and a preference a test can change. */
function page(data = {}, light = false, blocked = false) {
  const fns = [];
  const mq = { matches: light, addEventListener: (_, f) => fns.push(f) };
  const op = (f) => (k, v) => { keys.add(k); if (blocked) throw new Error('blocked'); return f(k, v); };
  const localStorage = { getItem: op((k) => data[k] ?? null), setItem: op((k, v) => { data[k] = v; }), removeItem: op((k) => { delete data[k]; }) };
  const buttons = ['system', 'light', 'dark'].map((choice) => ({ dataset: { choice }, a: {}, setAttribute(k, v) { this.a[k] = v; }, addEventListener(_, f) { this.click = f; } }));
  const root = { dataset: {} };
  const p = { data, buttons, location: { hash: '#range=6M' }, documentElement: root, querySelectorAll: () => buttons };
  p.set = (v) => { mq.matches = v; fns.forEach((f) => f()); };
  p.seen = () => [root.dataset.scheme, ...buttons.filter((b) => b.a['aria-pressed'] === 'true').map((b) => b.dataset.choice)];
  mount(p, { localStorage, matchMedia: () => mq, location: p.location });
  return p;
}

test('select_writes_and_removes_key', () => {
  const p = page({ 'dashboard.scheme': 'sepia' });
  assert.deepEqual(p.seen(), ['dark', 'system']);
  p.buttons[1].click();
  assert.deepEqual([p.data['dashboard.scheme'], ...p.seen()], ['light', 'light', 'light']);
  p.buttons[0].click();
  assert.deepEqual([p.data['dashboard.scheme'], ...p.seen()], [undefined, 'dark', 'system']);
  assert.equal(p.location.hash, '#range=6M');
  assert.deepEqual(page({ 'dashboard.scheme': 'dark' }, true).seen(), ['dark', 'dark']);
});

test('system_follows_preference_change', () => {
  const p = page();
  p.set(true);
  assert.deepEqual(p.seen(), ['light', 'system']);
  p.buttons[2].click();
  p.set(true);
  assert.deepEqual(p.seen(), ['dark', 'dark']);
  const b = page({}, true, true);
  b.buttons[2].click();
  assert.deepEqual(b.seen(), ['dark', 'dark']);
});

test('only_scheme_key_used', () => {
  assert.deepEqual([...keys], ['dashboard.scheme']);
});
