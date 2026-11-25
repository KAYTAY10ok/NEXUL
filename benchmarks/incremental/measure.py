"""Measures the incremental build (ADR 0016, validation §3).

usage: python -I measure.py <nxc> <work dir> [functions=1000] [repetitions=5]

For each repetition, with a fresh cache each time:
- cold: the program built once, monolithic (NEXUL_INCREMENTAL=0) and cut into units;
- one body changed: the body of g<N/2> changed, then the program built again, with the
  cache of the cold build (the cache of whole executables cannot answer: the text
  changed).

Each executable is run and its output compared with the monolithic one. Prints the
samples and medians in seconds, as Markdown.
"""
import os
import shutil
import statistics
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))

nxc, work = sys.argv[1], sys.argv[2]
n = int(sys.argv[3]) if len(sys.argv) > 3 else 1000
reps = int(sys.argv[4]) if len(sys.argv) > 4 else 5
os.makedirs(work, exist_ok=True)
src = os.path.join(work, "p.nx")
exe = os.path.join(work, "p")


def gen(variant):
    subprocess.run([sys.executable, "-I", os.path.join(HERE, "gen.py"), src, str(n), str(variant)], check=True)


def build(cache, incremental):
    env = dict(os.environ, NEXUL_CACHE_DIR=cache, NEXUL_INCREMENTAL="1" if incremental else "0", NEXUL_INCREMENTAL_REPORT="1")
    env.pop("NEXUL_NO_CACHE", None)
    t = time.perf_counter()
    r = subprocess.run([nxc, "build", src, "-o", exe], env=env, capture_output=True, text=True)
    dt = time.perf_counter() - t
    if r.returncode != 0:
        sys.exit(r.stderr)
    report = [l for l in r.stderr.splitlines() if l.startswith("incremental:")]
    out = subprocess.run([exe], capture_output=True, text=True, check=True).stdout
    return dt, out, report[0] if report else ""


samples = {k: [] for k in ["cold, monolithic", "cold, units", "one body changed, monolithic", "one body changed, units"]}
reports = set()
for r in range(reps):
    for incremental in (False, True):
        cache = os.path.join(work, f"cache-{r}-{int(incremental)}")
        shutil.rmtree(cache, ignore_errors=True)
        gen(0)
        dt, out0, rep = build(cache, incremental)
        samples["cold, " + ("units" if incremental else "monolithic")].append(dt)
        gen(1 + r)
        dt, out1, rep = build(cache, incremental)
        samples["one body changed, " + ("units" if incremental else "monolithic")].append(dt)
        if rep:
            reports.add(rep)
        if incremental:
            assert out0 == mono0 and out1 == mono1, "the units give another program"
        else:
            mono0, mono1 = out0, out1
        shutil.rmtree(cache, ignore_errors=True)

print(f"Program: {n} functions ({sum(1 for _ in open(src))} lines), {reps} repetitions, {os.cpu_count()} processors.\n")
print("| Build | Median (s) | Samples (s) |")
print("|---|---|---|")
for k, v in samples.items():
    print(f"| {k} | {statistics.median(v):.3f} | {', '.join(f'{x:.3f}' for x in v)} |")
print()
for rep in sorted(reports):
    print(f"After the change: `{rep}`.")
