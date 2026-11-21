import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { checkDocs } from './verify-markdown-links.mjs';

function fixture(fn) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'modbus-markdown-check-'));
  try { return fn(root); } finally { rmSync(root, { recursive: true, force: true }); }
}

test('valid relative link and local image pass; external and fragments ignored', () => fixture(root => {
  mkdirSync(path.join(root, 'docs')); mkdirSync(path.join(root, 'img'));
  writeFileSync(path.join(root, 'README.md'), '[Docs](docs/guide.md) [outside](https://example.org) [anchor](#foo)\n');
  writeFileSync(path.join(root, 'docs', 'guide.md'), '[Home](../README.md) ![img](../img/icon.png)\n');
  writeFileSync(path.join(root, 'img', 'icon.png'), 'data');
  assert.deepEqual(checkDocs(root).issues, []);
}));

test('broken targets fail, but fenced code examples are ignored', () => fixture(root => {
  writeFileSync(path.join(root, 'README.md'), '[missing](none.md)\n\n```md\n[example](fictitious.md)\n```\n');
  assert.deepEqual(checkDocs(root).issues.map(x=>x.target), ['none.md']);
}));

test('rejects path traversal and malformed percent-encoding', () => fixture(root => {
  writeFileSync(path.join(root, 'README.md'), '[outside](../../outside.md) [bad](%ZZ)\n');
  assert.equal(checkDocs(root).issues.length, 2);
}));

test('accepts encoded filenames and query or heading suffixes', () => fixture(root => {
  writeFileSync(path.join(root, 'README.md'), '[doc](hello%20world.md#heading)\n');
  writeFileSync(path.join(root, 'hello world.md'), '# Hi');
  assert.deepEqual(checkDocs(root).issues, []);
}));
