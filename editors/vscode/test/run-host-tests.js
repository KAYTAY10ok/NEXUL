'use strict';

const os = require('node:os');
const path = require('node:path');
const fs = require('node:fs');
const { downloadAndUnzipVSCode, runTests, runVSCodeCommand } = require('@vscode/test-electron');

async function main() {
  if (!process.env.NEXUL_NXC) {
    throw new Error('Set NEXUL_NXC to a built nxc executable before running npm run test:host.');
  }
  const extensionDevelopmentPath = fs.mkdtempSync(path.join(os.tmpdir(), 'nexul-vscode-e2e-harness-'));
  const extensionTestsPath = path.resolve(__dirname, 'extension-host', 'index.js');
  const workspace = fs.mkdtempSync(path.join(os.tmpdir(), 'nexul-vscode-e2e-workspace-'));
  const userDataDir = fs.mkdtempSync(path.join(os.tmpdir(), 'nexul-vscode-e2e-user-'));
  const extensionsDir = fs.mkdtempSync(path.join(os.tmpdir(), 'nexul-vscode-e2e-extensions-'));
  const vsixPath = path.resolve(__dirname, '..', 'dist', 'kaytay10ok.nexul-language-0.3.0.vsix');
  const lspLog = path.join(workspace, 'nxc-lsp.log');
  try {
    if (!fs.existsSync(vsixPath)) {
      throw new Error('Build the VSIX with npm run package before running npm run test:host.');
    }
    writeTestHarness(extensionDevelopmentPath);
    fs.cpSync(path.resolve(__dirname, 'fixtures', 'edition1'), workspace, { recursive: true });
    const userSettings = path.join(userDataDir, 'User', 'settings.json');
    fs.mkdirSync(path.dirname(userSettings), { recursive: true });
    fs.writeFileSync(userSettings, JSON.stringify({ nexul: {
      nxcPath: path.resolve(process.env.NEXUL_NXC)
    } }, null, 2));
    const vscodeVersion = process.env.VSCODE_TEST_VERSION || 'insiders';
    const vscodeExecutablePath = await downloadAndUnzipVSCode(vscodeVersion);
    const installation = await runVSCodeCommand([
      `--user-data-dir=${userDataDir}`,
      `--extensions-dir=${extensionsDir}`,
      '--install-extension',
      vsixPath,
      '--force'
    ], { version: vscodeVersion });
    console.log(installation.stdout.trim());
    let exitCode;
    try {
      exitCode = await runTests({
        vscodeExecutablePath,
        extensionDevelopmentPath,
        extensionTestsPath,
        extensionTestsEnv: { NEXUL_NXC: process.env.NEXUL_NXC, NEXUL_LSP_LOG: lspLog },
        launchArgs: [
          workspace,
          '--disable-workspace-trust',
          '--user-data-dir', userDataDir,
          '--extensions-dir', extensionsDir
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
    removeTempDirectory(extensionsDir);
    removeTempDirectory(extensionDevelopmentPath);
  }
}

function writeTestHarness(directory) {
  fs.writeFileSync(path.join(directory, 'package.json'), JSON.stringify({
    name: 'nexul-vscode-test-harness',
    displayName: 'NEXUL VS Code test harness',
    publisher: 'nexul',
    version: '0.0.0',
    engines: { vscode: '^1.90.0' },
    main: './extension.js'
  }, null, 2));
  fs.writeFileSync(path.join(directory, 'extension.js'),
    "'use strict';\nmodule.exports = { activate() {}, deactivate() {} };\n");
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
