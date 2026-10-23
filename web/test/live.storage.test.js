// @agents-index Unit test that the live modules read no storage API and no cookie.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';

const dir = new URL('../assets/js/', import.meta.url);
const files = readdirSync(dir).filter((f) => f.startsWith('live.'));

test('no_storage_access', () => {
  // Only the scheme modules use localStorage, for one key.
  assert.ok(files.length >= 4);
  for (const f of files) {
    const src = readFileSync(new URL(f, dir), 'utf8');
    assert.doesNotMatch(src, /localStorage|sessionStorage|indexedDB|cookie/i, f);
  }
});
