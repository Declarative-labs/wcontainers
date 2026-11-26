// The browser runner, as a module worker: one guest thread, its memory, the hooks, and a stand-in for the
// kernel that answers write, writev, exit, exit_group and brk. It does what the wasmtime runner does, with
// every name and number taken from the translator's own contract.

// Where the thread block and the permission table sit in the state memory.
const BLOCK_AT = 0;
const PERMISSIONS_AT = 4096;

// Whether the float flags (ARM64's FPSR's) are kept exact from the thread's start, from the setup.
let keepFloatFlags = false;

onmessage = ({ data }) => {
  keepFloatFlags = data.keepFloatFlags;
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
  linkAddress,
  // The key of the features the program is told, the default list when not given.
  offer,
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
  // The names and numbers the runner shares with the wasmtime runner.
  runner,
}) {
  // The translator's exports are wasm-bindgen's, named by its processing; its JavaScript is not used, and
  // nothing here calls what the module imports for it.
  const translatorModule = new WebAssembly.Module(translatorBytes);
  const translatorImports = {};
  for (const { module, name, kind } of WebAssembly.Module.imports(translatorModule)) {
    if (kind !== "function") throw new Error(`the translator imports the ${kind} ${module}.${name}`);
    (translatorImports[module] ??= {})[name] = () => {
      throw new Error(`the translator called ${module}.${name}`);
    };
  }
  const translator = new WebAssembly.Instance(translatorModule, translatorImports).exports;
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
  // What each page allows, as the kernel said; the table leaves out what the locks hold back.
  const pagePermissions = new Uint8Array(permissionPages);
  // The write locks held on each page, a bit for each reason (`runner.lockCode`: the page holds translated code).
  const LOCK_CODE = runner.lockCode;
  // The names of the kinds of stop, a trap's among them.
  const stops = runner.stopKinds;
  const locks = new Uint8Array(permissionPages);
  // Translate the instruction at this address without locking its page, at the next exact load there.
  let exactMark = null;
  // Each translation instantiated: the guest bytes it was made from, the addresses it gave pieces.
  let translations = [];
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
  // The thread left to give up its WebAssembly frames, and runs on from its registers.
  let framesGivenUp = false;
  function runThread() {
    do {
      framesGivenUp = false;
      thread.exports[contract.run]();
    } while (framesGivenUp);
  }

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
  // Each slow path's number by its name in camel case (`reason.memoryFault` for "memory fault").
  const reason = new Proxy(
    Object.fromEntries(contract.reasons.map((name, i) => [name?.replace(/ (\w)/g, (_, c) => c.toUpperCase()), i])),
    {
      get(numbers, key) {
        if (!(key in numbers)) throw new Error(`no slow path is named ${String(key)}`);
        return numbers[key];
      },
    },
  );

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

  // What the table says a page allows: what the kernel said, without writing while a lock is held.
  const allowed = (page) =>
    page < permissionPages ? (locks[page] ? pagePermissions[page] & ~contract.write : pagePermissions[page]) : 0;
  function writePermissionByte(page) {
    const bits = (p, read, write) => (p & contract.read ? read : 0) | (p & contract.write ? write : 0);
    new Uint8Array(state.buffer)[PERMISSIONS_AT + page] =
      bits(allowed(page), contract.pageRead, contract.pageWrite) |
      bits(allowed(page + 1), contract.nextPageRead, contract.nextPageWrite);
  }

  function setPermissions(start, end, permissions) {
    const first = Math.floor(start / pageSize);
    const last = Math.ceil(end / pageSize);
    for (let page = first; page < last; page++) pagePermissions[page] = permissions;
    for (let page = Math.max(first - 1, 0); page < Math.min(last, permissionPages); page++) writePermissionByte(page);
  }

  function setLock(page, lock, held) {
    const before = locks[page];
    locks[page] = held ? before | lock : before & ~lock;
    if ((before === 0) !== (locks[page] === 0)) {
      writePermissionByte(page);
      if (page > 0) writePermissionByte(page - 1);
    }
  }

  // Lock for code every page of the guest byte ranges a translation was made from. Given the stack pointer of
  // the thread in progress, whether a page newly locked may lie in a frame it proved writable: at or above the
  // stack pointer less the red zone, with every page from there to it readable and writable.
  function lockCode(read, sp = null) {
    if (!contract.codeChangesDetected) return false;
    const size = BigInt(pageSize);
    const pages = [];
    for (const [start, end] of read) {
      for (let page = start / size; page < (end + size - 1n) / size; page++) {
        if (page < BigInt(permissionPages)) pages.push(Number(page));
      }
    }
    let inFrame = false;
    if (sp !== null && sp >= BigInt(contract.redZone)) {
      const first = Number((sp - BigInt(contract.redZone)) / size);
      const above = pages.filter((p) => p >= first);
      const frame = contract.read | contract.write;
      let end = first;
      if (above.length) {
        const last = Math.max(...above);
        while (end <= last && (allowed(end) & frame) === frame) end++;
      }
      inFrame = above.some((p) => p < end);
    }
    for (const page of pages) setLock(page, LOCK_CODE, true);
    return inFrame;
  }

  // Code was written on a page locked for code: the lookup forgets every piece of every translation made from
  // the page, and the lock is given back. Counted.
  function codeWritten(page) {
    const start = BigInt(page * pageSize);
    const end = start + BigInt(pageSize);
    const touches = (read) => read.some(([s, e]) => s < end && start < e);
    translations = translations.filter((t) => {
      if (!touches(t.read)) return true;
      for (const address of t.addresses) setSlot(address, 0);
      return false;
    });
    exactPieces.clear();
    setLock(page, LOCK_CODE, false);
    slowPath(reason.codeWrittenWithoutAnnouncement, start, 0);
  }

  // The stand-in kernel.
  function standIn(number, a) {
    const EPERM = 1n;
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
        stop({ kind: stops.finished, status: Number(BigInt.asIntN(32, a[0])) });
        raiseStopFlag();
        return 0n;
      // brk: this stand-in never moves the break, and answers with the current one as Linux does when it
      // cannot.
      case BigInt(calls.brk):
        return BigInt(programBreak);
      default: {
        const set = calls.setBase;
        const base = set && number === BigInt(set.number) && set.bases.find(([code]) => BigInt(code) === a[0]);
        if (base) {
          if (a[1] >= BigInt(set.limit)) return -EPERM;
          new DataView(state.buffer, BLOCK_AT).setBigUint64(base[1], a[1], true);
          return 0n;
        }
        slowPath(reason.systemCallNotImplemented, number, 0);
        return -ENOSYS;
      }
    }
  }

  const hooks = {
    [contract.hookSyscall]: (number, ...a) => {
      exactMark = null;
      // Instruction cases end at a system call, as the reference's do.
      if (stopAtSyscall) {
        stop({ kind: stops.systemCall });
        raiseStopFlag();
        return 0n;
      }
      const result = standIn(BigInt.asUintN(64, number), a.map((v) => BigInt.asUintN(64, v)));
      if (!stopped && step(contract.stepAt.kernel, 0n) === "run") raiseStopFlag();
      return result;
    },
    [contract.hookStop]: () => {
      exactMark = null;
      Atomics.store(flags(), (BLOCK_AT + contract.stopFlag) / 4, 0);
      // The runner raised the flag itself to make the thread leave.
      if (stopped) return contract.unwindLeave;
      switch (step(contract.stepAt.stop, registers().pc)) {
        case "trap":
          stop({ kind: stops.singleStep });
          return contract.unwindLeave;
        case "run":
          raiseStopFlag();
          return 0;
        case "exact":
          raiseStopFlag();
          return contract.unwindGoToExact;
      }
      // Raised from outside: the stand-in clears it and the thread carries on.
      slowPath(reason.stoppedByTheFlag, registers().pc, 0);
      return 0;
    },
    [contract.hookMemoryFault]: (address, access) => {
      address = BigInt.asUintN(64, address);
      exactMark = null;
      const page = Number(address / BigInt(pageSize));
      if (access !== 0 && page < permissionPages && locks[page] & LOCK_CODE && pagePermissions[page] & contract.write) {
        codeWritten(page);
        exactMark = registers().pc;
        step(contract.stepAt.retry, exactMark);
        return contract.unwindGoToExact;
      }
      slowPath(reason.memoryFault, address, 0);
      const fault = memoryFault(address);
      if (fault === stops.memoryFault) stop({ kind: fault, address: address.toString(), write: access !== 0 });
      else stop({ kind: fault });
      return contract.unwindLeave;
    },
    // Code runs only from pages that allow it, as on Linux: a jump anywhere else faults there. Otherwise the
    // code is translated as it is in memory now, which may be code the program wrote: the blob's code, or the
    // run of pages that allow running code within the addresses the table covers.
    [contract.hookLoadCode]: (address, exact) => {
      address = BigInt.asUintN(64, address);
      const table = thread.exports[contract.lookup];
      const unlocked = exact && exactMark === address;
      exactMark = null;
      if (exact && !unlocked && exactPieces.has(address)) {
        table.set(contract.exactSlot, exactPieces.get(address));
        return 1;
      }
      const page = Number(address / BigInt(pageSize));
      const executable = (p) => p >= 0 && p < permissionPages && (pagePermissions[p] & contract.execute) !== 0;
      if (!executable(page)) {
        slowPath(reason.memoryFault, address, 0);
        stop({ kind: stops.memoryFault, address: address.toString(), write: false, execute: true });
        return 0;
      }
      const coveredEnd = tableBase + align * BigInt(slots);
      if (address < tableBase || address >= coveredEnd) {
        stop({ kind: stops.noCode, address: address.toString() });
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
      const sp = new DataView(state.buffer, BLOCK_AT).getBigUint64(contract.spOffset, true);
      let inFrame;
      if (exact) {
        inFrame = instantiateCode(code, offset, 2, 0, 1, true, unlocked, sp);
        // A piece of an unlocked page may be written over unseen, so it is not kept.
        if (!unlocked) exactPieces.set(address, table.get(contract.exactSlot));
      } else {
        // Code made at run time never takes the place of a piece the lookup finds.
        if (slotOf(address) !== 0) throw new Error(`code asked for at 0x${address.toString(16)}, which has a piece`);
        // The reference stays at none throughout.
        if (lookAhead === 0) inFrame = instantiateCode(code, offset, 0, 0, 0, false, false, sp);
        else inFrame = instantiateCode(code, offset, 2, 0, contract.bound, false, false, sp);
      }
      slowPath(reason.translatedAtRunTime, address, performance.now() - start);
      // A frame in progress may have been proved writable over the page just locked, so the thread gives up
      // its frames and carries on from its registers (plan 5.7, "Frames").
      if (inFrame) {
        slowPath(reason.codeTranslatedFromALiveStackFrame, address, 0);
        framesGivenUp = true;
        return 0;
      }
      return 1;
    },
    [contract.hookWarn]: (reasonIndex, detail) => slowPath(reasonIndex, BigInt.asUintN(64, detail), 0),
    [contract.hookTrap]: (kind, detail) => {
      exactMark = null;
      const name = contract.trapKinds[kind];
      stop({ kind: name, detail: BigInt.asUintN(64, detail).toString() });
      if (name === stops.unsupported) slowPath(reason.unsupportedInstructionReached, snapshot.pc, 0);
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
    return runner.memoryFaultStops[translator.cpu_memory_fault(address, code.length)];
  }
  // What single stepping does now (`cpu_step`, `kind` one of `contract.stepAt`), on a copy of the thread's
  // state that is copied back.
  function step(kind, pc) {
    if (!contract.cpuSteps) return "off";
    const size = contract.threadStateSize;
    const inState = new Uint8Array(state.buffer, BLOCK_AT, size);
    new Uint8Array(translator.memory.buffer, cpuState, size).set(inState);
    const answer = contract.steps[translator.cpu_step(kind, pc)];
    inState.set(new Uint8Array(translator.memory.buffer, cpuState, size));
    return answer;
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

  // Translate code `b` (its bytes, where they are, where it is linked when its code names its own addresses,
  // and where in them the code is, all of it when not said) from `start` and instantiate it, as
  // `instantiateModule` says.
  // With `unlocked`, the pages it was made from are not locked for code. Given the stack pointer of the thread
  // in progress, whether a page it locked may lie in a frame of the thread.
  function instantiateCode(b, start, lookAhead, omit, bound = 0, exact = false, unlocked = false, sp = null) {
    // The translator's memory may grow while it makes room, so its buffer is taken after.
    const at = translator.input(b.bytes.length);
    new Uint8Array(translator.memory.buffer, at, b.bytes.length).set(b.bytes);
    const linked = b.linkAddress === undefined || b.linkAddress === null ? 0 : 1;
    const length = translator.translate(
      b.codeStart ?? 0n,
      b.codeEnd ?? BigInt(b.bytes.length),
      linked,
      BigInt(b.linkAddress ?? 0),
      start,
      lookAhead,
      bound,
      probe,
      omit,
      optimise ?? 0,
      splitBytes ?? 0,
      offer === undefined || offer === null ? 0 : 1,
      BigInt(offer ?? 0),
    );
    if (length < 0) throw new Error(`no translation from ${start}`);
    const modules = [result(length)];
    for (let i = 0; i < translator.more_modules(); i++) modules.push(result(translator.more_module(i)));
    const words = new BigUint64Array(result(translator.translation_read()).slice().buffer);
    const read = [];
    for (let i = 0; i < words.length; i += 2) read.push([b.load + words[i], b.load + words[i + 1]]);
    const inFrame = !unlocked && lockCode(read, sp);
    const addresses = [];
    for (const bytes of modules) addresses.push(...instantiateModule(new WebAssembly.Module(bytes), b, exact));
    if (!exact) translations.push({ read, addresses });
    return inFrame;
  }

  // The blob offsets of a code module's pieces, in slot order, from its description section.
  function pieceOffsets(module) {
    const [section] = WebAssembly.Module.customSections(module, contract.descriptionSection);
    const bytes = new Uint8Array(section);
    // After the format and the guest: the offer, the look-ahead and what it is optimised for, its N.
    let at = 2;
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
    at += 2;
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
    if (exact) return [];
    offsets.forEach((offset, i) => setSlot(b.load + offset, first + i));
    return offsets.map((offset) => b.load + offset);
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

  for (const [start, end, permissions] of regions) setPermissions(start, end, permissions);
  // The code area of instruction cases holds the guest's filler around the instruction: [start, end, byte].
  // Fresh memory is already 0, and writing it would only take memory.
  if (fill && fill[2] !== 0) guest().fill(fill[2], fill[0], fill[1]);
  guest().set(blob, load);
  const loaded = { bytes: blob, load: BigInt(load), linkAddress, codeStart: 0n, codeEnd: BigInt(codeBytes) };
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
      // The case's code replaces the last one's, as a kernel's write would: its pieces and locks go.
      for (let page = 0; page < permissionPages; page++) if (locks[page]) setLock(page, LOCK_CODE, false);
      translations = [];
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
      runThread();
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
    if (keepFloatFlags) {
      new DataView(state.buffer, BLOCK_AT, contract.threadStateSize).setUint32(contract.floatTracking, 1, true);
    }

    const started = performance.now();
    try {
      runThread();
    } catch (error) {
      if (!(error instanceof WebAssembly.RuntimeError) || !/out of bounds/.test(error.message)) throw error;
      slowPath(reason.faultWithoutExactRegisters, 0, 0);
      stopped = { kind: stops.faultWithoutExactRegisters };
    }
    runs.push(performance.now() - started);
  }
  return {
    stopped,
    runs,
    pc: snapshot ? snapshot.pc.toString() : null,
    // The thread state when it stopped.
    stateAtStop: stateAtStop ? [...stateAtStop] : null,
    stdout,
    stderr,
    counters: [...counters].map(([reason, c]) => ({ reason, ...c })),
    warnings,
  };
}
