'use strict';

const path = require('node:path');

const COMMANDS = ['build', 'run', 'test', 'check'];

function taskLabel(command, file) {
  const label = 'NEXUL: ' + command;
  return file ? label + ' ' + path.basename(file) : label + ' project';
}

function createCliTask(vscode, executable, folder, command, file, filter, cwdOverride) {
  const args = [command];
  if (file) args.push(file);
  if (filter) args.push(filter);

  const cwd = cwdOverride || (folder && folder.uri ? folder.uri.fsPath : (file ? path.dirname(file) : process.cwd()));
  const scope = folder || vscode.TaskScope.Global;
  const definition = {
    type: 'nexul',
    command,
    file: file || '',
    cwd
  };
  const task = new vscode.Task(
    definition,
    scope,
    taskLabel(command, file),
    'NEXUL',
    new vscode.ProcessExecution(executable, args, { cwd })
  );
  task.presentationOptions = {
    echo: true,
    reveal: vscode.TaskRevealKind.Always,
    focus: false,
    panel: vscode.TaskPanelKind.Dedicated,
    clear: true
  };
  if (command === 'build') task.group = vscode.TaskGroup.Build;
  if (command === 'test') task.group = vscode.TaskGroup.Test;
  return task;
}

function registerTaskProvider(vscode, context, getExecutable) {
  const provider = {
    async provideTasks() {
      if (!vscode.workspace.isTrusted) return [];
      const tasks = [];
      for (const folder of vscode.workspace.workspaceFolders || []) {
        const manifest = await vscode.workspace.findFiles(
          new vscode.RelativePattern(folder, 'nexul.toml'),
          undefined,
          1
        );
        if (manifest.length > 0) {
          for (const command of COMMANDS) {
            tasks.push(createCliTask(vscode, getExecutable(), folder, command, null, null, folder.uri.fsPath));
          }
          continue;
        }

        const files = await vscode.workspace.findFiles(
          new vscode.RelativePattern(folder, '**/*.nx'),
          '**/{.git,node_modules,target}/**',
          50
        );
        for (const uri of files) {
          for (const command of COMMANDS) {
            tasks.push(createCliTask(vscode, getExecutable(), folder, command, uri.fsPath, null, folder.uri.fsPath));
          }
        }
      }
      return tasks;
    },

    resolveTask(task) {
      if (!vscode.workspace.isTrusted) return undefined;
      const definition = task.definition;
      if (!definition || definition.type !== 'nexul' || !COMMANDS.includes(definition.command)) return undefined;
      const folder = definition.cwd
        ? vscode.workspace.getWorkspaceFolder(vscode.Uri.file(definition.cwd))
        : (definition.file ? vscode.workspace.getWorkspaceFolder(vscode.Uri.file(definition.file)) : undefined);
      return createCliTask(
        vscode,
        getExecutable(),
        folder,
        definition.command,
        definition.file || null,
        null,
        definition.cwd || null
      );
    }
  };

  context.subscriptions.push(vscode.tasks.registerTaskProvider('nexul', provider));
}

module.exports = {
  COMMANDS,
  createCliTask,
  registerTaskProvider
};
