'use strict';

const fs = require('node:fs');
const path = require('node:path');
const vscode = require('vscode');
const {
  parseDiagnostics,
  parseJsonOutput,
  runNxc,
  scalarColumnToUtf16,
  utf16ColumnToScalar
} = require('./cli');
const { editionForFile } = require('./edition');
const { createCliTask, registerTaskProvider } = require('./tasks');

function activate(context) {
  const output = vscode.window.createOutputChannel('NEXUL');
  const diagnostics = vscode.languages.createDiagnosticCollection('nexul');
  const contributions = new Map();
  const nexulSelector = [{ language: 'nexul' }, { language: 'nexul1' }];
  const codeLensEmitter = new vscode.EventEmitter();
  let codeLensRefreshTimer;
  let missingCompilerWasReported = false;
  context.subscriptions.push(output, diagnostics, codeLensEmitter);

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
    if (error.code === 'NEXUL_EDITION1_FORMAT_UNAVAILABLE') {
      vscode.window.showInformationMessage(error.message);
      return;
    }
    if (error.code === 'ENOENT' && !missingCompilerWasReported) {
      missingCompilerWasReported = true;
      vscode.window.showErrorMessage(
        'nxc est introuvable. Installez NEXUL ou configurez nexul.nxcPath.',
        'Ouvrir les paramètres'
      ).then((choice) => {
        if (choice) vscode.commands.executeCommand('workbench.action.openSettings', 'nexul.nxcPath');
      });
    }
  }

  function compilerPosition(document, position) {
    const lineText = document.lineAt(position.line).text;
    return [
      String(position.line + 1),
      String(utf16ColumnToScalar(lineText, position.character))
    ];
  }

  async function ideRequest(document, request, position, extraArgs, token) {
    if (!vscode.workspace.isTrusted) {
      const error = new Error('Les outils nxc nécessitent un espace de travail approuvé.');
      error.code = 'NEXUL_UNTRUSTED_WORKSPACE';
      throw error;
    }
    const args = ['ide', request, document.uri.fsPath];
    if (position) args.push.apply(args, compilerPosition(document, position));
    if (extraArgs && extraArgs.length) args.push.apply(args, extraArgs);
    args.push('--stdin');
    const result = await runNxc(executable(), args, {
      cwd: workingDirectory(document),
      input: document.getText(),
      token
    });
    return result.stdout;
  }

  async function safeIdeJson(document, request, position, extraArgs, token) {
    try {
      return parseJsonOutput(await ideRequest(document, request, position, extraArgs, token), null);
    } catch (error) {
      reportFailure('nxc ide ' + request, error);
      return undefined;
    }
  }

  async function formatText(document, token) {
    if (documentEdition(document) === '1') {
      const error = new Error('Le formateur nxc ne prend pas encore en charge l’édition 1.');
      error.code = 'NEXUL_EDITION1_FORMAT_UNAVAILABLE';
      throw error;
    }
    return ideRequest(document, 'format', null, [], token);
  }

  function sourceUri(document, file) {
    const name = String(file || '');
    if (!name || name === path.basename(document.uri.fsPath)) return document.uri;
    if (name.startsWith('<')) return undefined;

    const candidates = path.isAbsolute(name)
      ? [name]
      : [path.resolve(path.dirname(document.uri.fsPath), name), path.resolve(workingDirectory(document), name)];
    for (const candidate of candidates) {
      if (fs.existsSync(candidate)) return vscode.Uri.file(candidate);
    }
    return vscode.Uri.file(candidates[0]);
  }

  function compilerPositionInDocument(document, lineNumber, scalarColumn) {
    const line = Math.max(0, Math.min(document.lineCount - 1, Number(lineNumber || 1) - 1));
    const text = document.lineAt(line).text;
    const character = scalarColumnToUtf16(text, scalarColumn || 1);
    return new vscode.Position(line, character);
  }

  async function compilerRange(document, location) {
    const uri = sourceUri(document, location.file);
    if (!uri) return undefined;
    let target = document;
    if (uri.toString() !== document.uri.toString()) {
      try {
        target = await vscode.workspace.openTextDocument(uri);
      } catch (_) {
        return undefined;
      }
    }
    const start = compilerPositionInDocument(target, location.line, location.col);
    const end = compilerPositionInDocument(
      target,
      location.end_line || location.line,
      location.end_col || location.col
    );
    return { uri, range: new vscode.Range(start, end.isBefore(start) ? start : end) };
  }

  async function diagnosticFromCompiler(document, item) {
    if (item.std || (item.file && String(item.file).startsWith('<'))) return undefined;
    const primary = await compilerRange(document, item);
    if (!primary) return undefined;

    const severity = item.level === 'warning'
      ? vscode.DiagnosticSeverity.Warning
      : (item.level === 'info'
        ? vscode.DiagnosticSeverity.Information
        : vscode.DiagnosticSeverity.Error);
    const notes = Array.isArray(item.notes) && item.notes.length ? '\n\n' + item.notes.join('\n') : '';
    const diagnostic = new vscode.Diagnostic(primary.range, String(item.message || '') + notes, severity);
    diagnostic.code = item.code || undefined;
    diagnostic.source = 'nxc';

    const related = [];
    for (const label of Array.isArray(item.labels) ? item.labels : []) {
      if (label.primary || !label.message) continue;
      const location = await compilerRange(document, label);
      if (location) {
        related.push(new vscode.DiagnosticRelatedInformation(
          new vscode.Location(location.uri, location.range),
          label.message
        ));
      }
    }
    if (related.length) diagnostic.relatedInformation = related;
    return { uri: primary.uri, diagnostic };
  }

  function republishDiagnostics() {
    const merged = new Map();
    for (const contribution of contributions.values()) {
      for (const [key, entry] of contribution) {
        if (!merged.has(key)) merged.set(key, { uri: entry.uri, diagnostics: [] });
        merged.get(key).diagnostics.push.apply(merged.get(key).diagnostics, entry.diagnostics);
      }
    }
    diagnostics.clear();
    for (const entry of merged.values()) diagnostics.set(entry.uri, entry.diagnostics);
  }

  async function checkDocument(document) {
    if (!vscode.workspace.isTrusted || !isNexul(document)) return;
    const rootKey = document.uri.fsPath;
    try {
      const result = await runNxc(executable(), ['check', document.uri.fsPath, '--json'], {
        cwd: workingDirectory(document),
        allowedExitCodes: [0, 1]
      });
      const raw = parseDiagnostics(result.stdout);
      const contribution = new Map();
      for (const item of raw) {
        const entry = await diagnosticFromCompiler(document, item);
        if (!entry) continue;
        const key = entry.uri.toString();
        if (!contribution.has(key)) contribution.set(key, { uri: entry.uri, diagnostics: [] });
        contribution.get(key).diagnostics.push(entry.diagnostic);
      }
      contributions.set(rootKey, contribution);
      republishDiagnostics();
    } catch (error) {
      reportFailure('nxc check', error);
    }
  }

  async function checkCurrentFile() {
    if (!vscode.workspace.isTrusted) return;
    const editor = vscode.window.activeTextEditor;
    if (!editor || !isNexul(editor.document)) {
      vscode.window.showInformationMessage('Ouvrez un fichier .nx pour utiliser NEXUL.');
      return;
    }
    if (editor.document.isDirty && !(await editor.document.save())) return;
    await checkDocument(editor.document);
  }

  async function formatCurrentDocument() {
    if (!vscode.workspace.isTrusted) return;
    const editor = vscode.window.activeTextEditor;
    if (!editor || !isNexul(editor.document)) {
      vscode.window.showInformationMessage('Ouvrez un fichier .nx pour utiliser NEXUL.');
      return;
    }
    try {
      const formatted = await formatText(editor.document);
      if (formatted === editor.document.getText()) return;
      const edit = new vscode.WorkspaceEdit();
      edit.replace(
        editor.document.uri,
        new vscode.Range(editor.document.positionAt(0), editor.document.positionAt(editor.document.getText().length)),
        formatted
      );
      await vscode.workspace.applyEdit(edit);
    } catch (error) {
      reportFailure('nxc ide format', error);
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

  const completionKinds = {
    function: vscode.CompletionItemKind.Function,
    method: vscode.CompletionItemKind.Method,
    field: vscode.CompletionItemKind.Field,
    variable: vscode.CompletionItemKind.Variable,
    constant: vscode.CompletionItemKind.Constant,
    static: vscode.CompletionItemKind.Constant,
    type: vscode.CompletionItemKind.TypeParameter,
    struct: vscode.CompletionItemKind.Struct,
    enum: vscode.CompletionItemKind.Enum,
    trait: vscode.CompletionItemKind.Interface,
    variant: vscode.CompletionItemKind.EnumMember,
    module: vscode.CompletionItemKind.Module,
    keyword: vscode.CompletionItemKind.Keyword
  };

  const completionProvider = {
    async provideCompletionItems(document, position, token) {
      if (!isNexul(document)) return undefined;
      const response = await safeIdeJson(document, 'completion', position, [], token);
      if (!Array.isArray(response)) return undefined;

      const before = document.lineAt(position.line).text.slice(0, position.character);
      const prefix = /[A-Za-z0-9_]*$/.exec(before);
      const start = position.character - (prefix ? prefix[0].length : 0);
      const range = new vscode.Range(position.line, start, position.line, position.character);
      return response.map((raw) => {
        const item = new vscode.CompletionItem(
          String(raw.label || ''),
          completionKinds[raw.kind] || vscode.CompletionItemKind.Text
        );
        item.range = range;
        if (raw.detail) item.detail = raw.detail;
        if (raw.doc) item.documentation = new vscode.MarkdownString(raw.doc);
        return item;
      });
    }
  };

  const hoverProvider = {
    async provideHover(document, position, token) {
      if (!isNexul(document)) return undefined;
      const response = await safeIdeJson(document, 'hover', position, [], token);
      if (!response || !response.code) return undefined;
      const markdown = new vscode.MarkdownString();
      markdown.appendCodeblock(response.code, 'nexul');
      if (response.doc) markdown.appendMarkdown('\n' + response.doc);
      return new vscode.Hover(markdown);
    }
  };

  const definitionProvider = {
    async provideDefinition(document, position, token) {
      if (!isNexul(document)) return undefined;
      const response = await safeIdeJson(document, 'definition', position, [], token);
      if (!response || typeof response !== 'object') return undefined;
      const location = await compilerRange(document, response);
      return location ? new vscode.Location(location.uri, location.range) : undefined;
    }
  };

  const referenceProvider = {
    async provideReferences(document, position, _context, token) {
      if (!isNexul(document)) return undefined;
      const response = await safeIdeJson(document, 'references', position, [], token);
      if (!Array.isArray(response)) return undefined;
      const locations = await Promise.all(response.map((item) => compilerRange(document, item)));
      return locations.filter(Boolean).map((item) => new vscode.Location(item.uri, item.range));
    }
  };

  const renameProvider = {
    async provideRenameEdits(document, position, newName, token) {
      if (!isNexul(document)) return undefined;
      const response = await safeIdeJson(document, 'rename', position, [newName], token);
      if (!Array.isArray(response)) return undefined;
      const edit = new vscode.WorkspaceEdit();
      const locations = await Promise.all(response.map((item) => compilerRange(document, item)));
      for (const location of locations) {
        if (location) edit.replace(location.uri, location.range, newName);
      }
      return edit;
    }
  };

  const formattingProvider = {
    async provideDocumentFormattingEdits(document, _options, token) {
      if (!isNexul(document)) return [];
      try {
        const formatted = await formatText(document, token);
        if (formatted === document.getText()) return [];
        return [vscode.TextEdit.replace(
          new vscode.Range(document.positionAt(0), document.positionAt(document.getText().length)),
          formatted
        )];
      } catch (error) {
        reportFailure('nxc ide format', error);
        return [];
      }
    }
  };

  const codeLensProvider = {
    onDidChangeCodeLenses: codeLensEmitter.event,
    async provideCodeLenses(document, token) {
      if (!isNexul(document)) return [];
      const response = await safeIdeJson(document, 'tests', null, [], token);
      if (!Array.isArray(response)) return [];
      return response.filter((item) => Number.isInteger(item.line) && item.line > 0 && item.name).map((item) => {
        const line = Math.min(document.lineCount - 1, item.line - 1);
        return new vscode.CodeLens(
          new vscode.Range(line, 0, line, 0),
          {
            title: 'Exécuter le test',
            command: 'nexul.runTest',
            arguments: [document.uri.fsPath, item.name]
          }
        );
      });
    }
  };

  registerTaskProvider(vscode, context, executable);
  context.subscriptions.push(
    vscode.languages.registerCodeLensProvider(nexulSelector, codeLensProvider),
    vscode.commands.registerCommand('nexul.checkCurrentFile', checkCurrentFile),
    vscode.commands.registerCommand('nexul.buildCurrentFile', () => runCurrentTask('build')),
    vscode.commands.registerCommand('nexul.runCurrentFile', () => runCurrentTask('run')),
    vscode.commands.registerCommand('nexul.testCurrentFile', () => runCurrentTask('test')),
    vscode.commands.registerCommand('nexul.formatDocument', formatCurrentDocument),
    vscode.commands.registerCommand('nexul.runTest', runSpecificTest),
    vscode.languages.registerCompletionItemProvider(nexulSelector, completionProvider, '.'),
    vscode.languages.registerHoverProvider(nexulSelector, hoverProvider),
    vscode.languages.registerDefinitionProvider(nexulSelector, definitionProvider),
    vscode.languages.registerReferenceProvider(nexulSelector, referenceProvider),
    vscode.languages.registerRenameProvider(nexulSelector, renameProvider),
    vscode.languages.registerDocumentFormattingEditProvider(nexulSelector, formattingProvider),
    vscode.workspace.onDidGrantWorkspaceTrust(() => {
      codeLensEmitter.fire();
      if (vscode.workspace.getConfiguration('nexul').get('checkOnSave', true)) {
        for (const document of vscode.workspace.textDocuments) {
          void updateEditionLanguage(document).then((activeDocument) => checkDocument(activeDocument))
            .catch((error) => reportFailure('détection de l’édition', error));
        }
      }
    }),
    vscode.workspace.onDidOpenTextDocument((document) => {
      void updateEditionLanguage(document).then((activeDocument) => {
        if (isNexul(activeDocument) && vscode.workspace.getConfiguration('nexul').get('checkOnSave', true)) {
          void checkDocument(activeDocument);
        }
      }).catch((error) => reportFailure('détection de l’édition', error));
    }),
    vscode.workspace.onDidSaveTextDocument((document) => {
      if (path.basename(document.uri.fsPath).toLowerCase() === 'nexul.toml') {
        for (const openDocument of vscode.workspace.textDocuments) {
          if (!isNexul(openDocument)) continue;
          void updateEditionLanguage(openDocument).then((activeDocument) => {
            if (vscode.workspace.getConfiguration('nexul').get('checkOnSave', true)) {
              void checkDocument(activeDocument);
            }
          }).catch((error) => reportFailure('détection de l’édition', error));
        }
        return;
      }
      if (isNexul(document)) {
        void updateEditionLanguage(document).then((activeDocument) => {
          if (vscode.workspace.getConfiguration('nexul').get('checkOnSave', true)) {
            void checkDocument(activeDocument);
          }
        }).catch((error) => reportFailure('détection de l’édition', error));
      }
    }),
    vscode.workspace.onDidCloseTextDocument((document) => {
      contributions.delete(document.uri.fsPath);
      republishDiagnostics();
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
    vscode.workspace.onWillSaveTextDocument((event) => {
      const document = event.document;
      if (!isNexul(document) || !vscode.workspace.getConfiguration('nexul').get('formatOnSave', false)) return;
      event.waitUntil(formatText(document).then((formatted) => {
        if (formatted === document.getText()) return [];
        return [vscode.TextEdit.replace(
          new vscode.Range(document.positionAt(0), document.positionAt(document.getText().length)),
          formatted
        )];
      }).catch((error) => {
        reportFailure('nxc ide format (format on save)', error);
        return [];
      }));
    }),
    new vscode.Disposable(() => {
      if (codeLensRefreshTimer) clearTimeout(codeLensRefreshTimer);
    })
  );

  for (const document of vscode.workspace.textDocuments) {
    void updateEditionLanguage(document).catch((error) => reportFailure('détection de l’édition', error));
  }
}

function deactivate() {}

module.exports = {
  activate,
  deactivate
};
