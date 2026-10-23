// @agents-index In-page probe of the render script: WCAG contrast of the rendered text, of the chart graphics, and of the focus outline.
//
// PROBE is page code as a string. Evaluated in the page, it defines window.__probe with:
// text(skip): the lowest contrast of each visible text on its rendered background; elements that match `skip` are left out.
// textOf(sel): the same over the elements that match `sel` only.
// graphic(): the lowest contrast of the series colors and heatmap levels 1 to 4 on their rendered background.
// focus(): the contrast of the outline of the focused element on the background around it, or null.
// Each result is { min, at }, where `at` names the element with the lowest value.
export const PROBE = String.raw`(() => {
  const parse = (s) => {
    const m = /rgba?\(([^)]+)\)/.exec(s || '');
    if (!m) return null;
    const p = m[1].split(/[\s,/]+/).filter(Boolean).map(Number);
    return [p[0], p[1], p[2], p.length > 3 ? p[3] : 1];
  };
  const over = (top, base) => [0, 1, 2].map((i) => top[i] * top[3] + base[i] * (1 - top[3])).concat(1);
  const lin = (v) => { const c = v / 255; return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; };
  const lum = (c) => 0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2]);
  const ratio = (a, b) => { const x = lum(a); const y = lum(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };
  const chain = (el) => { const out = []; for (let e = el; e && e.nodeType === 1; e = e.parentElement) out.push(e); return out.reverse(); };
  // The rendered background of el: each background color from the root down, over white, the canvas of a page with no color.
  const bg = (el) => chain(el).reduce((base, e) => { const c = parse(getComputedStyle(e).backgroundColor); return c && c[3] > 0 ? over(c, base) : base; }, [255, 255, 255, 1]);
  const opacity = (el) => chain(el).reduce((o, e) => o * Number(getComputedStyle(e).opacity), 1);
  const visible = (el) => el.getClientRects().length > 0 && getComputedStyle(el).visibility === 'visible';
  const name = (el) => el.tagName.toLowerCase() + (el.className && typeof el.className === 'string' ? '.' + el.className.trim().split(/\s+/).join('.') : '') + ' "' + (el.textContent || '').trim().slice(0, 30) + '"';
  const lowest = (items) => items.reduce((acc, it) => (it.v < acc.min ? { min: it.v, at: it.at } : acc), { min: Infinity, at: null });
  const round = (r) => ({ min: r.min === Infinity ? null : Number(r.min.toFixed(2)), at: r.at });
  const ownText = (el) => [...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim());
  const textItems = (els) => els.filter((el) => !el.closest('svg') && ownText(el) && visible(el)).map((el) => {
    const c = parse(getComputedStyle(el).color);
    const b = bg(el);
    return { v: ratio(over([c[0], c[1], c[2], c[3] * opacity(el)], b), b), at: name(el) };
  });
  const text = (skip) => round(lowest(textItems([...document.body.querySelectorAll('*')].filter((el) => !(skip && el.matches(skip))))));
  const textOf = (sel) => round(lowest(textItems([...document.querySelectorAll(sel)])));
  const GRAPHIC = '.line.c-input, .line.c-output, .line.c-positive, rect.c-input, rect.c-output, rect.c-positive, .seg-input, .seg-output, .seg-cacheRead, .seg-cacheWrite, rect.h1, rect.h2, rect.h3, rect.h4, .live-chart rect, .swatch';
  const graphic = () => round(lowest([...document.querySelectorAll(GRAPHIC)].filter(visible).filter((el) => !el.matches('.swatch.h0')).map((el) => {
    const s = getComputedStyle(el);
    const paint = el.closest('svg') ? (el.matches('.line') ? s.stroke : s.fill) : s.backgroundColor;
    const c = parse(paint);
    const b = bg(el.closest('svg') ? el.closest('svg').parentElement : el.parentElement);
    return { v: c ? ratio(over([c[0], c[1], c[2], c[3] * opacity(el)], b), b) : 0, at: name(el) + ' ' + paint };
  })));
  const focus = () => {
    const el = document.activeElement;
    if (!el || el === document.body) return null;
    const s = getComputedStyle(el);
    if (s.outlineStyle === 'none' || parseFloat(s.outlineWidth) === 0) return { min: 0, at: name(el) + ' has no outline' };
    const b = bg(el.parentElement);
    return round({ min: ratio(over(parse(s.outlineColor), b), b), at: name(el) });
  };
  window.__probe = { text, textOf, graphic, focus };
  return true;
})()`;
