import sys
inputs = open(sys.argv[1]).read().split('\n')
expected = open(sys.argv[2]).read().split('\n')
actual = open(sys.argv[3]).read().replace('\r', '').split('\n')
if len(expected) <= 1 or len(inputs) != len(expected) or len(actual) != len(expected):
    raise SystemExit('empty or inconsistent input/expected/actual record counts')
bad = [(inputs[k], expected[k], actual[k] if k < len(actual) else '<missing>') for k in range(len(expected)) if expected[k] != (actual[k] if k < len(actual) else None)]
print(len(expected) - 1, 'inputs,', len(bad), 'mismatches')
for b in bad[:10]:
    print(b[0][:80], 'expected', b[1], 'got', b[2])
sys.exit(1 if bad else 0)
