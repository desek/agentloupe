// @agents-index Drives headless Chrome over the DevTools protocol: draw time, headline, requests, first frame scheme, and rendered contrast.
//
// Usage: node web.render.cdp.mjs <chrome-path> <page-url> <profile-dir> [--scheme light|dark] [--stored none|light|dark|invalid]
//   --scheme   The emulated prefers-color-scheme. Default: dark.
//   --stored   The value put into the scheme key before the first navigation; none removes the key. Default: none.
// Prints one JSON object on stdout: draw_ms (the larger of the two widths), headline_ratio,
// headline_above_fold_360, headline_above_fold_1440, no_hscroll_360, remote_requests, scheme (the expected
// resolved scheme), first_frame_scheme_ok, min_text_contrast (the page and the states: empty, error, GitHub not
// connected, partial GitHub data, and the paused message), min_graphic_contrast, min_paused_figure_contrast,
// min_focus_contrast, and lowest (the element of each lowest value).
import { setTimeout as sleep } from 'node:timers/promises';
import { openChrome } from './web.cdp.session.mjs';
import { pixel } from './web.render.png.mjs';
import { PROBE } from './web.render.probe.mjs';

const [chrome, pageUrl, profile, ...flags] = process.argv.slice(2);
const flag = (k, d) => { const i = flags.indexOf(k); return i >= 0 ? flags[i + 1] : d; };
const system = flag('--scheme', 'dark');
const stored = flag('--stored', 'none');
// The value that the page must ignore, for the case invalid.
const STORED = { none: null, light: 'light', dark: 'dark', invalid: 'sepia' };
const expected = stored === 'light' || stored === 'dark' ? stored : system;
// The page background of each scheme, from tokens.css.
const BG = { dark: [0x0F, 0x10, 0x12], light: [0xF3, 0xF2, 0xEE] };
const MAGENTA = [255, 0, 255];

const cdp = await openChrome(chrome, profile);
const { send, evaluate, until } = cdp;
const origin = new URL(pageUrl).origin;
let remote = 0;
cdp.on((msg) => {
  if (msg.method !== 'Network.requestWillBeSent') return;
  const u = msg.params.request.url;
  if (!u.startsWith('data:') && !u.startsWith('about:') && new URL(u).origin !== origin) remote += 1;
});

await send('Network.enable');
await send('Network.setCacheDisabled', { cacheDisabled: true });
await send('Page.enable');
await send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: system }] });
// Keeps each socket of the page, so the probe can close it and show the paused state.
await send('Page.addScriptToEvaluateOnNewDocument', { source: 'window.__sockets = []; { const W = window.WebSocket; window.WebSocket = class extends W { constructor(...a) { super(...a); window.__sockets.push(this); } }; }' });

/** Puts the stored value into the scheme key on the page origin, or removes the key. */
async function store() {
  await send('Page.navigate', { url: `${origin}/favicon.ico` });
  await until('document.readyState === "complete"');
  const v = STORED[stored];
  await evaluate(v ? `localStorage.setItem('dashboard.scheme', ${JSON.stringify(v)})` : `localStorage.removeItem('dashboard.scheme')`);
}

/**
 * Loads the page with a screencast and checks the first frame that the page paints: it must show the page
 * background of the expected scheme. The blank page before it is painted magenta, so its frames are skipped.
 */
async function firstFrame() {
  await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  await send('Page.navigate', { url: 'about:blank' });
  await sleep(100);
  await send('Emulation.setDefaultBackgroundColorOverride', { color: { r: 255, g: 0, b: 255, a: 1 } });
  let first = null;
  const off = cdp.on((msg) => {
    if (msg.method !== 'Page.screencastFrame') return;
    send('Page.screencastFrameAck', { sessionId: msg.params.sessionId });
    if (first) return;
    const px = pixel(msg.params.data, 4, 4);
    if (px.some((v, i) => Math.abs(v - MAGENTA[i]) > 8)) first = px;
  });
  await send('Page.startScreencast', { format: 'png', everyNthFrame: 1 });
  await send('Page.navigate', { url: pageUrl });
  for (let i = 0; i < 200 && !first; i++) await sleep(25);
  await send('Page.stopScreencast');
  off();
  await send('Emulation.setDefaultBackgroundColorOverride', {});
  return !!first && first.every((v, i) => Math.abs(v - BG[expected][i]) <= 3);
}

/** Loads the page at a width and waits for the first full draw. */
async function load(width, height) {
  await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: width < 600 });
  await send('Page.navigate', { url: pageUrl });
  await sleep(50);
  await until('document.documentElement.dataset.drawMs || document.querySelector("#main .state")');
  await evaluate(PROBE);
}

