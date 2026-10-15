'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const {
  parseDiagnostics,
  parseJsonOutput,
  parseLocations,
  runNxc,
  scalarColumnToUtf16,
  utf16ColumnToScalar
} = require('../src/cli');

test('compiler columns and VS Code columns convert across Unicode correctly', () => {
  const line = 'é😀value';
  assert.equal(scalarColumnToUtf16(line, 1), 0);
  assert.equal(scalarColumnToUtf16(line, 3), 3);
  assert.equal(scalarColumnToUtf16(line, 20), line.length);
  assert.equal(utf16ColumnToScalar(line, 0), 1);
  assert.equal(utf16ColumnToScalar(line, 3), 3);
});

test('diagnostic JSON keeps positions, severity, labels and notes', () => {
  const items = parseDiagnostics('[{"level":"warning","code":"W1","message":"attention","file":"main.nx","line":2,"col":3,"end_line":2,"end_col":5,"labels":[{"primary":false,"message":"lié","file":"main.nx","line":1,"col":1,"end_line":1,"end_col":2}],"notes":["note: aide"]}]');
  assert.equal(items.length, 1);
  assert.equal(items[0].level, 'warning');
  assert.equal(items[0].line, 2);
  assert.equal(items[0].labels[0].message, 'lié');
  assert.throws(() => parseDiagnostics('{"error":"not an array"}'), /tableau/);
});

test('nxc ide location results accept null and arrays and report compiler errors', () => {
  assert.deepEqual(parseLocations('null'), []);
  assert.deepEqual(parseLocations('[{"file":"","line":1,"col":1}]'), [{ file: '', line: 1, col: 1 }]);
  assert.throws(() => parseLocations('{"error":"nothing to rename here"}'), /nothing to rename/);
  assert.equal(parseJsonOutput('\n', 'fallback'), 'fallback');
});

test('extension manifest registers .nx, grammar and snippets', () => {
  const manifest = JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf8'));
  const grammar = JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'syntaxes', 'nexul.tmLanguage.json'), 'utf8'));
  const snippets = JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'snippets', 'nexul.json'), 'utf8'));
  const languageConfig = JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'language-configuration.json'), 'utf8'));
  assert.ok(manifest.contributes.languages.some((language) => language.id === 'nexul' && language.extensions.includes('.nx')));
  assert.equal(grammar.scopeName, 'source.nexul');
  assert.ok(snippets['Main function']);
  assert.equal(languageConfig.comments.lineComment, '//');
});

test('nxc is spawned without a shell and preserves paths containing spaces', async () => {
  const pathWithSpaces = 'a folder/source file.nx';
  const result = await runNxc(
    process.execPath,
    ['-e', 'process.stdout.write(process.argv.slice(1).join("|"))', pathWithSpaces, '--json'],
    { cwd: process.cwd() }
  );
  assert.equal(result.stdout, pathWithSpaces + '|--json');
  assert.equal(result.code, 0);
});

test('non-zero nxc exits retain code and output for diagnostics', async () => {
  await assert.rejects(
    () => runNxc(process.execPath, ['-e', 'process.stderr.write("compile failed"); process.exit(1)'], {}),
    (error) => error.code === 'NXC_EXIT' && error.exitCode === 1 && error.stderr === 'compile failed'
  );
});
