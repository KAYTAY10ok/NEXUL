"""Summarizes an lcov file: line coverage per crate and the least covered files.
Usage: lcovsum.py lcov.info [repo_root]"""
import os
import sys
from collections import defaultdict

path = sys.argv[1]
root = os.path.normcase(os.path.abspath(sys.argv[2] if len(sys.argv) > 2 else '.'))
files = {}
cur = None
for line in open(path, encoding='utf-8'):
    line = line.strip()
    if line.startswith('SF:'):
        cur = os.path.normcase(os.path.abspath(line[3:]))
        files.setdefault(cur, {})
    elif line.startswith('DA:') and cur:
        n, hits = line[3:].split(',')[:2]
        d = files[cur]
        d[int(n)] = d.get(int(n), 0) + int(hits)
    elif line == 'end_of_record':
        cur = None

crates = defaultdict(lambda: [0, 0])
rows = []
for f, d in files.items():
    if not f.startswith(root):
        continue
    rel = os.path.relpath(f, root).replace('\\', '/')
    parts = rel.split('/')
    crate = '/'.join(parts[:2]) if parts[0] in ('compiler', 'studio') else parts[0]
    total = len(d)
    hit = sum(1 for h in d.values() if h > 0)
    crates[crate][0] += hit
    crates[crate][1] += total
    rows.append((rel, hit, total))

all_hit = sum(h for h, _ in crates.values())
all_total = sum(t for _, t in crates.values())
print(f'TOTAL {all_hit}/{all_total} = {100 * all_hit / max(all_total, 1):.1f}%')
for c, (h, t) in sorted(crates.items()):
    print(f'  {c:32} {h:6}/{t:6} = {100 * h / max(t, 1):5.1f}%')
print('Least covered files (>= 50 lines):')
for rel, h, t in sorted((r for r in rows if r[2] >= 50), key=lambda r: r[1] / r[2])[:25]:
    print(f'  {100 * h / t:5.1f}%  {h:5}/{t:5}  {rel}')
