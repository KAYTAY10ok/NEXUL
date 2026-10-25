import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {loadNexul, NexulExit} from '../../lib/web/nexul.mjs';

const [libraryFile] = process.argv.slice(2);
const libraryBytes = await readFile(libraryFile);

async function fresh() {
  let stderr = '';
  const nx = await loadNexul(libraryBytes, {
    write: (fd, text) => { if (fd === 2) stderr += text; },
  });
  return {nx, getStderr: () => stderr};
}

const valid = await fresh();
assert.equal(valid.nx.call('flag_flip', 0), 1);
assert.equal(valid.nx.call('flag_flip', 1), 0);
assert.equal(valid.nx.call('letter_code', 0x1F30D), 0x1F30D);
assert.equal(valid.nx.call('letter_code', 0x10FFFF), 0x10FFFF);
assert.equal(valid.nx.call('positive_value', 7), 7);

async function panics(name, value, message) {
  const {nx, getStderr} = await fresh();
  assert.throws(() => nx.call(name, value), error => error instanceof NexulExit && error.code === 101);
  assert.match(getStderr(), message);
}

for (const value of [2, 255]) await panics('flag_flip', value, /not a valid `bool`/);
for (const value of [0xD800, 0xDFFF, 0x110000]) await panics('letter_code', value, /not a valid `char`/);
await panics('positive_value', 0, /does not satisfy/);

console.log('distinct export ABI: bool, char and constrained values checked at O0/O2');
