// @agents-index Unit test that the live modules are plain ES modules that import only relative page modules.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';

const dir = new URL('../assets/js/', import.meta.url);

test('plain_es_modules', () => {
  for (const f of readdirSync(dir).filter((x) => x.startsWith('live.'))) {
    const src = readFileSync(new URL(f, dir), 'utf8');
    assert.doesNotMatch(src, /require\(/, f);
    for (const m of src.matchAll(/import[^'"]*['"]([^'"]+)['"]/g)) assert.match(m[1], /^\.\/[a-z.]+\.js$/, f);
  }
});
