# Native without a virtual machine: the plan for both forms

Status: plan (W0), 2026-11-23, run 2 (every finding of the first check answered). Brief: the owner's ruling of
2026-11-23 and brief-native.md (including its section on register groups). It builds on docs/native.md (the
virtual-machine form, "the VM form" below) and reuses its native output, count, position map and replay check; where
this plan says "as native.md", that section holds. The x86-64 native output's own plan, docs/x86-native-output.md
("the x86 plan"), already decides x86's side of both forms; section 3.10 says what it settles here.

Words used here:
- **P**: each guest process is a real host process (a **stub**); our kernel runs in the embedder's process.
- **I**: our kernel and every guest process of a container run in the embedder's process; translated code checks
  every guest memory access.
- **floor**: our code that runs on a guest thread's host thread between translated code and the kernel: entry and
  exit routines, signal handlers, the slot table (the "mirror"). In P it is the stub; in I a part of the executor.
- **round trip**: one guest kernel entry in P: the guest thread tells the kernel's runner and waits; the runner
  answers and wakes it.
- **kernel entry**: anything that leaves translated code for the kernel: a system call, a routed instruction
  (`Route::Kernel`, the time counter among them), a fault the kernel must see, a stop.
- Measurements are cited as "W0 probe" (this plan's own, section 11) or by the orchestrator's file.

## 1. What the two forms deliver, and what they share

Both run a guest of the host's own architecture as translated native code (native.md section 3) on a Linux host
with no hardware virtualization, as an ordinary unprivileged process. Both give the guest exactly what the VM form
gives (identical behaviour to the WebAssembly output instruction for instruction, exact count, replay), except the
named differences of section 9.

- **P** buys memory protection from the host's page tables, so translated code is the VM form's code with no
  check per access, and pays a round trip per kernel entry: 2.29 to 2.37 µs on Graviton4 and 1.89 to 1.96 µs on a Xeon
  8488C with the runner and the guest thread on one CPU, as P places them (4.3); 4.0 to 4.8 µs across two CPUs
  (W0 probe, bare metal); 1.7 to 5.1 µs on the cloud rows (costs-round-3-and-cloud-partial.json).
- **I** pays a range check on every guest access and a guard on every move of the stack pointer (3.2), and makes a
  kernel entry a plain call (Graviton4, cloud arm row: 2.1 ns bare, 12.0 ns switching the general registers, 16.0
  to 16.4 ns with q0 to q31; SVE adds about 7 ns, 22 against 15 ns per save and restore in the feature-switch
  research). Its check alone, with the stack exempt (X32-e / X32-nosp), measured +19.4% wall on
  Graviton4, +11.0 to +15.2% on Granite Rapids and +10.0 to +25.4% on Zen 4, the ranges being code layouts (same
  file); in one run on bare-metal Graviton4 the sound form with the stack pointer's guard measured +21.4% and the
  exempt form +21.8% (section 11).

Shared by both, built once (section 3): the address layout, the floor's entry and exit, position resolution, the
slot table, the code area, copy-on-write and written marks, the stop protocol, the register-group rules and the
`Cpu` side's bookkeeping. They differ only where the kernel is (another host process, or the same one) and in the
one place a memory access is emitted.

The crates: `crates/executor/implementation/host-process` (P, package `wcontainers-executor-host-process`) and
`crates/executor/implementation/in-process` (I, package `wcontainers-executor-in-process`). What both use lives in
host-process and is a public module that in-process depends on (one copy, no third crate: the owner's layout allows
an implementation to depend on another; if he prefers a shared crate, it is a move of files). P's stub has no binary
of its own: it is forked from the embedder's process (4.1).

## 2. Rules that shape this plan

- Guest code always passes through the translator; its native output is the only code that runs for the guest.
  Every page of guest memory is mapped without host execute permission, in both forms.
- No inline stop check: a thread is stopped by a real host signal (section 3.6). The count is the VM form's count
  (definition E, one add, only under `ReplayDeterminism::Exact`).
- Protection keys are never relied on (not used at all here).
- Nothing needs root, a capability, ptrace, user namespaces, userfaultfd, perf events, /proc, /dev/kvm or a kernel
  module. Section 6 lists every host call each form makes.
- The kernel is driven only through its existing interface and runners (section 8 says what is missing there).
- One primitive per job: one transition (a futex slot), one copy-on-write mechanism, one stop mechanism, one
  layout formula for both forms.
- Linux hosts on ARM64 and x86-64 first: ARM64 is built first (W1, W2) and x86-64 follows at W3, because x86's
  native output does not exist yet (the x86 track's XN1 and XN2); x86's side of both forms is decided now (3.10).
- Unsafe code: both crates deny `unsafe_code`; every use carries its own `#[expect(unsafe_code, reason = "..")]` on
  the smallest item, rustix first for every host call (MIT/Apache-2.0), libc only where rustix has no call. Expected
  uses, each to be listed in the status file as built: running translated code (a jump into generated code), the
  floor's assembly (`global_asm!` entry, exit and signal routines, and P's stub), mapping guest memory and the code
  area (`mmap` with a fixed address through rustix is `unsafe`), reading and writing through those mappings,
  reading a signal frame, and the fork of the zygote. Nothing for speed.

## 3. What both forms share

### 3.1 The address layout

Measured (W0 probe, section 11): the host's user space is 48 bits on the Graviton4 machine (4 KiB pages) and 56 bits
on the Xeon machine (five-level paging, where Linux maps above 2^47 only when asked with a hint); on ARM64 the host
puts the binary, the vDSO and the stack in [2^47, 2^48) and leaves [2^45, 2^47) free; on x86-64 the stack and vDSO
sit just under 2^47 and a PIE binary near 2^46.4.

One layout (ARM64 here; x86-64 in 3.10), every figure a named constant derived from the fact it describes:
- **The guest area** [`LOWEST_ADDRESS`, 2^44). P: the guest's own addresses, unchanged (guest address = host
  address). I: the container's guest processes, each in its own **slot** of 2^r bytes, rebased (host address =
  slot base + guest address); slot k starts at 2^32 + k·2^r, so that nothing a non-PIE embedder or the host's low
  mappings hold is inside. In every slot two bands are never mapped: the guest's own [0, `LOWEST_ADDRESS`) (64 KiB)
  and a top guard [2^r - `GUARD`, 2^r) with `GUARD` = 64 KiB. Between the mapped parts of two neighbouring slots
  there are therefore always 128 KiB that no one maps; 3.2 relies on it.
  I's reservation reaches 64 KiB below the first slot and stays unmapped for 64 KiB above the last, so the edge
  slots have the same bands.
- **I's slot count**: (2^44 - 2^32 - 64 KiB) / 2^r whole slots: 255 for r = 36 (64 GiB per process), 511 for 35,
  1023 for 34, 4094 for 32. r is chosen per container at start: the largest r, at most 36, that gives at least the
  container's process limit (its pids limit) in slots; with no limit given, 36. A container that needs more live
  processes than slots gets ENOMEM at fork from `memory_new`/`memory_duplicate` (`CpuError::OutOfMemory`, counted
  "in-process slots exhausted", warned); the evaluator (section 7) sends such containers to P.
- **The slot table** (ARM64; the VM form's entry mirror): a 16-byte slot for every host address H of the guest
  area at `SLOTS | (H << 2)`, `SLOTS` = 2^46, so the table spans [2^46, 2^47). It is one memfd per guest memory
  (P) or per container (I), written by the embedder through its own mapping, and mapped read-execute in the guest's
  view **page by page as pieces are loaded** (P: by a command the stub applies, which its filter allows in exactly
  this range, 3.7; I: by the executor itself). A jump to a slot in a page not mapped is a host SIGSEGV at a table
  address, which the floor treats exactly as a slot holding `udf` (no piece for that guest address). So a guest
  that jumps to many addresses costs no memory (a shared file mapping would allocate a page on every read fault of
  a hole). The jump through a register is the VM form's six instructions (native.md section 3.2) with the mask's
  `eor` replaced: P computes the slot from G itself (`ubfiz`, `bfxil`, `orr` of the single bit 2^46, a logical
  immediate); I first adds the slot base (`add t, base, xN`), the one more instruction it pays per jump. The reach
  test is against the process's ceiling (P: 2^44; I: 2^r).
- **Our area** [2^44, 2^46): I: the kernel's read-write view of slot k at 2^44 + k·2^r (so a guest address is one
  add away; 255 · 2^36 fits under 2^45), then code areas, floor, register blocks, signal stacks in [2^45, 2^46).
  P, in the stub: code area, channel and register pages, signal stacks, the stub's code and stack. P's embedder
  process maps its views of the stubs' memfds wherever the host places them (it runs no guest code).
