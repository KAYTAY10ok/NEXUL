'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');
const cli = require('../src/cli');
const tasks = require('../src/tasks');

const manifest = require('../package.json');

function activateWithWorkspace(isTrusted) {
  const callbacks = new Map();
  const commands = new Map();
  const providers = new Map();
  const compilerCalls = [];
  const executedTasks = [];
  const pathReads = [];
  const folderCompiler = path.join(process.cwd(), 'untrusted repository', 'folder-nxc.exe');
  const folder = { uri: { fsPath: path.dirname(folderCompiler) } };
  const document = {
    languageId: 'nexul',
    uri: { fsPath: path.join(process.cwd(), 'untrusted repository', 'main.nx') },
    isDirty: false,
    lineCount: 1,
    getText: () => 'fn main() {}',
    lineAt: () => ({ text: 'fn main() {}' })
  };
  const disposable = () => ({ dispose() {} });
  const listen = (name) => (callback) => {
    callbacks.set(name, callback);
    return disposable();
  };
  let codeLensRefreshes = 0;
  const vscode = {
    EventEmitter: class {
      event = disposable;
      fire() { codeLensRefreshes += 1; }
      dispose() {}
    },
    Disposable: class {
      constructor(callback) { this.dispose = callback; }
    },
    RelativePattern: class {},
    Task: class {
      constructor(definition, scope, name, source, execution) {
        Object.assign(this, { definition, scope, name, source, execution });
      }
    },
    ProcessExecution: class {
      constructor(executable, args, options) {
        Object.assign(this, { executable, args, options });
      }
    },
    TaskScope: { Global: 1 },
    TaskRevealKind: { Always: 1 },
    TaskPanelKind: { Dedicated: 1 },
    TaskGroup: { Build: 'build', Test: 'test' },
    CompletionItemKind: {},
    Uri: { file: (fsPath) => ({ fsPath }) },
    window: {
      activeTextEditor: { document },
      createOutputChannel: () => ({ appendLine() {}, dispose() {} }),
      showInformationMessage: async () => undefined
    },
    languages: {
      createDiagnosticCollection: () => ({ clear() {}, set() {}, dispose() {} })
    },
    commands: {
      registerCommand: (name, callback) => {
        commands.set(name, callback);
        return disposable();
      }
    },
    tasks: {
      registerTaskProvider: (_type, provider) => {
        providers.set('tasks', provider);
        return disposable();
      },
      executeTask: async (task) => { executedTasks.push(task); }
    },
    workspace: {
      isTrusted,
      textDocuments: [document],
      workspaceFolders: [folder],
      getConfiguration: () => ({
        get: (key, fallback) => {
          if (key === 'nxcPath') {
            pathReads.push(folderCompiler);
            // Deliberately expose the hostile folder override even in Restricted
            // Mode: the execution guards must also work without host filtering.
            return folderCompiler;
          }
          if (key === 'checkOnSave' || key === 'formatOnSave') return true;
          return fallback;
        }
      }),
      getWorkspaceFolder: () => folder,
      openTextDocument: async () => document,
      findFiles: async () => [{ fsPath: path.join(folder.uri.fsPath, 'nexul.toml') }],
      onDidGrantWorkspaceTrust: listen('trust'),
      onDidOpenTextDocument: listen('open'),
      onDidSaveTextDocument: listen('save'),
      onDidCloseTextDocument: listen('close'),
      onDidChangeTextDocument: listen('change'),
      onWillSaveTextDocument: listen('willSave')
    }
  };
  for (const [registration, name] of [
    ['registerCodeLensProvider', 'codeLens'],
    ['registerCompletionItemProvider', 'completion'],
    ['registerHoverProvider', 'hover'],
    ['registerDefinitionProvider', 'definition'],
    ['registerReferenceProvider', 'references'],
    ['registerRenameProvider', 'rename'],
    ['registerDocumentFormattingEditProvider', 'formatting']
  ]) {
    vscode.languages[registration] = (_selector, provider) => {
      providers.set(name, provider);
      return disposable();
    };
  }
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../src/extension.js'), 'utf8'), {
    module,
    exports: module.exports,
    setTimeout,
    clearTimeout,
    require: (name) => {
      if (name === 'vscode') return vscode;
      if (name === './tasks') return tasks;
      if (name === './cli') return {
        ...cli,
        runNxc: async (executable, args) => {
          compilerCalls.push({ executable, args });
          return { stdout: args[0] === 'check' ? '[]' : (args[1] === 'format' ? document.getText() : 'null') };
        }
      };
      return require(name);
    }
  }, { filename: 'extension.js' });
  const context = { subscriptions: [] };
  module.exports.activate(context);
  return {
    vscode, callbacks, commands, providers, document, compilerCalls, executedTasks,
    pathReads, folderCompiler, codeLensRefreshes: () => codeLensRefreshes,
    dispose: () => context.subscriptions.forEach((subscription) => subscription.dispose())
  };
}

