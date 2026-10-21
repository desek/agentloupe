// @agents-index Unit tests of the pull request card: the lines footer line format and the GitHub-null states.
import test from 'node:test';
import assert from 'node:assert/strict';
import { linesParts, signed } from '../assets/js/format.lines.js';
import { render } from '../assets/js/section.prs.js';

/** A minimal document that records every element it makes. */
function fakeDoc() {
  const made = [];
  const node = (tag) => {
    const n = { tag, children: [], textContent: '', className: '', append(...c) { this.children.push(...c); }, setAttribute() {} };
    made.push(n);
    return n;
  };
  return { made, createElement: node, createElementNS: (_, t) => node(t) };
}

const totals = { prsOpened: 3, prsMerged: 2, linesAdded: 12345, linesRemoved: 678 };
const view = (github) => ({ github, totals, weeks: [{ week: '2026-W43', start: '2026-10-19', opened: 3, merged: 2 }], from: '2026-07-22', to: '2026-10-20' });

test('footer_line_format', () => {
  const [a, r] = linesParts(12345, 678);
  assert.deepEqual([a.value, a.label, a.cls], ['12\u2009345', 'lines added', 'c-added']);
  assert.deepEqual([r.value, r.label, r.cls], ['678', 'lines removed', 'c-removed']);
  assert.equal(signed(-1200), '\u22121\u2009200');
});

test('connected_has_legend_chart_footer', () => {
  const doc = fakeDoc();
  const c = render(doc, view('connected'));
  assert.equal(c.className, 'card wide');
  assert.ok(doc.made.some((n) => n.className === 'pr-legend'));
  assert.ok(doc.made.some((n) => n.className === 'note pr-footer'));
  assert.equal(c.children.at(-1).className, 'note pr-footer');
  const texts = doc.made.map((n) => n.textContent);
  assert.ok(texts.includes(' lines added') && texts.includes(' lines removed'));
  assert.ok(!texts.some((t) => /net/i.test(t)));
});

test('not_connected_shows_only_note', () => {
  const doc = fakeDoc();
  const c = render(doc, view('not_connected'));
  assert.deepEqual(c.children.map((n) => n.textContent), ['Pull requests', 'GitHub not connected']);
  assert.ok(!doc.made.some((n) => /pr-legend|pr-footer/.test(n.className)));
});

test('partial_shows_one_note', () => {
  const doc = fakeDoc();
  render(doc, view('partial'));
  assert.equal(doc.made.filter((n) => n.textContent === 'Partial GitHub data').length, 1);
  assert.ok(doc.made.some((n) => n.className === 'note pr-footer'));
});
