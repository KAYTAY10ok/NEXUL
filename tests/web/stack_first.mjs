import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {WASI} from 'node:wasi';
import {loadNexul} from '../../lib/web/nexul.mjs';

const [webFile, wasiFile] = process.argv.slice(2);
const web = await loadNexul(await readFile(webFile));
const wasi = new WASI({version: 'preview1', args: [], env: {}});
const {instance} = await WebAssembly.instantiate(await readFile(wasiFile), wasi.getImportObject());
wasi.initialize(instance);
for (const [label, memory, ptr, overflow] of [
  ['web', web.memory, web.call('sentinel_ptr'), () => web.call('overflow')],
  ['wasi', instance.exports.memory, instance.exports.sentinel_ptr(), () => instance.exports.overflow()],
]) {
  assert(ptr >= 8388608, `${label}: static data must follow the stack`);
  const expected = [0x12345678, 0x87654321, 0xabcdef01, 0x10203040];
  const read = () => Array.from(new Uint32Array(memory.buffer, ptr, 4));
  assert.deepEqual(read(), expected);
  assert.throws(overflow, WebAssembly.RuntimeError);
  assert.deepEqual(read(), expected, `${label}: stack overflow corrupted static data`);
}
console.log('WASI and web: stack overflow traps with static data intact');
