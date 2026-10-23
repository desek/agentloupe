// @agents-index Unit tests of the scheme control with a fake document and storage: the key, the pressed button, the follow of the system.
import test from 'node:test';
import assert from 'node:assert/strict';
import { mount } from '../assets/js/scheme.control.js';
import { KEY } from '../assets/js/scheme.resolve.js';

/** A recording storage; `throws` makes each access fail as a blocked storage does. */
function fakeStorage(initial = {}, throws = false) {
  const data = { ...initial };
  const calls = [];
  const guard = (op, k) => { calls.push([op, k]); if (throws) throw new Error('SecurityError'); };
  return {
    data, calls,
    getItem(k) { guard('get', k); return k in data ? data[k] : null; },
    setItem(k, v) { guard('set', k); data[k] = String(v); },
    removeItem(k) { guard('remove', k); delete data[k]; },
  };
}

/** A media query list whose preference a test can change; it fires change to its listeners. */
function fakeMql(light) {
  const fns = [];
  return { matches: light, addEventListener: (k, f) => { if (k === 'change') fns.push(f); }, set(v) { this.matches = v; fns.forEach((f) => f()); } };
}

/** A document with the three buttons of the control and a root element with attributes. */
function fakeDoc() {
  const button = (choice) => {
    const b = { dataset: { choice }, attrs: {}, fns: [], setAttribute(k, v) { this.attrs[k] = v; }, addEventListener(k, f) { if (k === 'click') this.fns.push(f); }, click() { this.fns.forEach((f) => f()); } };
    return b;
  };
  const buttons = ['system', 'light', 'dark'].map(button);
  const root = { attrs: {}, setAttribute(k, v) { this.attrs[k] = v; } };
  return {
    buttons, documentElement: root,
    querySelectorAll: (s) => (s === '#schemes button' ? buttons : []),
    pressed() { return buttons.filter((b) => b.attrs['aria-pressed'] === 'true').map((b) => b.dataset.choice); },
    scheme() { return root.attrs['data-scheme']; },
  };
}

const win = (storage, mql) => ({ localStorage: storage, matchMedia: () => mql, location: { hash: '#range=6M' } });

const all = [];

test('select_writes_and_removes_key', () => {
  const doc = fakeDoc();
  const st = fakeStorage();
  all.push(st);
  const w = win(st, fakeMql(false));
  mount(doc, w);
  assert.deepEqual(doc.pressed(), ['system']);
  assert.equal(doc.scheme(), 'dark');
  doc.buttons[1].click();
  assert.equal(st.data[KEY], 'light');
  assert.equal(doc.scheme(), 'light');
  assert.deepEqual(doc.pressed(), ['light']);
  doc.buttons[0].click();
  assert.ok(!(KEY in st.data));
  assert.equal(doc.scheme(), 'dark');
  assert.deepEqual(doc.pressed(), ['system']);
  assert.equal(w.location.hash, '#range=6M');
});

test('stored_value_sets_pressed_button', () => {
  const doc = fakeDoc();
  const st = fakeStorage({ [KEY]: 'dark' });
  all.push(st);
  mount(doc, win(st, fakeMql(true)));
  assert.deepEqual(doc.pressed(), ['dark']);
  assert.equal(doc.scheme(), 'dark');
  const bad = fakeDoc();
  const st2 = fakeStorage({ [KEY]: 'sepia' });
  all.push(st2);
  mount(bad, win(st2, fakeMql(true)));
  assert.deepEqual(bad.pressed(), ['system']);
  assert.equal(bad.scheme(), 'light');
});

test('system_follows_preference_change', () => {
  const doc = fakeDoc();
  const st = fakeStorage();
  all.push(st);
  const mql = fakeMql(false);
  mount(doc, win(st, mql));
  mql.set(true);
  assert.equal(doc.scheme(), 'light');
  doc.buttons[2].click();
  mql.set(false);
  mql.set(true);
  assert.equal(doc.scheme(), 'dark');
  assert.deepEqual(doc.pressed(), ['dark']);
});

test('blocked_storage_still_switches', () => {
  const doc = fakeDoc();
  const st = fakeStorage({}, true);
  all.push(st);
  assert.doesNotThrow(() => mount(doc, win(st, fakeMql(true))));
  assert.equal(doc.scheme(), 'light');
  assert.doesNotThrow(() => doc.buttons[2].click());
  assert.equal(doc.scheme(), 'dark');
  assert.deepEqual(doc.pressed(), ['dark']);
});

test('only_scheme_key_used', () => {
  const keys = new Set(all.flatMap((s) => s.calls.map(([, k]) => k)));
  assert.deepEqual([...keys], [KEY]);
});
