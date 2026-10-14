import struct, math, sys
def f(b): return struct.unpack('<d', struct.pack('<Q', int(b)))[0]
def ulps(a, b):
    if (a != a and b != b) or a == b: return 0
    if math.isinf(a) or math.isinf(b) or a != a or b != b: return 10**9
    ia = struct.unpack('<q', struct.pack('<d', a))[0]; ib = struct.unpack('<q', struct.pack('<d', b))[0]
    if ia < 0: ia = -(ia & 0x7FFFFFFFFFFFFFFF)
    if ib < 0: ib = -(ib & 0x7FFFFFFFFFFFFFFF)
    return abs(ia - ib)
def safe(fn, over):
    try: return fn()
    except (ValueError, OverflowError) as e: return over if isinstance(e, OverflowError) else float('nan')
names = ['sin','cos','tan','atan','asin','acos','exp','ln','log2','log10','pow','atan2','hypot','floor','ceil','round','fmod']
worst = {n: (0, None) for n in names}; count = 0
for line in open(sys.argv[1]):
    p = line.split(); x = f(p[0]); y = f(p[1]); a = abs(x); vals = [f(v) for v in p[2:]]
    ref = [math.sin(x), math.cos(x), math.tan(x), math.atan(x), safe(lambda: math.asin(x/1e9), 0), safe(lambda: math.acos(x/1e9), 0),
           safe(lambda: math.exp(x/100), math.inf), math.log(a) if a > 0 else -math.inf, math.log2(a) if a > 0 else -math.inf, math.log10(a) if a > 0 else -math.inf,
           safe(lambda: math.pow(a, y), math.inf), math.atan2(x, y), math.hypot(x, y), float(math.floor(x)), float(math.ceil(x)),
           math.copysign(math.floor(abs(x) + 0.5), x) if abs(x) < 2**52 else x, math.fmod(x, y)]
    count += 1
    for n, v, r in zip(names, vals, ref):
        u = ulps(v, r)
        if u > worst[n][0]: worst[n] = (u, (x, y, v, r))
print(count, 'samples')
for n in names: print('%-6s max %s ulp %s' % (n, worst[n][0], '' if worst[n][0] <= 1 else worst[n][1]))
sys.exit(1 if any(worst[n][0] > 1 for n in names) else 0)
