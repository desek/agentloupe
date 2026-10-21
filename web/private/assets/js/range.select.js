// @agents-index The ranges of the private page and the first day of each; no DOM access.

import { addDays } from '/assets/js/format.date.js';

/** Days in each range, both ends included. */
export const RANGES = { '3M': 91, '6M': 182, '1Y': 365 };

/** The first day of range `key` that ends on `today` (YYYY-MM-DD); an unknown key gives 3M. */
export function fromOf(key, today) {
  return addDays(today, 1 - (RANGES[key] || RANGES['3M']));
}
