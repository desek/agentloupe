// @agents-index Drives the light mode scenario in headless Chrome: system preference, the header control, reload, follow, keyboard, layout, private page.
//
// Usage: node web.scheme.scenario.mjs <chrome-path> <page-url> <profile-dir> <admin-token> <screenshot-dir>
// Prints one JSON object on stdout: { checks: [{ width, name, ok, detail }], passed }, and writes the screenshots
// dashboard.light-mode.{light,dark}.{1440,360}.png into <screenshot-dir>. Reference: .agents/scenarios/dashboard.light-mode.md.
import { writeFile } from 'node:fs/promises';
import { setTimeout as sleep } from 'node:timers/promises';
import { openChrome } from './web.cdp.session.mjs';
import { PROBE } from './web.render.probe.mjs';

const [chrome, pageUrl, profile, admin, shots] = process.argv.slice(2);
const origin = new URL(pageUrl).origin;
const cdp = await openChrome(chrome, profile);
const { send, evaluate, until } = cdp;
const checks = [];
let width = 0;
const check = (name, ok, detail = '') => checks.push({ width, name, ok: !!ok, detail });
let daysRequests = 0;
cdp.on((msg) => { if (msg.method === 'Network.requestWillBeSent' && msg.params.request.url.includes('/v1/days')) daysRequests += 1; });

await send('Network.enable');
await send('Network.setCacheDisabled', { cacheDisabled: true });
await send('Page.enable');
await send('Page.addScriptToEvaluateOnNewDocument', { source: 'window.__sockets = []; { const W = window.WebSocket; window.WebSocket = class extends W { constructor(...a) { super(...a); window.__sockets.push(this); } }; }' });

