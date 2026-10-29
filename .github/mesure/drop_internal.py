import re, sys
path = sys.argv[1]
out, src = [], []
for line in open(path, encoding='utf-8'):
    if line.startswith('SF:'):
        try:
            src = open(line[3:].strip(), encoding='utf-8').read().split('\n')
        except OSError:
            src = []
    elif line.startswith('DA:'):
        n = int(line[3:].split(',')[0])
        if 0 < n <= len(src) and re.search(r'panic!\("internal|unreachable!\(', src[n - 1]):
            continue
    out.append(line)
open(path, 'w', encoding='utf-8').writelines(out)
