// @agents-index The dashboard ranges and the URL fragment that keeps the selected range; no DOM access.

/** Days in each range; each range ends on the `to` of the response, both ends included. */
export const RANGES = { '3M': 91, '6M': 182, '1Y': 365 };

export const DEFAULT_RANGE = '3M';

const WORDS = { '3M': '3 months', '6M': '6 months', '1Y': '12 months' };

/** The range of a fragment such as #range=6M; a missing or unknown fragment gives 3M. */
export function fromFragment(hash) {
  const m = /^#?range=(3M|6M|1Y)$/.exec(hash || '');
  return m ? m[1] : DEFAULT_RANGE;
}

export function toFragment(key) {
  return `#range=${key}`;
}

/** The range in words for the headline, for example "average over the last 3 months". */
export function words(key) {
  return `average over the last ${WORDS[key]}`;
}
