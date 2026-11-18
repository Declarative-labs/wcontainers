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
  fill,
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
  moduleBytes,
  codeStart,
  optimise,
  splitBytes,
  // The stand-in's system call numbers for the guest.
  standIn: calls,
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
  // Modules translated ahead of time from an image have an array of slots for their code alone, from its
  // start.
  const tableBase = BigInt(load + (codeStart ?? 0));
  // The array covers the whole blob, every page of it that may run code, so code the program writes runs;
  // for modules translated ahead of time from an image, its code; for instruction cases, the instruction's
  // address only, so control that lands anywhere else stops there.
  const align = BigInt(contract.align);
  const slots = cases
    ? 1
    : Math.ceil((moduleBytes.length ? codeBytes - codeStart : blob.length) / contract.align);
  if (BLOCK_AT + contract.threadStateSize > PERMISSIONS_AT) {
    throw new Error("the thread state does not fit before the permission table");
  }
  const slotsAt = Math.ceil((PERMISSIONS_AT + permissionPages + 1) / 4) * 4;
  const statePages = Math.ceil((slotsAt + 4 * slots) / 65536);
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
  // By address, the piece of look-ahead none of the one instruction there, for the exact slot.
  const exactPieces = new Map();
  // The lookup table's next free slot.
  let nextSlot = contract.exactSlot + 1;
  // Where the array holds the slot for an address, as an index of 32 bit words, or null if it covers none.
  function slotEntry(address) {
    if (address < tableBase || address >= tableBase + align * BigInt(slots) || address % align !== 0n) return null;
    return slotsAt / 4 + Number((address - tableBase) / align);
  }
  function setSlot(address, slot) {
    const at = slotEntry(address);
    if (at !== null) new Uint32Array(state.buffer)[at] = slot;
  }
  const slotOf = (address) => {
    const at = slotEntry(address);
    return at === null ? 0 : new Uint32Array(state.buffer)[at];
  };
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

  // The pc, from the thread block, where every register is whenever a hook runs or the thread has left.
  function registers() {
    return { pc: new DataView(state.buffer, BLOCK_AT).getBigUint64(contract.pcOffset, true) };
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
      case BigInt(calls.write): {
        const out = file(a[0]);
        if (!out) return -EBADF;
        const bytes = read(a[1], a[2]);
        if (!bytes) return -EFAULT;
        out.push(...bytes);
        return a[2];
      }
      case BigInt(calls.writev): {
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
      case BigInt(calls.exit):
      case BigInt(calls.exitGroup):
        stop({ kind: "finished", status: Number(BigInt.asIntN(32, a[0])) });
        raiseStopFlag();
        return 0n;
      // brk: this stand-in never moves the break, and answers with the current one as Linux does when it
      // cannot.
      case BigInt(calls.brk):
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
      address = BigInt.asUintN(64, address);
      slowPath(reason("memory fault"), address, 0);
      const fault = memoryFault(address);
      if (fault === "page") stop({ kind: "memory fault", address: address.toString(), write: access === 1 });
      else stop({ kind: fault });
    },
    // Code runs only from pages that allow it, as on Linux: a jump anywhere else faults there. Otherwise the
    // code is translated as it is in memory now, which may be code the program wrote: the blob's code, or the
    // run of pages that allow running code within the addresses the table covers.
    [contract.hookLoadCode]: (address, exact) => {
      address = BigInt.asUintN(64, address);
      const table = thread.exports[contract.lookup];
      if (exact && exactPieces.has(address)) {
        table.set(contract.exactSlot, exactPieces.get(address));
        return 1;
      }
      const page = Number(address / BigInt(pageSize));
      const executable = (p) => p >= 0 && p < permissionPages && (pagePermissions[p] & contract.execute) !== 0;
      if (!executable(page)) {
        slowPath(reason("memory fault"), address, 0);
        stop({ kind: "memory fault", address: address.toString(), write: false, execute: true });
        return 0;
      }
      const coveredEnd = tableBase + align * BigInt(slots);
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
        instantiateCode(code, offset, 2, 0, 1, true);
        exactPieces.set(address, table.get(contract.exactSlot));
      } else {
        // Code made at run time never takes the place of a piece the lookup finds.
        if (slotOf(address) !== 0) throw new Error(`code asked for at 0x${address.toString(16)}, which has a piece`);
        // The reference stays at none throughout.
        if (lookAhead === 0) instantiateCode(code, offset, 0, 0);
        else instantiateCode(code, offset, 2, 0, contract.bound);
      }
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
  // The guest's own hooks, by who does their work.
  const cpuState = translator.cpu_state();

  // What a memory fault at `address` is on the guest CPU, from the thread's state and the instruction's
  // bytes, which the translator's `cpu_memory_fault` reads at `cpu_state`.
  function memoryFault(address) {
    const size = contract.threadStateSize;
    new Uint8Array(translator.memory.buffer, cpuState, size).set(new Uint8Array(state.buffer, BLOCK_AT, size));
    const pc = Number(registers().pc);
    const memory = guest();
    const code = memory.subarray(pc, Math.min(pc + contract.maxLength, memory.length));
    new Uint8Array(translator.memory.buffer, cpuState + size, code.length).set(code);
    return contract.memoryFaults[translator.cpu_memory_fault(address, code.length)];
  }
  // An argument as `cpu_hook` takes it: i64 arguments arrive as BigInt, i32 ones as numbers.
  const wide = (value) => (typeof value === "bigint" ? value : BigInt(value));
  // A result as the hook returns it.
  const narrow = (value, type) => (type === "i32" ? Number(BigInt.asIntN(32, value)) : value);
  contract.guestHooks.forEach(({ name, body, line, stateAt, stateLength, results }, index) => {
    if (body === "clock") {
      hooks[name] = () => BigInt(Math.round((performance.now() - startedAt) * 1e6));
    } else if (body === "codeChanged") {
      // Forget the pieces the lookup finds for the line of code holding the address.
      const bytes = BigInt(line);
      hooks[name] = (address) => {
        const start = BigInt.asUintN(64, address) & ~(bytes - 1n);
        for (let at = start; at < start + bytes; at += BigInt(contract.align)) {
          setSlot(at, 0);
          exactPieces.delete(at);
        }
      };
    } else {
      // The work is the translator's `cpu_hook` on a copy of the hook's bytes of the thread's state; a counted
      // slow path.
      // The hook's bytes in the state memory, whose views stay valid as it never detaches, and at
      // `cpu_state`, made again only once the translator's memory has grown, which detaches them.
      const inState = new Uint8Array(state.buffer, BLOCK_AT + stateAt, stateLength);
      let inTranslator = new Uint8Array(translator.memory.buffer, cpuState, stateLength);
      const translatorView = () => {
        if (inTranslator.byteLength === 0) inTranslator = new Uint8Array(translator.memory.buffer, cpuState, stateLength);
        return inTranslator;
      };
      // `cpu_hook` takes four arguments after the index; the arguments a hook does not have are 0.
      if (contract.hookParams !== 4) throw new Error(`cpu_hook takes ${contract.hookParams} arguments`);
      hooks[name] = (a = 0, b = 0, c = 0, d = 0) => {
        translatorView().set(inState);
        translator.cpu_hook(index, wide(a), wide(b), wide(c), wide(d));
        inState.set(translatorView());
        const why = translator.cpu_hook_reason();
        // Registers are not saved here, so the detail is the first argument, not the address.
        if (why >= 0) slowPath(why, typeof a === "bigint" ? BigInt.asUintN(64, a) : a >>> 0, 0);
        if (results.length === 1) return narrow(translator.cpu_hook_result(0), results[0]);
        return results.map((type, i) => narrow(translator.cpu_hook_result(i), type));
      };
    }
  });

  // Translate code `b` (its bytes, where they are, and where in them the code is, all of it when not said)
  // from `start` and instantiate it, as `instantiateModule` says.
  function instantiateCode(b, start, lookAhead, omit, bound = 0, exact = false) {
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
      optimise ?? 0,
      splitBytes ?? 0,
    );
    if (length < 0) throw new Error(`no translation from ${start}`);
    const modules = [result(length)];
    for (let i = 0; i < translator.more_modules(); i++) modules.push(result(translator.more_module(i)));
    for (const bytes of modules) instantiateModule(new WebAssembly.Module(bytes), b, exact);
  }

  // The blob offsets of a code module's pieces, in slot order, from its description section.
  function pieceOffsets(module) {
    const [section] = WebAssembly.Module.customSections(module, contract.descriptionSection);
    const bytes = new Uint8Array(section);
    let at = 3;
    const leb = () => {
      let value = 0n;
      let shift = 0n;
      for (;;) {
        const byte = bytes[at++];
        value |= BigInt(byte & 0x7f) << shift;
        shift += 7n;
        if (byte < 0x80) return value;
      }
    };
    leb();
    at += 32;
    const count = Number(leb());
    return Array.from({ length: count }, leb);
  }

  // Instantiate a code module for code `b`, putting its pieces in new slots of the lookup table and giving
  // each piece's address its slot; with `exact`, its one piece goes to the exact slot and to no address.
  function instantiateModule(module, b, exact) {
    const offsets = pieceOffsets(module);
    const table = thread.exports[contract.lookup];
    const first = exact ? contract.exactSlot : nextSlot;
    if (!exact) {
      nextSlot += offsets.length;
      if (nextSlot > table.length) table.grow(nextSlot - table.length);
    }
    new WebAssembly.Instance(module, {
      [contract.hooks]: hooks,
      [contract.env]: {
        [contract.memory]: memory,
        [contract.state]: state,
        [contract.loadAddress]: constant(b.load, "i64"),
        [contract.slotBase]: constant(first, "i32"),
        [contract.tableBase]: constant(tableBase, "i64"),
        [contract.threadBlock]: constant(BLOCK_AT, "i32"),
        [contract.permissions]: constant(PERMISSIONS_AT, "i32"),
        [contract.permissionPages]: constant(BigInt(permissionPages), "i64"),
        [contract.slots]: constant(slotsAt, "i32"),
        [contract.slotsCover]: constant(align * BigInt(slots), "i64"),
      },
      [contract.thread]: thread.exports,
    });
    if (!exact) offsets.forEach((offset, i) => setSlot(b.load + offset, first + i));
  }

  const threadModule = new WebAssembly.Module(result(translator.thread_module()));
  thread = new WebAssembly.Instance(threadModule, {
    [contract.hooks]: hooks,
    [contract.env]: {
      [contract.state]: state,
      [contract.tableBase]: constant(tableBase, "i64"),
      [contract.threadBlock]: constant(BLOCK_AT, "i32"),
      [contract.slots]: constant(slotsAt, "i32"),
      [contract.slotsCover]: constant(align * BigInt(slots), "i64"),
    },
  });

  for (const [start, end, permissions] of regions) setPermissions(start, end, permissions, pagePermissions);
  // The code area of instruction cases holds the guest's filler around the instruction: [start, end, byte].
  // Fresh memory is already 0, and writing it would only take memory.
  if (fill && fill[2] !== 0) guest().fill(fill[2], fill[0], fill[1]);
  guest().set(blob, load);
  const loaded = { bytes: blob, load: BigInt(load), codeStart: 0n, codeEnd: BigInt(codeBytes) };
  programBreak = Math.ceil((load + blob.length) / pageSize) * pageSize;
  if (moduleBytes.length) return loadModules();
  if (cases) return runCases();
  instantiateCode(loaded, 0n, lookAhead, omit, bound);

  // Code modules translated ahead of time, loaded only: how long making each module and its instance took.
  function loadModules() {
    let compiling = 0;
    let instantiating = 0;
    for (const bytes of moduleBytes) {
      const started = performance.now();
      const module = new WebAssembly.Module(bytes);
      const compiled = performance.now();
      instantiateModule(module, loaded, false);
      compiling += compiled - started;
      instantiating += performance.now() - compiled;
    }
    return { compiling, instantiating };
  }

  // Instruction cases, one after another in the same memory and instances: each is the instruction's length
  // (a byte), then `codeBytes` of code (the instruction, then filler) put at the blob's place, of which the
  // instruction alone is translated there, the thread state, then the bytes of each of `areas`.
  // Each gives how it stopped, the thread state after, and the bytes of the areas that changed, as
  // [offset into the areas, value].
  function runCases() {
    const stateSize = contract.threadStateSize;
    const placed = areas.reduce((sum, [, length]) => sum + length, 0);
    const size = 1 + codeBytes + stateSize + placed;
    const stateAt = 1 + codeBytes;
    const results = [];
    for (let i = 0; i < cases; i++) {
      const bytes = caseBytes.subarray(i * size, (i + 1) * size);
      const code = bytes.slice(1, stateAt);
      guest().set(code, load);
      loaded.bytes = code;
      loaded.codeEnd = BigInt(bytes[0]);
      exactPieces.clear();
      instantiateCode(loaded, 0n, lookAhead, omit, bound);
      new Uint8Array(state.buffer, BLOCK_AT, stateSize).set(bytes.subarray(stateAt, stateAt + stateSize));
      let at = stateAt + stateSize;
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
      at = stateAt + stateSize;
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
    const start = new Uint8Array(state.buffer, BLOCK_AT, contract.threadStateSize);
    start.fill(0);
    start.set(result(translator.initial_block(BigInt(load), BigInt(sp))));

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
