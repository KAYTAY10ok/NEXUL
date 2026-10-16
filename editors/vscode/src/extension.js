'use strict';

const path = require('node:path');
const vscode = require('vscode');
const { LanguageClient } = require('vscode-languageclient/node');
const { parseDiagnostics, runNxc } = require('./cli');
const { editionForFile } = require('./edition');
const { createCliTask, registerTaskProvider } = require('./tasks');

let deactivateLanguageClient = async () => {};

function nexulProtocolUri(uri) {
  return uri.toString().replace(/^file:\/\/\/([a-z])%3[Aa]/i, (_match, drive) => {
    return 'file:///' + drive.toUpperCase() + ':';
  });
}

function activate(context) {
  const output = vscode.window.createOutputChannel('NEXUL');
  const codeLensEmitter = new vscode.EventEmitter();
  const nexulSelector = [{ language: 'nexul' }, { language: 'nexul1' }];
  const synchronizedText = new Map();
  let codeLensRefreshTimer;
  let missingCompilerWasReported = false;
  let languageClient;
  let startingClient;
  context.subscriptions.push(output, codeLensEmitter);

  function executable() {
    const configured = vscode.workspace.getConfiguration('nexul').get('nxcPath', 'nxc');
    return typeof configured === 'string' && configured.trim() ? configured.trim() : 'nxc';
  }

  function workspaceFolder(document) {
    return vscode.workspace.getWorkspaceFolder(document.uri);
  }

  function workingDirectory(document) {
    const folder = workspaceFolder(document);
    return folder ? folder.uri.fsPath : path.dirname(document.uri.fsPath);
  }

  function isNexul(document) {
    return document.languageId === 'nexul'
      || document.languageId === 'nexul1'
      || path.extname(document.uri.fsPath).toLowerCase() === '.nx';
  }

  function documentEdition(document) {
    return editionForFile(document.getText(), document.uri.fsPath);
  }

  async function updateEditionLanguage(document) {
    if (path.extname(document.uri.fsPath).toLowerCase() !== '.nx') return document;
    const language = documentEdition(document) === '1' ? 'nexul1' : 'nexul';
    if (document.languageId === language) return document;
    return vscode.languages.setTextDocumentLanguage(document, language);
  }

  function reportFailure(operation, error) {
    if (!error || error.name === 'CancellationError' || error.code === 'NEXUL_UNTRUSTED_WORKSPACE') return;
    const details = error.stderr || error.message || String(error);
    output.appendLine('[' + operation + '] ' + details.trim());
    console.error('[NEXUL ' + operation + '] ' + details.trim());
    if (error.code === 'NEXUL_EDITION1_FORMAT_UNAVAILABLE') {
      vscode.window.showInformationMessage(error.message);
      return;
    }
    if ((error.code === 'ENOENT' || error.code === 'NXC_EXIT') && !missingCompilerWasReported) {
      missingCompilerWasReported = true;
      vscode.window.showErrorMessage(
        'nxc est introuvable ou ne peut pas démarrer son serveur LSP. Installez une version récente de NEXUL ou configurez nexul.nxcPath.',
        'Ouvrir les paramètres'
      ).then((choice) => {
        if (choice) vscode.commands.executeCommand('workbench.action.openSettings', 'nexul.nxcPath');
      });
    }
  }

  function lspUnavailableError() {
    const error = new Error('Le serveur de langage nxc n’est pas disponible.');
    error.code = 'NEXUL_LSP_UNAVAILABLE';
    return error;
  }

  function sendLspRequest(method, params, token) {
    return token === undefined
      ? languageClient.sendRequest(method, params)
      : languageClient.sendRequest(method, params, token);
  }

  async function formattingEdits(document, token) {
    if (!vscode.workspace.isTrusted) return [];
    if (!languageClient || !languageClient.isRunning()) throw lspUnavailableError();
    const response = await sendLspRequest('textDocument/formatting', {
      textDocument: { uri: document.uri.toString() },
      options: { tabSize: 4, insertSpaces: true }
    }, token);
    if (!Array.isArray(response)) return [];
    return response.map((item) => vscode.TextEdit.replace(
      new vscode.Range(
        new vscode.Position(item.range.start.line, item.range.start.character),
        new vscode.Position(item.range.end.line, item.range.end.character)
      ),
      item.newText
    ));
  }

  async function checkCurrentFile() {
    if (!vscode.workspace.isTrusted) return;
    const editor = vscode.window.activeTextEditor;
    if (!editor || !isNexul(editor.document)) {
      vscode.window.showInformationMessage('Ouvrez un fichier .nx pour utiliser NEXUL.');
      return;
    }
    if (editor.document.isDirty && !(await editor.document.save())) return;
    try {
      const result = await runNxc(executable(), ['check', editor.document.uri.fsPath, '--json'], {
        cwd: workingDirectory(editor.document),
        allowedExitCodes: [0, 1]
      });
      const items = parseDiagnostics(result.stdout);
      const errors = items.filter((item) => item.level === 'error').length;
      const warnings = items.filter((item) => item.level === 'warning').length;
      output.appendLine('Vérification explicite : ' + errors + ' erreur(s), ' + warnings + ' avertissement(s).');
      vscode.window.showInformationMessage(
        errors || warnings
          ? 'Vérification terminée : ' + errors + ' erreur(s), ' + warnings + ' avertissement(s). Consultez le panneau Sortie NEXUL.'
          : 'Vérification NEXUL terminée sans erreur.'
      );
    } catch (error) {
      reportFailure('nxc check', error);
    }
  }

  async function formatCurrentDocument() {
    if (!vscode.workspace.isTrusted) return;
    const editor = vscode.window.activeTextEditor;
    if (!editor || !isNexul(editor.document)) {
      vscode.window.showInformationMessage('Ouvrez un fichier .nx pour utiliser NEXUL.');
      return;
    }
    try {
      const edits = await formattingEdits(editor.document);
      if (!edits.length) return;
      const edit = new vscode.WorkspaceEdit();
      for (const item of edits) edit.replace(editor.document.uri, item.range, item.newText);
      await vscode.workspace.applyEdit(edit);
    } catch (error) {
      reportFailure('nxc LSP formatting', error);
    }
  }

  async function runCurrentTask(command, filter) {
    if (!vscode.workspace.isTrusted) return;
    const editor = vscode.window.activeTextEditor;
    if (!editor || !isNexul(editor.document)) {
      vscode.window.showInformationMessage('Ouvrez un fichier .nx pour utiliser NEXUL.');
      return;
    }
    if (editor.document.isDirty && !(await editor.document.save())) return;
    const document = editor.document;
    const task = createCliTask(
      vscode,
      executable(),
      workspaceFolder(document),
      command,
      document.uri.fsPath,
      filter || null,
      workingDirectory(document)
    );
    await vscode.tasks.executeTask(task);
  }

  async function runSpecificTest(file, name) {
    if (!vscode.workspace.isTrusted) return;
    if (!file || !name) return;
    let document;
    try {
      document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
    } catch (error) {
      reportFailure('ouvrir le fichier de test', error);
      return;
    }
    if (document.isDirty && !(await document.save())) return;
    const task = createCliTask(
      vscode,
      executable(),
      workspaceFolder(document),
      'test',
      file,
      name,
      workingDirectory(document)
    );
    await vscode.tasks.executeTask(task);
  }

  function startLanguageClient() {
    if (!vscode.workspace.isTrusted || languageClient) return Promise.resolve();
    if (startingClient) return startingClient;

    const root = vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders[0];
    const serverArguments = ['lsp'];
    const lspLog = typeof process !== 'undefined' && process.env && process.env.NEXUL_LSP_LOG;
    if (lspLog) serverArguments.push('--log', lspLog);
    const serverOptions = {
      command: executable(),
      args: serverArguments
    };
    if (root) serverOptions.options = { cwd: root.uri.fsPath };
    const clientOptions = {
      documentSelector: nexulSelector.map((selector) => ({ scheme: 'file', language: selector.language })),
      outputChannel: output,
      workspaceFolder: root,
      uriConverters: {
        code2Protocol: nexulProtocolUri,
        protocol2Code: (uri) => vscode.Uri.parse(uri)
      },
      middleware: {
        didOpen(document, next) {
          synchronizedText.set(document.uri.toString(), document.getText());
          return next(document);
        },
        didChange(event, next) {
          const uri = event.document.uri.toString();
          let eventToSend = event;
          if (event.contentChanges.length > 1) {
            const previousText = synchronizedText.get(uri);
            if (previousText !== undefined) {
              const previousLines = previousText.split('\n');
              const lastLine = previousLines[previousLines.length - 1].replace(/\r$/, '');
              eventToSend = {
                ...event,
                contentChanges: [{
                  range: new vscode.Range(
                    new vscode.Position(0, 0),
                    new vscode.Position(previousLines.length - 1, lastLine.length)
                  ),
                  rangeOffset: 0,
                  rangeLength: previousText.length,
                  text: event.document.getText()
                }]
              };
            }
          }
          const result = next(eventToSend);
          if (result && typeof result.then === 'function') {
            return result.finally(() => synchronizedText.set(uri, event.document.getText()));
          }
          synchronizedText.set(uri, event.document.getText());
          return result;
        },
        didClose(document, next) {
          synchronizedText.delete(document.uri.toString());
          return next(document);
        },
        handleDiagnostics(uri, diagnostics, next) {
          next(uri, diagnostics);
        },
        provideDocumentFormattingEdits(document, options, token, next) {
          if (!vscode.workspace.isTrusted) return [];
          return next(document, options, token);
        }
      }
    };
    const client = new LanguageClient('nexul', 'NEXUL Language Server', serverOptions, clientOptions);
    languageClient = client;
    startingClient = client.start().catch((error) => {
      if (languageClient === client) languageClient = undefined;
      reportFailure('démarrage de nxc lsp', error);
    }).finally(() => {
      startingClient = undefined;
    });
    return startingClient;
  }

  async function restartLanguageClient() {
    if (startingClient) await startingClient;
    const previous = languageClient;
    languageClient = undefined;
    if (previous) await previous.stop();
    await startLanguageClient();
  }

  async function provideStandardLibrarySource(uri, token) {
    if (!vscode.workspace.isTrusted || !languageClient || !languageClient.isRunning()) {
      throw lspUnavailableError();
    }
    const response = await sendLspRequest('nexul/stdSource', { uri: uri.toString() }, token);
    if (!response || typeof response.text !== 'string') throw new Error('Réponse source nxc invalide.');
    return response.text;
  }

  const codeLensProvider = {
    onDidChangeCodeLenses: codeLensEmitter.event,
    async provideCodeLenses(document, token) {
      if (!vscode.workspace.isTrusted || !isNexul(document)) return [];
      if (!languageClient || !languageClient.isRunning()) return [];
      try {
        const response = await sendLspRequest('nexul/tests', {
          textDocument: { uri: document.uri.toString() }
        }, token);
        if (!Array.isArray(response)) return [];
        return response.filter((item) => item && item.name && item.range && item.range.start)
          .map((item) => new vscode.CodeLens(
            new vscode.Range(item.range.start.line, 0, item.range.start.line, 0),
            {
              title: 'Exécuter le test',
              command: 'nexul.runTest',
              arguments: [document.uri.fsPath, item.name]
            }
          ));
      } catch (error) {
        reportFailure('nxc LSP tests', error);
        return [];
      }
    }
  };

  registerTaskProvider(vscode, context, executable);
  context.subscriptions.push(
    vscode.languages.registerCodeLensProvider(nexulSelector, codeLensProvider),
    vscode.workspace.registerTextDocumentContentProvider('nexul-std', {
      provideTextDocumentContent: provideStandardLibrarySource
    }),
    vscode.commands.registerCommand('nexul.checkCurrentFile', checkCurrentFile),
    vscode.commands.registerCommand('nexul.buildCurrentFile', () => runCurrentTask('build')),
    vscode.commands.registerCommand('nexul.runCurrentFile', () => runCurrentTask('run')),
    vscode.commands.registerCommand('nexul.testCurrentFile', () => runCurrentTask('test')),
    vscode.commands.registerCommand('nexul.formatDocument', formatCurrentDocument),
    vscode.commands.registerCommand('nexul.runTest', runSpecificTest),
    vscode.workspace.onDidGrantWorkspaceTrust(() => {
      codeLensEmitter.fire();
      void startLanguageClient();
    }),
    vscode.workspace.onDidOpenTextDocument((document) => {
      void updateEditionLanguage(document)
        .catch((error) => reportFailure('détection de l’édition', error));
      codeLensEmitter.fire();
    }),
    vscode.workspace.onDidSaveTextDocument((document) => {
      if (path.basename(document.uri.fsPath).toLowerCase() === 'nexul.toml') {
        for (const openDocument of vscode.workspace.textDocuments) {
          if (isNexul(openDocument)) {
            void updateEditionLanguage(openDocument)
              .catch((error) => reportFailure('détection de l’édition', error));
          }
        }
      } else if (isNexul(document)) {
        void updateEditionLanguage(document)
          .catch((error) => reportFailure('détection de l’édition', error));
      }
    }),
    vscode.workspace.onDidChangeTextDocument((event) => {
      if (!isNexul(event.document)) return;
      if (event.contentChanges.some((change) => change.range.start.line < 3 || change.range.end.line < 3)) {
        void updateEditionLanguage(event.document)
          .catch((error) => reportFailure('détection de l’édition', error));
      }
      if (codeLensRefreshTimer) clearTimeout(codeLensRefreshTimer);
      codeLensRefreshTimer = setTimeout(() => codeLensEmitter.fire(), 300);
    }),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration('nexul.nxcPath') && vscode.workspace.isTrusted) {
        void restartLanguageClient().catch((error) => reportFailure('redémarrage de nxc lsp', error));
      }
    }),
    new vscode.Disposable(() => {
      if (codeLensRefreshTimer) clearTimeout(codeLensRefreshTimer);
    }),
    new vscode.Disposable(() => {
      const current = languageClient;
      languageClient = undefined;
      if (current) void current.stop();
    })
  );

  deactivateLanguageClient = async () => {
    const current = languageClient;
    languageClient = undefined;
    if (current) await current.stop();
  };

  const updateLanguages = Promise.all(vscode.workspace.textDocuments.map(updateEditionLanguage));
  return updateLanguages.then(() => startLanguageClient());
}

function deactivate() {
  return deactivateLanguageClient();
}

module.exports = {
  activate,
  deactivate
};
