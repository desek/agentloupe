// @agents-index Unit tests of the color tokens: light contrast rules, the same names in both schemes, dark kept, no literal elsewhere.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync as read, readdirSync } from 'node:fs';

const web = new URL('../', import.meta.url);
const css = read(new URL('assets/css/tokens.css', web), 'utf8');
const block = (s) => Object.fromEntries([...css.split(s)[1].split('}')[0].matchAll(/--([\w-]+): (#\w+)/g)].map((m) => m.slice(1)));
const dark = block(':root {');
const L = block('light"] {');
/** The WCAG 2.x contrast ratio of the light values of two tokens. */
function ratio(a, b) {
  const lum = (h) => [1, 3, 5].reduce((s, i, k) => { const c = parseInt(h.substr(i, 2), 16) / 255; return s + [0.2126, 0.7152, 0.0722][k] * (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4); }, 0);
  const [x, y] = [lum(L[a]), lum(L[b])].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}
const min = (list, bgs, n) => list.forEach((t) => bgs.forEach((b) => assert.ok(ratio(t, b) >= n, `${t} on ${b}`)));
const heat = ['heat-1', 'heat-2', 'heat-3', 'heat-4'];

test('light_text_contrast', () => {
  min(['text', 'text-body', 'text-muted', 'text-faint', 'link', 'link-hover'].map((t) => `color-${t}`), ['color-bg', 'color-surface'], 4.5);
  min(['color-ink-inverse'], ['color-text'], 4.5);
});

test('light_chart_contrast', () => {
  min(['input', 'output', 'positive', 'cache-read', 'cache-write'].map((t) => `color-${t}`).concat(heat), ['color-surface'], 3);
  min(['color-positive', 'color-output'], ['color-surface'], 4.5);
  heat.slice(1).forEach((t, i) => assert.ok(ratio(t, 'color-surface') > ratio(heat[i], 'color-surface'), t));
});

test('every_color_token_has_light_value', () => {
  assert.deepEqual(Object.keys(L).sort(), Object.keys(dark).sort());
  assert.equal(Object.values(dark).join(''), '#0F1012#111214#15161A#24262B#2A2C31#ECEAE4#B5B4AE#9A9AA0#85868C#5E86E0#BB7F35#3FA864#8C6BC8#4FA3A5#D9C27A#9DB6EE#C9D7F6#1D1F23#1B4A2F#21703E#2E9A55#4FCB74');
});

test('no_color_literal_outside_tokens', () => {
  for (const f of readdirSync(web, { recursive: true }).filter((f) => /^(?!test).*(?<!tokens)\.(css|js|html)$/.test(f))) {
    assert.doesNotMatch(read(new URL(f, web), 'utf8'), /#[\da-f]{3,8}\b(?![\w-])|\b(rgb|hsl)a?\(/i, f);
  }
});
