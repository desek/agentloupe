// @agents-index Pure share computation of the four token kinds of a range: counts, percentages, and bar segments.

const KINDS = [['input', 'Input'], ['cacheRead', 'Cache read'], ['cacheWrite', 'Cache write'], ['output', 'Output']];

/**
 * The shares of `t` ({input, cacheRead, cacheWrite, output}); null when the total is zero.
 * Each segment starts where the previous one ends, so the segments fill the bar exactly.
 */
export function shares(t) {
  const counts = KINDS.map(([k]) => Math.max(0, Number(t[k]) || 0));
  const total = counts.reduce((s, v) => s + v, 0);
  if (total === 0) return null;
  let start = 0;
  const kinds = KINDS.map(([key, label], i) => {
    const share = counts[i] / total;
    const k = { key, label, count: counts[i], share, start };
    start += share;
    return k;
  });
  return { total, kinds };
}
