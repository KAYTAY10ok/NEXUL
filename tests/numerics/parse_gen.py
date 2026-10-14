import random, struct, sys
from decimal import Decimal
random.seed(7)
out = []
def add(s): out.append(s)
for _ in range(15000):
    k = random.random()
    if k < 0.3:
        d = ''.join(random.choice('0123456789') for _ in range(random.randint(1, 25)))
        add(d + 'e' + str(random.randint(-340, 320)))
    elif k < 0.5:
        # exact halfway between two doubles
        b = random.getrandbits(64) & 0x7FEFFFFFFFFFFFFF
        x = struct.unpack('<d', struct.pack('<Q', b))[0]
        y = struct.unpack('<d', struct.pack('<Q', b + 1))[0]
        if x != x or y != y or abs(y) == float('inf'): continue
        h = (Decimal(x) + Decimal(y)) / 2
        add(format(h, 'f') if abs(h) < Decimal('1e30') and abs(h) > Decimal('1e-30') else str(h))
    elif k < 0.6:
        add(repr(struct.unpack('<d', struct.pack('<Q', random.getrandbits(63)))[0]))
    elif k < 0.7:
        add('0.' + '0' * random.randint(0, 330) + ''.join(random.choice('0123456789') for _ in range(random.randint(1, 30))))
    elif k < 0.8:
        add(''.join(random.choice('0123456789') for _ in range(random.randint(300, 900))) + '.' + str(random.randint(0, 99)) + 'e-' + str(random.randint(0, 600)))
    else:
        add(random.choice(['-', '+', '']) + str(random.randint(0, 10**random.randint(1, 18))) + '.' + str(random.randint(0, 10**6)) + random.choice(['', 'E' + str(random.randint(-30, 30))]))
add('inf'); add('-Infinity'); add('NaN'); add('1e309'); add('2.4703282292062328e-324'); add('2.4703282292062327e-324'); add('4.9406564584124654e-324'); add('1.7976931348623157e308'); add('1.7976931348623158e308'); add('1.797693134862315807937e308'); add('0.0'); add('-0.0'); add('1e'); add('abc'); add('')
open(sys.argv[1], 'w').write('\n'.join(out) + '\n')
def py(s):
    try:
        if s.strip() != s or s == '' or s.endswith('e'): raise ValueError
        v = float(s)
    except ValueError:
        return 'None'
    if v != v: return 'NaN'
    return str(struct.unpack('<Q', struct.pack('<d', v))[0])
open(sys.argv[2], 'w').write('\n'.join(py(s) for s in out) + '\n')
