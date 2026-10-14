import struct, sys
from decimal import Decimal, ROUND_HALF_EVEN
def f_of(bits): return struct.unpack('<d', struct.pack('<Q', bits))[0]
def digits_of(r):
    # significant digits of Python repr
    m = r.lower().lstrip('-').split('e')[0].replace('.', '').lstrip('0').rstrip('0')
    return m or '0'
def nx_digits(s):
    m = s.lower().lstrip('-+').split('e')[0].replace('.', '').lstrip('0').rstrip('0')
    return m or '0'
def sci(x, p):
    s = format(x, '.%de' % p)
    mant, exp = s.split('e')
    return mant + 'e' + str(int(exp))
bad = 0; n = 0
for line in open(sys.argv[1]):
    parts = line.split()
    if len(parts) != 6: continue
    bits = int(parts[0]); x = f_of(bits); n += 1
    short, f3, f0, e, e4 = parts[1:]
    errs = []
    if float(short) != x: errs.append('shortest does not round-trip')
    if nx_digits(short) != digits_of(repr(x)): errs.append('not shortest: %s vs %r' % (short, x))
    if f3 != format(x, '.3f'): errs.append('.3 %s vs %s' % (f3, format(x, '.3f')))
    if f0 != format(x, '.0f'): errs.append('.0 %s vs %s' % (f0, format(x, '.0f')))
    if float(e) != x or nx_digits(e) != digits_of(repr(x)): errs.append('e %s' % e)
    if e4 != sci(x, 4): errs.append('.4e %s vs %s' % (e4, sci(x, 4)))
    if errs:
        bad += 1
        if bad <= 15: print(bits, repr(x), errs)
print('checked', n, 'values,', bad, 'mismatches')
sys.exit(1 if bad else 0)
