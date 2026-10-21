// @agents-index Unit test of the live state: the snapshot sets the figures, and each event and batch adds to them.
import test from 'node:test';
import assert from 'node:assert/strict';
import { apply, empty } from '../assets/js/live.state.js';

test('applies_snapshot_and_events', () => {
  const t = { input: 1, output: 2, cache_read: 3, cache_write: 4, reasoning: 0 };
  let s = apply(empty(), { kind: 'snapshot', data: { calls: 2, tokens: t, cost_usd: '0.5', segments_open: 1, in_flight: 0 } }).state;
  assert.deepEqual(s, { calls: 2, tokens: 10, segments: 1, cost: 0.5, inFlight: 0 });
  const r = apply(s, { kind: 'call.stored', data: { ...t, cost_usd: '0.25' } });
  assert.equal(r.added, 10);
  s = r.state;
  s = apply(s, { kind: 'calls.batch', data: { calls: 3, ...t, cost_usd: null } }).state;
  s = apply(s, { kind: 'exchange.start', data: {} }).state;
  s = apply(s, { kind: 'exchanges.batch', data: { started: 2, ended: 0, in_flight: 3 } }).state;
  s = apply(s, { kind: 'exchange.end', data: { status: 200 } }).state;
  s = apply(s, { kind: 'segments.open', data: { count: 4 } }).state;
  assert.deepEqual(s, { calls: 6, tokens: 30, segments: 4, cost: 0.75, inFlight: 2 });
});
