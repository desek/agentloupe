// @agents-index Heatmap levels: level 0 for zero, levels 1 to 4 from the quartiles of the values above zero; no DOM access.

/** The nearest-rank value at fraction `p` of an ascending array. */
function rank(sorted, p) {
  return sorted[Math.max(0, Math.ceil(p * sorted.length) - 1)];
}

/** The quartile limits q1, q2, q3 of the values above zero, or null when no value is above zero. */
export function quartiles(values) {
  const pos = values.filter((v) => v > 0).sort((a, b) => a - b);
  if (pos.length === 0) return null;
  return [rank(pos, 0.25), rank(pos, 0.5), rank(pos, 0.75)];
}

/** The level 0 to 4 of each value, in the order of `values`. */
export function levels(values) {
  const q = quartiles(values);
  return values.map((v) => {
    if (!(v > 0) || !q) return 0;
    if (v <= q[0]) return 1;
    if (v <= q[1]) return 2;
    if (v <= q[2]) return 3;
    return 4;
  });
}
