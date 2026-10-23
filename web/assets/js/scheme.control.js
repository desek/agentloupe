// @agents-index The scheme control in the header: System, Light, and Dark; keeps the override in the one key and follows the system in the state System.

import { KEY, readStored, resolve, stateOf, systemOf } from './scheme.resolve.js';

/**
 * Wires the buttons of `#schemes` to the root element. A selection writes or removes the key, applies the
 * scheme at once, and touches neither the URL fragment nor the data; when the storage throws, the choice
 * still holds for this page view. Returns the current state for a test.
 */
export function mount(doc, win) {
  let storage = null;
  try { storage = win.localStorage; } catch { storage = null; }
  let mql = null;
  try { mql = win.matchMedia ? win.matchMedia('(prefers-color-scheme: light)') : null; } catch { mql = null; }
  let state = stateOf(readStored(storage));
  const buttons = [...doc.querySelectorAll('#schemes button')];

  const apply = () => {
    const { scheme } = resolve(state === 'system' ? null : state, systemOf(mql));
    doc.documentElement.setAttribute('data-scheme', scheme);
    for (const b of buttons) b.setAttribute('aria-pressed', String(b.dataset.choice === state));
  };

  const select = (next) => {
    state = stateOf(next);
    try {
      if (state === 'system') storage?.removeItem(KEY);
      else storage?.setItem(KEY, state);
    } catch { /* blocked storage: the choice holds for this page view only */ }
    apply();
  };

  for (const b of buttons) b.addEventListener('click', () => select(b.dataset.choice));
  // A change of the system preference matters only in the state System; resolve ignores it otherwise.
  mql?.addEventListener?.('change', apply);
  apply();
  return { state: () => state };
}
