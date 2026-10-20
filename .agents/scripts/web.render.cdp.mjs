// @agents-index Drives headless Chrome over the DevTools protocol to time the first full draw and measure the headline and requests.
//
// Usage: node web.render.cdp.mjs <chrome-path> <page-url> <profile-dir>
// Prints one JSON object on stdout: draw_ms (the larger of the two widths), headline_ratio,
// headline_above_fold_360, headline_above_fold_1440, no_hscroll_360, remote_requests.
// Node 22 has a global WebSocket, so the script needs no npm package.
import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { setTimeout as sleep } from 'node:timers/promises';

const [chrome, pageUrl, profile] = process.argv.slice(2);
const proc = spawn(chrome, ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check', '--disable-gpu', 'about:blank'], { stdio: 'ignore' });
const stop = () => { try { proc.kill('SIGKILL'); } catch { /* already gone */ } };
process.on('exit', stop);

async function port() {
  for (let i = 0; i < 100; i++) {
    try { return (await readFile(`${profile}/DevToolsActivePort`, 'utf8')).split('\n')[0]; } catch { await sleep(100); }
  }
  throw new Error('Chrome did not open a DevTools port');
}

const p = await port();
const targets = await (await fetch(`http://127.0.0.1:${p}/json/list`)).json();
const ws = new WebSocket(targets.find((t) => t.type === 'page').webSocketDebuggerUrl);
await new Promise((ok, no) => { ws.onopen = ok; ws.onerror = no; });
let id = 0;
const waiting = new Map();
const origin = new URL(pageUrl).origin;
let remote = 0;
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  if (msg.id && waiting.has(msg.id)) { waiting.get(msg.id)(msg); waiting.delete(msg.id); }
  if (msg.method === 'Network.requestWillBeSent') {
    const u = msg.params.request.url;
    if (!u.startsWith('data:') && !u.startsWith('about:') && new URL(u).origin !== origin) remote += 1;
  }
};
const send = (method, params = {}) => new Promise((ok) => { const n = ++id; waiting.set(n, ok); ws.send(JSON.stringify({ id: n, method, params })); });
const evaluate = async (expr) => (await send('Runtime.evaluate', { expression: expr, returnByValue: true })).result?.result?.value;

await send('Network.enable');
await send('Network.setCacheDisabled', { cacheDisabled: true });
await send('Page.enable');

async function measure(width, height) {
  await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: width < 600 });
  await send('Page.navigate', { url: pageUrl });
  await sleep(50);
  for (let i = 0; i < 200; i++) {
    const v = await evaluate('document.documentElement.dataset.drawMs || null');
    if (v) break;
    await sleep(25);
  }
  return evaluate(`(() => {
    const h = document.getElementById('headline-figure');
    const t = document.getElementById('totals-figure');
    const r = h ? h.getBoundingClientRect() : null;
    const size = (e) => e ? parseFloat(getComputedStyle(e).fontSize) : 0;
    return {
      draw: Number(document.documentElement.dataset.drawMs || -1),
      ratio: size(t) ? size(h) / size(t) : 0,
      fold: !!r && r.bottom <= innerHeight && r.top >= 0,
      hscroll: document.documentElement.scrollWidth > innerWidth,
    };
  })()`);
}

const narrow = await measure(360, 640);
const wide = await measure(1440, 900);
console.log(JSON.stringify({
  draw_ms: Math.max(narrow.draw, wide.draw),
  headline_ratio: Number(wide.ratio.toFixed(2)),
  headline_above_fold_360: narrow.fold,
  headline_above_fold_1440: wide.fold,
  no_hscroll_360: !narrow.hscroll,
  remote_requests: remote,
}));
ws.close();
stop();
process.exit(0);
