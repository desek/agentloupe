// @agents-index Drives the product header scenario in headless Chrome: header row, mark, eyebrow, headline sparkline, Today card, default range.
//
// Usage: node web.header.scenario.mjs <chrome-path> <page-url> <profile-dir> <screenshot-dir>
// Prints one JSON object on stdout: { checks: [{ width, name, ok, detail }], passed }, and writes the screenshots
// dashboard.product-header.{1440,360}.png into <screenshot-dir>. Reference: .agents/scenarios/dashboard.product-header.md.
import { writeFile } from 'node:fs/promises';
import { setTimeout as sleep } from 'node:timers/promises';
import { openChrome } from './web.cdp.session.mjs';

const [chrome, pageUrl, profile, shots] = process.argv.slice(2);
const cdp = await openChrome(chrome, profile);
const { send, evaluate, until } = cdp;
const checks = [];
let width = 0;
const check = (name, ok, detail = '') => checks.push({ width, name, ok: !!ok, detail: typeof detail === 'string' ? detail : JSON.stringify(detail) });
/** A length within half a pixel; layout rounds subpixels. */
const near = (a, b, tol = 0.5) => Math.abs(a - b) <= tol;

await send('Page.enable');
await send('Network.enable');
await send('Network.setCacheDisabled', { cacheDisabled: true });

/** Opens `url` as a fresh document and waits for the first draw and the first live frame (the note empties). */
async function load(url) {
  await send('Page.navigate', { url: 'about:blank' });
  await send('Page.navigate', { url });
  await sleep(50);
  await until('document.documentElement.dataset.drawMs && document.getElementById("headline-figure")');
  await until('document.querySelector(".live-note") && document.querySelector(".live-note").textContent === ""');
}

