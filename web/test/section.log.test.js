// @agents-index Unit test of the daily log with a fake document: the summary goes in as text, never as HTML.
import test from 'node:test';
import assert from 'node:assert/strict';
import { render } from '../assets/js/section.log.js';

/** A minimal document whose elements throw on any HTML write. */
function fakeDoc() {
  const made = [];
  const node = (tag) => {
    const n = { tag, children: [], textContent: '', className: '', append(...c) { this.children.push(...c); }, setAttribute() {} };
    Object.defineProperty(n, 'innerHTML', { set() { throw new Error('innerHTML write'); } });
    made.push(n);
    return n;
  };
  return { made, createElement: node, createElementNS: (_, t) => node(t) };
}

test('summary_is_text', () => {
  const doc = fakeDoc();
  const evil = '<img src=x onerror=alert(1)>';
  render(doc, { log: [{ day: '2026-10-20', summary: evil, prs: 1, tokens: 10 }, { day: '2026-10-19', summary: null, prs: 0, tokens: 0 }] });
  assert.equal(doc.made.filter((n) => n.tag === 'img').length, 0);
  const texts = doc.made.map((n) => n.textContent);
  assert.ok(texts.includes(evil));
  assert.ok(texts.includes('No summary'));
});

test('null_prs_omits_pr_part', () => {
  const doc = fakeDoc();
  render(doc, { log: [{ day: '2026-10-20', summary: 's', prs: null, tokens: 10 }] });
  assert.ok(!doc.made.some((n) => n.textContent.endsWith(' PRs')));
  assert.ok(doc.made.some((n) => n.textContent.endsWith(' tokens')));
});

test('number_prs_shows_pr_part', () => {
  const doc = fakeDoc();
  render(doc, { log: [{ day: '2026-10-20', summary: 's', prs: 0, tokens: 10 }] });
  assert.ok(doc.made.some((n) => n.textContent === '0 PRs'));
});
