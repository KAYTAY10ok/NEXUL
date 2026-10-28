import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {loadNexul, NexulExit} from '../../lib/web/nexul.mjs';

const [exampleFile, libraryFile, programFile] = process.argv.slice(2);
const exampleBytes = await readFile(exampleFile);
const libraryBytes = await readFile(libraryFile);
const programBytes = await readFile(programFile);
let greeting;
let nx;
nx = await loadNexul(exampleBytes, {imports: {app: {
  host_double: x => x * 2,
  host_text: (p, n) => { greeting = nx.readText(p, n); },
}}});
assert.equal(nx.call('calculate', 20.5), 42);
assert.equal(nx.call('nx_abi_version'), 1);
for (const text of ['', 'été 中文 🌍', 'a\0b', '🌍'.repeat(530000)]) {
  nx.withText(text, (p, n) => nx.call('greet', p, n));
  assert.equal(greeting, `Bonjour, ${text} !`);
}
assert(WebAssembly.Module.imports(nx.module).every(i => i.module !== 'wasi_snapshot_preview1'));
await assert.rejects(loadNexul(exampleBytes), TypeError);
await assert.rejects(loadNexul(exampleBytes, {imports: {app: {}}}), WebAssembly.LinkError);
await assert.rejects(loadNexul(exampleBytes, {imports: {nexul_v1: {}}}), /reserved/);
assert.throws(() => nx.alloc(-1), RangeError);
assert.throws(() => nx.alloc(1.5), RangeError);
assert.throws(() => nx.free(12), /live host allocation/);
const p = nx.alloc(8);
nx.writeBytes(p, new Uint8Array([1, 2, 3]));
assert.deepEqual(nx.readBytes(p, 3), new Uint8Array([1, 2, 3]));
nx.writeBytes(p, new Uint8Array([255]));
assert.throws(() => nx.readText(p, 1), TypeError);
nx.writeBytes(p, new Uint8Array([1, 2, 3]));
assert.throws(() => nx.writeBytes(p, new Uint8Array(9)), /exceeds/);
const before = nx.memory.buffer;
const big = nx.alloc(4 * 1024 * 1024);
assert.notEqual(nx.memory.buffer, before);
assert.deepEqual(nx.readBytes(p, 3), new Uint8Array([1, 2, 3]));
nx.free(big);
const pages = nx.memory.buffer.byteLength;
for (let i = 0; i < 50; i++) nx.free(nx.alloc(4 * 1024 * 1024));
assert.equal(nx.memory.buffer.byteLength, pages, 'large freed regions must be reused');
nx.free(p);
assert.throws(() => nx.free(p), /live host allocation/);
assert.throws(() => nx.readBytes(0xffffffff, 4), RangeError);
assert.throws(() => nx.withText('x', () => { throw new Error('host callback'); }), /host callback/);
assert.equal(nx.call('calculate', 1), 3);
assert.throws(() => nx.withText('x', () => Promise.resolve()), /synchronous/);
let reentrant;
reentrant = await loadNexul(exampleBytes, {imports: {app: {
  host_double: () => reentrant.call('calculate', 1), host_text: () => {},
}}});
assert.throws(() => reentrant.call('calculate', 1), /reentrant/);
assert.throws(() => reentrant.call('calculate', 1), /terminated/);
const throwing = await loadNexul(exampleBytes, {imports: {app: {
  host_double: () => { throw new Error('host import failure'); }, host_text: () => {},
}}});
assert.throws(() => throwing.call('calculate', 1), /host import failure/);
assert.throws(() => throwing.call('calculate', 1), /terminated/);

let stdout = '', stderr = '';
const make = () => loadNexul(libraryBytes, {
  write: (fd, text) => { if (fd === 1) stdout += text; else stderr += text; },
  nowNs: monotonic => monotonic ? 456n : 123n,
  randomBytes: bytes => { bytes.fill(77); },
});
const lib = await make();
assert.equal(lib.call('plus_one', 41), 42);
assert.equal(lib.call('clock'), 123n);
assert.equal(lib.call('monotonic'), 456n);
assert.equal(lib.call('random_byte'), 77);
assert.equal(lib.call('no_files'), 1);
for (const x of [0, -0, Infinity, -Infinity, NaN, Number.MIN_VALUE, 1 / 3]) {
  assert(Object.is(lib.call('identity64', x), x));
  assert(Object.is(lib.call('identity32', x), Math.fround(x)));
}
lib.withText('été 🌍', (p, n) => lib.call('print_text', p, n));
assert.equal(stdout, 'été 🌍\n');
assert.throws(() => lib.call('plus_one', 2147483647), e => e instanceof NexulExit && e.code === 101);
assert.match(stderr, /overflow/);
assert.throws(() => lib.call('plus_one', 0), /terminated/);
for (const name of ['fail', 'sleep']) {
  const failed = await make();
  assert.throws(() => failed.call(name), e => e instanceof NexulExit && e.code === 101);
  assert.throws(() => failed.alloc(1), /terminated/);
}
// ADR 0011 T4 (task C4): an export checks the `char` and `bool` values the host passes.
const values = await make();
assert.equal(values.call('char_code', 0x1F30D), 0x1F30D);
assert.equal(values.call('bool_flip', 1), 0);
for (const [name, bad, message] of [['char_code', 0xD800, /not a valid `char`/], ['char_code', 0x110000, /not a valid `char`/], ['bool_flip', 2, /not a valid `bool`/]]) {
  const checked = await make();
  stderr = '';
  assert.throws(() => checked.call(name, bad), e => e instanceof NexulExit && e.code === 101);
  assert.match(stderr, message);
}
const trap = await make();
assert.throws(() => trap.call('print_text', 0xfffffff0, 16), WebAssembly.RuntimeError);
assert.throws(() => trap.call('clock'), /terminated/);
let out = '';
const program = await loadNexul(programBytes, {write: (fd, text) => { out += text; }});
program.run();
assert.equal(out, 'NEXUL navigateur 🌍\n');
assert.throws(() => program.run(), /terminated/);
console.log('web ABI: imports, exports, Unicode, ownership, memory growth, IEEE, host services and failure paths passed');
