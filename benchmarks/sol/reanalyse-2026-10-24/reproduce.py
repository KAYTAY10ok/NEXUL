#!/usr/bin/env python3
"""Audit probes. Exit 1 while either confirmed invariant violation remains."""
import argparse
import json
import pathlib
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--nxc', required=True, type=pathlib.Path)
parser.add_argument('--output', type=pathlib.Path)
args = parser.parse_args()
here = pathlib.Path(__file__).resolve().parent
nxc = str(args.nxc.resolve())
report = {'observations': [], 'violations': []}
with tempfile.TemporaryDirectory(prefix='nexul-invariants-') as folder:
    work = pathlib.Path(folder)
    for opt in ['-O0', '-O2']:
        program = work / 'float-check'
        compile_result = subprocess.run([nxc, 'build', str(here / 'f32_constraint.nx'), '--no-debug',
                                         opt, '-o', str(program)], capture_output=True, text=True)
        row = {'case': 'f32 constraint', 'opt': opt, 'compile_exit': compile_result.returncode,
               'diagnostics': compile_result.stderr}
        if compile_result.returncode == 0:
            result = subprocess.run([str(program)], capture_output=True, text=True)
            row.update(exit=result.returncode, stdout=result.stdout, stderr=result.stderr)
            if result.returncode != 0 or result.stdout != 'true\n':
                report['violations'].append({'case': 'f32 compile/runtime predicate disagreement', 'opt': opt})
        elif 'E0252' not in compile_result.stderr:
            raise RuntimeError(compile_result.stderr)
        report['observations'].append(row)
        wasm = work / 'exports.wasm'
        subprocess.run([nxc, 'build', str(here / 'export_distinct.nx'), '--lib', '--target',
                        'wasm32-web', '--no-debug', opt, '-o', str(wasm)], check=True)
        result = subprocess.run(['node', str(here / 'check-exports.mjs'), str(wasm)],
                                capture_output=True, text=True, check=True)
        rows = [json.loads(line) for line in result.stdout.splitlines()]
        for row in rows:
            row.pop('path', None)
            row['opt'] = opt
            report['observations'].append(row)
            invalid = row['input'] in [0xd800, 0x110000] if row['name'] == 'letter_code' else row['input'] not in [0, 1]
            if invalid and not (row['panic'] and row.get('code') == 101):
                report['violations'].append({'case': row['name'], 'input': row['input'], 'opt': opt})
            if not invalid:
                assert not row['panic'] and row['result'] == row['input'], row
if args.output:
    args.output.write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
raise SystemExit(1 if report['violations'] else 0)
