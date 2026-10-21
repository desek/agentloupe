// @agents-index The socket of the live section: connect, backoff 1 to 30 seconds, reset on snapshot, close when hidden, give up after 3 failed connects.

export const DELAYS = [1, 2, 4, 8, 16, 30];

/** The wait in milliseconds before the connect after `n` closes in a row. */
export const delay = (n) => DELAYS[Math.min(n, DELAYS.length - 1)] * 1000;

/** The socket URL at the page origin, path /live. */
export const url = (loc) => `${loc.protocol === 'https:' ? 'wss' : 'ws'}://${loc.host}/live`;

/**
 * Starts the connection loop. `o.open(url)` makes a socket, `o.timer` and `o.clear` schedule a
 * connect, and the callbacks `frame`, `paused`, and `hide` reach the section. A browser cannot
 * read the status of a failed upgrade, so 3 sockets in a row with no snapshot, before any
 * snapshot since the page load, hide the section until a reload.
 */
export function start(o) {
  const doc = o.doc;
  let ws = null;
  let closes = 0;
  let failed = 0;
  let seen = false;
  let dead = false;
  let timer = null;
  const wait = () => { if (timer !== null) o.clear(timer); timer = null; };
  function connect() {
    timer = null;
    if (dead || doc.hidden || ws) return;
    const s = o.open(o.url);
    let got = false;
    ws = s;
    s.onmessage = (e) => {
      let f;
      try { f = JSON.parse(e.data); } catch { return; }
      if (f.kind === 'snapshot') { got = true; seen = true; closes = 0; failed = 0; }
      o.frame(f);
    };
    s.onclose = () => {
      if (ws !== s) return;
      ws = null;
      failed = got ? 0 : failed + 1;
      if (!seen && failed >= 3) { dead = true; o.hide(); return; }
      o.paused();
      if (!doc.hidden) timer = o.timer(connect, delay(closes++));
    };
  }
  doc.addEventListener('visibilitychange', () => {
    wait();
    if (doc.hidden) {
      const s = ws;
      ws = null;
      if (s) { s.close(); o.paused(); }
    } else connect();
  });
  connect();
}
