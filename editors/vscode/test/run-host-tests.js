'use strict';

const os = require('node:os');
const path = require('node:path');
const fs = require('node:fs');
const { downloadAndUnzipVSCode, runTests } = require('@vscode/test-electron');

async function main() {
  if (!process.env.NEXUL_NXC) {
    throw new Error('Set NEXUL_NXC to a built nxc executable before running npm run test:host.');
  }
  const extensionDevelopmentPath = path.resolve(__dirname, '..');
  const extensionTestsPath = path.resolve(__dirname, 'extension-host', 'index.js');
  const workspace = fs.mkdtempSync(path.join(os.tmpdir(), 'nexul-vscode-e2e-workspace-'));
  const userDataDir = fs.mkdtempSync(path.join(os.tmpdir(), 'nexul-vscode-e2e-user-'));
  const lspLog = path.join(workspace, 'nxc-lsp.log');
  try {
    fs.cpSync(path.resolve(__dirname, 'fixtures', 'edition1'), workspace, { recursive: true });
    const userSettings = path.join(userDataDir, 'User', 'settings.json');
    fs.mkdirSync(path.dirname(userSettings), { recursive: true });
    fs.writeFileSync(userSettings, JSON.stringify({ nexul: {
      nxcPath: path.resolve(process.env.NEXUL_NXC)
    } }, null, 2));
    const vscodeExecutablePath = await downloadAndUnzipVSCode(process.env.VSCODE_TEST_VERSION || 'insiders');
    let exitCode;
    try {
      exitCode = await runTests({
        vscodeExecutablePath,
        extensionDevelopmentPath,
        extensionTestsPath,
        extensionTestsEnv: { NEXUL_NXC: process.env.NEXUL_NXC, NEXUL_LSP_LOG: lspLog },
        launchArgs: [
          workspace,
          '--disable-extensions',
          '--disable-workspace-trust',
          '--user-data-dir', userDataDir
        ]
      });
    } catch (error) {
      if (fs.existsSync(lspLog)) console.error('\nnxc lsp debug log:\n' + fs.readFileSync(lspLog, 'utf8'));
      throw error;
    }
    if (exitCode !== 0) {
      if (fs.existsSync(lspLog)) console.error('\nnxc lsp debug log:\n' + fs.readFileSync(lspLog, 'utf8'));
      process.exitCode = exitCode || 1;
    }
  } finally {
    removeTempDirectory(workspace);
    removeTempDirectory(userDataDir);
  }
}

function removeTempDirectory(directory) {
  const temporaryRoot = path.resolve(os.tmpdir());
  const resolvedDirectory = path.resolve(directory);
  if (path.dirname(resolvedDirectory) !== temporaryRoot) {
    throw new Error('Refusing to remove a directory outside the temporary root.');
  }
  fs.rmSync(resolvedDirectory, { recursive: true, force: true });
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
