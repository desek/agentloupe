// @agents-index Test fakes of the live section: a document with visibility, a socket, and a manual timer.

/** A document whose `hidden` can change and that fires visibilitychange. */
export function fakeDoc() {
  const fns = [];
  const made = [];
  const node = (tag) => {
    const n = { tag, attrs: {}, children: [], textContent: '', className: '', append(...c) { this.children.push(...c); }, setAttribute(k, v) { this.attrs[k] = v; } };
    made.push(n);
    return n;
  };
  return {
    hidden: false, made, createElement: node, createElementNS: (_, t) => node(t),
    addEventListener: (k, f) => { if (k === 'visibilitychange') fns.push(f); },
    setHidden(h) { this.hidden = h; fns.forEach((f) => f()); },
  };
}

/** The options of start() with recorded sockets and timers. */
export function harness(doc) {
  const o = { doc, url: 'ws://h/live', sockets: [], delays: [], timers: [], frames: [], pausedCount: 0, hidden: false };
  o.open = () => { const s = { closed: false, close() { this.closed = true; } }; o.sockets.push(s); return s; };
  o.timer = (f, ms) => { o.delays.push(ms); o.timers.push(f); return o.timers.length; };
  o.clear = () => {};
  o.frame = (f) => o.frames.push(f);
  o.paused = () => { o.pausedCount += 1; };
  o.hide = () => { o.hidden = true; };
  o.last = () => o.sockets[o.sockets.length - 1];
  o.fire = () => o.timers.shift()();
  o.snapshot = () => o.last().onmessage({ data: JSON.stringify({ v: 1, kind: 'snapshot', data: {} }) });
  return o;
}
