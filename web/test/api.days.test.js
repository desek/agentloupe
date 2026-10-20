// @agents-index Unit tests of the API client: the request has no `to`, the view ends on the response `to`, one request per range.
import test from 'node:test';
import assert from 'node:assert/strict';
import { createClient } from '../assets/js/api.days.js';
import { build } from '../assets/js/view.build.js';
import { localDay } from '../assets/js/format.date.js';

function fake(to) {
  const urls = [];
  const fetchFn = async (u) => {
    urls.push(u);
    return { status: 200, json: async () => ({ from: '2025-09-17', to, updated_at: null, subscription_cost_usd: 6.5, days: [] }) };
  };
  return { urls, fetchFn };
}

test('request_omits_to', async () => {
  // At 23:30 on 2026-10-19 in Los Angeles it is already 2026-10-20 in Stockholm.
  process.env.TZ = 'America/Los_Angeles';
  const browser = localDay(new Date('2026-10-20T06:30:00Z'));
  assert.equal(browser, '2026-10-19');
  const { urls, fetchFn } = fake('2026-10-20');
  const resp = await createClient(fetchFn, browser).load();
  assert.equal(urls.length, 1);
  const q = new URL(urls[0], 'http://x').searchParams;
  assert.equal(q.get('from'), '2025-09-15');
  assert.equal(q.has('to'), false);
  assert.equal(build(resp, '3M').points.at(-1), '2026-10-20');
});

test('one_request_per_range', async () => {
  const { urls, fetchFn } = fake('2026-10-20');
  const c = createClient(fetchFn, '2026-10-20');
  await c.load();
  await c.subscription('3M', '2026-10-20');
  const before = urls.length;
  assert.equal(await c.subscription('6M', '2026-10-20'), 6.5);
  await c.subscription('3M', '2026-10-20');
  await c.subscription('6M', '2026-10-20');
  const six = urls.filter((u) => u === '/v1/days?from=2026-04-22');
  assert.equal(six.length, 1);
  assert.equal(urls.length, before + 1);
  assert.ok(urls.every((u) => !u.includes('to=')));
});

test('retry_once_on_refused_range', async () => {
  const urls = [];
  const c = createClient(async (u) => {
    urls.push(u);
    return urls.length === 1
      ? { status: 400, json: async () => ({ error: { message: 'range too long' } }) }
      : { status: 200, json: async () => ({ to: '2026-10-20', days: [] }) };
  }, '2026-10-20');
  await c.load();
  assert.deepEqual(urls, ['/v1/days?from=2025-09-16', '/v1/days?from=2025-09-17']);
});
