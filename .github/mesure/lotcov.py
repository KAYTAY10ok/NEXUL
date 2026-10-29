"""Coverage of the production Rust lines each lot introduced, as they stand at HEAD.
Each line of HEAD is attributed (git blame) to the first lot branch that contains its
commit; its coverage comes from an lcov file measured at HEAD. Lines that lcov does not
list are not executable (comments, blank lines, declarations) and are not counted.
Usage: lotcov.py repo lcov.info base name1=sha1 name2=sha2 ... (in stack order)"""
import os
import re
import subprocess
import sys
from collections import defaultdict

repo, lcov, base, *pairs = sys.argv[1:]
tips = dict(p.split('=', 1) for p in pairs)
lots = list(tips)


def git(*args):
    return subprocess.run(['git', '-C', repo, *args], capture_output=True, text=True, encoding='utf-8', check=True).stdout


cov = {}
cur = None
for line in open(lcov, encoding='utf-8'):
    line = line.strip()
    if line.startswith('SF:'):
        cur = os.path.relpath(line[3:], repo).replace('\\', '/')
        cov.setdefault(cur, {})
    elif line.startswith('DA:') and cur:
        n, hits = line[3:].split(',')[:2]
        cov[cur][int(n)] = cov[cur].get(int(n), 0) + int(hits)

TEST = re.compile(r'(^|/)(tests?|benches|examples|fuzz)(/|$)|(^|/)[^/]*[_-]tests?\.rs$|(^|/)tests?\.rs$')
changed = [p for p in git('diff', '--name-only', '--diff-filter=ACMR', f'{base}...HEAD', '--', 'compiler/', 'studio/src/').split('\n')
           if p.endswith('.rs') and not TEST.search(p)]

owner = {}


def lot_of(commit):
    if commit not in owner:
        owner[commit] = None
        if subprocess.run(['git', '-C', repo, 'merge-base', '--is-ancestor', commit, base]).returncode == 0:
            return None
        for b in lots:
            if subprocess.run(['git', '-C', repo, 'merge-base', '--is-ancestor', commit, tips[b]]).returncode == 0:
                owner[commit] = b
                break
    return owner[commit]


stats = defaultdict(lambda: [0, 0])
missed = defaultdict(lambda: defaultdict(list))
for path in changed:
    blame = git('blame', '--line-porcelain', 'HEAD', '--', path)
    lineno = 0
    commit = None
    for l in blame.split('\n'):
        m = re.match(r'^([0-9a-f]{40}) \d+ (\d+)', l)
        if m:
            commit, lineno = m.group(1), int(m.group(2))
            continue
        if l.startswith('\t'):
            lot = lot_of(commit)
            hits = cov.get(path, {}).get(lineno)
            if lot is None or hits is None:
                continue
            stats[lot][1] += 1
            if hits > 0:
                stats[lot][0] += 1
            else:
                missed[lot][path].append(lineno)

all_h = sum(h for h, _ in stats.values())
all_t = sum(t for _, t in stats.values())
print(f'Stack vs {base[:9]}: {all_h}/{all_t} executable changed lines covered = {100 * all_h / max(all_t, 1):.1f}%')
for b in lots:
    h, t = stats.get(b, (0, 0))
    if t:
        print(f'  {b:36} {h:5}/{t:5} = {100 * h / t:5.1f}%')
print()
for b in lots:
    if missed.get(b):
        print(f'== {b}: uncovered lines')
        for path, ls in sorted(missed[b].items(), key=lambda kv: -len(kv[1])):
            ranges = []
            for n in ls:
                if ranges and n == ranges[-1][1] + 1:
                    ranges[-1][1] = n
                else:
                    ranges.append([n, n])
            print(f'   {len(ls):4}  {path}: ' + ', '.join(f'{a}' if a == z else f'{a}-{z}' for a, z in ranges)[:300])
