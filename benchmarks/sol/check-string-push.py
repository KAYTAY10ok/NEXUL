#!/usr/bin/env python3
"""Check UTF-8 boundaries, growth, literals and an optimized overflow control."""
import argparse
import os
import pathlib
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--nxc', required=True, type=pathlib.Path)
args = parser.parse_args()
root = pathlib.Path(__file__).resolve().parents[2]
points = [0, 0x7f, 0x80, 0x7ff, 0x800, 0xd7ff, 0xe000, 0xffff, 0x10000, 0x10ffff]
expected = b''.join(chr(c).encode() + b'x' + chr(c).encode() + chr(c).encode() for c in points)
with tempfile.TemporaryDirectory(prefix='nexul-string-push-') as folder:
    work = pathlib.Path(folder)
    source = work / 'unicode.nx'
    chars = ', '.join("'\\u{" + format(c, 'x') + "}'" for c in points)
    source.write_text('fn main() {\n    let points = [' + chars + ''']
    for c in points {
        var s = String.new()
        s.push(c)
        print(s)
        var literal = "x"
        literal.push(c)
        print(literal)
        var small = String.with_capacity(1)
        small.push(c)
        print(small)
    }
}
''')
    env = dict(os.environ, NEXUL_LEAK_CHECK='1')
    for opt in ['-O0', '-O2']:
        out = work / 'unicode'
        subprocess.run([str(args.nxc.resolve()), 'build', str(source), '--no-debug', opt, '-o', str(out)], check=True)
        r = subprocess.run([str(out)], capture_output=True, env=env)
        assert r.returncode == 0 and r.stdout == expected and not r.stderr, r
        print(opt, 'UTF-8 boundaries, literal copy, capacity growth and leak check passed')
    control = work / 'checked.nx'
    control.write_text('@export("add")\npub fn add(a: i64, b: i64) -> i64 { return a + b }\n')
    ir = work / 'checked.ll'
    subprocess.run([str(args.nxc.resolve()), 'build', str(control), '--lib', '--release', '--no-debug',
                    '--emit', 'llvm-ir', '-o', str(ir)], check=True)
    text = ir.read_text()
    assert '@llvm.sadd.with.overflow.i64' in text and 'panic_arith' in text
    wasm = work / 'checked.wasm'
    subprocess.run([str(args.nxc.resolve()), 'build', str(control), '--lib', '--release', '--no-debug',
                    '--target', 'wasm32-web', '-o', str(wasm)], check=True)
    script = work / 'check.mjs'
    script.write_text('''import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
const {loadNexul, NexulExit} = await import(pathToFileURL(process.argv[2]));
const bytes = await readFile(process.argv[3]);
let stderr = '';
const nx = await loadNexul(bytes, {write: (fd, text) => { if(fd === 2) stderr += text; }});
assert.equal(nx.call('add', 20n, 22n), 42n);
assert.throws(() => nx.call('add', 9223372036854775807n, 1n), e => e instanceof NexulExit && e.code === 101);
assert.match(stderr, /overflow/);
assert.match(stderr, /checked.nx:2:/);
console.log('O2: checked i64 addition retained in IR and overflow panics with a position');
''')
    subprocess.run(['node', str(script), str(root / 'lib/web/nexul.mjs'), str(wasm)], check=True)
