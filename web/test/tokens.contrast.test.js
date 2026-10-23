// @agents-index Unit tests of the color tokens: the light values meet the contrast rules, both schemes name the same tokens, no color literal elsewhere.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, statSync } from 'node:fs';

const css = readFileSync(new URL('../assets/css/tokens.css', import.meta.url), 'utf8');

/** The color tokens (`--color-*` and `--heat-*`) of the first rule block whose selector is `selector`. */
function colors(selector) {
  const start = css.indexOf(`${selector} {`);
  assert.ok(start >= 0, `tokens.css has no block ${selector}`);
  const body = css.slice(start, css.indexOf('}', start));
  const out = {};
  for (const m of body.matchAll(/(--(?:color|heat)-[\w-]+):\s*(#[0-9A-Fa-f]{6})\s*;/g)) out[m[1]] = m[2].toUpperCase();
  return out;
}

const dark = colors(':root');
const light = colors(':root[data-scheme="light"]');

/** WCAG 2.x relative luminance of a #RRGGBB color. */
function luminance(hex) {
  const n = parseInt(hex.slice(1), 16);
  const [r, g, b] = [n >> 16, (n >> 8) & 255, n & 255].map((v) => {
    const c = v / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG 2.x contrast ratio of two colors. */
function contrast(a, b) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const TEXT = ['--color-text', '--color-text-body', '--color-text-muted', '--color-text-faint', '--color-link', '--color-link-hover'];
const CHART = ['--color-input', '--color-output', '--color-positive', '--color-cache-read', '--color-cache-write', '--heat-1', '--heat-2', '--heat-3', '--heat-4'];

// The dark values of the specification, which this change keeps.
const DARK = {
  '--color-bg': '#0F1012', '--color-ink-inverse': '#111214', '--color-surface': '#15161A', '--color-border': '#24262B',
  '--color-rule': '#2A2C31', '--color-text': '#ECEAE4', '--color-text-body': '#B5B4AE', '--color-text-muted': '#9A9AA0',
  '--color-text-faint': '#85868C', '--color-input': '#5E86E0', '--color-output': '#BB7F35', '--color-positive': '#3FA864',
  '--color-cache-read': '#8C6BC8', '--color-cache-write': '#4FA3A5', '--color-highlight': '#D9C27A', '--color-link': '#9DB6EE',
  '--color-link-hover': '#C9D7F6', '--heat-0': '#1D1F23', '--heat-1': '#1B4A2F', '--heat-2': '#21703E', '--heat-3': '#2E9A55',
  '--heat-4': '#4FCB74',
};

test('light_text_contrast', () => {
  for (const t of TEXT) {
    for (const bg of ['--color-bg', '--color-surface']) {
      assert.ok(contrast(light[t], light[bg]) >= 4.5, `${t} on ${bg}: ${contrast(light[t], light[bg]).toFixed(2)}`);
    }
  }
});

test('light_ink_inverse_contrast', () => {
  assert.ok(contrast(light['--color-ink-inverse'], light['--color-text']) >= 4.5);
});

test('light_chart_contrast', () => {
  for (const t of CHART) {
    assert.ok(contrast(light[t], light['--color-surface']) >= 3, `${t}: ${contrast(light[t], light['--color-surface']).toFixed(2)}`);
  }
  for (const t of ['--color-positive', '--color-output']) {
    assert.ok(contrast(light[t], light['--color-surface']) >= 4.5, `${t} as text`);
  }
});

test('light_heat_order', () => {
  const r = ['--heat-1', '--heat-2', '--heat-3', '--heat-4'].map((t) => contrast(light[t], light['--color-surface']));
  for (let i = 1; i < r.length; i++) assert.ok(r[i] > r[i - 1], `level ${i + 1} is not darker than level ${i}`);
});

test('light_surface_lighter_than_bg', () => {
  assert.ok(luminance(light['--color-surface']) > luminance(light['--color-bg']));
});

test('every_color_token_has_light_value', () => {
  assert.deepEqual(Object.keys(light).sort(), Object.keys(dark).sort());
  assert.deepEqual(dark, DARK);
});

/** Every page file under web/ outside the tests, recursively. */
function pageFiles(dir) {
  return readdirSync(dir).flatMap((f) => {
    const u = new URL(f, dir);
    if (statSync(u).isDirectory()) return f === 'test' ? [] : pageFiles(new URL(`${f}/`, dir));
    return /\.(css|js|html)$/.test(f) ? [u] : [];
  });
}

test('no_color_literal_outside_tokens', () => {
  const files = pageFiles(new URL('../', import.meta.url)).filter((u) => !u.pathname.endsWith('/assets/css/tokens.css'));
  assert.ok(files.length > 10);
  for (const u of files) {
    const src = readFileSync(u, 'utf8');
    assert.doesNotMatch(src, /#[0-9A-Fa-f]{3,8}\b(?![\w-])|\brgba?\(|\bhsla?\(/, u.pathname);
  }
});
