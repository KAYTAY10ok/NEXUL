'use strict';

const { spawn } = require('node:child_process');

function scalarColumnToUtf16(line, oneBasedScalarColumn) {
  const wanted = Math.max(0, Number(oneBasedScalarColumn || 1) - 1);
  let scalars = 0;
  let utf16 = 0;
  for (const character of String(line)) {
    if (scalars >= wanted) break;
    utf16 += character.length;
    scalars += 1;
  }
  return utf16;
}

function utf16ColumnToScalar(line, zeroBasedUtf16Column) {
  const limit = Math.max(0, Math.min(String(line).length, Number(zeroBasedUtf16Column || 0)));
  return Array.from(String(line).slice(0, limit)).length + 1;
}

function parseJsonOutput(output, fallbackValue) {
  const text = String(output || '').replace(/^\uFEFF/, '').trim();
  return text.length === 0 ? fallbackValue : JSON.parse(text);
}

function parseDiagnostics(output) {
  const value = parseJsonOutput(output, []);
  if (!Array.isArray(value)) {
    throw new Error('La sortie JSON de nxc check doit être un tableau.');
  }
  return value;
}

function parseLocations(output) {
  const value = parseJsonOutput(output, []);
  if (value && !Array.isArray(value)) {
    if (typeof value.error === 'string') throw new Error(value.error);
    throw new Error('La sortie JSON de nxc ide doit être un tableau ou null.');
  }
  return value || [];
}

function runNxc(executable, args, options) {
  const opts = options || {};
  return new Promise((resolve, reject) => {
    let child;
    let stdout = '';
    let stderr = '';
    let cancelled = false;
    let settled = false;
    let cancellationSubscription;

    const finishError = (error) => {
      if (settled) return;
      settled = true;
      if (cancellationSubscription) cancellationSubscription.dispose();
      reject(error);
    };

    try {
      child = spawn(executable, args, {
        cwd: opts.cwd,
        windowsHide: true,
        shell: false,
        stdio: ['pipe', 'pipe', 'pipe']
      });
    } catch (error) {
      finishError(error);
      return;
    }

    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');
    child.stdout.on('data', (chunk) => { stdout += chunk; });
    child.stderr.on('data', (chunk) => { stderr += chunk; });
    child.stdin.on('error', () => {});

    child.once('error', finishError);
    child.once('close', (code, signal) => {
      if (settled) return;
      settled = true;
      if (cancellationSubscription) cancellationSubscription.dispose();
      if (cancelled) {
        const error = new Error('La requête NEXUL a été annulée.');
        error.name = 'CancellationError';
        reject(error);
        return;
      }
      const allowed = opts.allowedExitCodes || [0];
      if (allowed.includes(code)) {
        resolve({ code, signal, stdout, stderr });
        return;
      }
      const error = new Error(stderr.trim() || ('nxc a terminé avec le code ' + code + '.'));
      error.code = 'NXC_EXIT';
      error.exitCode = code;
      error.stdout = stdout;
      error.stderr = stderr;
      reject(error);
    });

    if (opts.token && typeof opts.token.onCancellationRequested === 'function') {
      cancellationSubscription = opts.token.onCancellationRequested(() => {
        cancelled = true;
        if (child && child.pid) child.kill();
      });
    }

    if (opts.input === undefined) child.stdin.end();
    else child.stdin.end(opts.input);
  });
}

module.exports = {
  parseDiagnostics,
  parseJsonOutput,
  parseLocations,
  runNxc,
  scalarColumnToUtf16,
  utf16ColumnToScalar
};
