// The browser runner, as a module worker: one guest thread, its memory, the hooks, and a stand-in for the
// kernel that answers write, writev, exit, exit_group and brk. It does what the wasmtime runner does, with
// every name and number taken from the translator's own contract.

// Where the thread block and the permission table sit in the state memory.
const BLOCK_AT = 0;
const PERMISSIONS_AT = 4096;

onmessage = ({ data }) => {
  try {
    postMessage(run(data));
  } catch (error) {
    postMessage({ error: String(error?.stack ?? error) });
  }
};

/// A 64 bit memory of `pages` 64 KiB pages. The descriptor takes BigInt sizes for 64 bit memories.
function memory64(pages) {
  return new WebAssembly.Memory({
    initial: BigInt(pages),
    maximum: BigInt(pages),
    address: "i64",
  });
}

function constant(value, type) {
  return new WebAssembly.Global({ value: type, mutable: false }, value);
}

function run({ translatorBytes, blob, codeBytes, load, memoryBytes, regions, sp, stack, lookAhead, omit }) {
  const translator = new WebAssembly.Instance(new WebAssembly.Module(translatorBytes)).exports;
  const result = (length) =>
    new Uint8Array(translator.memory.buffer, translator.output(), length).slice();
  const contract = JSON.parse(new TextDecoder().decode(result(translator.contract())));

  const pageSize = contract.pageSize;
  // The table covers the whole memory, and one more byte, always zero, follows it.
  const permissionPages = (Math.ceil(memoryBytes / 65536) * 65536) / pageSize;
  const memory = memory64(Math.ceil(memoryBytes / 65536));
  const statePages = Math.ceil((PERMISSIONS_AT + permissionPages + 1) / 65536);
  const state = new WebAssembly.Memory({ initial: statePages, maximum: statePages, shared: true });
  const guest = () => new Uint8Array(memory.buffer);
  const flags = () => new Int32Array(state.buffer);

  const stdout = [];
  const stderr = [];
  const counters = new Map();
  const places = new Map();
  const warnings = [];
  let stopped = null;
  let snapshot = null;
  let thread = null;
  const blobs = [];
  // Addresses code was translated from at run time: the table holds a piece of look-ahead none there.
  const translatedFrom = new Set();
  let programBreak = 0;

  // Count a slow path taken at an address (or for a number) and the time it took; warn once per address
  // when its count there reaches the threshold.
  function slowPath(reasonIndex, detail, milliseconds) {
    const reason = contract.reasons[reasonIndex];
    const counter = counters.get(reason) ?? { count: 0, nanoseconds: 0 };
    counter.count += 1;
    counter.nanoseconds += Math.round(milliseconds * 1e6);
    counters.set(reason, counter);
    const place = `${reason} ${detail}`;
    const here = (places.get(place) ?? 0) + 1;
    places.set(place, here);
    if (here === contract.warningThreshold) {
      const warning = { reason, count: counter.count, detail: `0x${BigInt(detail).toString(16)}` };
      console.warn(`wcontainers warning: ${reason}, ${warning.detail}, ${counter.count} so far`);
      warnings.push(warning);
    }
  }
  const reason = (name) => contract.reasons.indexOf(name);

  function registers() {
    thread.exports[contract.saveState]();
    const view = new DataView(state.buffer, BLOCK_AT);
    const x = [];
    for (let i = 0; i < 31; i++) x.push(view.getBigUint64(contract.registerX + 8 * i, true));
    return {
      x,
      sp: view.getBigUint64(contract.registerSp, true),
      pc: view.getBigUint64(contract.registerPc, true),
    };
  }

  function stop(why) {
    snapshot = registers();
    stopped = why;
  }

  function raiseStopFlag() {
    Atomics.store(flags(), (BLOCK_AT + contract.stopFlag) / 4, 1);
  }

  function setPermissions(start, end, permissions, table) {
    const first = Math.floor(start / pageSize);
    const last = Math.ceil(end / pageSize);
    for (let page = first; page < last; page++) table[page] = permissions;
    const bits = (p, read, write) => (p & contract.read ? read : 0) | (p & contract.write ? write : 0);
    const bytes = new Uint8Array(state.buffer);
    for (let page = Math.max(first - 1, 0); page < Math.min(last, permissionPages); page++) {
      const next = page + 1 < permissionPages ? table[page + 1] : 0;
      bytes[PERMISSIONS_AT + page] =
        bits(table[page], contract.pageRead, contract.pageWrite) |
        bits(next, contract.nextPageRead, contract.nextPageWrite);
    }
  }

  // The stand-in kernel.
  function standIn(number, a) {
    const EBADF = 9n;
    const EFAULT = 14n;
    const EINVAL = 22n;
    const ENOSYS = 38n;
    // Linux's limit on the vectors of one writev.
    const IOV_MAX = 1024n;
    // Guest bytes, if every one is on a page the guest may read. Like a write to a pipe on Linux, a buffer
    // that is only partly readable gives a fault and writes nothing.
    const read = (address, length) => {
      const end = address + length;
      if (end > BigInt(memoryBytes)) return null;
      const table = new Uint8Array(state.buffer, PERMISSIONS_AT, permissionPages);
      const size = BigInt(pageSize);
      for (let page = address / size; length > 0n && page <= (end - 1n) / size; page++) {
        if (!(table[Number(page)] & contract.pageRead)) return null;
      }
      return guest().slice(Number(address), Number(end));
    };
    const file = (fd) => (fd === 1n ? stdout : fd === 2n ? stderr : null);
    switch (number) {
      case 64n: {
        const out = file(a[0]);
        if (!out) return -EBADF;
        const bytes = read(a[1], a[2]);
        if (!bytes) return -EFAULT;
        out.push(...bytes);
        return a[2];
      }
      case 66n: {
        const out = file(a[0]);
        if (!out) return -EBADF;
        // Linux takes the vector count as a 32 bit number.
        const count = BigInt.asUintN(32, a[2]);
        if (count > IOV_MAX) return -EINVAL;
        const bytes = read(a[1], 16n * count);
        if (!bytes) return -EFAULT;
        const view = new DataView(bytes.buffer);
        const vectors = [];
        for (let i = 0; i < bytes.length; i += 16) {
          vectors.push([view.getBigUint64(i, true), view.getBigUint64(i + 8, true)]);
        }
        // A length that is negative as a signed number is invalid, checked before anything is read.
        if (vectors.some(([, length]) => BigInt.asIntN(64, length) < 0n)) return -EINVAL;
        const gathered = [];
        for (const [base, length] of vectors) {
          const bytes = read(base, length);
          if (!bytes) return -EFAULT;
          gathered.push(...bytes);
        }
        out.push(...gathered);
        return BigInt(gathered.length);
      }
      case 93n:
      case 94n:
        stop({ kind: "finished", status: Number(BigInt.asIntN(32, a[0])) });
        raiseStopFlag();
        return 0n;
      // brk: this stand-in never moves the break, and answers with the current one as Linux does when it
      // cannot.
      case 214n:
        return BigInt(programBreak);
      default:
        slowPath(reason("system call not implemented"), number, 0);
        return -ENOSYS;
    }
  }

  const hooks = {
    [contract.hookSyscall]: (number, ...a) => standIn(BigInt.asUintN(64, number), a.map((v) => BigInt.asUintN(64, v))),
    [contract.hookStop]: () => {
      Atomics.store(flags(), (BLOCK_AT + contract.stopFlag) / 4, 0);
      // The runner raised the flag itself to make the thread leave.
      if (stopped) return 1;
      // Raised from outside: the stand-in clears it and the thread carries on.
      slowPath(reason("stopped by the flag"), registers().pc, 0);
      return 0;
    },
    [contract.hookMemoryFault]: (address, access) => {
      slowPath(reason("memory fault"), BigInt.asUintN(64, address), 0);
      stop({ kind: "memory fault", address: BigInt.asUintN(64, address).toString(), write: access === 1 });
    },
    [contract.hookLoadCode]: (address) => {
      address = BigInt.asUintN(64, address);
      if (translatedFrom.has(address)) return 1;
      const index = blobs.findIndex(
        (b) => address % 4n === 0n && address >= b.load + b.codeStart && address < b.load + b.codeEnd,
      );
      if (index < 0) {
        stop({ kind: "no code", address: address.toString() });
        return 0;
      }
      const start = performance.now();
      instantiateCode(index, address - blobs[index].load);
      translatedFrom.add(address);
      slowPath(reason("translated at run time"), address, performance.now() - start);
      return 1;
    },
    [contract.hookWarn]: (reasonIndex, detail) => slowPath(reasonIndex, BigInt.asUintN(64, detail), 0),
    [contract.hookTrap]: (kind, detail) => {
      const name = contract.trapKinds[kind];
      stop({ kind: name, detail: BigInt.asUintN(64, detail).toString() });
      if (name === "unsupported") slowPath(reason("unsupported instruction reached"), snapshot.pc, 0);
    },
  };

  // Translate blob `index` from `start` and instantiate it, adding its pieces to the lookup table. Code reached
  // at run time is translated at look-ahead none.
  function instantiateCode(index, start, lookAhead = 0, omit = 0) {
    const b = blobs[index];
    // The translator's memory may grow while it makes room, so its buffer is taken after.
    const at = translator.input(b.bytes.length);
    new Uint8Array(translator.memory.buffer, at, b.bytes.length).set(b.bytes);
    const length = translator.translate(b.codeStart, b.codeEnd, start, lookAhead, omit);
    if (length < 0) throw new Error(`no translation from ${start}`);
    const module = new WebAssembly.Module(result(length));
    new WebAssembly.Instance(module, {
      [contract.hooks]: hooks,
      [contract.env]: {
        [contract.memory]: memory,
        [contract.state]: state,
        [contract.loadAddress]: constant(b.load, "i64"),
        [contract.slotBase]: constant(Number((b.load - tableBase) / 4n), "i32"),
        [contract.tableBase]: constant(tableBase, "i64"),
        [contract.threadBlock]: constant(BLOCK_AT, "i32"),
        [contract.permissions]: constant(PERMISSIONS_AT, "i32"),
        [contract.permissionPages]: constant(BigInt(permissionPages), "i64"),
      },
      [contract.thread]: thread.exports,
    });
  }

  const tableBase = BigInt(load);
  const threadModule = new WebAssembly.Module(result(translator.thread_module(Math.ceil(codeBytes / 4))));
  thread = new WebAssembly.Instance(threadModule, {
    [contract.hooks]: hooks,
    [contract.env]: {
      [contract.state]: state,
      [contract.tableBase]: constant(tableBase, "i64"),
      [contract.threadBlock]: constant(BLOCK_AT, "i32"),
    },
  });

  const table = new Uint8Array(permissionPages);
  for (const [start, end, permissions] of regions) setPermissions(start, end, permissions, table);
  guest().set(blob, load);
  guest().set(stack, sp);
  blobs.push({ bytes: blob, load: BigInt(load), codeStart: 0n, codeEnd: BigInt(codeBytes) });
  programBreak = Math.ceil((load + blob.length) / pageSize) * pageSize;
  instantiateCode(0, 0n, lookAhead, omit);

  const block = new DataView(state.buffer, BLOCK_AT, contract.threadBlockSize);
  for (let i = 0; i < contract.threadBlockSize; i++) block.setUint8(i, 0);
  block.setBigUint64(contract.registerPc, BigInt(load), true);
  block.setBigUint64(contract.registerSp, BigInt(sp), true);
  thread.exports[contract.loadState]();

  const started = performance.now();
  try {
    thread.exports[contract.run]();
  } catch (error) {
    if (!(error instanceof WebAssembly.RuntimeError) || !/out of bounds/.test(error.message)) throw error;
    slowPath(reason("fault without exact registers"), 0, 0);
    stopped = { kind: "fault without exact registers" };
  }
  const runMilliseconds = performance.now() - started;
  return {
    stopped,
    runMilliseconds,
    pc: snapshot ? snapshot.pc.toString() : null,
    stdout,
    stderr,
    counters: [...counters].map(([reason, c]) => ({ reason, ...c })),
    warnings,
  };
}