test('manifest restricts workspace compiler overrides and execution commands by trust', () => {
  assert.equal(manifest.contributes.configuration.properties['nexul.nxcPath'].scope, 'machine-overridable');
  assert.equal(manifest.capabilities.untrustedWorkspaces.supported, 'limited');
  assert.ok(manifest.capabilities.untrustedWorkspaces.restrictedConfigurations.includes('nexul.nxcPath'));
  assert.ok(manifest.capabilities.untrustedWorkspaces.description);
  for (const command of manifest.contributes.commands) {
    assert.equal(command.enablement, 'isWorkspaceTrusted', command.command);
  }
});

test('untrusted folder compiler is never used by editor hooks, providers, commands or tasks', async (t) => {
  const host = activateWithWorkspace(false);
  t.after(host.dispose);
  const { callbacks, commands, providers, document } = host;
  callbacks.get('open')(document);
  callbacks.get('save')(document);
  let formatOnSave;
  callbacks.get('willSave')({ document, waitUntil: (result) => { formatOnSave = result; } });
  await formatOnSave;
  const position = { line: 0, character: 0 };
  await providers.get('completion').provideCompletionItems(document, position);
  await providers.get('hover').provideHover(document, position);
  await providers.get('definition').provideDefinition(document, position);
  await providers.get('references').provideReferences(document, position, {});
  await providers.get('rename').provideRenameEdits(document, position, 'renamed');
  await providers.get('formatting').provideDocumentFormattingEdits(document, {});
  await providers.get('codeLens').provideCodeLenses(document);
  for (const [name, command] of commands) {
    await (name === 'nexul.runTest' ? command(document.uri.fsPath, 'test_name') : command());
  }
  assert.equal((await providers.get('tasks').provideTasks()).length, 0);
  assert.equal(providers.get('tasks').resolveTask({ definition: { type: 'nexul', command: 'run' } }), undefined);
  assert.equal(host.pathReads.length, 0, 'the untrusted folder setting must not be consumed');
  assert.equal(host.compilerCalls.length, 0, 'no compiler or IDE process may run before trust');
  assert.equal(host.executedTasks.length, 0, 'programs and tests must not run before trust');
});

test('granting trust resumes open-document checks and enables compiler tasks', async (t) => {
  const host = activateWithWorkspace(false);
  t.after(host.dispose);
  host.vscode.workspace.isTrusted = true;
  host.callbacks.get('trust')();
  assert.equal(host.codeLensRefreshes(), 1);
  assert.equal(host.compilerCalls.length, 1);
  assert.equal(host.compilerCalls[0].executable, host.folderCompiler);
  assert.equal(host.compilerCalls[0].args[0], 'check');
  await host.commands.get('nexul.buildCurrentFile')();
  assert.equal(host.executedTasks.length, 1);
  assert.equal(host.executedTasks[0].execution.executable, host.folderCompiler);
  assert.equal(host.executedTasks[0].execution.args[0], 'build');
  const resolved = host.providers.get('tasks').resolveTask({ definition: { type: 'nexul', command: 'test' } });
  assert.equal(resolved.execution.executable, host.folderCompiler);
  const provided = await host.providers.get('tasks').provideTasks();
  assert.equal(provided.length, 4);
  assert.ok(provided.every((task) => task.execution.executable === host.folderCompiler));
});
