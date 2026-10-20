// @agents-index Client of GET /v1/days: the load of the last 400 days with one retry, and the subscription cost per range, cached.

import { addDays } from './format.date.js';
import { RANGES } from './range.select.js';

/** A failed request: the HTTP status or the error name, and the error.message of a 400 body. */
export class ApiError extends Error {
  constructor(status, detail) {
    super(detail || String(status));
    this.status = status;
    this.detail = detail;
  }
}

/**
 * Makes a client. `fetchFn` is the fetch of the page, and `today` the browser date as YYYY-MM-DD.
 * The client never sends `to`, so the server sets the end of the range in its own time zone.
 */
export function createClient(fetchFn, today) {
  const cache = new Map();

  async function get(from) {
    let res;
    try {
      res = await fetchFn(`/v1/days?from=${from}`, { headers: { accept: 'application/json' } });
    } catch (e) {
      throw new ApiError(e?.name || 'NetworkError');
    }
    if (res.status !== 200) {
      let detail = null;
      if (res.status === 400) {
        try { detail = (await res.json())?.error?.message ?? null; } catch { detail = null; }
      }
      throw new ApiError(res.status, detail);
    }
    return res.json();
  }

  return {
    /** The response for the browser date minus 399 days; a refused range retries one day later. */
    async load() {
      const from = addDays(today, -399);
      try {
        return await get(from);
      } catch (e) {
        if (e.status !== 400) throw e;
        return get(addDays(from, 1));
      }
    },
    /** The subscription_cost_usd of range `key` that ends on `to`; one request per range. */
    subscription(key, to) {
      if (!cache.has(key)) {
        const p = get(addDays(to, 1 - RANGES[key])).then((b) => b.subscription_cost_usd);
        p.catch(() => cache.delete(key));
        cache.set(key, p);
      }
      return cache.get(key);
    },
  };
}
