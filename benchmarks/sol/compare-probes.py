#!/usr/bin/env python3
"""Compare two nxc binaries on the unchanged P13 NEXUL workloads (Linux)."""
import argparse
import json
import pathlib
import platform
import statistics
import subprocess
import tempfile


def run(*args):
    return subprocess.check_output(args, text=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', required=True, type=pathlib.Path)
    parser.add_argument('--candidate', required=True, type=pathlib.Path)
    parser.add_argument('--sources-ref', required=True)
    parser.add_argument('--sizes', nargs='+', type=int, default=[1_000_000, 10_000_000])
    parser.add_argument('--repetitions', type=int, default=9)
    parser.add_argument('--output', required=True, type=pathlib.Path)
    args = parser.parse_args()
    if args.repetitions < 2 or any(n <= 0 for n in args.sizes):
        parser.error('use at least two repetitions and positive sizes')
    report = dict(platform=platform.platform(), machine=platform.machine(),
                  sources_commit=run('git', 'rev-parse', args.sources_ref).strip(),
                  compiler_base=run('git', 'rev-parse', 'HEAD').strip(),
                  repetitions=args.repetitions, options=['--release', '--no-debug'], rows=[])
    with tempfile.TemporaryDirectory(prefix='nexul-probes-') as folder:
        work = pathlib.Path(folder)
        binaries = {}
        for name in ['scientific', 'dynamic']:
            source = work / (name + '.nx')
            source.write_text(run('git', 'show', f'{args.sources_ref}:benchmarks/astral/src/{name}.nx'))
            for label in ['baseline', 'candidate']:
                out = work / (name + '-' + label)
                run(str(getattr(args, label).resolve()), 'build', str(source), '--release', '--no-debug', '-o', str(out))
                binaries[name, label] = out
        for mode in ['scientific', 'vectors', 'allocations', 'strings']:
            for size in args.sizes:
                values = dict(baseline=[], candidate=[])
                reference = None
                for i in range(args.repetitions):
                    # Alternate execution order to reduce thermal/order bias.
                    for label in (['baseline', 'candidate'] if i % 2 == 0 else ['candidate', 'baseline']):
                        name = 'scientific' if mode == 'scientific' else 'dynamic'
                        params = [str(size)] if name == 'scientific' else [mode, str(size)]
                        fields = dict(line.split('=', 1) for line in run(str(binaries[name, label]), *params).splitlines())
                        elapsed = int(fields.pop('elapsed_ns'))
                        if reference is None:
                            reference = fields
                        assert fields == reference, (mode, size, label, fields, reference)
                        values[label].append(elapsed / 1_000_000)
                row = dict(workload=mode, size=size, outputs=reference)
                for label, samples in values.items():
                    row[label] = dict(samples_ms=samples, median_ms=statistics.median(samples), stdev_ms=statistics.stdev(samples))
                row['change_percent'] = 100 * (row['candidate']['median_ms'] / row['baseline']['median_ms'] - 1)
                report['rows'].append(row)
                print(mode, size, f"{row['change_percent']:+.2f}%", flush=True)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
