// @agents-index Pure state of the live section: the figures of the current day from the snapshot and each later live event.

const KEYS = ['input', 'output', 'cache_read', 'cache_write', 'reasoning'];
const num = (v) => Number(v) || 0;

/** The sum of the five token counts of an object. */
export const sum = (d) => KEYS.reduce((a, k) => a + num(d && d[k]), 0);

/** The figures before the first snapshot. */
export const empty = () => ({ calls: 0, tokens: 0, segments: 0, cost: 0, inFlight: 0 });

/** Applies one frame; returns the new figures and the tokens that the frame added, for the chart. */
export function apply(s, f) {
  const d = f.data || {};
  const n = { ...s };
  let added = 0;
  switch (f.kind) {
    case 'snapshot':
      return {
        state: { calls: num(d.calls), tokens: sum(d.tokens), segments: num(d.segments_open), cost: num(d.cost_usd), inFlight: num(d.in_flight) },
        added,
      };
    case 'call.stored':
    case 'calls.batch':
      added = sum(d);
      n.calls += f.kind === 'call.stored' ? 1 : num(d.calls);
      n.tokens += added;
      n.cost += num(d.cost_usd);
      break;
    case 'exchange.start': n.inFlight += 1; break;
    case 'exchange.end': n.inFlight = Math.max(0, n.inFlight - 1); break;
    case 'exchanges.batch': n.inFlight = num(d.in_flight); break;
    case 'segments.open': n.segments = num(d.count); break;
    default: break;
  }
  return { state: n, added };
}
