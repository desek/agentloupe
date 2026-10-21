// @agents-index Unit tests of the live socket loop: the backoff and its reset, the hide after 3 failed connects, the pause, the visibility rule.
import test from 'node:test';
import assert from 'node:assert/strict';
import { start, url } from '../assets/js/live.socket.js';
import { fakeDoc, harness } from './live.fake.js';

test('reconnect_backoff_sequence', () => {
  const o = harness(fakeDoc());
  start(o);
  o.snapshot();
  for (let i = 0; i < 7; i += 1) { o.last().onclose(); o.fire(); }
  assert.deepEqual(o.delays, [1000, 2000, 4000, 8000, 16000, 30000, 30000]);
  o.snapshot();
  o.last().onclose();
  assert.equal(o.delays.at(-1), 1000);
  assert.equal(url({ protocol: 'https:', host: 'h:1' }), 'wss://h:1/live');
});

test('hides_after_three_failed_connects', () => {
  const o = harness(fakeDoc());
  start(o);
  for (let i = 0; i < 3; i += 1) { o.last().onclose(); if (o.timers.length) o.fire(); }
  assert.equal(o.hidden, true);
  assert.equal(o.sockets.length, 3);
});

test('paused_state_keeps_figures', () => {
  const o = harness(fakeDoc());
  start(o);
  o.snapshot();
  o.last().onclose();
  assert.equal(o.pausedCount, 1);
  assert.equal(o.frames.length, 1);
  assert.equal(o.hidden, false);
});

test('stops_when_hidden_reopens_when_visible', () => {
  const doc = fakeDoc();
  const o = harness(doc);
  start(o);
  const first = o.last();
  doc.setHidden(true);
  assert.equal(first.closed, true);
  doc.setHidden(false);
  assert.equal(o.sockets.length, 2);
  assert.notEqual(o.last(), first);
});
