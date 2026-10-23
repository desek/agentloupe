// @agents-index The header scheme switch: checked is dark; a choice that equals the system preference removes the stored key, so the page follows the system again.

import { KEY, resolve } from './scheme.resolve.js';

/** Wires the switch #scheme-switch; a blocked storage keeps the choice for this page view only. */
export function mount(doc, win) {
  const store = (f) => { try { return f(win.localStorage); } catch { return null; } };
  let mq = null;
  try { mq = win.matchMedia('(prefers-color-scheme: light)'); } catch { /* no matchMedia: dark */ }
  const system = () => (mq?.matches ? 'light' : 'dark');
  let stored = resolve(store((s) => s.getItem(KEY))).state;
  const sw = doc.getElementById('scheme-switch');
  const apply = () => {
    const scheme = resolve(stored, system()).scheme;
    doc.documentElement.dataset.scheme = scheme;
    sw.setAttribute('aria-checked', String(scheme === 'dark'));
  };
  sw.addEventListener('click', () => {
    const next = resolve(stored, system()).scheme === 'dark' ? 'light' : 'dark';
    // The same value as the system preference means no override: the key goes, and the system rules again.
    stored = next === system() ? 'system' : next;
    store((s) => (stored === 'system' ? s.removeItem(KEY) : s.setItem(KEY, stored)));
    apply();
  });
  mq?.addEventListener('change', apply);
  apply();
}
