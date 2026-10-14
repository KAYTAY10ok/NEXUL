// Runs a NEXUL program compiled for `wasm32-wasi` with the WASI implementation of Node.js:
//   node tools/wasi-run.mjs program.wasm [arguments...]
// The current directory and the root of the file system are granted to the program.
import { WASI } from 'node:wasi';
import { readFileSync } from 'node:fs';

const [file, ...args] = process.argv.slice(2);
const preopens = { '.': process.cwd() };
if (process.platform !== 'win32') preopens['/'] = '/';
const wasi = new WASI({ version: 'preview1', args: [file, ...args], env: process.env, preopens, returnOnExit: true });
const module = await WebAssembly.compile(readFileSync(file));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
process.exitCode = wasi.start(instance);
