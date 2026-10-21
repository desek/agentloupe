// @agents-index Number formats of the dashboard that do not depend on the browser locale: grouped integers, short forms, US dollars.

const THIN = ' ';

/** An integer with a thin space as the group mark, for example 12 345. */
export function grouped(n) {
  const v = Math.round(Number(n) || 0);
  const s = String(Math.abs(v)).replace(/\B(?=(\d{3})+(?!\d))/g, THIN);
  return v < 0 ? `-${s}` : s;
}

/** The short form with one decimal for chart labels and the headline: 950, 12.3k, 1.3M. */
export function short(n) {
  const v = Number(n) || 0;
  const a = Math.abs(v);
  const sign = v < 0 ? '-' : '';
  if (a < 1000) return sign + String(Math.round(a));
  // A value that rounds to 1000.0k shows as 1.0M, so the unit never carries four digits.
  if (a < 999950) return `${sign}${(a / 1e3).toFixed(1)}k`;
  return `${sign}${(a / 1e6).toFixed(1)}M`;
}

/** A US dollar amount with two decimals and the group mark, for example $1 234.50. */
export function usd(n) {
  const v = Math.round((Number(n) || 0) * 100) / 100;
  const [int, frac] = Math.abs(v).toFixed(2).split('.');
  return `${v < 0 ? '-' : ''}$${grouped(int)}.${frac}`;
}

/** A share (0 to 1) as a percentage with one decimal, for example 42.5%. */
export function percent(share) {
  return `${(Math.round((Number(share) || 0) * 1000) / 10).toFixed(1)}%`;
}
