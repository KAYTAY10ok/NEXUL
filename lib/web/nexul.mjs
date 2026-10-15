// NEXUL browser host ABI v1. Also usable in Node; no Node-specific imports.
export class NexulExit extends Error {
  constructor(code) { super(`NEXUL exited with code ${code}`); this.code = code; }
}

/** Instantiate a wasm32-web module. source: URL/string, BufferSource or Module. */
export async function loadNexul(source, options = {}) {
  if (typeof source === 'string' || source instanceof URL) {
    const response = await fetch(source);
    if (!response.ok) throw new Error(`NEXUL fetch failed: HTTP ${response.status}`);
    source = await response.arrayBuffer();
  }
  const module = source instanceof WebAssembly.Module ? source : await WebAssembly.compile(source);
  let instance;
  let dead = false;
  let active = false;
  const allocations = new Map();
  const decoders = new Map([[1, new TextDecoder()], [2, new TextDecoder()]]);
  const encoder = new TextEncoder();
  const write = options.write ?? ((fd, text) => (fd === 2 ? console.error : console.log)(text));
  function alive() { if (dead) throw new Error('NEXUL instance terminated; create a new instance'); }
  function uint(value) {
    // Wasm i32 results are signed; addresses are interpreted as unsigned bits.
    if (!Number.isInteger(value) || value < -0x80000000 || value > 0xffffffff) {
      throw new RangeError('expected a wasm32 integer');
    }
    return value >>> 0;
  }
  function bytes(pointer, length) {
    const p = uint(pointer), n = uint(length);
    const buffer = instance.exports.memory.buffer;
    if (p > buffer.byteLength || n > buffer.byteLength - p) throw new RangeError('Wasm memory range out of bounds');
    // Never retain a view across a Wasm call: memory.grow detaches old buffers.
    return new Uint8Array(buffer, p, n);
  }
  const host = {
    nx_write(fd, p, n) {
      if (!decoders.has(fd)) return 1;
      const text = decoders.get(fd).decode(bytes(p, n), {stream: true});
      if (text) write(fd, text);
      return 0;
    },
    nx_exit(code) {
      for (const [fd, decoder] of decoders) {
        const tail = decoder.decode();
        if (tail) write(fd, tail);
      }
      throw new NexulExit(code);
    },
    nx_now_ns(monotonic) {
      return options.nowNs ? options.nowNs(Boolean(monotonic)) :
        monotonic ? BigInt(Math.floor(performance.now() * 1e6)) : BigInt(Date.now()) * 1000000n;
    },
    nx_random(p, n) {
      const view = bytes(p, n);
      if (options.randomBytes) return options.randomBytes(view) === false ? 1 : 0;
      if (!globalThis.crypto?.getRandomValues) return 1;
      for (let start = 0; start < view.length; start += 65536) {
        globalThis.crypto.getRandomValues(view.subarray(start, start + 65536));
      }
      return 0;
    },
  };
  if (options.imports && Object.hasOwn(options.imports, 'nexul_v1')) {
    throw new Error('nexul_v1 is reserved for the runtime host');
  }
  instance = await WebAssembly.instantiate(module, {...options.imports, nexul_v1: host});
  if (!(instance.exports.memory instanceof WebAssembly.Memory)) throw new Error('module must export memory');
  if (instance.exports.nx_abi_version && instance.exports.nx_abi_version() !== 1) throw new Error('unsupported NEXUL web ABI');
  function invoke(name, args, flush = true) {
    alive();
    if (active) throw new Error('reentrant NEXUL calls are unsupported');
    const fn = instance.exports[name];
    if (typeof fn !== 'function') throw new Error(`missing NEXUL export: ${name}`);
    active = true;
    try {
      const result = fn(...args);
      if (flush && instance.exports.nx_flush) instance.exports.nx_flush();
      return result;
    } catch (error) {
      // Neither traps nor JS exceptions unwind NEXUL owners or restore its stack.
      dead = true;
      allocations.clear();
      throw error;
    } finally { active = false; }
  }
  function alloc(length) {
    if (!Number.isInteger(length) || length < 0 || length > 0x7fff0000) throw new RangeError('invalid allocation length');
    const p = uint(invoke('nx_alloc', [length], false));
    allocations.set(p, length);
    return p;
  }
  function free(pointer) {
    alive();
    const p = uint(pointer);
    if (!allocations.has(p)) throw new Error('buffer is not a live host allocation');
    invoke('nx_free', [p], false);
    allocations.delete(p);
  }
  return {
    module,
    memory: instance.exports.memory,
    call(name, ...args) {
      if (name === 'nx_alloc' || name === 'nx_free' || name === '_start') throw new Error('use alloc, free or run');
      return invoke(name, args);
    },
    alloc, free,
    readBytes: (p, n) => bytes(p, n).slice(),
    readText: (p, n) => new TextDecoder('utf-8', {fatal: true}).decode(bytes(p, n)),
    writeBytes(p, data) {
      alive();
      const address = uint(p);
      if (!(data instanceof Uint8Array)) throw new TypeError('expected Uint8Array');
      if (!allocations.has(address) || data.length > allocations.get(address)) throw new RangeError('write exceeds host allocation');
      bytes(address, data.length).set(data);
    },
    withText(text, callback) {
      const data = encoder.encode(text);
      const p = alloc(data.length);
      bytes(p, data.length).set(data);
      try {
        const result = callback(p, data.length);
        if (result && typeof result.then === 'function') throw new TypeError('withText callback must be synchronous');
        return result;
      } finally { if (!dead) free(p); }
    },
    run() {
      try { invoke('_start', [], false); }
      catch (error) { if (error instanceof NexulExit && error.code === 0) return; throw error; }
    },
  };
}
