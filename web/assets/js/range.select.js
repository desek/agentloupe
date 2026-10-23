// @agents-index The dashboard ranges and the URL fragment that keeps the selected range; no DOM access.

/** Days in each range; each range ends on the `to` of the response, both ends included. */
export const RANGES = { '1M': 30, '3M': 91, '6M': 182, '1Y': 365 };

export const DEFAULT_RANGE = '1M';

/** The range of a fragment such as #range=6M; a missing or unknown fragment gives 1M. */
export function fromFragment(hash) {
  const m = /^#?range=(1M|3M|6M|1Y)$/.exec(hash || '');
  return m ? m[1] : DEFAULT_RANGE;
}

export function toFragment(key) {
  return `#range=${key}`;
}
