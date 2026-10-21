// @agents-index The tag keys of the proxy tokens of a token report, for the group selector; no DOM access.

/** The sorted tag keys of every row that has tags. */
export function tagKeys(rows) {
  const keys = new Set();
  for (const r of rows || []) for (const k of Object.keys(r.tags || {})) keys.add(k);
  return [...keys].sort();
}

/** Fills the `<select>` with token and one tag option per key, and keeps `current` when it exists. */
export function fill(doc, select, keys, current) {
  const opts = [['token', 'token'], ...keys.map((k) => [`tag:${k}`, `tag ${k}`])];
  select.replaceChildren(...opts.map(([v, t]) => {
    const o = doc.createElement('option');
    o.value = v;
    o.textContent = t;
    return o;
  }));
  select.value = opts.some(([v]) => v === current) ? current : 'token';
}
