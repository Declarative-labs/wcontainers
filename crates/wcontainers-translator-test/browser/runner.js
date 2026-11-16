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

function run({
  translatorBytes,
  blob,
  codeBytes,
  load,
  memoryBytes,
  regions,
  sp,
  stack,
  lookAhead,
  bound,
  omit,
  repeat,
  cases,
  caseBytes,
  areas,
  stopAtSyscall,
}) {
  const translator = new WebAssembly.Instance(new WebAssembly.Module(translatorBytes)).exports;
  const result = (length) =>
    new Uint8Array(translator.memory.buffer, translator.output(), length).slice();
  const contract = JSON.parse(new TextDecoder().decode(result(translator.contract())));
  // What this engine does where WebAssembly leaves results open.
  const probe = new WebAssembly.Instance(new WebAssembly.Module(result(translator.probe_module()))).exports[
    contract.probe
  ]();
  const startedAt = performance.now();

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
  // The whole thread state when the thread stopped, which a system call's result may change afterwards.
  let stateAtStop = null;
  let thread = null;
  // What each page allows, as the permission table says.
  const pagePermissions = new Uint8Array(permissionPages);
  // By address, the piece of look-ahead none of the one instruction there, for the table's last slot.
  const exactPieces = new Map();
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

  // The registers, from the thread block, where they all are whenever a hook runs or the thread has left.
  function registers() {
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
    stateAtStop = new Uint8Array(state.buffer, BLOCK_AT, contract.threadStateSize).slice();
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
    [contract.hookSyscall]: (number, ...a) => {
      // Instruction cases end at a system call, as the reference's do.
      if (stopAtSyscall) {
        stop({ kind: "syscall" });
        raiseStopFlag();
        return 0n;
      }
      return standIn(BigInt.asUintN(64, number), a.map((v) => BigInt.asUintN(64, v)));
    },
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
    // Code runs only from pages that allow it, as on Linux: a jump anywhere else faults there. Otherwise the
    // code is translated as it is in memory now, which may be code the program wrote: the blob's code, or the
    // run of pages that allow running code within the addresses the table covers.
    [contract.hookLoadCode]: (address, exact) => {
      address = BigInt.asUintN(64, address);
      const table = thread.exports[contract.lookup];
      const last = table.length - 1;
      if (exact && exactPieces.has(address)) {
        table.set(last, exactPieces.get(address));
        return 1;
      }
      const page = Number(address / BigInt(pageSize));
      const executable = (p) => p >= 0 && p < permissionPages && (pagePermissions[p] & contract.execute) !== 0;
      if (!executable(page)) {
        slowPath(reason("memory fault"), address, 0);
        stop({ kind: "memory fault", address: address.toString(), write: false, execute: true });
        return 0;
      }
      const coveredEnd = tableBase + 4n * BigInt(last);
      if (address < tableBase || address >= coveredEnd) {
        stop({ kind: "no code", address: address.toString() });
        return 0;
      }
      let first = page;
      while (executable(first - 1)) first--;
      let end = page + 1;
      while (executable(end)) end++;
      const known = address >= loaded.load + loaded.codeStart && address < loaded.load + loaded.codeEnd;
      const runStart = known ? loaded.load + loaded.codeStart : BigInt(Math.max(first * pageSize, Number(tableBase)));
      const runEnd = known ? loaded.load + loaded.codeEnd : BigInt(Math.min(end * pageSize, Number(coveredEnd)));
      const start = performance.now();
      const code = { bytes: guest().slice(Number(runStart), Number(runEnd)), load: runStart };
      const offset = address - runStart;
      if (exact) {
        instantiateCode(code, offset, 2, 0, 1, BigInt(last) - offset / 4n);
        exactPieces.set(address, table.get(last));
      } else {
        // Code made at run time never takes the place of a piece the table holds.
        const slot = Number((address - tableBase) / 4n);
        if (table.get(slot) !== null) throw new Error(`code asked for at 0x${address.toString(16)}, which has a piece`);
        // The reference stays at none throughout.
        if (lookAhead === 0) instantiateCode(code, offset, 0, 0);
        else instantiateCode(code, offset, 2, 0, contract.bound);
      }
      slowPath(reason("translated at run time"), address, performance.now() - start);
      return 1;
    },
    [contract.hookWarn]: (reasonIndex, detail) => slowPath(reasonIndex, BigInt.asUintN(64, detail), 0),
    [contract.hookFloat]: (operation, a, b, c) => {
      const block = new DataView(state.buffer, BLOCK_AT, contract.threadStateSize);
      const fpcr = block.getUint32(contract.registerFpcr, true);
      const tracking = block.getUint32(contract.floatTracking, true);
      const value = translator.float(operation, a, b, c, fpcr, block.getUint32(contract.registerFpsr, true));
      block.setUint32(contract.registerFpsr, translator.float_status(), true);
      const fused = translator.float_reason(operation, fpcr, tracking) === 1;
      // Registers are not saved here, so the detail is the operation, not the address.
      slowPath(reason(fused ? "software fused multiply-add" : "exact floating point requested"), operation >>> 0, 0);
      return value;
    },
    [contract.hookClock]: () => BigInt(Math.round((performance.now() - startedAt) * 1e6)),
    // ic ivau: forget the pieces the table holds for the 64 byte line.
    [contract.hookCodeChanged]: (address) => {
      const line = BigInt.asUintN(64, address) & ~63n;
      const table = thread.exports[contract.lookup];
      for (let at = line; at < line + 64n; at += 4n) {
        const slot = (at - tableBase) / 4n;
        if (at >= tableBase && slot < BigInt(table.length - 1)) table.set(Number(slot), null);
        exactPieces.delete(at);
      }
    },
    [contract.hookTrap]: (kind, detail) => {
      const name = contract.trapKinds[kind];
      stop({ kind: name, detail: BigInt.asUintN(64, detail).toString() });
      if (name === "unsupported") slowPath(reason("unsupported instruction reached"), snapshot.pc, 0);
    },
  };

  // Translate code `b` (its bytes, where they are, and where in them the code is, all of it when not said)
  // from `start` and instantiate it, adding its pieces to the lookup table at their addresses, or from
  // `slotBase` when given.
  function instantiateCode(b, start, lookAhead, omit, bound = 0, slotBase = null) {
    // The translator's memory may grow while it makes room, so its buffer is taken after.
    const at = translator.input(b.bytes.length);
    new Uint8Array(translator.memory.buffer, at, b.bytes.length).set(b.bytes);
    const length = translator.translate(
      b.codeStart ?? 0n,
      b.codeEnd ?? BigInt(b.bytes.length),
      start,
      lookAhead,
      bound,
      probe,
      omit,
    );
    if (length < 0) throw new Error(`no translation from ${start}`);
    const module = new WebAssembly.Module(result(length));
    new WebAssembly.Instance(module, {
      [contract.hooks]: hooks,
      [contract.env]: {
        [contract.memory]: memory,
        [contract.state]: state,
        [contract.loadAddress]: constant(b.load, "i64"),
        [contract.slotBase]: constant(Number(BigInt.asIntN(32, slotBase ?? (b.load - tableBase) / 4n)), "i32"),
        [contract.tableBase]: constant(tableBase, "i64"),
        [contract.threadBlock]: constant(BLOCK_AT, "i32"),
        [contract.permissions]: constant(PERMISSIONS_AT, "i32"),
        [contract.permissionPages]: constant(BigInt(permissionPages), "i64"),
      },
      [contract.thread]: thread.exports,
    });
  }

  const tableBase = BigInt(load);
  // The table covers the whole blob, every page of it that may run code, so code the program writes runs.
  const threadModule = new WebAssembly.Module(result(translator.thread_module(Math.ceil(blob.length / 4))));
  thread = new WebAssembly.Instance(threadModule, {
    [contract.hooks]: hooks,
    [contract.env]: {
      [contract.state]: state,
      [contract.tableBase]: constant(tableBase, "i64"),
      [contract.threadBlock]: constant(BLOCK_AT, "i32"),
    },
  });

  for (const [start, end, permissions] of regions) setPermissions(start, end, permissions, pagePermissions);
  guest().set(blob, load);
  const loaded = { bytes: blob, load: BigInt(load), codeStart: 0n, codeEnd: BigInt(codeBytes) };
  programBreak = Math.ceil((load + blob.length) / pageSize) * pageSize;
  instantiateCode(loaded, 0n, lookAhead, omit, bound);

  if (cases) return runCases();

  // Instruction cases, one after another in the same memory and instances: each is `codeBytes` of code put
  // at the blob's place and translated there, the thread state, then the bytes of each of `areas`.
  // Each gives how it stopped, the thread state after, and the bytes of the areas that changed, as
  // [offset into the areas, value].
  function runCases() {
    const stateSize = contract.threadStateSize;
    const placed = areas.reduce((sum, [, length]) => sum + length, 0);
    const size = codeBytes + stateSize + placed;
    const results = [];
    for (let i = 0; i < cases; i++) {
      const bytes = caseBytes.subarray(i * size, (i + 1) * size);
      const code = bytes.slice(0, codeBytes);
      guest().set(code, load);
      loaded.bytes = code;
      exactPieces.clear();
      instantiateCode(loaded, 0n, lookAhead, omit, bound);
      new Uint8Array(state.buffer, BLOCK_AT, stateSize).set(bytes.subarray(codeBytes, codeBytes + stateSize));
      let at = codeBytes + stateSize;
      for (const [address, length] of areas) {
        guest().set(bytes.subarray(at, at + length), address);
        at += length;
      }
      stopped = null;
      snapshot = null;
      stateAtStop = null;
      thread.exports[contract.run]();
      const changed = [];
      let offset = 0;
      at = codeBytes + stateSize;
      for (const [address, length] of areas) {
        const now = guest().subarray(address, address + length);
        for (let j = 0; j < length; j++) if (now[j] !== bytes[at + j]) changed.push([offset + j, now[j]]);
        offset += length;
        at += length;
      }
      results.push({ stopped, state: [...stateAtStop], changed });
    }
    return { cases: results, counters: [...counters].map(([reason, c]) => ({ reason, ...c })) };
  }

  // Each run starts the program afresh in the same instances: the blob and the stack placed again, the
  // registers as at the start, and the output of earlier runs dropped.
  // The page raises the stop flag a few times early on (see run.mjs), so it needs the state memory.
  postMessage({ stopFlag: { buffer: state.buffer, index: (BLOCK_AT + contract.stopFlag) / 4 } });
  const runs = [];
  for (let run = 0; run < repeat; run++) {
    guest().set(blob, load);
    guest().set(stack, sp);
    stdout.length = 0;
    stderr.length = 0;
    stopped = null;
    snapshot = null;
    const block = new DataView(state.buffer, BLOCK_AT, contract.threadStateSize);
    for (let i = 0; i < contract.threadStateSize; i++) block.setUint8(i, 0);
    block.setBigUint64(contract.registerPc, BigInt(load), true);
    block.setBigUint64(contract.registerSp, BigInt(sp), true);

    const started = performance.now();
    try {
      thread.exports[contract.run]();
    } catch (error) {
      if (!(error instanceof WebAssembly.RuntimeError) || !/out of bounds/.test(error.message)) throw error;
      slowPath(reason("fault without exact registers"), 0, 0);
      stopped = { kind: "fault without exact registers" };
    }
    runs.push(performance.now() - started);
  }
  return {
    stopped,
    runs,
    pc: snapshot ? snapshot.pc.toString() : null,
    stdout,
    stderr,
    counters: [...counters].map(([reason, c]) => ({ reason, ...c })),
    warnings,
  };
}