const prefer = (value) => send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value }] });
const state = () => evaluate(`({
  scheme: document.documentElement.dataset.scheme,
  colorScheme: getComputedStyle(document.documentElement).colorScheme,
  bg: getComputedStyle(document.body).backgroundColor,
  pressed: [...document.querySelectorAll('#schemes button[aria-pressed="true"]')].map((b) => b.textContent),
  key: localStorage.getItem('dashboard.scheme'),
  hash: location.hash,
  sockets: window.__sockets.length,
  open: window.__sockets.filter((s) => s.readyState === 1).length,
  marker: window.__marker || 0,
})`);
async function load(url = pageUrl) {
  await send('Page.navigate', { url });
  await sleep(50);
  await until('document.documentElement.dataset.drawMs');
  await until('window.__sockets.some((s) => s.readyState === 1)');
  await evaluate(PROBE);
  await evaluate('window.__marker = 1');
}
/** Clicks the scheme button `label` with the mouse at its center. */
async function click(label) {
  const r = await evaluate(`(() => { const b = [...document.querySelectorAll('#schemes button')].find((x) => x.textContent === ${JSON.stringify(label)}); b.scrollIntoView({ block: 'nearest' }); const r = b.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
  for (const type of ['mousePressed', 'mouseReleased']) await send('Input.dispatchMouseEvent', { type, x: r.x, y: r.y, button: 'left', clickCount: 1 });
}
async function shot(name) {
  const r = await send('Page.captureScreenshot', { format: 'png' });
  await writeFile(`${shots}/dashboard.light-mode.${name}.${width}.png`, Buffer.from(r.result.data, 'base64'));
}
const pressed = (s, label) => s.pressed.length === 1 && s.pressed[0] === label;

for (const [w, h] of [[1440, 900], [360, 740]]) {
  width = w;
  await send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: w < 600 });
  // Steps 3 and 4: preference light, no key, range 6M.
  await prefer('light');
  await send('Page.navigate', { url: `${origin}/favicon.ico` });
  await until('document.readyState === "complete"');
  await evaluate("localStorage.removeItem('dashboard.scheme')");
  await load(`${pageUrl}#range=6M`);
  let s = await state();
  check('AC-1 light from the system preference, System pressed', s.scheme === 'light' && s.colorScheme === 'light' && pressed(s, 'System'), JSON.stringify(s));
  // Step 5: the contrast of each section.
  const text = await evaluate('window.__probe.text()');
  const graphic = await evaluate('window.__probe.graphic()');
  check('step 5 text contrast at least 4.5', text.min >= 4.5, JSON.stringify(text));
  check('step 5 graphic contrast at least 3.0', graphic.min >= 3, JSON.stringify(graphic));
  await shot('light');
  // Step 7: select Dark.
  const before = daysRequests; const sockets = s.sockets;
  await click('Dark');
  s = await state();
  await sleep(500);
  const after = await state();
  check('AC-3 Dark at once, key dark', s.scheme === 'dark' && s.key === 'dark' && pressed(s, 'Dark'), JSON.stringify(s));
  check('AC-3 no new /v1/days request, socket open, fragment kept, no reload', daysRequests === before && after.sockets === sockets && after.open === 1 && after.hash === '#range=6M' && after.marker === 1, `requests ${daysRequests - before}, ${JSON.stringify(after)}`);
  await evaluate(PROBE);
  const darkText = await evaluate('window.__probe.text()');
  check('step 7 text contrast at least 4.5 in dark', darkText.min >= 4.5, JSON.stringify(darkText));
  await shot('dark');
  // Step 9: reload.
  await load(`${pageUrl}#range=6M`);
  s = await state();
  check('step 9 reload keeps Dark', s.scheme === 'dark' && pressed(s, 'Dark'), JSON.stringify(s));
  // Step 10: select System.
  await click('System');
  s = await state();
  check('AC-3 System gives light, key absent', s.scheme === 'light' && s.key === null && pressed(s, 'System'), JSON.stringify(s));
  // Step 11: invalid key.
  await evaluate("localStorage.setItem('dashboard.scheme', 'sepia')");
  await load(`${pageUrl}#range=6M`);
  s = await state();
  check('invalid key: system preference, System pressed', s.scheme === 'light' && pressed(s, 'System'), JSON.stringify(s));
  // Step 12 and AC-8: follow the system only in the state System.
  await evaluate("localStorage.removeItem('dashboard.scheme')");
  await prefer('dark');
  await load(`${pageUrl}#range=6M`);
  await prefer('light');
  await sleep(100);
  s = await state();
  check('AC-8 System follows dark to light with no reload', s.scheme === 'light' && s.marker === 1, JSON.stringify(s));
  await click('Dark');
  await prefer('dark');
  await sleep(100);
  await prefer('light');
  await sleep(100);
  s = await state();
  check('AC-8 Dark stays dark on a change to light', s.scheme === 'dark' && s.marker === 1, JSON.stringify(s));
  // AC-4: Light with the preference dark survives a reload.
  await prefer('dark');
  await click('Light');
  await load(`${pageUrl}#range=6M`);
  s = await state();
  check('AC-4 Light survives a reload with the preference dark', s.scheme === 'light' && pressed(s, 'Light'), JSON.stringify(s));
  // AC-11: the place of the control and its buttons.
  const lay = await evaluate(`(() => {
    const box = (e) => e.getBoundingClientRect();
    const head = box(document.querySelector('.header')); const brand = box(document.querySelector('.brand'));
    const ranges = box(document.getElementById('ranges')); const g = document.getElementById('schemes'); const schemes = box(g);
    const buttons = [...g.querySelectorAll('button')];
    return {
      name: g.getAttribute('aria-label'), labels: buttons.map((b) => b.textContent),
      heights: buttons.map((b) => b.getBoundingClientRect().height),
      sameRow: Math.abs(schemes.top - ranges.top) < 2 && Math.abs(ranges.top - brand.top) < 4,
      after: schemes.left > ranges.right, right: Math.abs(head.right - schemes.right) < 2, brandLeft: Math.abs(brand.left - head.left) < 2,
      below: schemes.top >= ranges.bottom, visible: schemes.width > 0 && schemes.bottom <= innerHeight,
      hscroll: document.documentElement.scrollWidth > innerWidth,
    };
  })()`);
  check('AC-11 group "Color scheme" with System, Light, Dark, each at least 44 px', lay.name === 'Color scheme' && lay.labels.join() === 'System,Light,Dark' && lay.heights.every((x) => x >= 44), JSON.stringify(lay));
  if (w === 1440) check('AC-11 brand left, ranges then the control at the right, one row', lay.sameRow && lay.after && lay.right && lay.brandLeft, JSON.stringify(lay));
  else check('AC-11 control visible below the range group, no horizontal scroll', lay.below && lay.visible && !lay.hscroll, JSON.stringify(lay));
  // AC-11 keyboard: Tab reaches each button; Enter and a space select.
  // Start from the last range button, so the next Tab goes to the first button of the control.
  await evaluate('document.querySelector(\'#ranges button[data-range="1Y"]\').focus()');
  const reached = [];
  let afterEnter = null;
  for (let i = 0; i < 10; i++) {
    await cdp.press('Tab', 'Tab', 9);
    const t = await evaluate('document.activeElement.closest("#schemes") ? document.activeElement.textContent : null');
    if (t) reached.push(t);
    if (t === 'System') { await cdp.press('Enter', 'Enter', 13, '\r'); afterEnter = await state(); }
    if (t === 'Dark') { await cdp.press(' ', 'Space', 32, ' '); break; }
  }
  s = await state();
  check('AC-11 keyboard reaches System, Light, Dark; Enter and Space select', reached.join() === 'System,Light,Dark' && afterEnter && pressed(afterEnter, 'System') && afterEnter.key === null && pressed(s, 'Dark') && s.key === 'dark', `${reached.join()} after Enter ${JSON.stringify(afterEnter)} after Space ${JSON.stringify(s)}`);
}

// Step 15 and AC-9: the private page stays dark with the preference light and the key light.
width = 1440;
await prefer('light');
await evaluate("localStorage.setItem('dashboard.scheme', 'light')");
await send('Network.setExtraHTTPHeaders', { headers: { authorization: `Bearer ${admin}` } });
await send('Page.navigate', { url: `${origin}/private/` });
await until('document.readyState === "complete" && document.getElementById("groups")');
const priv = await evaluate(`({ bg: getComputedStyle(document.body).backgroundColor, scheme: document.documentElement.dataset.scheme || null, colorScheme: getComputedStyle(document.documentElement).colorScheme, control: !!document.getElementById('schemes') })`);
await send('Network.setExtraHTTPHeaders', { headers: {} });
check('AC-9 private page dark with no control', priv.bg === 'rgb(15, 16, 18)' && priv.scheme === null && priv.colorScheme === 'dark' && !priv.control, JSON.stringify(priv));

console.log(JSON.stringify({ checks, passed: checks.every((c) => c.ok) }));
cdp.stop();
process.exit(0);
