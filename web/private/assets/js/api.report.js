// @agents-index Client of the private API: the report and the session list, with the same-origin cookie; 401 is its own error.

/** A failed request: the HTTP status, or the error name of a network failure. */
export class PrivateError extends Error {
  constructor(status) {
    super(String(status));
    this.status = status;
  }
}

/** Makes a client over `fetchFn`; every path is under /private/api/v1/ on the same origin. */
export function createClient(fetchFn) {
  async function get(path, params) {
    const q = new URLSearchParams(Object.entries(params).filter(([, v]) => v !== undefined && v !== null));
    let res;
    try {
      res = await fetchFn(`/private/api/v1/${path}?${q}`, { headers: { accept: 'application/json' }, credentials: 'same-origin' });
    } catch (e) {
      throw new PrivateError(e?.name || 'NetworkError');
    }
    if (res.status !== 200) throw new PrivateError(res.status);
    return res.json();
  }
  return {
    /** The report of the range from `from` to the server day, by token or by the tag `tagKey`. */
    report(from, tagKey) {
      return get('report', tagKey ? { from, group: 'tag', tag_key: tagKey } : { from, group: 'token' });
    },
    /** One page of the sessions of the token `name`; `cursor` is the `next` of the page before. */
    sessions(name, from, cursor) {
      return get('sessions', { token: name, from, cursor });
    },
  };
}
