#!/usr/bin/env python3
"""Run the real Cortex-M interrupt/allocator stress test; missing tools fail."""
import argparse
import pathlib
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--nxc', required=True, type=pathlib.Path)
parser.add_argument('--qemu', default='qemu-system-arm')
args = parser.parse_args()
root = pathlib.Path(__file__).resolve().parent.parent
with tempfile.TemporaryDirectory(prefix='nexul-irq-') as work:
    for optimize in ['-O0', '-O2']:
        exe = pathlib.Path(work) / 'interrupt.elf'
        subprocess.run([str(args.nxc.resolve()), 'build', str(root / 'tests/bare/interrupt_alloc.nx'),
                        '--target', 'thumbv7em-none', '--no-debug', optimize, '-o', str(exe)], check=True)
        result = subprocess.run([args.qemu, '-machine', 'mps2-an386', '-nographic', '-monitor', 'none',
                                 '-serial', 'none', '-semihosting-config', 'enable=on,target=native',
                                 '-kernel', str(exe)], capture_output=True, text=True, timeout=60)
        lines = (result.stdout + result.stderr).splitlines()
        assert result.returncode == 0, result
        assert lines.count('main') == 100 and lines.count('irq') >= 1 and lines.count('ok') == 1, lines
        assert all(line in ['main', 'irq', 'ok'] for line in lines), lines
        print(optimize, 'interrupt delivery, allocation checksums, zero live allocations and intact output passed')
