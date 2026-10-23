// @agents-index Date helpers on YYYY-MM-DD strings in UTC arithmetic: day shifts, windows, ISO weeks, and the update time text.

const DAY_MS = 86400000;

function ms(day) {
  const [y, m, d] = day.split('-').map(Number);
  return Date.UTC(y, m - 1, d);
}

function str(t) {
  return new Date(t).toISOString().slice(0, 10);
}

/** `day` plus `n` days, both as YYYY-MM-DD. */
export function addDays(day, n) {
  return str(ms(day) + n * DAY_MS);
}

/** The local browser date of `date` as YYYY-MM-DD. */
export function localDay(date) {
  const p = (v) => String(v).padStart(2, '0');
  return `${date.getFullYear()}-${p(date.getMonth() + 1)}-${p(date.getDate())}`;
}

/** The weekday of `day`, 0 for Monday to 6 for Sunday. */
export function weekday(day) {
  return (new Date(ms(day)).getUTCDay() + 6) % 7;
}

/** The ISO week of `day` as YYYY-Www, the year being the ISO week-numbering year. */
export function isoWeek(day) {
  const thursday = ms(day) + (3 - weekday(day)) * DAY_MS;
  const year = new Date(thursday).getUTCFullYear();
  const week = 1 + Math.floor((thursday - Date.UTC(year, 0, 1)) / (7 * DAY_MS));
  return `${year}-W${String(week).padStart(2, '0')}`;
}

/** A short date label such as 20 Oct 2026, fixed to English month names. */
export function label(day) {
  const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  const [y, m, d] = day.split('-').map(Number);
  return `${d} ${months[m - 1]} ${y}`;
}
