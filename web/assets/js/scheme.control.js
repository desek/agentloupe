// @agents-index The header scheme control: System, Light, Dark; keeps the override in the one key and follows the system in the state System.

import { KEY, resolve } from './scheme.resolve.js';

/** Wires the buttons of #schemes; a blocked storage keeps the choice for this page view only. */
export function mount(doc, win) {
  const store = (f) => { try { return f(win.localStorage); } catch { return null; } };
  let mq = null;
  try { mq = win.matchMedia('(prefers-color-scheme: light)'); } catch { /* no matchMedia: dark */ }
  let state = resolve(store((s) => s.getItem(KEY))).state;
  const buttons = doc.querySelectorAll('#schemes button');
  const apply = () => {
    doc.documentElement.dataset.scheme = resolve(state, mq?.matches ? 'light' : null).scheme;
    for (const b of buttons) b.setAttribute('aria-pressed', String(b.dataset.choice === state));
  };
  for (const b of buttons) {
    b.addEventListener('click', () => {
      state = b.dataset.choice;
      store((s) => (state === 'system' ? s.removeItem(KEY) : s.setItem(KEY, state)));
      apply();
    });
  }
  mq?.addEventListener('change', apply);
  apply();
}
