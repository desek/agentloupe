// @agents-index The color scheme rule: the control state and the scheme from the stored value and the system preference; no DOM access.

export const KEY = 'dashboard.scheme';

/** An override `light` or `dark` wins; else the preference (`light`, `dark`, or null), and no preference is dark. */
export function resolve(stored, system) {
  const state = stored === 'light' || stored === 'dark' ? stored : 'system';
  return { state, scheme: state === 'system' ? (system === 'light' ? 'light' : 'dark') : state };
}
