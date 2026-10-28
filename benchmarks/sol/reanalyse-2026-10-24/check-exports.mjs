import {readFile} from 'node:fs/promises';
import {loadNexul} from '../../../lib/web/nexul.mjs';
for (const path of process.argv.slice(2)) {
  const bytes = await readFile(path);
  for (const [name, value] of [['letter_code', 0x1f30d], ['letter_code', 0xd800], ['letter_code', 0x110000], ['flag_value', 0], ['flag_value', 1], ['flag_value', 2], ['flag_value', 255]]) {
    const nx = await loadNexul(bytes, {write: () => {}});
    try { console.log(JSON.stringify({path, name, input:value, result:nx.call(name, value), panic:false})); }
    catch(e) { console.log(JSON.stringify({path, name, input:value, panic:true, code:e.code, error:e.message})); }
  }
}
