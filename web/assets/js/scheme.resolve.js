// @agents-index The color scheme rule: the state of the scheme control and the resolved scheme from the stored value and the system preference; no DOM access.

/** The one localStorage key of the page; it holds the scheme override, `light` or `dark`. */
export const KEY = 'dashboard.scheme';

/** The control state of a stored value: `light` or `dark` is an override, anything else (none, invalid) is `system`. */
export function stateOf(stored) {
  return stored === 'light' || stored === 'dark' ? stored : 'system';
}

/**
 * The scheme for a stored value and a system preference (`light`, `dark`, or null for no preference).
 * An override wins; in the state System the preference decides, and no preference gives dark.
 */
export function resolve(stored, system) {
  const state = stateOf(stored);
  const scheme = state === 'system' ? (system === 'light' ? 'light' : 'dark') : state;
  return { state, scheme };
}

/** The stored value, or null when the storage is missing, blocked, or throws. */
export function readStored(storage) {
  try {
    return storage ? storage.getItem(KEY) : null;
  } catch {
    return null;
  }
}

/** The system preference from a `prefers-color-scheme: light` media query list, or null when there is none. */
export function systemOf(mql) {
  if (!mql) return null;
  return mql.matches ? 'light' : 'dark';
}
