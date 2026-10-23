// @agents-index Shared DevTools session of the web scripts: starts headless Chrome, sends commands, evaluates in the page, waits for events.
//
// Node 22 has a global WebSocket, so the helper needs no npm package. The caller must call stop().
import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { setTimeout as sleep } from 'node:timers/promises';

/** Starts Chrome with `profile` as its user data dir and connects to its first page target. */
export async function openChrome(chrome, profile) {
  const proc = spawn(chrome, ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check', '--disable-gpu', 'about:blank'], { stdio: 'ignore' });
  const stop = () => { try { proc.kill('SIGKILL'); } catch { /* already gone */ } };
  process.on('exit', stop);
  let p = null;
  for (let i = 0; i < 100 && !p; i++) {
    try { p = (await readFile(`${profile}/DevToolsActivePort`, 'utf8')).split('\n')[0]; } catch { await sleep(100); }
  }
  if (!p) { stop(); throw new Error('Chrome did not open a DevTools port'); }
  const targets = await (await fetch(`http://127.0.0.1:${p}/json/list`)).json();
  const ws = new WebSocket(targets.find((t) => t.type === 'page').webSocketDebuggerUrl);
  await new Promise((ok, no) => { ws.onopen = ok; ws.onerror = no; });
  let id = 0;
  const waiting = new Map();
  const listeners = new Set();
  ws.onmessage = (m) => {
    const msg = JSON.parse(m.data);
    if (msg.id && waiting.has(msg.id)) { waiting.get(msg.id)(msg); waiting.delete(msg.id); }
    if (msg.method) for (const f of listeners) f(msg);
  };
  const send = (method, params = {}) => new Promise((ok) => { const n = ++id; waiting.set(n, ok); ws.send(JSON.stringify({ id: n, method, params })); });
  const evaluate = async (expr) => (await send('Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true })).result?.result?.value;
  /** Polls `expr` in the page until it is truthy or about 5 seconds pass; returns the last value. */
  const until = async (expr, tries = 200) => {
    let v = null;
    for (let i = 0; i < tries && !v; i++) { v = await evaluate(expr); if (!v) await sleep(25); }
    return v;
  };
  /** Presses one key, for example Tab, Enter, or a space, as a keyboard does. */
  const press = async (key, code, keyCode, text) => {
    await send('Input.dispatchKeyEvent', { type: 'keyDown', key, code, windowsVirtualKeyCode: keyCode, text });
    await send('Input.dispatchKeyEvent', { type: 'keyUp', key, code, windowsVirtualKeyCode: keyCode });
  };
  const on = (f) => { listeners.add(f); return () => listeners.delete(f); };
  return { send, evaluate, until, press, on, stop: () => { try { ws.close(); } catch { /* closed */ } stop(); } };
}