/** Reads every value the checks grade, in one evaluation, from the rendered page. */
const read = () => evaluate(`(() => {
  const box = (e) => { const r = e.getBoundingClientRect(); return { l: r.left, r: r.right, t: r.top, b: r.bottom, w: r.width, h: r.height }; };
  const cs = (e) => getComputedStyle(e);
  // Resolves a design token at the current width and scheme with a probe element.
  const token = (prop, value) => { const p = document.createElement('div'); p.style[prop] = value; document.body.append(p); const v = cs(p)[prop]; p.remove(); return v; };
  const header = document.querySelector('.header'); const brand = document.querySelector('.brand');
  const logo = brand && brand.querySelector('svg'); const word = document.querySelector('h1');
  const ranges = document.getElementById('ranges');
  const eb = document.querySelector('.headline .eyebrow'); const fig = document.getElementById('headline-figure');
  const spark = document.querySelector('.headline svg');
  // A zero-height inline block in the figure sits on the baseline of the digits.
  const probe = document.createElement('span'); probe.style.cssText = 'display:inline-block;width:0;height:0;vertical-align:baseline';
  fig.append(probe); const baseline = probe.getBoundingClientRect().bottom; probe.remove();
  const live = document.querySelector('section.live'); const dot = live.querySelector('.live-dot'); const note = live.querySelector('.live-note');
  const chart = live.querySelector('.live-chart'); const figs = [...live.querySelectorAll('.live-fig')];
  const lcs = cs(live);
  return {
    title: document.title, h1: word.textContent.trim(), h1Count: document.querySelectorAll('h1').length, h1InBrand: brand.contains(word),
    header: box(header), lastRight: box(header.lastElementChild).r, afterRanges: ranges.nextElementSibling ? ranges.nextElementSibling.id : null, headerMinH: cs(header).minHeight, brand: box(brand), brandGap: cs(brand).columnGap,
    logo: logo ? { tag: logo.tagName.toLowerCase(), box: box(logo), hidden: logo.getAttribute('aria-hidden'), focusable: logo.getAttribute('focusable'),
      strokes: [...logo.querySelectorAll('[stroke]')].map((e) => e.getAttribute('stroke')), bars: [...logo.querySelectorAll('rect')].map((e) => [e.getAttribute('fill'), e.getAttribute('height')]),
      firstInBrand: brand.firstElementChild === logo, gapToWord: box(word).l - box(logo).r } : null,
    imgs: document.querySelectorAll('header img, header object, header use[href], header image').length,
    word: { box: box(word), size: cs(word).fontSize, weight: cs(word).fontWeight },
    ranges: box(ranges), buttons: [...ranges.querySelectorAll('button')].map((b) => [b.textContent.trim(), b.getAttribute('aria-pressed')]),
    headerText: header.innerText, updatedEl: !!document.getElementById('updated') || !!document.querySelector('.updated'),
    noUpdateYet: document.body.innerText.includes('No update yet'), updatedWord: /\\bUpdated\\b/.test(header.innerText),
    eyebrow: { text: eb && eb.textContent, shown: eb && eb.innerText, box: eb && box(eb), font: eb && cs(eb).fontFamily, size: eb && cs(eb).fontSize,
      color: eb && cs(eb).color, transform: eb && cs(eb).textTransform, spacing: eb && cs(eb).letterSpacing, firstInHeadline: eb && eb.parentElement.firstElementChild === eb },
    want: { mono: token('fontFamily', 'var(--font-mono)'), label: token('fontSize', 'var(--size-label)'), muted: token('color', 'var(--color-text-muted)'),
      headline: parseFloat(token('fontSize', 'var(--size-headline)')), small: parseFloat(token('fontSize', 'var(--size-figure-small)')), positive: token('backgroundColor', 'var(--color-positive)') },
    fig: { box: box(fig), size: parseFloat(cs(fig).fontSize), text: fig.textContent }, baseline,
    spark: spark ? { box: box(spark), points: (spark.querySelector('polyline')?.getAttribute('points') || '').trim().split(/\\s+/).filter(Boolean).length } : null,
    live: { name: live.getAttribute('aria-label'), heading: live.querySelector('h2')?.textContent, headingInRow: live.querySelector('.live-head')?.contains(live.querySelector('h2')),
      dot: { box: box(dot), bg: cs(dot).backgroundColor, anim: cs(dot).animationName, beforeHeading: dot.nextElementSibling === live.querySelector('h2') },
      note: { role: note.getAttribute('role'), text: note.textContent, rightEnd: near(box(note).r, box(live).r - parseFloat(lcs.paddingRight) - parseFloat(lcs.borderRightWidth)) },
      labels: figs.map((f) => f.querySelector('.note').textContent), tops: figs.map((f) => Math.round(box(f).t)),
      openSegments: live.innerText.includes('Open segments') || document.body.innerText.includes('Open segments'), oldHeading: document.body.innerText.includes('Live, today'),
      chart: chart ? { box: box(chart), content: box(live).w - parseFloat(lcs.paddingLeft) - parseFloat(lcs.paddingRight) - parseFloat(lcs.borderLeftWidth) - parseFloat(lcs.borderRightWidth) } : null },
    hash: location.hash, scrollW: document.documentElement.scrollWidth, innerW: innerWidth,
  };
  function near(a, b) { return Math.abs(a - b) <= 1; }
})()`);

