// @agents-index Drives the light mode scenario in headless Chrome: system preference, the header scheme switch, reload, follow, keyboard, layout, private page.
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
  checked: document.getElementById('scheme-switch').getAttribute('aria-checked'),
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
/** Clicks the scheme switch with the mouse at its center. */
async function click() {
  const r = await evaluate(`(() => { const b = document.getElementById('scheme-switch'); b.scrollIntoView({ block: 'nearest' }); const r = b.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
  for (const type of ['mousePressed', 'mouseReleased']) await send('Input.dispatchMouseEvent', { type, x: r.x, y: r.y, button: 'left', clickCount: 1 });
  await sleep(50);
}
async function shot(name) {
  const r = await send('Page.captureScreenshot', { format: 'png' });
  await writeFile(`${shots}/dashboard.light-mode.${name}.${width}.png`, Buffer.from(r.result.data, 'base64'));
}
/** The page shows `scheme`, and the switch is checked exactly when the scheme is dark. */
const shows = (s, scheme) => s.scheme === scheme && s.checked === String(scheme === 'dark');
/** The WCAG ratio of the knob on the track and of the track on the header background, in the current scheme. */
const switchContrast = () => evaluate(`(() => {
  const lin = (v) => { const c = v / 255; return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; };
  const lum = (s) => { const p = s.match(/[\\d.]+/g).map(Number); return 0.2126 * lin(p[0]) + 0.7152 * lin(p[1]) + 0.0722 * lin(p[2]); };
  const ratio = (a, b) => { const x = lum(a); const y = lum(b); return Number(((Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05)).toFixed(2)); };
  const sw = document.getElementById('scheme-switch');
  const track = getComputedStyle(sw).backgroundColor; const knob = getComputedStyle(sw.querySelector('.switch-knob')).backgroundColor;
  const page = getComputedStyle(document.body).backgroundColor;
  return { knob: ratio(knob, track), track: ratio(track, page) };
})()`);

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
  check('AC-1 light from the system preference, switch aria-checked false', shows(s, 'light') && s.colorScheme === 'light' && s.key === null, JSON.stringify(s));
  // Step 5: the contrast of each section, and of the switch off.
  const text = await evaluate('window.__probe.text()');
  const graphic = await evaluate('window.__probe.graphic()');
  check('step 5 text contrast at least 4.5', text.min >= 4.5, JSON.stringify(text));
  check('step 5 graphic contrast at least 3.0', graphic.min >= 3, JSON.stringify(graphic));
  const offContrast = await switchContrast();
  await shot('light');
  // Step 7: a click on the switch sets dark, which is not the system value, so the key holds dark.
  const before = daysRequests; const sockets = s.sockets;
  await click();
  s = await state();
  await sleep(500);
  const after = await state();
  check('AC-3 click gives dark at once, aria-checked true, key dark', shows(s, 'dark') && s.colorScheme === 'dark' && s.key === 'dark', JSON.stringify(s));
  check('AC-3 no new /v1/days request, socket open, fragment kept, no reload', daysRequests === before && after.sockets === sockets && after.open === 1 && after.hash === '#range=6M' && after.marker === 1, `requests ${daysRequests - before}, ${JSON.stringify(after)}`);
  await evaluate(PROBE);
  const darkText = await evaluate('window.__probe.text()');
  check('step 7 text contrast at least 4.5 in dark', darkText.min >= 4.5, JSON.stringify(darkText));
  const onContrast = await switchContrast();
  check('switch knob on track and track on page at least 3 to 1, off in light and on in dark', [offContrast, onContrast].every((c) => c.knob >= 3 && c.track >= 3), `off ${JSON.stringify(offContrast)} on ${JSON.stringify(onContrast)}`);
  await shot('dark');
  // Step 9: reload.
  await load(`${pageUrl}#range=6M`);
  s = await state();
  check('step 9 reload keeps dark, aria-checked true', shows(s, 'dark') && s.key === 'dark', JSON.stringify(s));
  // Step 10: the click that sets the system value (light) removes the key.
  await click();
  s = await state();
  check('AC-3 click to the system value light removes the key', shows(s, 'light') && s.key === null, JSON.stringify(s));
  // Step 11: invalid key.
  await evaluate("localStorage.setItem('dashboard.scheme', 'sepia')");
  await load(`${pageUrl}#range=6M`);
  s = await state();
  check('invalid key: system preference light, aria-checked false', shows(s, 'light'), JSON.stringify(s));
  // Step 12 and AC-8: follow the system only with no key.
  await evaluate("localStorage.removeItem('dashboard.scheme')");
  await prefer('dark');
  await load(`${pageUrl}#range=6M`);
  await prefer('light');
  await sleep(100);
  s = await state();
  check('AC-8 no key: follows dark to light with no reload', shows(s, 'light') && s.marker === 1, JSON.stringify(s));
  await click();
  await prefer('dark');
  await sleep(100);
  await prefer('light');
  await sleep(100);
  s = await state();
  check('AC-8 key dark: stays dark on a change to light', shows(s, 'dark') && s.key === 'dark' && s.marker === 1, JSON.stringify(s));
  // AC-4: with the preference dark, a click sets light, which survives a reload.
  await prefer('dark');
  await click();
  s = await state();
  const stored = s.key;
  await load(`${pageUrl}#range=6M`);
  s = await state();
  check('AC-4 light with the preference dark survives a reload', stored === 'light' && shows(s, 'light') && s.key === 'light', `key after click ${stored}, ${JSON.stringify(s)}`);
  // The click that sets the system value dark removes the key, and the page follows the system again.
  await click();
  s = await state();
  await prefer('light');
  await sleep(100);
  const follow = await state();
  check('click to the system value dark removes the key, then the page follows the system', shows(s, 'dark') && s.key === null && shows(follow, 'light') && follow.key === null && follow.marker === 1, `${JSON.stringify(s)} then ${JSON.stringify(follow)}`);
  // AC-11: the place of the switch, its role, name, and size.
  const lay = await evaluate(`(() => {
    const box = (e) => e.getBoundingClientRect();
    const head = box(document.querySelector('.header')); const brand = box(document.querySelector('.brand'));
    const ranges = box(document.getElementById('ranges')); const sw = document.getElementById('scheme-switch'); const r = box(sw);
    return {
      tag: sw.tagName, role: sw.getAttribute('role'), name: sw.getAttribute('aria-label'), text: sw.textContent.trim(),
      size: [r.width, r.height], switches: document.querySelectorAll('[role="switch"]').length, buttonsLeft: document.querySelectorAll('#schemes, [data-choice]').length,
      next: sw.previousElementSibling === document.getElementById('ranges'),
      sameRow: Math.abs((r.top + r.bottom) / 2 - (ranges.top + ranges.bottom) / 2) < 2 && Math.abs(ranges.top - brand.top) < 4,
      after: r.left > ranges.right, right: Math.abs(head.right - r.right) < 2, brandLeft: Math.abs(brand.left - head.left) < 2,
      below: r.top >= ranges.bottom, visible: r.width > 0 && r.bottom <= innerHeight,
      hscroll: document.documentElement.scrollWidth > innerWidth,
    };
  })()`);
  check('AC-11 one switch "Dark mode", role switch, 44 by 24 px, no three buttons', lay.tag === 'BUTTON' && lay.role === 'switch' && lay.name === 'Dark mode' && lay.text === '' && lay.size.join() === '44,24' && lay.switches === 1 && lay.buttonsLeft === 0 && lay.next, JSON.stringify(lay));
  if (w === 1440) check('AC-11 brand left, ranges then the switch at the right, one row', lay.sameRow && lay.after && lay.right && lay.brandLeft, JSON.stringify(lay));
  else check('AC-11 switch visible after the range group, no horizontal scroll', (lay.after || lay.below) && lay.visible && !lay.hscroll, JSON.stringify(lay));
  // AC-11 keyboard: from the last range button, one Tab reaches the switch; Space and then Enter each set the other scheme.
  await prefer('light');
  await evaluate("localStorage.removeItem('dashboard.scheme')");
  await load(`${pageUrl}#range=6M`);
  await evaluate('document.querySelector(\'#ranges button[data-range="1Y"]\').focus()');
  await cdp.press('Tab', 'Tab', 9);
  const reached = await evaluate('document.activeElement.id');
  await cdp.press(' ', 'Space', 32, ' ');
  await sleep(50);
  const afterSpace = await state();
  await cdp.press('Enter', 'Enter', 13, '\r');
  await sleep(50);
  s = await state();
  check('AC-11 Tab reaches the switch; Space sets dark with key dark, Enter sets light with no key', reached === 'scheme-switch' && shows(afterSpace, 'dark') && afterSpace.key === 'dark' && shows(s, 'light') && s.key === null, `focus ${reached} after Space ${JSON.stringify(afterSpace)} after Enter ${JSON.stringify(s)}`);
}

// Step 15 and AC-9: the private page stays dark with the preference light and the key light.
width = 1440;
await prefer('light');
await evaluate("localStorage.setItem('dashboard.scheme', 'light')");
await send('Network.setExtraHTTPHeaders', { headers: { authorization: `Bearer ${admin}` } });
await send('Page.navigate', { url: `${origin}/private/` });
await until('document.readyState === "complete" && document.getElementById("groups")');
const priv = await evaluate(`({ bg: getComputedStyle(document.body).backgroundColor, scheme: document.documentElement.dataset.scheme || null, colorScheme: getComputedStyle(document.documentElement).colorScheme, control: !!document.getElementById('scheme-switch') })`);
await send('Network.setExtraHTTPHeaders', { headers: {} });
check('AC-9 private page dark with no control', priv.bg === 'rgb(15, 16, 18)' && priv.scheme === null && priv.colorScheme === 'dark' && !priv.control, JSON.stringify(priv));

console.log(JSON.stringify({ checks, passed: checks.every((c) => c.ok) }));
cdp.stop();
process.exit(0);