- Each form reserves its areas at start (`mmap` PROT_NONE, MAP_NORESERVE, MAP_FIXED_NOREPLACE): the host never
  places anything inside a reservation, so our code and the guest cannot collide; an embedder mapping made later
  goes elsewhere, and `brk` stops below a reservation (malloc then maps). If a reservation is refused, the form is
  unavailable on that host (the evaluator's probe, section 7).
- **The ceiling**: `Cpu::address_space()` is [LOWEST_ADDRESS, 2^44) in P (the VM form has 2^45: the slot table at
  2^47 would need the host's stack and vDSO moved; 16 TiB is the ruled "lower ceiling", brief 2026-11-18) and
  [LOWEST_ADDRESS, 2^r - GUARD) in I. The kernel places mappings low first and never depends on the ceiling (K2),
  so the guest sees the same addresses under every executor; a process that reaches its ceiling is the existing
  `on_process_outgrown` case.

### 3.2 The four places the native output changes

The native output stays one emitter (native.md section 3); a form is a value the executor gives the translator,
part of the translation's cache key, with three variants (`Vm`, `HostProcess`, `InProcess`). It changes exactly:
1. **`access`** (crates/wcontainers-arm64/src/carry.rs, the single place a guest access is emitted) and the
   instructions that read or write sp. `Vm` and `HostProcess` emit them as today. `InProcess`:
   - An access through any register but sp: `ubfx u, a, #r, #(56 - r); cbnz u, fault; <access> [base, a]`, where
     `a` is the access's first byte in t (or the base register itself when the access has no offset), `u` a second
     withheld scratch, `base` a withheld register holding the slot base. The check ignores the top byte as Linux's
     Top Byte Ignore does, touches no flags, and the host's own Top Byte Ignore drops the tag from `base + a`. Pairs
     and multi-register forms check their last byte too (the access's span, as the Aligned route computes it).
     `fault` is out of line and reports the guest address and access with exact registers (the check precedes the
     access, so nothing has changed).
   - **The stack pointer.** sp holds the host address (slot base plus the guest's sp), since an ARM64 access
     through sp cannot be rebased. The rules keep one invariant: at every guest instruction boundary the guest's sp
     is inside its slot, [0, 2^r).
     (a) An access through sp with an immediate offset is emitted unchanged. Its reach is at most 1 KiB down and
     64 KiB up, size included (`ldr q, [sp, #65520]` is the largest; an SVE `[sp, #imm, mul vl]` form is exempt only
     where its reach at the host's vector length fits, else checked). From an sp inside the slot it lands in the
     slot or in the 128 KiB band no one maps, so it faults there exactly where Linux faults.
     (b) A pre- or post-indexed access through sp (a move of at most 1 KiB) is emitted unchanged: a pre-indexed one
     succeeds only if its new sp is on a mapped page of its own slot, a post-indexed one moves from a mapped page by
     at most 1 KiB, which stays inside [0, 2^r) because both unmapped bands are 64 KiB; a fault leaves sp unchanged.
     (c) `add`/`sub sp, sp, #imm` with imm at most 60 KiB is followed by a probe, one load whose value is unused:
     `ldr wzr, [sp]` after a `sub` (the new frame's lowest word) and `ldur wzr, [sp, #-4]` after an `add` (the last
     word of the frame just freed, mapped even when sp is now the very top of a stack mapping, where a probe at sp
     faulted at the outermost return in the measurement's first build). A probe that succeeds proves the new sp is
     on, or 4 bytes above, a mapped page of its own slot, so inside [0, 2^r). A probe that faults (an unmapped or
     protected page, a watch, sp not 16-aligned) is never the guest's fault: the floor takes
     the new sp less the base; inside [0, 2^r) it resumes at the next guest instruction (the probe ends the `add`'s
     or `sub`'s code, so that is a boundary), otherwise the thread goes displaced (e).
     (d) Every other write of sp (`mov sp, xN`, `add sp, xN, ...`, a larger immediate) computes the new guest value
     into u, checks it as an access address is checked, and then writes `add sp, base, u`; a value outside the slot
     makes the thread displaced (e).
     (e) **Displaced**: the guest's sp is outside its slot, which Linux allows until an access through it. The floor
     keeps the guest's sp in the thread's register block and runs the thread one guest instruction at a time, each
     translated alone with sp read from and written to the block and every access through it checked like any other
     register (so it faults at the guest address Linux faults at). When an instruction brings sp back inside the slot,
     the thread re-enters ordinary translated code. A slow path: counted "stack pointer outside its slot", warned;
     no compiler's code does this.
     A read of sp as a value (`mov x29, sp`, `add x0, sp, #n`) is re-emitted with the base subtracted (`sub x29, sp,
     base`: the same length). The position map gives the guest sp as the host sp less the base. Every handler runs
     on its own signal stack (`SA_ONSTACK`).
2. **The jump through a register**: the slot table's base and reach (3.1), and I's base add.
3. **The call to the floor** (`Arm64::call`, today `brk #code`): a `brk` in a host process is a host SIGTRAP
   (1.2 to 1.3 µs measured for a signal round trip, W0 probe), so `HostProcess` and `InProcess` emit
   `str x30, [page, #saved_link]; bl entry_<code>`, the entries being one page of floor routines placed in the code
   area at most a direct branch's reach from every piece (one such page per 128 MiB of code area). The position
   map records the saved x30 as a relocated guest value, as it records stand-in moves.
4. **The replay check** (native.md section 5.7): `TPIDRRO_EL0` is the host kernel's in a host process, so the stop
   word is a word of the thread's register page: `ldr t, [page, #stop]; sub t, t, c; cbz t, stop`.
Withheld registers: the VM form's three (count x18, scratch x23, register page x28) in P; two more in I (`base`
and `u`), chosen by the same dynamic-use rule (native.md section 3.3). Everything else of the native output (routes,
pieces, linking, count placement, the position map) is unchanged. These are the translator track's files: section
10 asks for them.

### 3.3 Exits, the floor and position resolution

- An inside call enters the floor's entry for its code with the guest's registers in the hardware. The entry
  first stores every general register, sp, pc (from the call site through the position map) and the condition
  flags into the thread's register block, and then the FP/SIMD registers (and on an SVE host the Z, P and FFR
  registers at the host's vector length): the whole file, at every exit, in both forms. It costs about 4 ns more
  than the general registers alone (16 against 12 ns for a save and load, and about 7 ns more with SVE; section 1)
  and it means `Cpu::registers` never needs the thread: in P the kernel's side reads the block in the channel page;
  in I the kernel's own Rust code, which uses vector registers, may run at once. It forms the `Exit` exactly as the
  VM form's inside does.
- A host signal in translated code (SIGSEGV, SIGBUS, SIGILL, SIGTRAP from a guest `brk`, and the stop signal)
  gives the floor the host pc and every register in the signal frame (general, `fpsimd_context`, `sve_context`,
  `esr_context` for the access kind on ARM64). The handler copies the frame once; the host pc it holds is only
  looked up in the position map (a value the map does not know ends the thread as corrupted, counted). The guest
  state at a guest instruction boundary comes from the position map with the VM form's resolution
  (`hardware-virtualization-inside`'s `stop::resolve` and `stop::guest_state`). Those functions take the slot
  table's base from `wcontainers_arm64::mirror::MIRROR`, the VM form's, and their crate pulls in the VM's
  dependencies, so section 10 asks for the base as a parameter and for `stop` and `translation` in a crate of
  their own; W1 needs both before it can resolve a stop.
- Resuming is always at a guest instruction boundary through the guest pc's slot, never at a host pc in the middle
  of a piece and never from an address in a frame. A fault the floor resolves itself (copy-on-write, written marks,
  a code page's first write, a probe) resumes at the resolved boundary too: faults are precise and the position map
  rolls the state back to the guest instruction's start, so running it again from there is exact.
  `Resume::Continue` after a stop is the same re-entry, with the count set from the map.
- Loading pieces, filling slots, linking exits and the kept translations are the VM form's
  (hardware-virtualization-inside `translation`, the monitor's `Translator`); section 10 asks for the monitor's kept
  translations to be usable without the hypervisor crates.

### 3.4 The code area and code a program writes

- One memfd per guest memory holds its code area: mapped read-write in the embedder's process (the translator
  writes there) and read-execute in the guest's view (P: in the stub; I: a second mapping in the same process).
  Measured (W0 probe): a memfd written in one process and run from a read-execute mapping in another runs, on both
  machines; a memfd made with `MFD_NOEXEC_SEAL` still maps executable. After writing, the writer does the
  architecture's cache maintenance on its own mapping (ARM64 `dc cvau`/`ic ivau`, which act on the physical
  line); the guest thread runs the new code only after leaving the host kernel (a futex wait or a signal), which
  is context synchronizing on ARM64. x86-64: 3.10.
- Where `memfd_create` is refused: guest data in an unlinked `O_TMPFILE` file in /dev/shm (data is never mapped
  executable, so its usual `noexec` does not matter), and the code area and slot table in an `O_TMPFILE` file in the
  first directory of: one the embedder names, `$TMPDIR`, /tmp, whose mapping read-execute succeeds at the start-up
  probe. If none does (a read-only root with a `noexec` /tmp), the form is unavailable. Measured in Docker 29.9.3
  (section 11): /dev/shm refuses read-execute, /tmp on the container's root allows it, and with `--read-only
  --tmpfs /tmp` neither does; memfd is allowed by Docker's default profile.
- A read-execute mapping without read is execute-only on both machines (W0 probe: a load from it faults; on the
  Xeon the host kernel makes execute-only from a protection key of its own, which we neither use nor rely on). The
  code area and the slot table are mapped so wherever the host allows it; where it does not, they are readable by a
  P guest (section 9).
- A fork child's code area and slot table start empty; its pieces are loaded on first run from the kept
  translations, as the VM form loads them into a new memory, so a fork copies no code and a child that execs at
  once loads only the pieces it runs before the exec. The exec itself starts a fresh memory (4.1, 5.1).
- Code a program writes: every guest page that holds translated code is write-protected in the guest's view of
  every memory that maps it (shared blocks included); the first write faults to the floor, which drops the
  translations of that page in every process holding them (their other threads stopped first, 4.2), gives the page
  its guest permission back, and resumes (3.3). `Cpu::code_changed` is honoured as in the VM form.

### 3.5 Memory: frames, copy-on-write, written marks

- Guest memory is in memfds, never anonymous host memory: P needs the kernel's process to see it; I needs a second,
  read-write view for the kernel (`Cpu` reads and writes ignore page permissions). Each guest memory has its own
  memfd whose offset is the guest address (sparse: holes cost nothing); each shared block (`shared_new`) has its
  own memfd. The guest's view maps a memfd range with the guest's permissions, one host mapping per guest mapping,
  so the number of host mappings follows the guest's as on Linux.
- A page's permission in the guest's view is one function of its marks: the guest's permission, less write while
  it is lent to a fork child or holds translated code or has no written mark yet (when marks are being taken), less
  everything while it is borrowed (below) or watched for all accesses. Every change of a mark recomputes it.
- The kernel's view: I maps each memfd once, read-write, in our area (3.1); P maps, in the embedder's process, each
  guest mapping's range of its memfd when the kernel first touches it, found through a range map per memory.
  `Cpu::read`, `write` and `zero` (a hole punched with `fallocate`) go through it in both forms and are seen at once
  by every view.
- **Fork** (`memory_duplicate`): one copy-on-write mechanism, the same in both forms. Every private page of every
  memory is either **owned** (its bytes are at its own offset in its own memfd) or **borrowed** from exactly one
  owner (its bytes are still the owner's page at the same offset). Rules, all kept under the executor's own lock:
  - Duplicating M into a new child C: every page M owns becomes lent to C (C borrows it from M); every page M itself
    borrows from O is borrowed by C from O too, so a borrower always points at an owner and chains never form.
  - Before `memory_duplicate` returns, M's lent pages are write-protected in M's view **synchronously**: in P by a
    command round trip to the forking thread (which is in its kernel entry; its `mprotect` changes the page tables
    of all its stub's threads, and the host kernel has shot down their TLB entries before it returns), in I by the
    executor's own `mprotect`. This instant is the fork's instant: a sibling thread's write lands before it (and is
    in the child) or faults after it. The child cannot run, and the kernel cannot read or write it, before the call
    returns.
  - A borrowed page is inaccessible in the borrower's view. The borrower's touch (a fault, or a `Cpu` read or
    write of it) copies the owner's page into the borrower's memfd, which then owns it.
  - Before an owner's page that is lent changes (the guest's first write faults; a `Cpu::write` or `zero`; an
    unmapping; the owner's end or exec), the page is copied into one borrower, which becomes the owner, and every
    other borrower now borrows from it: one copy per event, whatever the number of borrowers. Then the old owner's
    page is free to change. The new owner's view gains access with its next applied command (a touch before that
    faults and applies it).
  - A borrower that ends or unmaps the page leaves the lent set without a copy.
  The copy source is always write-protected in every view of its owner (it is lent), so every copy is of the page as
  it was at the fork's instant, as on Linux. Copies are made in units of 2^u pages (u by measurement in W1, from 0
  to 9, against the number of host mappings it creates: mprotect splits a mapping at the unit's edges, and the host's
  vm.max_map_count is 65530 by default, 1048576 on the Ubuntu machines here).
- **Written marks** (`take_dirty_memory`): taking them write-protects every writable page that has a mark; the
  first write after that faults, sets the mark and gives write back (ARM64 Linux has no soft-dirty bit, and
  userfaultfd's write protection is withheld by sandboxes). The kernel sets the mark for its own writes.
- Shared blocks map the same memfd in every view, so two guest processes writing a shared mapping see each other's
  writes through the host's coherent memory, with the hardware's ordering; the kernel's writes into one show in
  all. The kernel checks watches before its own writes, as native.md section 5.3 says.
- Measured costs of the pieces (W0 probe, Graviton4 / Xeon): mprotect of one page 391 / 295 ns, mmap of one memfd
  page over another 1.15 / 0.90 µs, a hole punched in one page 0.69 / 0.54 µs, a fault handled in process and
  returned from 1.31 / 1.33 µs.

### 3.6 Stopping a thread, slices and events

- A stop is a real-time signal sent with `tgkill` to the guest thread's host thread. The signal itself is the
  request: nothing in memory the guest could write decides whether a thread stops. A stop that finds the thread
  already leaving is harmless: the kernel side sees an exit it would have got anyway, or a `Stop` it did not need
  and continues (counted "stale stop"). Measured (W0 probe): from the call to the handler running on a spinning
  target, 1.78 µs median in the same process and 1.68 µs in another (Graviton4); 1.75 / 1.90 µs (Xeon).
- Every handler of ours is installed with `SA_SIGINFO | SA_ONSTACK | SA_NODEFER` and `sa_mask` = {the stop signal}:
  a second fault is delivered (never fatal for being blocked), and the stop signal is held off only while a handler
  runs. No handler returns through a frame the guest can write: in P none returns at all; in I one returns
  (`rt_sigreturn`, which restores the mask) only when the signal interrupted code that is neither guest code nor the
  floor, its frame being on our own signal stack, out of every guest access's reach. Every other handler leaves by
  jumping to one of our routines, after unblocking the stop signal with one `rt_sigprocmask` (per handled signal,
  never per kernel entry). Where the signal landed decides what it does:
  - translated code: resolve the position (3.3), then the exit (`Stop`, or the fault's exit, or the floor's own
    resolution and resume);
  - the floor's entry: the entry first stores every register, so up to its last store the frame holds the guest's
    values and after it the block does; the handler finishes the store from the frame (by the pc) and goes on to the
    exit the entry was making;
  - the resume sequence (after the decision to enter guest code, before the guest's first instruction): the
    guest's registers are all still in the block, so the handler leaves as `Exit::Stop` with them;
  - the wait (P: the futex wait for the answer): it restarts the wait, which reads the answer word again;
  - the handler's own tail, after its unblock: it takes the same exit the interrupted handler had already written.
  So no signal mask is changed on any kernel entry path (a mask change is a host system call, 120 ns).
- A slice (`Next::Run { slice_nanoseconds }`): P: the runner's futex wait carries the slice as its time-out and
  sends the stop on expiry (nothing is paid when the thread comes back first). I: a per-runner POSIX timer
  (`timer_create` with SIGEV_THREAD_ID) re-armed only when a new deadline is earlier than the armed one. P: a thread
  that has not come back 100 ms after its stop is sent it again every 100 ms, and after 10 s its process is killed,
  counted ("thread ignored its stop"; only a compromised stub can block the signal).
- A turn that must end at an exact count is the VM form's open decision 10; until the owner rules it runs one host
  instruction at a time, which in a host process is not available (no single-step without ptrace): such a turn is
  refused as the VM form refuses it without the count (`Failure`, "turn limit without stepping"), counted.

### 3.7 What protects us from the guest

- Control flow: the guest reaches host code only through pieces we translated and their slots; a slot without a
  piece holds `udf` or is unmapped. Nothing read from memory the guest can write is ever used as a host code address.
- I: every guest access is checked against its slot or kept inside it by 3.2's stack rules, so no guest load or
  store reaches our area, the kernel, the embedder, other guest processes or the frames of a signal. This is the
  WebAssembly output's guarantee made natively.
- I, signals that are not ours: guest code runs on the embedder's threads with the guest's sp and `TPIDR_EL0`.
  So for the length of each `Runner::run`, the runner thread blocks every asynchronous signal but the stop signal
  with the raw `rt_sigprocmask` (glibc's own SIGCANCEL and SIGSETXID included: an embedder's `set*id` or
  `pthread_cancel` waits until the runner returns, named in section 9), sets its own signal stack, and puts both
  back when it returns. Our fault handlers are installed at the executor's start, remembering the previous ones; a
  fault whose pc is not in our code area or floor goes to the previous handler as it expects (Rust's and wasmtime's
  chain the same way). Each `Runner::run` checks (one `sigaction` read per signal) that ours are still installed and
  otherwise refuses to run, named ("fault handler replaced"). Our handlers' first instructions put the host's
  `TPIDR_EL0` back before any Rust code runs. The stop signal is the first real-time signal whose disposition is
  the default at start.
- P: a guest load or store can reach anything in its stub that is mapped readable or writable: its register and
  channel pages, its signal stacks. So the stub is built so that this gives the guest nothing it does not have:
  - its code is one position-independent assembly unit of its own section (no Rust, no libc, no data of the
    embedder), keeping its own state in registers while it waits (the host kernel preserves registers; another
    guest thread cannot change them), and using no stack in guest-writable memory;
  - everything the stub reads from its pages after a wait is untrusted: guest register values (the guest's own to
    change), and mapping commands, which it checks itself before obeying (the range inside the guest area or the
    slot table, the right fd, no execute permission but for slot-table pages);
  - its handlers find their thread's pages from the signal stack's address (the host kernel's, unforgeable), copy
    what they need from the frame once, and never return through the frame (3.6);
  - position resolution, translation and every decision are in the embedder's process; the stub only reports.
  - A stub holds none of the embedder's memory: the zygote it is forked from removed it (4.1).
  Defence in depth: the stub runs under a seccomp filter installed by itself at start (`PR_SET_NO_NEW_PRIVS`, then
  `SECCOMP_SET_MODE_FILTER`, both unprivileged) that allows only its own calls (section 6) with their arguments
  bounded: `mmap` inside the guest area without `PROT_EXEC` and not of the slot table's fd, `mmap` inside the slot
  table only of the slot table's fd, read-execute, shared and fixed; `mprotect` only inside the guest area and
  without `PROT_EXEC`; `munmap` only inside those two ranges; `recvmsg` only on its socket. Its fds are its
  memory's memfd, the slot table's memfd (at fixed numbers, set before the filter) and its socket, plus the shared
  blocks' memfds it receives. A compromised stub can then reach only its own guest's memory, its own translations
  and its channel, through which every request is validated by the kernel as a system call is.
- The kernel's side treats every word from a stub as untrusted (read once, bounds-checked), like a system call's
  arguments.

### 3.8 Register groups (brief-native.md, 2026-11-23)

- The start-up probe of which groups the host offers runs on its own short-lived host thread, which then exits.
- P: each guest thread is its own host thread, so the host kernel saves and restores every group lazily at host
  thread switches; the floor saves the whole file at every exit (3.3) only so that the kernel can read it. The
  stub's routines use no FP/SIMD register but to store and load the guest's. On an x86-64 host, before an embedder
  runner thread blocks it returns the vector state it dirtied to the initial state: `vzeroupper`, and, where the
  host has AVX-512 and `XGETBV` with ECX=1 shows zmm16 to zmm31 or the mask registers in use, `XRSTOR` from an image
  whose header marks those groups as initial (writing zeros does not clear their in-use bit; feature-switch
  research, REPORT.md).
- I: we switch guest threads on a runner, so the floor saves eagerly at every guest thread switch, the incoming
  thread's values or zeros loaded before its first instruction: ARM64 general, FP/SIMD and SVE in the host's vector
  length (4 / 15 / 22 ns measured, feature-switch research), SME streaming mode and ZA left off and so discarded;
  x86-64 `XSAVEC` and `XRSTOR` over the offered groups (52 to 78 ns), which skip and reset unused groups themselves.
  No learning of whether a thread used a group. The floor also swaps the thread register the host's runtime uses for
  its own thread-local storage (ARM64 `TPIDR_EL0`, x86-64 FS base) at every entry and exit, since the guest's value
  is its own (ARM64 `msr`/`mrs`; x86-64 `wrfsbase` where the host enables FSGSBASE, else `arch_prctl`, a counted
  slow path).
- Tests (hostile-guest part of each suite): guest threads each fill every offered group with distinct values,
  interleaved in every order of the bounded search, each reads back exactly its own and an unused group reads zero;
  in a test build, the floor asserts at the moment a guest thread is about to run that the registers hold its values
  or zero.

### 3.9 Count, replay and parallel threads

The count is in x18 as in the VM form; the host kernel preserves it across its calls and signals (in I the floor
saves it, since the kernel's Rust code may use x18 on Linux). Positions are resolved with the VM form's map, so a
position logged here is one the WebAssembly output and the VM form reach and the reverse (native.md section 5.7).
Replay to (count, pc) uses the check of 3.2 item 4, against the target thread's own stop word (a P guest that
overwrites its own stop word only spoils its own replay).

Parallel threads: the kernel (`wcontainers-kernel-linux`) runs one thread at a time today (its `next` refuses a
second `Run` while one runs) and accepts only `(Serial, Exact)`. So W1 and W2 run one guest thread at a time
whatever the number of runners. Both forms are built so that N runners can each run a thread at once when the
kernel allows it (section 8; asked of the kernel track in section 10): P's guest threads are host threads, I's
runners each hold their own block. `(Parallel, Exact)` needs the order of accesses between threads, which the brief
gives to per-thread watches; neither W1 nor W2 builds those (section 8 says how P can later), so these executors
refuse that configuration at `Kernel::new` as the kernel already refuses it.

### 3.10 x86-64 hosts

The x86 plan (docs/x86-native-output.md sections 2 to 6) decides x86's side of P and I, against this plan's
layout, and this plan takes it as it stands:
- no slot table: calls, returns and jumps through a register go through its two-level mirror (its 3.3), whose first
  level is per thread area (per guest thread in P, per runner in I);
- the thread area's segment: GS in P, FS in I, with the guest's memory window at the GS base in I (its 2 and 4);
- I's slots are 2^36 bytes from 2^32 as here, each holding a 2^32 window (I's ceiling 2^32 in the first build) with
  its guards; P's ceiling is 2^44 (its 6); our area lies between 2^44 and the host's own mappings near 2^47, placed
  by `MAP_FIXED_NOREPLACE` around a PIE embedder's binary, and above 2^47 by hint on a five-level host;
- code another core wrote is made safe by `membarrier(MEMBARRIER_CMD_PRIVATE_EXPEDITED_SYNC_CORE)` in the process
  that runs it (in P a stub thread makes the call at the embedder's request, once per batch of links); a sandbox
  that refuses `membarrier` leaves links made only while the process's other threads are stopped (its 3.2);
- the exact check's sequence with live and dead flags, the stack's handling and the string instructions (its 4);
  its stack measurement (XN2) decides between rewriting implicit stack accesses and this plan's 3.2 stack rules.
Everything else of this plan (stops, frames, copy-on-write, stubs, runners, the evaluator) is the same on x86-64,
with the signal frame's x86 fields. Building it is W3 (section 13).

## 4. Form P: each guest process a host process

### 4.1 Processes and threads

- A guest memory (`MemoryId`) is one stub process. A guest thread (`ThreadId`) is one host thread of it.
- The **zygote**: at the executor's start, a thread of the executor forks it from the embedder's process. In the
  child only direct host calls run (no allocation, no lock: the embedder may have other threads). It maps the stub's
  code (copied by the executor before the fork into a memfd) read-execute at a fixed address in our area and a stack
  beside it, jumps there, closes every fd but its socket (`close_range`), unmaps every range outside its own (a few
  `munmap` calls over [0, 2^44), [2^47, 2^48) and the rest of our area: no /proc needed), blocks every signal and
  waits for requests on its socket. So no stub holds any of the embedder's memory. The embedder's death closes the
  socket and the zygote exits.
- `memory_new` and `memory_duplicate` ask the zygote for a child (measured, W0 probe: a request until the new child
  runs, 80 µs Graviton4, 95 µs Xeon; a plain fork of a small static process 111 / 121 µs). Each request carries
  (`SCM_RIGHTS`) the fds the child needs, its own socket's end among them, which the zygote closes after the fork.
  The child sets `PR_SET_PDEATHSIG` to SIGKILL (and checks that its parent is still the zygote), maps its memory's
  memfd, its code area (read-execute, then closes that fd), the slot table's fd at its fixed number, installs its
  handlers and signal stacks, unblocks the stop and fault signals, installs its filter, and becomes the memory's
  stub. Exec is `memory_new` plus `memory_drop`: a fresh stub, as
  Linux gives a fresh address space.
- `thread_new` on a memory: the stub's thread that is in a kernel entry for that memory (the `clone` caller) makes
  the host thread with `clone` before it resumes; the first thread of a new memory is the stub's first thread.
- A stub has no thread of its own for commands: every change to a memory's guest view is a command the kernel's
  side queues and the next thread of that memory to resume applies before running guest code (4.2). New shared
  blocks' memfds reach the stub over its socket (`SCM_RIGHTS`), received by the thread that applies the command.
- One embedder process serves every stub of every container it runs. The executor learns of a stub's death through
  a pidfd (or, where `pidfd_open` is refused, from the zygote, which reaps its children and reports over its socket)
  and ends the process as killed by SIGKILL, counted ("stub ended outside the kernel").

### 4.2 Memory

- Layout and frames as 3.1 and 3.5: the guest's own addresses, the memfd per memory.
- Mapping commands (`set_permissions`, `shared_map`, `shared_unmap`, a slot-table page, a protection for code or
  marks) change only the guest view, which no one but the memory's own threads can observe. They are queued per
  memory and applied by the resuming thread in the order made, before it runs guest code, so a system call that
  maps or protects costs no extra round trip, and the effect is in place when the call returns, as on Linux. A
  command on a memory none of whose threads is in a kernel entry rides on the resume of one of its threads, which the
  kernel's side first stops (3.6). The kernel's side counts the host mappings each command creates, so
  `CpuError::OutOfMemory` is returned up front instead of a host failure.
- Two things are applied at once, by a command round trip to the thread in the kernel entry (counted "mapping
  applied at once"), not left in the queue: the write protection a fork makes (3.5), and any command that takes a
  view's access away from a range whose storage is about to change (an unmapping, `mmap` with `MAP_FIXED` over a
  mapping, before the kernel's side writes or punches it). Otherwise a sibling thread could read the new storage
  through the old permission, or write a page while it is copied, which no order of the two calls gives on Linux.
- Dropping translations while other threads of a memory run (code changed, a range unmapped) stops those threads
  first, as the VM form's group stop does (docs/threads.md sections 7.5 and 10); slot table and code area are memfds
  the kernel's side writes directly.

### 4.3 The system call and its return

- Each guest thread has a channel page (shared memfd, mapped in its stub and in the embedder's process): a request
  word, an answer word, the exit and the register block. The runner (`Runner::run` on an embedder thread) writes the
  `Resume` and the queued commands, wakes the thread's futex word, and waits on its own with the slice as time-out;
  the guest thread's exit writes the `Exit` and wakes it back. No spin: on one CPU spinning only adds its bound
  (measured, cloud arm and intel rows), and across CPUs it burns a core per waiting side.
- **Placement**: a runner and the guest thread it drives share one CPU. For the length of a `Runner::run` the runner
  pins itself to one CPU of the process's allowed set, each runner its own, and pins a guest thread to that CPU when
  a turn brings the thread to it from another CPU (`sched_setaffinity` from the embedder, unprivileged for the
  user's own threads; one call, only on such a move); the runner sleeps while the thread runs, so they never
  compete, and each transition is a direct switch on that CPU. When a round trip takes more than 20 times the
  start-up probe's figure, the runner moves both to the next CPU of the set, counted ("runner moved to another
  CPU"). The reason is measured (W0 probe, section 11): with the scheduler placing both sides, runs of 2,000 round
  trips took over 2 ms per round trip on average in 2 of 3 runs on Graviton4 and in 1 of 4 on the idle Xeon, while
  every run pinned to one idle CPU or to two idle CPUs held its median.
- Measured round trips for this exact shape (futex slot, ppid-like request), bare metal (W0 probe): Graviton4 one
  CPU 2.29 to 2.37 µs, two CPUs 4.58 to 4.83 µs, unpinned 4.59 to 4.74 µs when it finishes; Xeon 8488C one CPU 1.89
  to 1.96 µs, two CPUs 4.02 to 4.16 µs, unpinned 4.39 to 4.56 µs when it finishes. Cloud rows: Graviton4 c8g 2.20 /
  5.58 µs, Granite Rapids 1.72 / 4.18 µs, Zen 4 5.07 / 8.46 µs. When the CPU a side needs is busy with another task,
  a round trip waits for the scheduler: 1.4 ms and more (W0 probe), 1.99 to 2.00 ms (cloud rows); placement's move
  is the answer, and P is for hosts with idle cores (an evaluator input, section 7).
- Rejected: seccomp user notification on an untranslated `svc` (as fast on one CPU and 0.4 µs cheaper across CPUs,
  but it gives only the number and six arguments, so exits that need the whole register file would need a second
  path; and it is not offered by every sandbox); pipes, sockets and ptrace (all dearer, measured).
- Everything the guest reads that the kernel answers is a round trip in P, the time counter included (our vDSO has
  only `__kernel_rt_sigreturn`, so `clock_gettime` is a system call): this is what the evaluator counts as kernel
  entries.

### 4.4 Fork, exec, shared mappings, signals

- Fork while another thread of the parent is in a system call, and a third runs: `memory_duplicate` is made during
  the forking thread's kernel entry and applies the parent's write protection at once (3.5, 4.2) before it returns;
  the thread in the other system call waits in its stub and is unaffected (its call completes in the kernel, whose
  `Cpu` calls on the parent's memory come before or after the duplicate as the kernel orders them); the running
  thread's writes land before the protection's instant or fault after it. The child holds only the forking thread,
  as on Linux, whose host thread is the child stub's first thread. A parent that forks twice before a page is copied,
  and a child that forks before copying, are 3.5's rules (one owner per page, one copy per event).
- A shared mapping written by two guest processes: one memfd mapped in both stubs (3.5).
- Guest signals are the kernel's: it writes the frame and resumes the thread `FromRegisters`; nothing host-level.
- A host signal arriving inside the transition: 3.6 (the entry finishes from the frame; the resume window leaves as
  a stop; the wait restarts).
- A host signal in the stub's own code that 3.6 does not name is a fault of ours: the stub reports it if it can and
  ends; the process ends as killed, counted ("stub fault").

### 4.5 What is saved at a guest thread switch, and by whom

The host kernel, lazily, at every switch of host threads (3.8). At every exit the floor saves the whole register
file to the channel's block (3.3); `Cpu::registers` and `set_registers` read and write the block, never the
thread; a thread resumed `FromRegisters` loads everything from it, and one resumed with `Return` or `Continue`
loads what the exit stored. The host's lazy SVE handling adds about 150 ns to each wake of a thread that has used
SVE (feature-switch research, Graviton4); W1's getppid figure is taken both with and without SVE in use.

### 4.6 How the kernel's runners drive it

`Kernel::new(config, parallelism)` gives N runners; the embedder runs each on a thread of its own with an executor
handle of its own (section 8). A runner's `run` asks the kernel for the next thread under the kernel's lock, lets the
lock go, hands the turn to that thread's host thread through the channel, blocks until it comes back, takes the lock
again and gives the kernel the `Exit`, and repeats. With today's kernel one thread runs at a time (3.9).

## 5. Form I: the kernel in process

### 5.1 Processes and threads

- Every guest memory of a container is a slot in the embedder's process (3.1); every guest thread is a register
  block. Runners are the embedder's threads, each running whichever guest thread the kernel gives it: the kernel's
  own model (a call that cannot finish parks its thread and returns), with no host thread per guest thread.
- Fork, exec and thread creation make no host process or thread: a slot and a memfd, or a block.

### 5.2 Memory

- The guest's view is its memfd mapped into its slot with the guest's permissions; host page tables enforce the
  guest's permissions inside the slot, the check of 3.2 keeps every access inside it. A fault inside the slot is a
  host SIGSEGV whose address less the slot base is the guest address.
- Mapping commands are made at once in the same process (no queue). Copy-on-write, written marks and code pages are
  3.5 and 3.4, with the faults handled in process (1.31 µs measured for the handler's round trip, plus the copy).
- Exact against wrapping checks: built exact (3.2 item 1): every access Linux faults on faults here at the same
  address with the same registers. The wrapping form (`and t, a, #(2^r - 1)` before the access, or 32-bit
  addressing with a 4 GiB slot) measured cheaper (+9.2% against +19.4% on Graviton4, +1.8 to +4.3% against +11.0
  to +15.2% on Granite Rapids, both with the stack exempt) but sends an access outside the slot to the process's own
  memory at that address modulo 2^r, where Linux faults: a behaviour difference no counter can see. It is the
  owner's to allow (section 12).

### 5.3 The system call and its return

A plain call: `bl entry` (3.2 item 3) saves the whole register file (16 ns measured for a save and load with q0 to
q31 on Graviton4, about 7 ns more with SVE; section 1), switches to the runner's host stack in our area (the guest's
stack is guest memory), swaps the thread register, and returns `Exit::Syscall` to the runner's loop as a function
return; `Resume::Return` is a return into the call site with x0 set and the file loaded. No host system call is made
on the path.

### 5.4 Fork, exec, shared mappings, signals

As 4.4 with every step in one process: the forking runner protects the parent's view itself before returning (3.5);
a racing sibling on another runner writes before that instant or faults after it. A shared mapping is one memfd
mapped into both slots. A host signal inside the transition: 3.6. A stop signal arriving while the runner is in the
kernel or the embedder's code returns at once (3.6) after marking the runner's current turn to end at its next
entry into guest code, which the resume sequence reads from the runner's own block (out of the guest's reach).

### 5.5 What is saved, by whom; the runners

The floor, eagerly, at every guest thread switch (3.8). The runners are `Runner::run` on the embedder's threads,
each calling its executor handle's run-a-thread call in a loop, as 4.6 (section 8).

## 6. What each form needs from the host

Both (embedder's process): `memfd_create` (or 3.4's `O_TMPFILE` files through `openat`), `ftruncate`, `fallocate`
with `FALLOC_FL_PUNCH_HOLE`, `mmap` (fixed addresses, `MAP_FIXED_NOREPLACE`, `MAP_NORESERVE`, shared file mappings,
read-execute), `munmap`, `mprotect`, `futex` (wait with time-out, wake; shared futexes on memfd mappings),
`tgkill`, `rt_sigaction`, `rt_sigprocmask` (at each `Runner::run` start and end, and once per handled signal that
does not return), `sigaltstack`, `clone` for threads, `sched_getaffinity`, and a 4 KiB host page; on x86-64
`membarrier` (optional, 3.10).
- P adds: in the embedder, `fork` (or `clone` without namespace flags) for the zygote, `socketpair`, `sendmsg`,
  `sched_setaffinity`, `pidfd_open` (optional); in the zygote, `close_range`, `munmap`, `recvmsg`, `fork`, `wait4`
  (where pidfds are refused), `exit_group`; in each stub, `prctl` (`PR_SET_PDEATHSIG`, `PR_SET_NO_NEW_PRIVS`),
  `getppid`, `seccomp(SECCOMP_SET_MODE_FILTER)`, `dup3` (fds to their fixed numbers, before the filter). The stub's
  own filter then allows: `futex`; `mmap`, `munmap`, `mprotect` bounded as 3.7; `clone` with thread flags; `recvmsg`
  on its socket; `rt_sigprocmask`; `sigaltstack`; `exit`; `exit_group`;
  `membarrier` on x86-64. No `rt_sigreturn` (3.6).
- I adds: `timer_create`, `timer_settime` and `timer_delete` for slices, `rt_sigreturn`; `arch_prctl` on x86-64
  hosts without FSGSBASE. No process is ever made, so I runs where `fork` is refused.
- Neither needs: ptrace, user namespaces, mount, chroot, capabilities, `process_vm_readv/writev`, `userfaultfd`,
  `perf_event_open`, /proc (nothing is read from it), `execve`. Docker's default seccomp profile allows every call
  listed (it withholds only namespace flags of `clone`, which are not used; it allows `process_vm_readv` too, which
  is simply not needed); gVisor implements each of them, expected and not yet checked. W1 and W2 run their suites
  inside a container started with Docker's default profile, and under gVisor's runsc where it can be installed on
  the rented machine (if not, named as unchecked).
- P makes one host process per guest process, so a host limit on processes (a pids cgroup, `--pids-limit`) is also
  the guest's; I makes none.
- macOS: P and I need a 4 KiB host page to give the guest its own page permissions; Apple silicon has 16 KiB, so
  there neither form is offered (the VM form, Hypervisor.framework, works on every Mac with no entitlement beyond
  an ad hoc one). Intel Macs would need `os_sync_wait_on_address` for the futex (macOS 14.4 on; measured, confinement
  costs), `shm_open` for the memfd, Mach exception ports or signals for faults, `thread_suspend` for stops; not planned.
- Windows: no fork (our copy-on-write is our own, so none needed); a section per memory mapped with
  `MapViewOfFile3` into the stub process from outside (Windows maps into another process without its help, which
  would remove 4.2's queue), 64 KiB view granularity (the identity layout of 3.5 maps a whole slot as one view; a
  shared block at a 4 KiB boundary would need copying, a named slow path); `VirtualProtectEx` for permissions,
  `SuspendThread`/`GetThreadContext` for stops, a vectored exception handler in the stub, events for the wake.
  Not planned (Windows hosts are left out, brief-native.md decision 8).

## 7. The evaluator's inputs and the crossover

What the embedder can know before choosing, all without running the workload:
- The host, probed once at start (about 50 ms): whether KVM is usable (then the VM form; nothing here replaces it);
  the user-space width and page size; whether each form's reservations and calls of section 6 succeed and where its
  code area can live (a refusal removes the form); P's round trip on this host with P's placement (2,000 round trips
  on one CPU of the allowed set; the mean, which the busy-CPU waits dominate, not only the median); I's check cost,
  from a fixed translated loop run in both forms (or the per-CPU figure of section 11); the number of idle CPUs now.
- The container's configuration: its memory and process limits (pids), which bound I's slot size and count (3.1).
- What earlier runs of the same workload recorded, as the embedder defines "the same" (image digest and command):
  kernel entries per second of guest CPU time, processes started per second, peak live processes, peak memory per
  process. Our kernel does not report kernel entries yet (section 8).

The crossover. Per second of guest CPU time, P adds k·c (k kernel entries per second, c the round trip less I's
call) and I adds f (its checks' share of the time). I beats P when k > f / c. f is the sound form's (section 11:
X32-e plus the stack pointer's guard), and it still leaves out I's other costs: the extra instruction per jump
through a register, the eager register switch at guest thread switches and the second check of pairs; W2 measures
them. With P's placement (c on one CPU) and the measured f:
- Graviton4: f = 0.214 (X32-p, section 11), c = 2.29 to 2.37 µs -> 90,000 to 94,000 entries per second; at the
  two-CPU c of 4.58 to 4.83 µs, 44,000 to 47,000.
- Xeon 8488C: f is not measured on this CPU; with Granite Rapids's f with the stack exempt, 0.110 to 0.152 by
  layout, plus the stack guard's share measured on Graviton4 (none within the pairs' spread), c = 1.89 to 1.96 µs ->
  56,000 to 80,000.
- Zen 4 (c7a, cloud rows): f = 0.100 to 0.254 by layout with the stack exempt, c = 5.1 µs -> 20,000 to 50,000.
- On a host whose cores are all busy, c is about 2 ms: I wins above about 100 entries per second.
Process creation adds to P only: 80 µs (Graviton4) or 95 µs (Xeon) per new guest process, twice for a fork and an
exec (W0 probe), against a slot and a memfd in I.
The workload split measured on the laptop (confinement-costs.json, "measure workload-mix"): a shell loop starting
2,000 programs makes about 256,200 system calls in 0.73 s of user time (about 350,000 per second) and 2,000
processes: I by a factor; a Python start with 70 imports makes about 3,460 calls in 0.9 s (about 4,000 per second):
P (under 1% against I's f); a compute benchmark makes 9: P. These are expected outcomes from the arithmetic;
W2 measures them on both forms.
Rule given to the embedder (it chooses, not us): with no history, P (the owner's default for agentic workloads);
with history, the form whose arithmetic above is lower at the recorded rates on this host's measured c and f. The
form is chosen per container at start and fixed for its run. Changing it at an exec of a guest process (the kernel is
in the embedder's process in both forms, so one executor could hold memories of both) is possible later and not
built (section 12).

## 8. The interface question

These executors implement, as they stand:
- `wcontainers_executor::Executor` (one instruction, `execute_single_instruction_in_place`): each translates the
  instruction at look-ahead none and runs it in a scratch memory (P: a scratch stub; I: a scratch slot) with the
  given processor state and memory deltas, through the executor conformance suite.
- `wcontainers_abi::Cpu`, which `wcontainers-kernel-linux` requires of its executor (`C: Cpu + Executor`), every
  method as 3.5, 4.1 and 5.1 say.
- Not `PerThreadWatch` (either trait) in W1 and W2: threads of one host process share one set of host page tables.
  Watches per process (`Process::watch_memory`) work through the guest view's protections. Not closed off: P can
  give per-thread watches later by making each guest thread of a watched process its own host process over the same
  memfds (its own page tables, mapping commands applied in each), at a process creation (80 to 95 µs) per thread
  creation; I cannot (one process). Listed for the owner (section 12).
- `Runtime`, `Container`, `Interrupter` are the runtime's and need nothing new: the embedder picks the form by which
  runtime it makes.

Where an interface would have to change, the smallest change, for the owner (nothing here is edited by this track):
1. Running a thread, and a handle per runner. `KernelRunner::run` is `unimplemented!` because "the executor
   interface has no call that runs a thread" (crates/kernel/implementation/linux/executing.rs), and every executor
   needs one. The kernel holds its executor (the `Cpu` that made every memory and thread) behind its one lock, while
   `Runner::run(&mut self, executor: &mut E)` takes an executor value from the embedder. For a runner to run a thread
   without holding the kernel's lock for the whole turn, the value it gets must reach the same threads and memories.
   The smallest change gives the executor a way to make such a value:
   ```rust
   pub trait Executor {
       // ...as today...
       /// Makes another handle to this executor's memories and threads, for one runner to run them through.
       fn runner_handle(&self) -> Self
       where
           Self: Sized;
       /// Runs the thread until it comes back, raising its stop after the slice if one is given.
       fn run_thread(&mut self, thread: ThreadId, resume: Resume, slice_nanoseconds: Option<u64>) -> Exit;
   }
   ```
   (`ThreadId`, `Resume`, `Exit` from wcontainers-abi; `slice_nanoseconds` as `Next::Run` has it). The embedder makes
   N handles before it gives the executor to the kernel's config, and passes handle i to runner i. `run_thread` on a
   handle touches only that thread's state (its channel or block); everything shared is behind the executor's own
   lock, which `Cpu` calls take. The alternative, the owner's to choose instead: an associated `type RunnerHandle` and
   `Runner::run(&mut self, executor: &mut E::RunnerHandle)`, which changes the kernel interface too. Running N
   threads at once needs the kernel's own change as well (3.9).
2. The evaluator's count of kernel entries, so that the embedder can record the rate section 7 needs:
   ```rust
   pub trait ProcessInfo {
       // ...as today...
       /// Returns how many times the process's threads have entered the kernel, ended ones included.
       fn kernel_entries(&self) -> u64;
   }
   ```
3. Stopping a running thread from outside the runner. The kernel's `suspend_tasks` and the runtime's `Interrupter`
   already exist; the executor needs from the runner only the thread's host thread, which it keeps itself. No change.
4. Nothing else: `Cpu::address_space` carries the ceiling, `ProcessInfo::address_ceiling_distance` and
   `on_process_outgrown` cover growth, `CpuError::OutOfMemory` covers slots running out.
Considered and not asked: a clock the system publishes for the floor to read without a kernel entry (as Linux's
vDSO does) would remove P's round trip per time read, but it changes what the system decides (every read is the
system's answer today); listed for the owner in section 12 instead.

## 9. Named differences from the WebAssembly output and Linux

- P: a guest load or store to our area in its stub (register and channel pages, signal stacks; the code area and
  slot table where the host has no execute-only) succeeds where Linux and the other outputs fault. Only a guest that
  uses an address it never mapped can see it; it changes nothing of ours (3.7). This extends the VM form's accepted
  "our reachable pages" difference (native.md decision 4) to these pages. Counted: no (a successful access cannot
  be seen); listed for the owner.
- P and I: no per-thread watches in W1 and W2 (8), so no `(Parallel, Exact)`. The VM form's single-step turns are
  not available (3.6).
- P and I: pointer authentication keys and memory tags are the host kernel's in a host process; ARM64 features
  whose results depend on them (FEAT_PAuth, FEAT_MTE) are not offered to the guest by these executors until the
  owner rules (section 12). They are reported as absent the way a CPU without them is.
- I: an embedder's `set*id` call or `pthread_cancel` of a runner thread waits until that runner's `Runner::run`
  returns (3.7).
- I with the wrapping form, if the owner allows it (5.2).
- P's ceiling is 2^44 against the VM form's 2^45; I's is 2^r - 64 KiB per process (2^32 on x86-64 at first).

## 10. Asked of other tracks (written up in the status file; not made here)

- Translator and ARM64 crates: the form value of 3.2 with its four places (the stack pointer's rules and the
  displaced one-instruction pieces inside the first), keyed in the translation cache; `REACH_BITS` and `MIRROR`
  (crates/wcontainers-arm64/src/mirror.rs) made per form; two more withheld registers for I.
- Native (VM) track: `stop::resolve`, `stop::guest_state` and the slot arithmetic taking the slot table's base as a
  parameter instead of `mirror::MIRROR`, and `stop` and `translation` in a crate that does not pull the hypervisor,
  kernel or runtime crates; the monitor's kept translations (`Translator` in hardware-virtualization's
  translate.rs) usable from such a crate too.
- Kernel: K2 as native.md asks (placement low first, the memory size given at start); several threads running at
  once (`next` handing out a second `Run` while one runs) for `ThreadOrdering::Parallel`; `kernel_entries` if the
  owner takes 8.2.
- x86 track: the x86-64 native output (XN1, XN2) with the `Way` of its plan, which 3.10 takes as it stands.

## 11. Measurements behind this plan (W0 probe)

Programs, in scratch native-without-virtual-machine/w0: probe.c (gcc 13.3 -O2 -static; 21 batches of 20,000, or of
2,000 for the repeated futex runs, median and minimum of the batch means; the two CPUs it pins chosen at run time as
the least busy over 2 s), fallback.c (3.4), and spcost/ (the cloud ARM64 cost harness of
costs-cloud/arm-recheck/native with two stack modes added to its rewriter, below). Run script run2.sh; outputs in
w0/run2/ (arm5-*.out, x865-*.out, spcost-*.txt). Machines: the rented ARM64 machine (Graviton4, Neoverse V2, 96
cores, bare metal, Linux 7.0.0-1018-aws, 4 KiB pages) at load average 14 to 27 from other tracks; the rented
x86-64 machine (Xeon Platinum 8488C, 96 processors, bare metal, Linux 7.0.0-1018-aws) at load 0.05 to 0.13. All
figures here come from this one run (run 2 of W0); run 1's outputs (probe-*.out, probe2-*.out, probe3-*.out,
probe4-arm.out) are kept beside it but no figure is taken from them.
- Address space: user space 48 bits (ARM64) and 56 bits (x86-64). [2^45, 2^47) is free on ARM64; the binary, vDSO
  and stack sit in [2^47, 2^48) on ARM64 and the stack and vDSO just under 2^47 on x86-64. A non-PIE static binary
  sits at 0x400000 with its heap just above, below 2^32 (hence the slots from 2^32 in I).
- Code: `PROT_EXEC` alone runs and a load from it faults (execute-only) on both; a memfd written in one process and
  mapped read-execute in another runs on both; a memfd with `MFD_NOEXEC_SEAL` maps executable.
- Faults and signals in one process, Graviton4 / Xeon: a fault on a PROT_NONE page whose handler skips the
  instruction, 1.31 / 1.33 µs; `tgkill` to itself with an empty handler 1.20 / 1.05 µs; `mprotect` of one page
  391 / 295 ns; `mmap` of one memfd page over another 1.15 / 0.90 µs; a hole punched in one memfd page 0.69 /
  0.54 µs.
- Stop latency, from the sender's `tgkill` to the target's handler, target spinning on another CPU: same process
  1.78 µs (p90 2.21) / 1.75 µs (p90 1.93); another process 1.68 / 1.90 µs.
- Process creation: a fork of this small static process until the child runs and is reaped 111 / 121 µs; a request
  to a zygote until its new child runs 80 / 95 µs.
- The futex slot between two processes, ppid-like, median of the batch means: Graviton4 one CPU 2.29 µs, two CPUs
  4.58 µs (the full probe), unpinned: the run of 20,000 per batch did not finish in 14 minutes (over 2 ms per round
  trip on average), and of three runs of 2,000 per batch one gave 4.63 µs and two did not finish in 120 s; repeated
  one CPU 2.36 to 2.37 µs and two CPUs 4.66 to 4.83 µs (3 runs each, CPUs chosen anew each time); Xeon one CPU 1.89
  to 1.96 µs (4 runs), two CPUs 4.02 to 4.16 µs (4 runs), unpinned 4.39 to 4.56 µs in 3 of 4 runs, and one run of
  2,000 per batch did not finish in 120 s (over 2.8 ms per round trip on average) on the idle machine. The first
  check's own re-run on Graviton4 at load 15 to 23 (w0-check/futex-arm.out) agrees: one CPU 2.32 to 2.38 µs, two
  CPUs 3.52 to 4.77 µs, unpinned 4.59 to 4.74 µs, and 3 of 5 unpinned runs of 20,000 per batch killed at 100 s.
  Hence P's placement (4.3).
- Code area homes (fallback.c; Docker 29.9.3, image alpine:3.22, on the Xeon machine, and the same results on the
  Graviton4 machine): natively, a memfd and `O_TMPFILE` files in /dev/shm and /tmp all map read-execute. In Docker
  with its default profile, memfd works, /dev/shm (`noexec`) refuses read-execute (Operation not permitted) and an
  `O_TMPFILE` in /tmp or / works. With a profile refusing only `memfd_create`: memfd refused, /tmp and / work. With
  `--read-only --tmpfs /tmp` (memfd allowed): /dev/shm and /tmp refuse read-execute and / refuses the file, so only
  memfd works there. Guest data maps read-write shared in every case.
- The stack pointer's guard (3.2 item 1), cloud harness: the 8 bench.c programs and calls, clang 21 -O2 as the cloud
  rows built them, X32 with the stack exempt (X32-e, the measured unsound form), X32-p (plus 3.2's probe after every
  `add`/`sub sp, sp, #imm`: `ldr wzr, [sp]` after a `sub`, `ldur wzr, [sp, #-4]` after an `add`) and X32-c (plus the
  3-instruction range check there instead), each against the plain build, 18 alternating pairs per program and
  variant on one CPU (3 rounds of 6, the variant order rotated), load 5.4 to 13.7 from other tracks. Output and exit
  status equal the plain build's for every program and variant on 3 runs. Geometric mean of the per-program medians,
  wall time, 8 programs (with calls): X32-e +21.8% (+19.1%), X32-p +21.4% (+18.8%), X32-c +21.7% (+19.0%);
  instructions +60.3%, +60.4%, +60.4%. Per program the three agree within their pairs' ranges (for example sort
  +33.9 / +34.3 / +34.3%, matrix +76.2 / +74.2 / +73.9%). So on this set the guard costs nothing measurable, because
  these programs rarely move sp by an immediate (7 such instructions per program, statically; their hot code uses
  writeback forms). In real binaries such moves are 0.16% of instructions in python3.12, 0.37% in bash, 0.61% in git
  (Ubuntu's gcc builds) and 1.95% in a Rust release binary of this workspace (statically, objdump), so one load each
  should add at most about 2% instructions there; W2 measures it on busybox, Python and a Rust program. A first
  build that probed at sp after an `add` too ended each program it was checked on (fib to matrix, before it was
  stopped) with a fault at 0x180800000, the top of the harness's stack mapping, at the outermost return; that is why
  3.2 probes 4 bytes below the new sp after an `add` (that verify output was overwritten by the second's). A
  campaign mixed with a leftover process of the first build was discarded too (w0/run2/spcost-discarded).
- So by section 7's arithmetic, with f = 0.214 (X32-p, 8 programs): Graviton4 c = 2.29 to 2.37 µs on one CPU gives
  90,000 to 94,000 kernel entries per second, 44,000 to 47,000 at the two-CPU c; the Xeon 8488C (Granite Rapids's f,
  0.110 to 0.152) 56,000 to 80,000 at c = 1.89 to 1.96 µs.

## 12. Decisions for the owner

1. Names: the two crates (`host-process`, `in-process`) and the form value's variants.
2. I's wrapping check (5.2): about 10 points faster on Graviton4, and a guest access outside its slot lands in its
   own memory instead of faulting.
3. P's reachable stub pages (9).
4. PAuth and MTE under P and I (9).
5. A clock the system publishes for reads without a kernel entry (8).
6. Changing the form at a guest exec (7).
7. The two interface additions of section 8, and for the first, a handle method on `Executor` or a handle type in
   `Runner::run`.
8. Per-thread watches, and so `(Parallel, Exact)`, under P by a host process per guest thread (8): wanted, and when.
9. I's runner threads holding off the embedder's SIGCANCEL and SIGSETXID while they run (9).

## 13. Milestones

**W1: form P on ARM64 Linux.** Needs the translator's `HostProcess` form and the `stop` crate with a parameter for
the slot table's base (10). Builds: the shared layout, floor, code area, memory with copy-on-write, stops (section
3), the zygote and stubs, the channel, placement and the runners (section 4), the start-up probe and the seccomp
filter. Pass bar, every item on the rented ARM64 machine:
- the executor conformance suite and the abi `Cpu` behaviour the kernel relies on (the kernel test crate's
  programs: hello, the fork, exec and signal program, and busybox's `text` and `files` commands, as N3's bar) give
  the reference's output and status; exits, registers and memory equal the WebAssembly output's at every look-ahead,
  with and without the count (the VM form's differential test, seeded, with its case knob; cases run recorded);
- the count at every stop is identical to the WebAssembly output's (forced interrupts at every counted event);
- the adversarial cases, each a kept test: a fork while a sibling thread is in a system call and another writes a
  page in a loop, words rising by one per pass (seeded, case knob): in every child's copy the words fall at most once,
  as a copy taken at one instant gives; a parent that forks twice before the page is copied and a child that forks
  before copying, each process's contents checked against the reference; a shared mapping written by two guest
  processes, checked against the reference; a stop signal landing at every instruction of the floor's entry, resume,
  wait and handler-tail windows (forced by a seeded delay); a guest that writes its own channel and register pages
  with forged commands and registers (it changes only itself; no host code runs that we did not translate); a stub
  that tries a host call outside its filter is killed; a stub killed from outside ends its process as SIGKILL; the
  embedder killed leaves no stub or zygote behind; the register-group isolation tests of 3.8;
- the suites pass inside a container started with Docker's default seccomp profile;
- speed, measured with machine and load: the guest's getppid round trip with P's placement, its median under 1.25
  times the bare futex slot on one CPU measured in the same run, alternating, and no round trip over 1 ms in a run of
  1,000,000 on idle CPUs; busybox `sh` starting 1,000 programs, its time at most 1.25 times the model's prediction
  (the reference's own time plus, per program, two process creations and its kernel entries times the round trip,
  plus the translation time, each measured in the same run); every slow path with its counter.

**W2: form I on ARM64 Linux, and the crossover.** Needs the translator's `InProcess` form. Builds: slots, the
check and the stack pointer's rules, in-process entry and eager switch, the signal rules of 3.7, the slice timer
(section 5). Pass bar:
- the same suites as W1 (conformance, kernel test programs, busybox, differential, count, adversarial,
  register groups, Docker's default profile);
- exactness of the check: a seeded random test (knob for cases; count recorded) of accesses at, just inside and
  just outside the slot edges, tagged and untagged, every access form, each faulting exactly where the reference
  does with the reference's registers; sp moved out of the slot by every form of 3.2 item 1 (immediates up to 16 MiB,
  writebacks, register writes), then read, moved back and accessed, each matching the reference, the displaced
  counter recorded; slot exhaustion gives ENOMEM at fork, counted;
- speed: the translator's benchmark set under I against the same programs run plainly, alternating pairs, within
  the sound form's figure (X32-p, section 11) plus 3 points on the same machine (re-measured at W2 start); the
  getppid entry under 100 ns median;
- the crossover measured with both forms built on the ARM64 machine: the compute benchmark, a Python start, and
  busybox `sh` starting 1,000 programs, each in both forms with its kernel entries per second, and the measured k at
  which the two cost the same, against section 7's arithmetic.

**W3: both forms on x86-64 Linux.** Needs the x86 native output with its `Way` (XN1, XN2). Builds 3.10's parts.
Pass bar: W1's and W2's bars on the rented x86-64 machine, the crossover there included, and the x86 plan's own way
P and way I tests.

