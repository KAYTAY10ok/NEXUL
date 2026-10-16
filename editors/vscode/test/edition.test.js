'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const {
  editionForFile,
  editionFromFileHeader,
  editionFromManifest
} = require('../src/edition');

test('file edition marker is read only from the first three lines', () => {
  assert.equal(editionFromFileHeader('// edition: 1\nfn main() {}'), '1');
  assert.equal(editionFromFileHeader('// title\r\n// edition: 0.2\r\nfn main() {}'), '0.2');
  assert.equal(editionFromFileHeader('// title\n// note\n// edition: 1\n// edition: 0.2'), '1');
  assert.equal(editionFromFileHeader('// title\n// note\n// third\n// edition: 1'), undefined);
  assert.equal(editionFromFileHeader('// edition: invalid'), undefined);
});

test('manifest edition is read only from the package table', () => {
  assert.equal(editionFromManifest('[package]\nname = "demo"\nedition = "1"'), '1');
  assert.equal(editionFromManifest('[dependencies]\nedition = "1"\n[package]\nedition = "0.2"'), '0.2');
  assert.equal(editionFromManifest('[package]\nedition = "1" # comment'), undefined);
  assert.equal(editionFromManifest('[package]\nname = "demo"'), undefined);
});

test('file header overrides the nearest project edition', () => {
  const filePath = path.join(os.tmpdir(), 'nexul-edition-1', 'src', 'main.nx');
  assert.equal(editionForFile('// edition: 1', filePath), '1');
  assert.equal(editionForFile('fn main() {}', filePath), '0.2');
});

test('project edition follows the nearest manifest and a file header takes precedence', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'nexul-edition-test-'));
  try {
    const source = path.join(root, 'src', 'main.nx');
    const nested = path.join(root, 'nested');
    fs.mkdirSync(path.dirname(source), { recursive: true });
    fs.mkdirSync(nested, { recursive: true });
    fs.writeFileSync(path.join(root, 'nexul.toml'), '[package]\nname = "outer"\nedition = "1"\n');
    assert.equal(editionForFile('fn main() {}', source), '1');
    fs.writeFileSync(path.join(nested, 'nexul.toml'), '[package]\nname = "inner"\nedition = "0.2"\n');
    assert.equal(editionForFile('fn main() {}', path.join(nested, 'main.nx')), '0.2');
    assert.equal(editionForFile('// edition: 1\nfn main() {}', path.join(nested, 'main.nx')), '1');
  } finally {
    const tempRoot = path.resolve(os.tmpdir());
    const resolvedRoot = path.resolve(root);
    if (path.dirname(resolvedRoot) !== tempRoot) throw new Error('Refusing to remove a directory outside the temporary root');
    fs.rmSync(resolvedRoot, { recursive: true, force: true });
  }
});