/** Clicks the range button `key` with the mouse at its center. */
async function clickRange(key) {
  const r = await evaluate(`(() => { const b = document.querySelector('#ranges button[data-range="${key}"]'); b.scrollIntoView({ block: 'nearest' }); const r = b.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
  for (const type of ['mousePressed', 'mouseReleased']) await send('Input.dispatchMouseEvent', { type, x: r.x, y: r.y, button: 'left', clickCount: 1 });
  await until(`location.hash === '#range=${key}' && document.querySelector('.headline .eyebrow').textContent.includes('${key}')`);
}

const pressed = (s, key) => s.buttons.every(([k, p]) => p === String(k === key));

for (const [w, h] of [[1440, 900], [360, 740]]) {
  width = w;
  await send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: w < 600 });
  await send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'dark' }, { name: 'prefers-reduced-motion', value: 'no-preference' }] });
  // Step 3: no fragment.
  await load(pageUrl);
  const s = await read();
  const shot = await send('Page.captureScreenshot', { format: 'png' });
  await writeFile(`${shots}/dashboard.product-header.${w}.png`, Buffer.from(shot.result.data, 'base64'));

  // Header (AC-1, AC-3).
  check('AC-1 title and h1 are Agentloupe', s.title === 'Agentloupe' && s.h1 === 'Agentloupe' && s.h1Count === 1 && s.h1InBrand, { title: s.title, h1: s.h1, h1Count: s.h1Count });
  const brandLeft = near(s.brand.l, s.header.l, 1);
  const rangesRight = s.ranges.l >= s.brand.r;
  if (w === 1440) {
    const oneRow = near((s.brand.t + s.brand.b) / 2, (s.ranges.t + s.ranges.b) / 2, 2) && s.header.h <= Math.max(48, s.brand.h, s.ranges.h) + 1;
    check('AC-3 one row: brand at the left, ranges at the right (then only the scheme switch of CR-0020), centered, min height 48 px', oneRow && brandLeft && rangesRight && near(s.lastRight, s.header.r, 1) && [null, "scheme-switch"].includes(s.afterRanges) && s.headerMinH === "48px", { header: s.header, brand: s.brand, ranges: s.ranges, minHeight: s.headerMinH, lastRight: s.lastRight, afterRanges: s.afterRanges });
  } else {
    const wrapped = s.ranges.t >= s.brand.b - 1;
    check('AC-3 brand at the left; ranges at the right in the row or on a second line', brandLeft && (rangesRight || wrapped) && s.header.r <= s.innerW, { header: s.header, brand: s.brand, ranges: s.ranges, wrapped });
  }
  const L = s.logo;
  check('AC-3 mark: inline SVG 44 by 44 px, first in the brand, hidden from assistive technology, no file',
    L && L.tag === 'svg' && near(L.box.w, 44) && near(L.box.h, 44) && L.firstInBrand && L.hidden === 'true' && L.focusable === 'false' && s.imgs === 0, L);
  check('AC-3 mark: ring and handle in currentColor, three bars of different heights in --color-input',
    L && L.strokes.length >= 2 && L.strokes.every((c) => c === 'currentColor') && L.bars.length === 3 && L.bars.every(([f]) => f === 'var(--color-input)') && new Set(L.bars.map(([, hh]) => hh)).size === 3, L && { strokes: L.strokes, bars: L.bars });
  check('AC-3 wordmark 20 px, weight 600, 10 px after the mark', s.word.size === '20px' && s.word.weight === '600' && s.brandGap === '10px' && L && near(L.gapToWord, 10), { word: s.word, gap: s.brandGap, gapToWord: L && L.gapToWord });
  check('AC-3 range buttons 1M 3M 6M 1Y in this order', s.buttons.map(([k]) => k).join(' ') === '1M 3M 6M 1Y', s.buttons);
  check('AC-3 no update line and no "No update yet"', !s.updatedEl && !s.noUpdateYet && !s.updatedWord, { headerText: s.headerText, updatedEl: s.updatedEl, noUpdateYet: s.noUpdateYet });

  // Headline (AC-4, AC-5).
  const e = s.eyebrow;
  check("AC-4 eyebrow 'TOKENS PER DAY · 1M AVG', first in the headline, above the figure",
    e.text === 'Tokens per day · 1M avg' && e.shown === 'TOKENS PER DAY · 1M AVG' && e.firstInHeadline && e.box.b <= s.fig.box.t + 1, { text: e.text, shown: e.shown, eyebrow: e.box, figure: s.fig.box });
  check('AC-4 eyebrow mono label style: --font-mono, --size-label, --color-text-muted, uppercase, 0.04 em',
    e.font === s.want.mono && e.size === s.want.label && e.color === s.want.muted && e.transform === 'uppercase' && near(parseFloat(e.spacing), 0.04 * parseFloat(e.size), 0.05),
    { font: e.font, size: e.size, color: e.color, transform: e.transform, spacing: e.spacing, want: s.want });
  const sp = s.spark;
  check('AC-4 headline sparkline height 0.72 x --size-headline', sp && near(sp.box.h, 0.72 * s.want.headline), { height: sp && sp.box.h, want: 0.72 * s.want.headline, headline: s.want.headline, figureSize: s.fig.size });
  if (w === 1440) check('AC-4 sparkline beside the figure, bottom edge on the baseline of the digits', sp && sp.box.l >= s.fig.box.r && near(sp.box.b, s.baseline, 1), { sparkBottom: sp && sp.box.b, baseline: s.baseline, sparkLeft: sp && sp.box.l, figureRight: s.fig.box.r });
  else check('AC-5 sparkline under the figure', sp && sp.box.t >= s.fig.box.b - 1, { sparkTop: sp && sp.box.t, figureBottom: s.fig.box.b });

  // Live card (AC-6).
  const lv = s.live;
  check('AC-6 heading Today with an 8 px dot in --color-positive, name "Today, live"',
    lv.name === 'Today, live' && lv.heading === 'Today' && lv.headingInRow && lv.dot.beforeHeading && near(lv.dot.box.w, 8) && near(lv.dot.box.h, 8) && lv.dot.bg === s.want.positive && lv.dot.anim === 'live-pulse', { name: lv.name, heading: lv.heading, dot: lv.dot, positive: s.want.positive });
  check('AC-6 status note: role status at the right end, empty while frames arrive', lv.note.role === 'status' && lv.note.text === '' && lv.note.rightEnd, lv.note);
  check('AC-6 five figures: In flight, Calls, Tokens, Tokens per minute, API-equivalent cost*',
    lv.labels.join('|') === 'In flight|Calls|Tokens|Tokens per minute|API-equivalent cost*', lv.labels);
  if (w === 1440) check('AC-6 the five figures in one row', lv.tops.length === 5 && new Set(lv.tops).size === 1, lv.tops);
  check('AC-6 no Open segments and no "Live, today"', !lv.openSegments && !lv.oldHeading, { openSegments: lv.openSegments, oldHeading: lv.oldHeading });
  check('AC-6 live sparkline 0.72 x --size-figure-small, card width', lv.chart && near(lv.chart.box.h, 0.72 * s.want.small) && near(lv.chart.box.w, lv.chart.content, 1),
    { chart: lv.chart, want: 0.72 * s.want.small });

  // Ranges (AC-9).
  check('AC-9 no fragment: 1M pressed, 30 days in the sparkline', pressed(s, '1M') && sp && sp.points === 30, { buttons: s.buttons, points: sp && sp.points, hash: s.hash });
  check('AC-5 no horizontal scroll', s.scrollW <= s.innerW, { scrollWidth: s.scrollW, innerWidth: s.innerW });
  await clickRange('3M');
  const t = await read();
  check("AC-9 click 3M: fragment '#range=3M', 3M pressed, eyebrow '3M avg', 91 days", t.hash === '#range=3M' && pressed(t, '3M') && t.eyebrow.text === 'Tokens per day · 3M avg' && t.spark && t.spark.points === 91,
    { hash: t.hash, buttons: t.buttons, eyebrow: t.eyebrow.text, points: t.spark && t.spark.points });
  for (const frag of ['#range=9Z', '#x']) {
    await load(pageUrl + frag);
    const u = await read();
    check(`AC-9 unknown fragment ${frag}: 1M pressed, eyebrow '1M avg', 30 days`, pressed(u, '1M') && u.eyebrow.text === 'Tokens per day · 1M avg' && u.spark && u.spark.points === 30,
      { hash: u.hash, buttons: u.buttons, eyebrow: u.eyebrow.text, points: u.spark && u.spark.points });
  }
}

console.log(JSON.stringify({ checks, passed: checks.every((c) => c.ok) }));
cdp.stop();
process.exit(0);