async function measure(width, height) {
  await load(width, height);
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
      scheme: document.documentElement.dataset.scheme,
      colorScheme: getComputedStyle(document.documentElement).colorScheme,
    };
  })()`);
}

/** Tabs through the page and returns the lowest contrast of the focus outline of each button it reaches. */
async function focusAll() {
  const out = [];
  for (let i = 0; i < 12; i++) {
    await cdp.press('Tab', 'Tab', 9);
    const r = await evaluate('document.activeElement && document.activeElement.tagName === "BUTTON" ? window.__probe.focus() : null');
    if (r) out.push(r);
  }
  return out;
}

/** Answers GET /v1/days with the response changed for a state: empty, error, none (GitHub not connected), or partial. */
async function withDays(mode, fn) {
  await send('Fetch.enable', { patterns: [{ urlPattern: '*/v1/days*', requestStage: 'Response' }] });
  const off = cdp.on(async (msg) => {
    if (msg.method !== 'Fetch.requestPaused') return;
    const id = msg.params.requestId;
    if (mode === 'error') { await send('Fetch.fulfillRequest', { requestId: id, responseCode: 500, body: '' }); return; }
    const raw = await send('Fetch.getResponseBody', { requestId: id });
    const text = raw.result.base64Encoded ? Buffer.from(raw.result.body, 'base64').toString('utf8') : raw.result.body;
    const j = JSON.parse(text);
    if (mode === 'empty') j.days = [];
    for (const [i, d] of j.days.entries()) {
      if (mode === 'none' || (mode === 'partial' && i % 2)) for (const k of ['prs_opened', 'prs_merged', 'lines_added', 'lines_removed']) d[k] = null;
    }
    const body = Buffer.from(JSON.stringify(j)).toString('base64');
    await send('Fetch.fulfillRequest', { requestId: id, responseCode: 200, responseHeaders: [{ name: 'content-type', value: 'application/json' }], body });
  });
  try { return await fn(); } finally { off(); await send('Fetch.disable'); }
}

await store();
const firstOk = await firstFrame();
const narrow = await measure(360, 640);
const wide = await measure(1440, 900);
const lowest = {};
const texts = [];
const add = (key, r) => { if (r && r.min !== null) { texts.push(r.min); if (!lowest[key] || r.min < lowest[key].min) lowest[key] = r; } };

add('page', await evaluate('window.__probe.text()'));
const graphic = await evaluate('window.__probe.graphic()');
const focus = await focusAll();
for (const mode of ['empty', 'error', 'none', 'partial']) {
  await withDays(mode, async () => {
    await load(1440, 900);
    add(mode, await evaluate('window.__probe.text()'));
    if (mode === 'error') focus.push(...await focusAll());
  });
}
// The paused state: the socket closes after a snapshot, so the section pauses and does not hide.
await load(1440, 900);
await until('window.__sockets.some((s) => s.readyState === 1) && document.querySelector(".live-note") && document.querySelector(".live-note").textContent === ""');
await evaluate('window.__sockets.forEach((s) => s.close())');
await until('document.querySelector(".live-paused")');
add('paused', await evaluate('window.__probe.text(".live-paused .live-value")'));
const pausedMsg = await evaluate('window.__probe.textOf(".live-paused .live-note")');
const pausedFig = await evaluate('window.__probe.textOf(".live-paused .live-value")');
add('paused message', pausedMsg);

const focusMin = focus.reduce((a, r) => (r.min < a.min ? r : a), { min: Infinity, at: 'no button reached' });
console.log(JSON.stringify({
  draw_ms: Math.max(narrow.draw, wide.draw),
  headline_ratio: Number(wide.ratio.toFixed(2)),
  headline_above_fold_360: narrow.fold,
  headline_above_fold_1440: wide.fold,
  no_hscroll_360: !narrow.hscroll,
  remote_requests: remote,
  scheme: expected,
  first_frame_scheme_ok: firstOk && wide.scheme === expected && wide.colorScheme === expected,
  min_text_contrast: Math.min(...texts),
  min_graphic_contrast: graphic.min,
  min_paused_figure_contrast: pausedFig.min,
  min_focus_contrast: focusMin.min === Infinity ? 0 : focusMin.min,
  lowest: { ...lowest, graphic, paused_figure: pausedFig, focus: focusMin, paused_seen: pausedMsg.min !== null },
}));
cdp.stop();
process.exit(0);
