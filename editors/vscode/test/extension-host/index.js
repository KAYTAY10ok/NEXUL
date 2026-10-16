'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vscode = require('vscode');

async function run() {
  const compiler = process.env.NEXUL_NXC;
  assert.ok(compiler, 'Set NEXUL_NXC to a built nxc executable before running npm run test:host.');
  assert.ok(fs.existsSync(compiler), 'NEXUL_NXC must point to an existing executable.');

  const extension = vscode.extensions.getExtension('KAYTAY10ok.nexul-language');
  assert.ok(extension, 'The development extension must be available to the Extension Host.');
  await extension.activate();

  const folder = vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders[0];
  assert.ok(folder, 'The edition 1 fixture workspace must be open.');
  const file = path.join(folder.uri.fsPath, 'src', 'main.nx');
  const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
  await vscode.window.showTextDocument(document);
  await waitFor(() => document.languageId === 'nexul1', 'NEXUL Edition 1 language mode');

  await waitForAsync(async () => {
    const codeLenses = await vscode.commands.executeCommand('vscode.executeCodeLensProvider', document.uri);
    return codeLenses.some((lens) => lens.command.command === 'nexul.runTest'
      && lens.command.arguments[1] === 'addition');
  }, 'nexul/tests CodeLens');
  assert.deepEqual(vscode.languages.getDiagnostics(document.uri), [],
    'valid Edition 1 source must not receive false LSP errors');
  const editorSettings = vscode.workspace.getConfiguration('editor', document.uri);
  assert.equal(editorSettings.get('defaultFormatter'), extension.id,
    'the NEXUL extension must be the default Edition 1 formatter');
  assert.equal(editorSettings.get('formatOnSave'), true,
    'Edition 1 formatting must be enabled on save by default');

  await vscode.commands.executeCommand('nexul.checkCurrentFile');

  for (const command of ['build', 'run', 'test']) {
    const exitCode = await runNexulTask(command, file);
    assert.equal(exitCode, 0, 'nxc ' + command + ' must succeed on the Edition 1 project');
  }
  console.log('Edition 1 detection, check, build, run and test passed.');

  const originalText = document.getText();
  const unformattedText = originalText
    .replace('fn main() {', 'fn main(){')
    .replace('    println(', 'println(')
    .replace('test "addition" {', 'test "addition"{')
    .replace('    assert(2 + 2 == 4)', 'assert(2+2==4)');
  const unformattedEdit = new vscode.WorkspaceEdit();
  unformattedEdit.replace(document.uri,
    new vscode.Range(document.positionAt(0), document.positionAt(originalText.length)), unformattedText);
  await vscode.workspace.applyEdit(unformattedEdit);
  await document.save();
  assert.notEqual(document.getText(), unformattedText, 'saving Edition 1 source must format it automatically');
  assert.match(document.getText(), /fn main\(\) \{/);
  assert.match(document.getText(), /    println\(/);

  const legacyFile = path.join(folder.uri.fsPath, 'src', 'legacy.nx');
  const legacyDocument = await vscode.workspace.openTextDocument(vscode.Uri.file(legacyFile));
  await vscode.window.showTextDocument(legacyDocument);
  await waitFor(() => legacyDocument.languageId === 'nexul', 'NEXUL 0.2 language mode');
  const unformatted = legacyDocument.getText();
  await vscode.commands.executeCommand('nexul.formatDocument');
  assert.notEqual(legacyDocument.getText(), unformatted, 'nxc formatting must work for Edition 0.2');
  await vscode.commands.executeCommand('nexul.checkCurrentFile');

  const validLegacy = legacyDocument.getText();
  const badStatement = 'let broken = ;';
  const badText = validLegacy.replace('println(', badStatement + ' // ');
  const badEdit = new vscode.WorkspaceEdit();
  badEdit.replace(legacyDocument.uri,
    new vscode.Range(legacyDocument.positionAt(0), legacyDocument.positionAt(validLegacy.length)), badText);
  await vscode.workspace.applyEdit(badEdit);
  await waitFor(() => vscode.languages.getDiagnostics(legacyDocument.uri).some((item) => item.severity === vscode.DiagnosticSeverity.Error),
    'unsaved Edition 0.2 LSP diagnostics');
  const restoreLegacy = new vscode.WorkspaceEdit();
  restoreLegacy.replace(legacyDocument.uri,
    new vscode.Range(legacyDocument.positionAt(0), legacyDocument.positionAt(legacyDocument.getText().length)), validLegacy);
  await vscode.workspace.applyEdit(restoreLegacy);
  await waitFor(() => vscode.languages.getDiagnostics(legacyDocument.uri).length === 0,
    'Edition 0.2 LSP diagnostics to clear after correction');
  console.log('LSP CodeLens, live diagnostics and automatic format-on-save passed for Editions 0.2 and 1.');
}

function waitFor(predicate, label) {
  const deadline = Date.now() + 10000;
  return new Promise((resolve, reject) => {
    const poll = () => {
      if (predicate()) return resolve();
      if (Date.now() >= deadline) return reject(new Error('Timed out waiting for ' + label));
      setTimeout(poll, 50);
    };
    poll();
  });
}

function waitForAsync(predicate, label) {
  const deadline = Date.now() + 10000;
  return new Promise((resolve, reject) => {
    const poll = async () => {
      try {
        if (await predicate()) return resolve();
        if (Date.now() >= deadline) return reject(new Error('Timed out waiting for ' + label));
        setTimeout(poll, 50);
      } catch (error) {
        reject(error);
      }
    };
    poll();
  });
}

function runNexulTask(command, file) {
  return new Promise((resolve, reject) => {
    let timer;
    const subscription = vscode.tasks.onDidEndTaskProcess((event) => {
      const definition = event.execution.task.definition;
      if (definition.type !== 'nexul' || definition.command !== command
        || path.resolve(definition.file) !== path.resolve(file)) return;
      clearTimeout(timer);
      subscription.dispose();
      resolve(event.exitCode);
    });
    timer = setTimeout(() => {
      subscription.dispose();
      reject(new Error('Timed out waiting for the nxc ' + command + ' task.'));
    }, 60000);
    vscode.commands.executeCommand('nexul.' + command + 'CurrentFile').catch((error) => {
      clearTimeout(timer);
      subscription.dispose();
      reject(error);
    });
  });
}

module.exports = { run };
