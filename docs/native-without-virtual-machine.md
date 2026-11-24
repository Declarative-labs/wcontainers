# Native without a virtual machine: the plan for both forms

Status: plan (W0) accepted, 2026-11-23; W1 run 1 answered the fourth check's findings (the zygote's inherited
restartable sequences, K2 named as W1's need, the SVE ABI, the zygote's end, I's checked address, the probe fault's
counter, the wait window, aliasing instruction caches).
Brief: the owner's ruling of 2026-11-23 and brief-native.md (including its section on register groups). It builds on docs/native.md (the
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
  to 16.4 ns with q0 to q31). Each guest process gets a 4 GiB window, and the access after the check is cut to that
  window by its own addressing mode, so no speculation past the check reaches outside it (3.2). This exact sequence,
  with the stack exempt (X32-e / X32-nosp), measured +19.4% wall on Graviton4, +11.0 to +15.2% on Granite Rapids
  and +10.0 to +25.4% on Zen 4, the ranges being code layouts (costs-cloud.json); in one run on bare-metal
  Graviton4 the sound form with the stack pointer's guard measured +21.4% and the exempt form +21.8% (section 11).

Shared by both, built once (section 3): the address layout, the floor's entry and exit, position resolution, the
slot table, the code area, copy-on-write, the stop protocol, the register-group rules and the
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
  address). I: guest processes, each in its own **slot** of 2^33 bytes, rebased (host address = slot base + guest
  address): slot k (k from 1 to 2047) starts at k·2^33. Its lower half, the **window** [0, 2^32), holds the guest
  process; its upper half [2^32, 2^33) is never mapped. In the window the guest's own [0, `LOWEST_ADDRESS`)
  (64 KiB) and its top [2^32 - `GUARD`, 2^32) with `GUARD` = 64 KiB are never mapped either (the process's ceiling
  stops below it). So below and above every window lie 4 GiB that no one maps (below slot 1, the
  reservation's own [2^32, 2^33)); 3.2 relies on it. Nothing a non-PIE embedder or the host's low mappings hold is
  inside, since the reservation starts at 2^32.
- **I's slot count**: 2047 per embedder process, shared by every I container it runs (one layout per process,
  below). A fork that finds no free slot gets ENOMEM from `memory_new`/`memory_duplicate`
  (`CpuError::OutOfMemory`, counted "in-process slots exhausted", warned); the evaluator (section 7) sends
  workloads with many live processes to P. The pids limit plays no part: it counts threads as well.
- **One layout per embedder process.** I's reservation, slot pool, slot table and signal handlers (which Linux
  keeps per process anyway) are made once, by the **in-process host** the embedder makes at start (section 8.5);
  every I executor, one per container, is made from it and takes its slots from the shared pool. Two I
  containers in one process therefore never collide: they hold different slots of one layout. P needs no fixed
  range in the embedder's process (its views are windows the host places, 4.2), so P and I containers mix in one
  process; P's zygote removes I's reservation in its child like everything else of the embedder (4.1). A second
  in-process host in the same process is refused ("in-process host exists"), as is a first one whose reservation
  the host refuses.
- As built (W1 round 3 run 2): a slot page's bytes are the next free page of the code area's memfd past the code
  area (128 MiB of them), never reused, mapped by a `MapSlots` command execute-only; all in use ends the process,
  counted "slot pages exhausted".
- **The slot table** (ARM64; the VM form's entry mirror): a 16-byte slot for every host address H of the guest
  area at `SLOTS | (H << 2)`, `SLOTS` = 2^46, so the table spans [2^46, 2^47). It is one memfd per guest memory
  (P) or one per embedder process (I: the slots of different containers are different host addresses), written by
  the embedder (P: through its windows, 4.2; I: through its own mapping), and mapped read-execute in the guest's
  view **page by page as pieces are loaded** (P: by a command the stub applies, which its filter allows in exactly
  this range, 3.7; I: by the executor itself). A jump to a slot in a page not mapped is a host SIGSEGV at a table
  address, which the floor treats exactly as a slot holding `udf` (no piece for that guest address). So a guest
  that jumps to many addresses costs no memory (a shared file mapping would allocate a page on every read fault of
  a hole). The jump through a register is the VM form's six instructions (native.md section 3.2) with the mask's
  `eor` replaced: P computes the slot from G itself (`ubfiz`, `bfxil`, `orr` of the single bit 2^46, a logical
  immediate); I first adds the slot base to G cut to the window (`add t, base, wN, uxtw`), the one more
  instruction it pays per jump, so that even a misspeculated jump reads only its own process's slots. The reach
  test is against the process's ceiling (P: 2^44; I: 2^32).
- **Our area** [2^44, 2^46): I: the kernel's read-write view of slot k's window at 2^44 + k·2^32 (so a guest
  address is one add away; 2048 · 2^32 = 2^43), then code areas, floor, register blocks, signal stacks in [2^45,
  2^46). P, in the stub: code area, control pages (read-only to the stub, 4.3), register pages, signal stacks, the
  stub's code and stack. P's
  embedder process maps its views of the stubs' memfds as windows wherever the host places them (4.2; it runs no
  guest code).
- Each form reserves its areas at start (`mmap` PROT_NONE, MAP_NORESERVE, MAP_FIXED_NOREPLACE): the host never
  places anything inside a reservation, so our code and the guest cannot collide; an embedder mapping made later
  goes elsewhere, and `brk` stops below a reservation (malloc then maps). If a reservation is refused, the form is
  unavailable on that host (the evaluator's probe, section 7). That is also what happens to I in an embedder that
  maps fixed addresses inside [2^32, 2^47) before it (a sanitizer's shadow, such as AddressSanitizer's on ARM64
  from 2^36), named "in-process reservation refused". Code of the embedder that later maps with `MAP_FIXED` over
  someone else's reservation breaks any program and is not detected.
- **The ceiling**: `Cpu::address_space()` is [LOWEST_ADDRESS, 2^44) in P (the VM form has 2^45: the slot table at
  2^47 would need the host's stack and vDSO moved; 16 TiB is the ruled "lower ceiling", brief 2026-11-18) and
  [LOWEST_ADDRESS, 2^32 - GUARD) in I. Today's kernel takes its ceiling from `Cpu::address_space().end`
  (crates/kernel/implementation/linux/container.rs, where a container starts) and places mappings top-down below it,
  so under P a guest gets other addresses and another memory size than under the WebAssembly output (about 2^32),
  and a program whose path depends on a pointer counts differently. K2 (placement low first, the memory size given
  at start; asked of the kernel track, section 10, as native.md asks it) removes the difference, and is in the
  kernel since its KF milestone: mappings are placed in tiers lowest first, the first tier's top is the lower of
  4 GiB and the container's ceiling, and the embedder may give the ceiling (`Start::ceiling`) and the memory size
  (`Start::memory_bytes`). A comparison with the WebAssembly output gives both that output's ceiling and memory size;
  W1's kernel test does, and addresses, memory size, mappings and the turns of one-thread programs are then equal
  (`addresses_memory_size_and_mappings_are_the_web_assembly_output_s`). A process that reaches its ceiling is the
  existing `on_process_outgrown` case. I's 4 GiB per process is the price of an access cut to its window for free; a larger
  window needs one more instruction per access (an `and` with the window's mask), not measured, the owner's choice
  (section 12).

### 3.2 The four places the native output changes

The native output stays one emitter (native.md section 3); a form is a value the executor gives the translator,
part of the translation's cache key, with three variants (`Vm`, `HostProcess`, `InProcess`). It changes exactly:
1. **`access`** (crates/wcontainers-arm64/src/carry.rs, the single place a guest access is emitted) and the
   instructions that read or write sp. `Vm` and `HostProcess` emit them as today. `InProcess`:
   - An access through any register but sp: the check `ubfx u, a, #32, #24; cbnz u, fault`, then the access cut to
     the window: `<access> [base, wA, uxtw]` for the forms that have a register offset (plain loads and stores of
     one register, general or FP/SIMD); `add t, base, wA, uxtw; <access> [t, #imm]` with an immediate offset or a
     pair; `add t, base, wA, uxtw; <access> [t]` for the forms that take only a base register (exclusives
     `LDXR`/`STXR` and their pairs, acquire and release `LDAR`/`STLR`/`LDAPR` and their unscaled forms, LSE atomics
     `CAS`, `SWP`, `LDADD` and the rest, `LD1` to `LD4` and `ST1` to `ST4` structures and their replicating and lane
     forms). Here `a` is the guest address the access starts from before its immediate: the base register itself
     for an access with an immediate offset or none, and the base plus the register offset (with its extend or
     shift, computed into t) for an access with a register offset; the immediate is never added into `a`, only by
     the access's own `[t, #imm]`. `u` is a second withheld scratch, `base` a withheld register holding the slot
     base. The base-only forms pay one instruction more than the measured sequence (its harness never emitted them;
     section 7 lists them among I's costs left out). The check ignores the top byte as Linux's Top Byte Ignore
     does and touches no flags. Only `a` is checked, as the measured harness checks only the base register: what an
     access reaches past `a` (its immediate, at most 64 KiB up and 1 KiB down, size included; a pair's or a
     multi-register form's further bytes, at most about 1 KiB) lands, from an `a` inside the window, in the window,
     in its unmapped top `GUARD` or in the unmapped 4 GiB beside it, so it faults there with no second check. `fault` is out of line and reports the guest address and access with exact registers (the check
     precedes the access, so nothing has changed). This is the measured X32 sequence (section 11; the harness
     tests `lsr #33` only because its region is [4 GiB, 8 GiB)).
   - **Speculation.** A mispredicted `cbnz` lets the access run speculatively with a bad address (a bounds-check
     bypass, Spectre variant 1). Here the access's address is `base + (a mod 2^32)` by its addressing mode (`uxtw`),
     so even then it reads or writes only this process's own window, which the guest can read anyway; no
     speculation of a guest access reaches another slot, our area, the kernel or the embedder. The cut is data, not
     a prediction, so it needs no barrier (Cranelift masks its bounds-checked heap accesses for the same reason). The
     wrapping form (5.2) is safe by the same construction. Measured on Graviton4 for every shape above that loads
     speculatively (section 11, W1 run 6): behind a check with a long window the uncut access read a byte outside
     the window in about 98% of tries and the cut one never did, reading its in-window byte instead.
   - **The stack pointer.** sp holds the host address (slot base plus the guest's sp), since an ARM64 access
     through sp cannot be rebased. The rules keep one invariant: at every guest instruction boundary the guest's sp
     is inside its window, [0, 2^32).
     (a) An access through sp with an immediate offset is emitted unchanged. Its reach is at most 1 KiB down and
     64 KiB up, size included (`ldr q, [sp, #65520]` is the largest). From an sp inside the window it lands in the
     window or in the unmapped 4 GiB beside it, so it faults there exactly where Linux faults.
     (b) A pre- or post-indexed access through sp by an immediate (a move of at most 1 KiB) is emitted unchanged: a
     pre-indexed one succeeds only if its new sp is on a mapped page of its own window, a post-indexed one moves
     from a mapped page by at most 1 KiB, which stays inside [0, 2^32) because the window's own unmapped bands at
     both ends are 64 KiB; a fault leaves sp unchanged.
     (c) `add`/`sub sp, sp, #imm` with imm at most 60 KiB is followed by a probe, one load whose value is unused:
     `ldr wzr, [sp]` after a `sub` (the new frame's lowest word) and `ldur wzr, [sp, #-4]` after an `add` (the last
     word of the frame just freed, mapped even when sp is now the very top of a stack mapping, where a probe at sp
     faulted at the outermost return in the measurement's first build). A probe that succeeds proves the new sp is
     on, or 4 bytes above, a mapped page of its own window, so inside [0, 2^32). A probe that faults (an unmapped or
     protected page, a watch, sp not 16-aligned) is never the guest's fault: the floor takes
     the new sp less the base; inside [0, 2^32) it resumes at the next guest instruction (the probe ends the `add`'s
     or `sub`'s code, so that is a boundary), otherwise the thread goes displaced (e). A swallowed probe fault is a
     slow path of about 1.3 µs (a fault handled in process, 3.5), counted "stack probe faulted", warned; a stack
     growing into a page not yet grown pays it and then the guest's own fault there, which the kernel answers.
     (d) Every other write of sp (`mov sp, xN`, `add sp, xN, ...`, a larger immediate, a writeback by a register)
     computes the new guest value into u, checks it as an access address is checked, and then writes `add sp, base,
     wU, uxtw` (cut to the window, as an access is); a value outside the window makes the thread displaced (e).
     (e) **Displaced**: the guest's sp is outside its window, which Linux allows until an access through it. The
     floor keeps the guest's sp in the thread's register block and runs the thread one guest instruction at a time,
     each translated alone with sp read from and written to the block and every access through it checked like any
     other register (so it faults at the guest address Linux faults at). When an instruction brings sp back inside
     the window, the thread re-enters ordinary translated code. A slow path: counted "stack pointer outside its
     window", warned; no compiler's code does this.
     (f) Every other access through sp is emitted as an access through any other register, with `a` computed as
     `sub t, sp, base` and then the access's own offset added as written (`add t, t, x1` for `ldr x0, [sp, x1]`,
     `add t, t, w0, sxtw` for `strb w1, [sp, w0, sxtw]`), then checked and cut as above. A structure access
     post-indexed by a register (`ld1 {v0.16b}, [sp], x2`) is the access at `[sp]` by (a) followed by the write
     `add sp, sp, x2` by (d). With (a) to (f), every A64 and Advanced SIMD addressing form with sp as base is
     covered: immediate (signed, unsigned, unscaled), pre- and post-index by an immediate, register offset with its
     extend or shift, post-index by a register, and the no-offset forms of exclusives and atomics (a). SVE and SME
     are not offered to the guest (the translator decodes neither, so a guest finds them absent as on a CPU without
     them), so none of their forms reaches the emitter; if they are offered later, each of their sp forms whose reach
     is not fixed and small is emitted by (f).
     Speculation of sp: (d) cuts sp to the window; (a) to (c) leave the address unmasked, but a misspeculated run
     past a probe's fault (taken only when the probe retires) can move sp by at most 60 KiB per `add` or `sub` it
     executes, and a speculation window holds at most a reorder buffer's worth of instructions (a few hundred on
     these cores, published figures), so a speculative sp stays within a few tens of MiB of the window: inside the
     4 GiB that no one maps on either side.
     A read of sp as a value (`mov x29, sp`, `add x0, sp, #n`) is re-emitted with the base subtracted (`sub x29, sp,
     base`: the same length). The position map gives the guest sp as the host sp less the base. Every handler runs
     on its own signal stack (`SA_ONSTACK`).
2. **The jump through a register**: the slot table's base and reach (3.1), and I's base add.
3. **The call to the floor** (`Arm64::call`, today `brk #code`): a `brk` in a host process is a host SIGTRAP
   (1.2 to 1.3 µs measured for a signal round trip, W0 probe), so `HostProcess` and `InProcess` call the floor's
   entry for the call's code. As built for P (W1, `wcontainers_arm64::Way::Process`): the call is one branch in
   line, as `brk` is one word, so a jump's last-target layout is the same in every way, to code out of line:
   `str x30, [page, #link]; movz/movk x30, entry; blr x30; ldr x30, [page, #link]; b back`. The entry's address
   is absolute (`PROCESS_ENTRIES` + code · 16, the zygote's code at 2^45), so a piece needs no relocation where it
   is loaded; `bl` would. The return address names the call (the floor reports it at the `blr`, as the VM form's
   exception address names the `brk`), and a call the kernel's side answers in place goes on at the `ldr`. The
   position map records x30 (and any register x30 stood in for, and a jump's target x30 held) as held in the page's
   `link` word from the store on, except where the stop's fix-ups give x30 its whole value (a `blr`'s return
   address), which then stands (W1 round 7 run 2, found by `differential`). Each entry keeps x23 in the stub's register page before naming itself there (`str x23, [x28,
   #call_scratch]; movz x23, #n; b save`), since a jump's last-target call leaves its target in x23; the embedder
   puts it back into the block. The native output's register page is the first 128 bytes of the stub's register
   page, so x28 is the same address for both.
4. **The replay check** (native.md section 5.7): `TPIDRRO_EL0` is the host kernel's in a host process, so the stop
   word is a word of the thread's register page: `ldr t, [page, #stop]; sub t, t, c; cbz t, stop`. As of W1 the
   native output emits no replay check in any way (none reads `TPIDRRO_EL0`), so nothing changes here yet.
The slot table's base and reach, the entries' address and size are `wcontainers_arm64::way`'s constants, which the
host-process layout asserts; stops resolve through `wcontainers-executor-arm64-stop` with the way as a parameter.
Withheld registers: the VM form's three (count x18, scratch x23, register page x28) in P; two more in I (`base`
and `u`), chosen by the same dynamic-use rule (native.md section 3.3). Everything else of the native output (routes,
pieces, linking, count placement, the position map) is unchanged. These are the translator track's files: section
10 asks for them.

### 3.3 Exits, the floor and position resolution

As built for P on ARM64 (W1 run 7, before the translator's form exists, with pieces written by the tests standing
in): the register block in the register page is the hardware's state. A turn's `Enter` loads every register from it
(FP/SIMD, FPSR, FPCR, `TPIDR_EL0`, NZCV, sp, the general registers) but x23, which holds the code's address, and
x28, the register page, and branches there. The floor's 64 entries (`str x23, [x28, #call_scratch]; mov x23, #n;
b save`, 16 bytes each; x23 is put back by the embedder) save every register
into the block, the link in x30 and pc, and publish `Called` with n. A stop or a fault (SIGSEGV, SIGBUS, SIGILL,
SIGTRAP) whose pc is in the code area saves every register from the signal frame (its `fpsimd_context`, checked by
its tag and size, for the vector registers, FPSR and FPCR) and publishes `Interrupted` with the host pc, or `Fault`.
A stop during the enter's loads ends the turn as `Stopped` before the code runs, the block untouched; a stop in an
entry or the save finishes the save from the frame, the entry's number taken from the pc in the table, then from
x23, then from the block (counted apart). Resolution to guest state is the embedder's, waiting on section 10.

- An inside call enters the floor's entry for its code with the guest's registers in the hardware. The entry
  first stores every general register, sp, pc (from the call site through the position map) and the condition
  flags into the thread's register block, and then the FP/SIMD registers with FPCR and FPSR: the whole file the
  guest has (no SVE or SME, 3.2), at every exit, in both forms. It costs about 4 ns more than the general registers
  alone (16 against 12 ns for a save and load; section 1)
  and it means `Cpu::registers` never needs the thread: in P the kernel's side reads the block in the register page;
  in I the kernel's own Rust code, which uses vector registers, may run at once. It forms the `Exit` exactly as the
  VM form's inside does.
- A host signal in translated code (SIGSEGV, SIGBUS, SIGILL, SIGTRAP from a guest `brk`, and the stop signal)
  gives the floor the host pc and every register in the signal frame (general, `fpsimd_context`, `esr_context`
  for the access kind on ARM64). The handler copies the frame once; the host pc it holds is only
  looked up in the position map (a value the map does not know ends the thread as corrupted, counted). The guest
  state at a guest instruction boundary comes from the position map with the VM form's resolution
  (`hardware-virtualization-inside`'s `stop::resolve` and `stop::guest_state`). Those functions take the slot
  table's base from `wcontainers_arm64::mirror::MIRROR`, the VM form's, and their crate pulls in the VM's
  dependencies, so section 10 asks for the base as a parameter and for `stop` and `translation` in a crate of
  their own; W1 needs both before it can resolve a stop.
- Resuming is always at a guest instruction boundary through the guest pc's slot, never at a host pc in the middle
  of a piece and never from an address in a frame. A fault the floor resolves itself (copy-on-write, a code page's
  first write, a probe) resumes at the resolved boundary too: faults are precise and the position map
  rolls the state back to the guest instruction's start, so running it again from there is exact.
  `Resume::Continue` after a stop is the same re-entry, with the count set from the map.
- Loading pieces, filling slots, linking exits and the kept translations are the VM form's
  (hardware-virtualization-inside `translation`, the monitor's `Translator`); section 10 asks for the monitor's kept
  translations to be usable without the hypervisor crates.

### 3.4 The code area and code a program writes

As built for P on ARM64 (W1 run 7): the code area is [2^45 + 128 KiB, 2^45 + 128 MiB), right after the zygote's
pages, so every piece reaches the floor's entries at the start of the zygote's code with one `bl` (one code area of
just under 128 MiB per guest memory until more is needed); the stub receives its memfd like a shared block and maps
it `PROT_EXEC` alone by a `MapCode` command (the filter allows exactly that call); the embedder writes through its
own read-write mapping, then cleans and invalidates by `CTR_EL0` and ends with `dsb ish` (crate code.rs).

- One memfd per guest memory holds its code area: mapped read-write in the embedder's process (the translator
  writes there) and read-execute in the guest's view (P: in the stub; I: a second mapping in the same process).
  Measured (W0 probe): a memfd written in one process and run from a read-execute mapping in another runs, on both
  machines; a memfd made with `MFD_NOEXEC_SEAL` still maps executable. After writing, the writer does the
  architecture's cache maintenance on its own mapping (ARM64 `dc cvau`/`ic ivau`); the guest thread runs the new
  code only after leaving the host kernel (a futex wait or a signal), which is context synchronizing on ARM64.
  Invalidation by the writer's address covers the runner's mapping of the same line only where the instruction
  cache is physically indexed, so the start-up probe reads `CTR_EL0` (Linux lets user space read it): with DIC set
  no invalidation is needed, with L1Ip PIPT (Neoverse V2's) the writer's `ic ivau` covers every mapping, and with a
  VIPT instruction cache, which can alias (where Linux invalidates the whole cache, which user space cannot), both
  forms are unavailable on that host, named "aliasing instruction cache". W1 records both rented machines' value.
  x86-64: 3.10.
- Where `memfd_create` is refused: guest data in unlinked `O_TMPFILE` files in whichever of one the embedder
  names, `$TMPDIR`, /dev/shm and /tmp has the most free space at start among those that are tmpfs (`fstatfs`,
  `TMPFS_MAGIC`; data is never mapped executable, so `noexec` does not matter), or among all four if none is
  (a disk file's dirty pages are written back to the disk: counted "guest memory on disk", warned), and the code area and slot table in `O_TMPFILE` files in the first of the
  embedder's, `$TMPDIR` and /tmp whose mapping read-execute succeeds at the start-up probe. If none does (a
  read-only root with a `noexec` /tmp), the form is unavailable. Such a file can fill up (Docker's /dev/shm is a
  64 MiB tmpfs: a write past it is a SIGBUS, measured by the second check), so in this fallback the kernel's side
  allocates a page range with `fallocate` (mode 0) before its first write there (no space: `CpuError::OutOfMemory`),
  and a guest write that takes a SIGBUS on such a file ends the guest process as killed by SIGKILL, as the host's
  out-of-memory killer would (counted "guest memory file full", warned; the fallback itself is counted "guest
  memory in a file"). Measured in Docker 29.9.3
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
- A page's permission in the guest's view is one function: the permission the kernel last gave it through
  `Cpu::set_permissions` (which already carries the kernel's own watches and written marks, below), less write while
  it is lent to a fork child or holds translated code, less everything while it is borrowed (below). Every change
  of either recomputes it.
- The kernel's view: I gives each slot a read-write view in our area (3.1) laid out as the guest's address
  space: the memory's memfd over the whole window (holes read as zero) and each shared block's memfd over the range
  it is mapped at, so a guest address is one add away whatever backs it; P maps, in the embedder's process, 2 MiB
  windows of its memfds when the kernel first touches them, a bounded cache (4.2), finding a shared block's memfd
  from the executor's record of `shared_map`.
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
  - A borrowed page is inaccessible in the borrower's view. The borrower's touch (a fault, or a `Cpu::write` of
    it) copies the owner's page into the borrower's memfd, which then owns it. Reads included: a fork
    child pays for every unit it touches, not only those it writes. In P that is a round trip (c, 4.3) plus the
    copy and one protection command (391 ns measured for an `mprotect` of a page, 3.5's figures), so about 3 µs per
    unit expected; in I the in-process fault (1.31 µs) plus the copy. Section 7 counts it.
  - Before an owner's page that is lent changes (the guest's first write faults; a `Cpu::write` or `zero`; a
    shared block mapped over it), the page is copied into one borrower, which becomes the owner, and every
    other borrower now borrows from it: one copy per event, whatever the number of borrowers. Then the old owner's
    page is free to change. The new owner's view gains access at its next touch (a fault the executor resolves).
  - A borrower that ends or unmaps the page leaves the lent set without a copy. An owner that ends or execs while
    it lends pages keeps its memfd, not its process (its stub is ended and reaped), until no page is borrowed from
    it, each page punched out as its last borrower leaves it: nothing is copied for it, as on Linux, so a parent that
    forks and exits at once (a daemon's double fork) pays nothing more; counted ("owner ended while lending").
  - A `Cpu::read` of a borrowed page reads the owner's page; it copies nothing.
  - Views are never looser than these rules allow, and may be stricter: a fork child's view starts with no access
    at all, `set_permissions` over a range with a borrowed page gives the view no access there and over one with a
    lent page no write, and a fork takes write from every writable range of the parent's view. A fault the
    kernel's permissions allow is the executor's (it never reaches the kernel): it copies or gives away the page as
    the access needs and gives the widest range around it whose pages all share one state what the kernel allows,
    at the thread's next resume. The decision is made from the executor's records alone, so a forged fault report
    does only what a real access of that guest would.
  The copy source is always write-protected in every view of its owner (it is lent), so every copy is of the page as
  it was at the fork's instant, as on Linux. Copies are made in units of 2^u pages (u by measurement in W1, from 0
  to 9, against the number of host mappings it creates: mprotect splits a mapping at the unit's edges, and the host's
  vm.max_map_count is 65530 by default, 1048576 on the Ubuntu machines here); built as `COPY_UNIT` with u = 0 (one
  page). Measured in W1 run 5 with stores standing in for guest code (each store a turn, about 5.3 µs alone; section
  11): a child storing to all 16,384 written pages in order pays 20 µs a store at u = 0, 7.8 at u = 3, 5.4 at u = 6;
  storing to every 16th page in a seeded order, 25 µs at u = 0, 36 at u = 3, 31 at u = 9; that child holds 2,054
  host mappings at u up to 3 and 8 from u = 4. Measured again on translated stores (W1 round 5 run 1, Graviton4,
  CPU 41, load 12 to 15; 16,384 pages): in order 18.2 to 18.9 µs a store at u = 0, 4.1 to 4.3 at u = 3, 2.3 to 2.5
  at u = 6, 2.1 to 2.2 at u = 9; every 16th page in a seeded order 23.4 to 24.3, 34.8 to 35.0, 31.0 to 31.5 and
  27.4 to 27.5 µs; busybox `sh` starting 1,000 programs 4.52 to 4.56 s at u = 3 against 4.67 to 4.78 s at u = 0
  (alternating, 3 rounds). Built with u = 3.
- **Written marks and watches are the kernel's** (`Process::take_dirty_memory`, `Process::watch_memory`): the
  kernel keeps both itself and makes them with `Cpu::set_permissions` (write taken away until the first write,
  which comes back as a fault exit; crates/kernel/implementation/linux/memory.rs `take_dirty` and `reapply`), and
  marks its own writes itself. So these executors add no mark of their own: they honour `set_permissions` (in P a
  queued command, 4.2) and report the fault. No host mechanism could do better here: ARM64 Linux has no soft-dirty
  bit, x86-64's needs /proc, and userfaultfd's write protection is withheld by sandboxes.
- Shared blocks map the same memfd in every view, so two guest processes writing a shared mapping see each other's
  writes through the host's coherent memory, with the hardware's ordering; the kernel's writes into one show in
  all. The kernel checks watches before its own writes, as native.md section 5.3 says.
- Measured costs of the pieces (W0 probe, Graviton4 / Xeon): mprotect of one page 391 / 295 ns, mmap of one memfd
  page over another 1.15 / 0.90 µs, a hole punched in one page 0.69 / 0.54 µs, a fault handled in process and
  returned from 1.31 / 1.33 µs.

### 3.6 Stopping a thread, slices and events

- A stop is a signal sent with `tgkill` to the guest thread's host thread. The signal itself is the
  request: nothing in memory the guest could write decides whether a thread stops. Under P it is a standard signal,
  so stops sent while one is pending are that one: a burst of stops can hold a thread in its handler only while it
  lasts, and never fills the host's queue of pending signals, which the embedder's whole user shares
  (`RLIMIT_SIGPENDING`). A real-time stop queued every one (measured, W1 run 6, Graviton4 at load 4 to 10: a thread
  told to end under stops sent back to back, its two CPUs shared with the sender, found with 768,008 stops queued
  and ended 443 ms after the sender stopped). A stop that finds the thread
  already leaving is harmless: the kernel side sees an exit it would have got anyway, or a `Stop` it did not need
  and continues (counted "stale stop"). Measured (W0 probe): from the call to the handler running on a spinning
  target, 1.78 µs median in the same process and 1.68 µs in another (Graviton4); 1.75 / 1.90 µs (Xeon).
- Every handler of ours is installed with `SA_SIGINFO | SA_ONSTACK | SA_NODEFER` and `sa_mask` = {the stop signal}:
  a second fault is delivered (never fatal for being blocked), and the stop signal is held off only while a handler
  runs. On x86-64 also `SA_RESTORER` with `sa_restorer` set to our own `rt_sigreturn` routine: the host kernel
  refuses to deliver to a handler without one and kills the process (measured by the third check, through the raw
  `rt_sigaction`, which rustix uses and which adds no restorer as glibc's does); in P that routine is never reached
  (no handler returns to it; x86-64's stub filter refuses `rt_sigreturn`), in I it is the one way back below. ARM64 needs
  none (the vDSO's).
  No handler returns through a frame the guest can write: in P none returns at all; in I one returns
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
  - P, applying the queued commands of a resume (4.2; a `clone` among them is not idempotent): the stub blocks the
    stop signal with one `rt_sigprocmask` before the first command and unblocks it after the last, so a stop that
    arrives there is pending until every command is applied and then lands in the resume sequence above, with the
    commands done exactly once. Only a resume that carries commands pays the two calls (2 × 125 ns on Graviton4),
    after anything that changes the guest's view or makes a thread, whose own host call (`mprotect` 391 ns,
    `mmap` 1.15 µs, section 3.5) costs more;
  - the wait (P: from the store of the request word that publishes the exit, through the wake of the runner and
    the futex wait for the answer to its return; and a new thread's start from its unblock on, 4.1): it goes to the
    wait, which reads the answer word again and never publishes again, so a turn's exit is published once. The
    entry's window ends at its last register store, before the request word, so the two windows meet and no
    instruction belongs to neither;
  - the handler's own tail, after its unblock: it takes the same exit the interrupted handler had already written.
  As built in P (W1 run 3): the handler resets sp to its thread's signal stack top, reads the interrupted pc from the
  frame only to choose among these windows (a forged pc can only pick another of our own targets, with our own
  registers), and leaves through an unblock whose next instruction lies in the window it leaves for, so a stop that
  lands just after the unblock takes the same way; a stop in the publishing of an exit publishes it again (the same
  words), and a stop in a thread's `exit` lets it end.
  A frame without the FP/SIMD record (W1 run 10): Linux puts the interrupted FP/SIMD registers in the frame's first
  record (`fpsimd_context`), but gVisor on ARM64 (release 20261109.0, systrap) puts no record there, keeps them
  itself until the handler returns, and enters the handler with them zeroed (measured, scratch w1/run10/gframe.c).
  gVisor also keeps that state for good when a handler never returns: a stop flood of 42 million stops grew its
  sentry to 377 GB until the host's out-of-memory killer ended it (W1 run 10). So where the record is absent every
  P handler that goes on in the stub leaves by returning through its frame (`rt_sigreturn`) into the routine it
  would have jumped to, with every general register and sp as the handler holds them, pstate zero and the stop
  added to the frame's mask (so the stop is blocked there as after a jump, and its unblock is unchanged); a handler
  that ends the thread or the stub needs none. A stop or fault in translated code saves the general registers from
  the frame as before and returns into a routine that stores the FP/SIMD registers the host restored (counted per
  guest process, "signal frame without FP/SIMD registers", warned once per guest process). The stub's filter allows
  `rt_sigreturn` on ARM64 for it, which gives an escaped stub nothing it lacks (it already sets its own registers,
  and `rt_sigprocmask` and `sigaltstack` are allowed as they come). Forced on every handler on Linux, the stop,
  floor and step tests pass too.
  So no signal mask is changed on a kernel entry path but a resume that carries commands (a mask change is a host
  system call: 125 ns on Graviton4 and 87 to 89 ns on the Xeon, W0 probe run 3).
- A slice (`Next::Run { slice_nanoseconds }`): P: the runner's futex wait carries the slice as its time-out and
  sends the stop on expiry (nothing is paid when the thread comes back first). I: a per-runner POSIX timer
  (`timer_create` with SIGEV_THREAD_ID) re-armed only when a new deadline is earlier than the armed one. P: a thread
  that has not come back 100 ms after its stop is sent it again every 100 ms, and after 10 s its process is killed
  by the zygote (4.1), counted ("thread ignored its stop"; only a compromised stub can block the signal). A stop
  that lands before the thread has taken its turn lands in the wait and changes nothing, so the runner sends a
  slice's stop only once the thread has written the turn it took to its register page (`TAKEN`, looked at every
  1 ms), or 100 ms after the slice at the latest (the word is the guest's to forge, which delays only its own stop).
  Measured (W1 run 4, Graviton4): without that, a 1 ms slice on a CPU shared with a spinning program ran 104 ms.
- A turn that must end at an exact count is the VM form's open decision 10. Until the owner rules, P has no
  single-step (not without ptrace), so an executor made for a container whose policy gives count limits
  (`HostProcessExecutor::exact`, a stand-in for the embedder's choice) translates every piece as one instruction,
  never linked and never given a slot, as the VM form's look-ahead none: every way out lands on an empty slot, the
  thread comes back at every instruction's end, and a limited turn ends at the first end where the count reached the
  limit, with no slice. A jump to a misaligned target ends at that target too, as on the hardware, which takes the
  fault at the target's fetch: where its count reached the limit the turn ends there, and the next turn's load gives
  the fault (the VM executor's single step does the same). That costs a turn per guest instruction, the slow path "turn with a count limit run in
  steps", counted per limited turn. An ordinary executor (linked pieces) ends a limited turn past its count, as 4.6
  says, counted "turn limit passed".

### 3.7 What protects us from the guest

- Control flow: the guest reaches host code only through pieces we translated and their slots; a slot without a
  piece holds `udf` or is unmapped. Nothing read from memory the guest can write is ever used as a host code address,
  but for P's return through a signal frame where the host's frames hold no FP/SIMD registers (gVisor on ARM64, 3.6,
  decision 14).
- I: every guest access is checked against its window or kept inside it by 3.2's stack rules, so no guest load or
  store reaches our area, the kernel, the embedder, other guest processes (of its container or another) or the
  frames of a signal, and every access is also cut to its window or bounded by the unmapped 4 GiB beside it, so no
  speculative one does either (3.2). This is the WebAssembly output's guarantee made natively. What I does not
  give: isolation from the other speculation attacks that cross any in-process boundary (branch target injection
  into our code, and timing channels through shared caches between guests on one core); the host kernel's
  mitigations for those apply between processes, so P keeps them between guest code of different guest processes.
  Neither form keeps them for our own kernel: it runs in the one embedder process in both, holding every
  container's data, and serves every guest's calls with arguments the guest chooses, so a bounds-check bypass
  gadget in its Rust code could read another container's data under P as under I (and P runs the guest on the
  kernel's CPU, 4.3). A workload that needs a line between containers against speculation needs an embedder
  process per container, in either form; that is the embedder's choice (an evaluator input, section 7; named in
  section 9).
- I, signals that are not ours: guest code runs on the embedder's threads with the guest's sp and `TPIDR_EL0`.
  So for the length of each `Runner::run`, the runner thread blocks every asynchronous signal but the stop signal
  with the raw `rt_sigprocmask` (glibc's own SIGCANCEL and SIGSETXID included: an embedder's `set*id` or
  `pthread_cancel` waits until the runner returns, named in section 9), sets its own signal stack, and puts both
  back when it returns. Our fault handlers are installed at the executor's start, remembering the previous ones; a
  fault whose pc is not in our code area or floor goes to the previous handler as it expects (Rust's and wasmtime's
  chain the same way). Each `Runner::run` checks (one `sigaction` read per signal) that ours are still installed and
  otherwise refuses to run, named ("fault handler replaced"). Our handlers' first instructions put the host's
  `TPIDR_EL0` back before any Rust code runs. The stop signal is the first signal from libc's `SIGRTMIN()` on (34
  with glibc 2.39, static and dynamic, measured by the third check) whose disposition is the default at start; the
  kernel's 32 and 33 below it are glibc's own (32 is SIGCANCEL, installed at the first `pthread_cancel`, and
  blocked by I's runners above). That choice is I's; P's stubs hold no C library and the signal is only ever sent to
  them, so P's stop signal is `SIGUSR1` (10), fixed, a standard signal for the reason above.
- P: a guest store can reach anything in its stub that is mapped writable (its register pages, its signal stacks),
  and a load anything mapped readable (its control pages too). Guest code makes no host call (it has no `svc`; the
  only host calls are the stub's own routines, reached through the floor's entries), so what the guest can do to
  its stub is write those pages. The stub is built so that this gives the guest nothing it does not have:
  - Everything the stub obeys is in its thread's **control page**, which the embedder writes through its window
    and the stub maps read-only (`PROT_READ`, shared): the `Resume`, the answer word, the queued mapping commands
    and placement requests (4.2, 4.3). Its filter refuses `mprotect`, `mmap` and `munmap` outside the guest area
    and the slot table (below), so the stub itself cannot make it writable. A guest store to it faults like a store
    to any page it has not mapped (a SIGSEGV for the guest, as on Linux). So no command can be forged: write
    protection that copy-on-write, written marks, code pages and watches rely on changes only by the embedder's
    word. (The x86 plan asks the same of its read-only part, docs/x86-native-output.md section 2.)
  - Everything the stub writes for the embedder is in its thread's **register page** (guest-writable): the request
    word, the exit and the register block. The embedder treats all of it as untrusted, read once and
    bounds-checked like a system call's arguments, and decides only from its own records: an exit that reports a
    fault on a page is resolved against the embedder's marks for that page (a forged copy-on-write or code-page
    exit makes it do only what a real access of that guest would), a reported thread id is used only with the
    stub's own thread group id in `tgkill` (4.1), and a sibling thread that rewrites another's registers changes
    only its own guest process, as a resume through the guest pc's slot (3.3) never jumps to a host address.
  - Its code is one position-independent assembly unit of its own section (no Rust, no libc, no data of the
    embedder), keeping its own state in registers while it waits (the host kernel preserves registers; another
    guest thread cannot change them), and using no stack in guest-writable memory.
  - Its handlers find their thread's pages from the signal stack's address (the host kernel's, unforgeable), copy
    what they need from the frame once, and never return through the frame (3.6).
  - Position resolution, translation and every decision are in the embedder's process; the stub only reports.
  - A stub holds none of the embedder's memory: the zygote it is forked from removed it (4.1).
  Defence in depth: the stub runs under a seccomp filter installed by itself at start (`PR_SET_NO_NEW_PRIVS`, then
  `SECCOMP_SET_MODE_FILTER`, both unprivileged) that allows only its own calls (section 6) with their arguments
  bounded: `mmap` inside the guest area without `PROT_EXEC` and not of the slot table's fd, and of the memory's own
  memfd only at the offset equal to the address (the identity layout; the filter compares the two arguments'
  words); `mmap` inside the slot table only of the slot table's fd, at the offset equal to the address less
  `SLOTS`, read-execute, shared and fixed; `mprotect` only inside the guest area and without `PROT_EXEC`; `munmap`
  only inside those two ranges; `clone` only with 4.1's flags; `recvmsg` (and, with parallel threads, `write`) only
  on its socket; `sched_setaffinity` only on the calling thread (4.3); `close`. Comparing two arguments is plain
  classic BPF (load one word, `tax`, load the other, `jeq x`), and the whole rule was measured to work on both
  machines (W0 probe run 5: the same fd at an offset other than the address, or with only the low words equal,
  refused; at the address, mapped; an `mprotect` to make a read-only shared page writable refused, while without
  the filter it succeeds, because the fd was opened read-write).
  Its fds are its memory's memfd, the slot table's memfd (at fixed numbers, set before
  the filter) and its socket, plus each shared block's memfd it receives, from its receipt until it is mapped,
  when it is closed (so a guest with many shared mappings holds no fd for each).
  What this bounds, if translated code were ever escaped (a translator fault) and the stub ran code of the guest's
  choosing: it reaches no memory of the embedder, the zygote or another stub, and no fd it was not given. Within
  what it maps it can change its own guest area's permissions, which the embedder's word no longer guards then, so
  it can write: its own memory; the pages of its memfd that it lends to its fork children and they have not yet
  copied (they would copy its later bytes, not the fork's); the shared blocks it maps, past the code-page and watch
  protections other processes rely on (their translations of such a page could then run stale). That is a breach
  of the fork's isolation in the defence in depth, never in the primary line; closing it needs lent bytes in a file
  the lender cannot write (a memfd sealed with `F_SEAL_WRITE` per fork, which needs the parent's whole view
  remapped at every fork and keeps overwritten pages until the file closes). Not planned; the owner's (section 12).
- The kernel's side treats every word from a stub as untrusted (read once, bounds-checked), like a system call's
  arguments.

### 3.8 Register groups (brief-native.md, 2026-11-23)

- The start-up probe of which groups the host offers runs on its own short-lived host thread, which then exits.
- P: each guest thread is its own host thread, so the host kernel saves and restores every group lazily at host
  thread switches; the floor saves the whole file at every exit (3.3) only so that the kernel can read it. The
  stub's routines use no FP/SIMD register but to store and load the guest's. The embedder's runner thread (our
  kernel's side) may leave state the guest never has (glibc's memcpy uses SVE on Neoverse V2), and the brief asks
  that it hand back what it dirtied before it blocks. On an ARM64 host, its block is a system call (the futex
  wait), and the host kernel drops SVE state at every system call, keeping only the V registers' 128 bits:
  measured on Graviton4 (W0 probe run 4: p0 and FFR set all true and z0 to a pattern, then `getppid`; after it p0
  and FFR are zero and z0's low 16 bytes kept, without the call all kept; its vector is 16 bytes, so Z has no upper
  bits there). The kernel's documented arm64 ABI promises it (Documentation/arch/arm64/sve.rst, [system call
  behaviour](https://docs.kernel.org/arch/arm64/sve.html#:~:text=become%20zero%20on%20return%20from%20a%20syscall):
  "All other bits of Z0..Z31, and all of P0..P15 and FFR become zero on return from a syscall"). The start-up
  probe still repeats the test on its short-lived thread, as a check of the host against that ABI, and if the state
  survives (a host kernel that breaks it), the runner does what x86-64 does below for its groups, here a `smstart sm; smstop sm`
  pair that zeroes them by the architecture, where the host has SME, and otherwise counts "SVE state kept across
  the block" and warns). ZA, where the host has SME, survives a system call by the kernel's ABI, so a runner on
  such a host runs `smstop za` before it blocks (one instruction; neither rented machine has SME, so untested,
  named). On an x86-64 host, before an embedder
  runner thread blocks it returns the vector state it dirtied to the initial state: `vzeroupper`, and, where the
  host has AVX-512 and `XGETBV` with ECX=1 shows zmm16 to zmm31 or the mask registers in use, `XRSTOR` from an image
  whose header marks those groups as initial (writing zeros does not clear their in-use bit; feature-switch
  research, REPORT.md).
- I: we switch guest threads on a runner, so the floor saves eagerly at every guest thread switch, the incoming
  thread's values or zeros loaded before its first instruction: ARM64 general and FP/SIMD (4 / 15 ns measured,
  feature-switch research). The guest is offered neither SVE nor SME (3.2), so no guest instruction puts state in
  Z's upper bits, P, FFR, ZA or streaming mode (a write of a V register clears Z's upper bits), and the floor saves
  none of them; if they are offered later, the floor saves Z, P and FFR, and ZA and streaming mode where SVCR shows
  them in use. x86-64: `XSAVEC` and `XRSTOR` over the offered groups (52 to 78 ns), which skip and reset unused
  groups themselves.
  No learning of whether a thread used a group. The floor also swaps the thread register the host's runtime uses for
  its own thread-local storage (ARM64 `TPIDR_EL0`, x86-64 FS base) at every entry and exit, since the guest's value
  is its own (ARM64 `msr`/`mrs`; x86-64 `wrfsbase` where the host enables FSGSBASE, else `arch_prctl`, a counted
  slow path).
- Tests (hostile-guest part of each suite): guest threads each fill every offered group with distinct values,
  interleaved in every order of the bounded search, each reads back exactly its own and an unused group reads zero;
  in a test build, the floor asserts at the moment a guest thread is about to run that the registers hold its values
  or zero. Built for P on ARM64 (W1): the interleaving in a seeded random order (tests/floor.rs, tests/run.rs), not
  every order; and instead of an assertion in the floor, the test reads every register from outside (ptrace) at the
  instruction where a thread's code is entered, for a thread with random values, then none, then other values
  (tests/step.rs); the thread register has no register set there and is checked by read-back only.

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
- I's slots are 2^36 bytes from 2^32, each holding a 2^32 window (I's ceiling 2^32 in the first build) with its
  guards, larger than ARM64's because an x86 displacement reaches 2 GiB, so 255 slots per embedder process, shared
  as in 3.1; P's ceiling is 2^44 (its 6); our area lies between 2^44 and the host's own mappings near 2^47, placed
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
- The **zygote**: forked from the embedder's process when the host-process host is made (8.5). In the
  child only direct host calls run (no allocation, no lock: the embedder may have other threads). It maps the stub's
  code (copied by the executor before the fork into a memfd) read-execute at a fixed address in our area and a stack
  beside it (`mmap`, then the memfd is closed) and jumps there, every signal blocked since before the fork (the
  forking thread blocks them around it, so no handler of the embedder ever runs in the child), and drops what the fork carried of
  the forking thread's registrations that point into the embedder's memory. The one that kills: the forking
  thread's restartable-sequences area (glibc registers one per thread), which the host kernel writes at every
  migration, preemption or signal; once its page is unmapped that write fails and the host kernel kills the
  process with SIGSEGV (measured by the fourth check on both rented machines: unmapping kills the child, unregistering
  first or keeping the page does not). So the zygote calls `rseq` with `RSEQ_FLAG_UNREGISTER` and the area, length
  and signature the embedder's C library registered (glibc's `__rseq_offset` from the thread pointer and
  `__rseq_size`, read before the fork; none where the C library registers none), then proves that none is left
  by registering an area of its own stack (a registration left over refuses it with EBUSY: the zygote then exits,
  and the form is unavailable, named "restartable sequences left registered") and unregistering that again; a host
  that answers that registration ENOSYS (gVisor) has no restartable sequences, so none can be left. The
  robust-futex list and the exit-clear word are read and written by the host kernel only at a thread's end, with
  failures ignored; they are cleared all the same (`set_robust_list` with none, `set_tid_address` with none). The
  alternate signal stack goes with the memory, harmless while every signal is blocked; each stub sets its own.
  Then the zygote closes every fd but its socket (`close_range`), unmaps every range outside its own (two `munmap`
  calls, below and above its own pages up to the user-space top the start-up probe found, so an in-process host's
  reservation and table and the window cache go too: no /proc needed), and waits for requests on its socket. So no
  stub holds any of the embedder's memory, and every stub, forked from the zygote, inherits no registration. The
  embedder's death closes the socket and the zygote exits.
- The zygote's own end (the host's out-of-memory killer, or any fault of its own) takes every stub with it
  (`PR_SET_PDEATHSIG`), so every guest process of every P container of that embedder process ends as killed by
  SIGKILL, as the host's out-of-memory killer would end them, counted "zygote ended", warned. The embedder learns it
  from the zygote's socket hanging up, reaps it, and forks a new zygote on the next `memory_new` by the same path as
  at start, which is safe from the by-then many-threaded embedder because only direct host calls run in the child;
  its cost is a fork of the then-larger embedder's page tables, measured in W1. If that fork fails, P is
  unavailable from then on, named.
- `memory_new` and `memory_duplicate` ask the zygote for a child (measured, W0 probe: a request until the new child
  runs, 80 µs Graviton4, 95 µs Xeon; a plain fork of a small static process 111 / 121 µs). Each request carries
  (`SCM_RIGHTS`) the fds the child needs, its own socket's end among them, which the zygote closes after the fork.
  The child sets `PR_SET_PDEATHSIG` to SIGKILL (and checks that its parent is still the zygote), maps its memory's
  memfd, its thread pages and signal stacks (4.3, then closes
  that fd), the slot table's fd at its fixed number, installs its
  handlers and signal stacks, unblocks the stop and fault signals, installs its filter, and becomes the memory's
  stub. Its start is thread 0's first turn: the embedder writes the filter into that thread's control page before
  the fork and waits for the turn's `Done`, so no turn reaches a stub without its filter, and a stub that ends
  instead (its filter refused, in a sandbox that withholds seccomp) is reaped and P is unavailable, named by the
  stub's exit status. Exec is `memory_new` plus `memory_drop`: a fresh stub, as
  Linux gives a fresh address space.
- `thread_new` on a memory: the stub's thread that is in a kernel entry for that memory (the `clone` caller) makes
  the host thread with `clone` before it resumes, a command of its control page (3.6: applied once, with the stop
  held off); the first thread of a new memory is the stub's first thread. The stub's process id comes from the
  zygote (its `fork` result, trusted); a thread's host id is known only inside the stub (`clone`'s result), which
  writes it to the new thread's register page, so it is untrusted and is used only in `tgkill(stub's process id,
  thread id, stop)`: a forged id there can reach only another thread of the same stub, which takes a stale stop
  (3.6). Placement never uses it (4.3). The command names the new thread's index, which the embedder chose free;
  `clone` gets `CLONE_VM | CLONE_FS | CLONE_FILES | CLONE_SIGHAND | CLONE_THREAD | CLONE_SYSVSEM |
  CLONE_PARENT_SETTID | CLONE_CHILD_CLEARTID`, with the exit-clear word of the new thread's register page as the word
  to set before the call returns (so the index is never free between the command and the thread's start) and to clear (the
  stub's first thread, index 0, names its own with `set_tid_address` before its filter), and the top of the new
  index's signal stack as its stack, so the new thread's sp is never the maker's guest sp (a signal frame for a fault
  of ours before its `sigaltstack` lands in our area, not in guest memory). The new
  host thread starts with the signal mask its maker had while applying commands (the stop blocked) and with no
  alternate signal stack (Linux clears it for a thread that shares the memory), so its first acts are
  `sigaltstack` onto its own stack, writing its id, unblocking the stop, and the wait on its answer word; a stop
  sent before that is pending and lands at the unblock or in the wait, both of which the handler treats as the
  wait window of 3.6 (it goes to the wait).
- A guest thread ends by a command of its control page that makes it call `exit` (`thread_drop`; one not in a
  kernel entry is stopped first). The host kernel then zeroes its exit-clear word and wakes it, after the thread
  has left every page of its index, and only then does the embedder give that index to a new thread (it reads
  the word first; a forged zero only lets a live thread of the same stub share its pages with a new one of that
  same stub). `memory_drop` ends every thread so, the last with `exit_group`.
- The zygote reaps a child only when the embedder asks, after it has learned the child ended (its socket hung up)
  and has stopped using its process id, so `tgkill` and the CPU clock of 4.3 can never reach a process that got the
  id later. A stub that has not ended 100 ms after the command that ends it, or 10 s after an ignored stop (3.6), is
  killed by the zygote (`kill` with SIGKILL, safe for the same reason: it has not reaped the child). Only a
  compromised stub needs either.
- A stub has no thread of its own for commands: every change to a memory's guest view is a command the kernel's
  side queues and the next thread of that memory to resume applies before running guest code (4.2). Each
  `shared_map` command's memfd reaches the stub over its socket (`SCM_RIGHTS`), received by the thread that applies
  the command and closed once mapped.
- One embedder process serves every stub of every container it runs, through one **host-process host** (one
  zygote, one window cache, 4.2) that the embedder makes once and makes each container's P executor from (8.5).
- A stub's death is learned one way: its socket hangs up (the embedder holds the other end). A runner waits on a
  thread's futex with a time-out of at most 100 ms and, on each time-out, polls that stub's socket (`ppoll`, zero
  wait); any other use of the socket sees the hang-up too. The executor then ends the process as killed by SIGKILL,
  counted ("stub ended outside the kernel"), and tells the zygote to reap it (`wait4` on that process id), so an
  ended stub stays a zombie at most until the embedder learns of its end, within 100 ms.

### 4.2 Memory

- Layout and frames as 3.1 and 3.5: the guest's own addresses, the memfd per memory.
- **The embedder's views are windows.** Every view the embedder's process has of a stub's memfds (guest memory, the
  slot table, the code area; shared blocks included), except the thread-pages memfd (4.3), is a window of 2 MiB at a 2 MiB-aligned offset of one memfd,
  mapped shared read-write at the kernel's side's first touch into one reservation of the host-process host, and
  found through a table keyed by (memfd, window). The windows are a cache: beyond 16,384 mapped windows (32 GiB of
  views, against vm.max_map_count's default of 65,530 host mappings) the least recently used one, as the clock algorithm approximates it (one store per touch), is unmapped
  (counted "view window evicted"). Eviction happens only inside an executor call, which holds `&mut self`, so no
  slice handed out earlier is still borrowed. Measured (W0 probe run 3): mapping a window, touching one page and
  unmapping it, 8.5 µs on Graviton4 and 4.8 to 5.2 µs on the Xeon; mapping and unmapping alone 3.2 / 2.0 µs. So neither
  the number of stubs nor the size of their reservations is bounded by the embedder's own address space, which
  holds only three whole 2^46-byte slot-table views (measured by the second check).
- Mapping commands (`set_permissions`, `shared_map`, `shared_unmap`, a slot-table page, a protection for code or
  marks) change only the guest view, which no one but the memory's own threads can observe. They are queued per
  memory, written into the control page (3.7) of the thread that resumes next, and applied by it in the order
  made (with the stop held off, 3.6), before it runs guest code, so a system call that
  maps or protects costs no extra round trip, and the effect is in place when the call returns, as on Linux. A
  command on a memory none of whose threads is in a kernel entry rides on the resume of one of its threads, which the
  kernel's side first stops (3.6). The kernel's side counts the host mappings each command creates, so
  `CpuError::OutOfMemory` is returned up front instead of a host failure: it keeps each stub's guest-area mappings as
  the queue leaves them (neighbours of equal permission and backing, the memory's own memfd or one block at
  contiguous offsets, are one, as the host merges them), adds the stub's five own mappings, and refuses a change of c
  commands unless the count plus 2c stays within the host's `vm.max_map_count` (read once at start; Linux's default
  of 65,530, warned, where it cannot be read), since a host call splits at most two mappings and the host refuses a
  split at its limit. Counted "host mappings exhausted"; tested against /proc's count after every applied queue of the
  seeded model and up to a million mappings. A control page holds the commands that
  fit beside the `Resume` and its words; a longer queue (a fork's protection of many mappings, a large unmapping)
  goes in rounds: the stub applies a page of commands and exits with "applied" instead of running guest code, and
  the runner writes the next page, one round trip per page, the `Resume` only with the last (counted "mapping
  commands past one page").
- Two things are applied at once, by a command round trip to the thread in the kernel entry (counted "mapping
  applied at once"), not left in the queue: the write protection a fork makes (3.5), and any command that takes a
  view's access away from a range whose storage is about to change (an unmapping, `mmap` with `MAP_FIXED` over a
  mapping, before the kernel's side writes or punches it). Otherwise a sibling thread could read the new storage
  through the old permission, or write a page while it is copied, which no order of the two calls gives on Linux.
- The queue's order against a command applied at once. With one guest thread running at a time (W1 and W2, 3.9),
  the thread in the kernel entry is the only thread of its memory out of its wait, so it applies the whole queue,
  in order, before the command applied at once, and no older command (a write given back, say) can land after a
  fork's protection. With several threads of a memory running at once (when the kernel allows it), another one may
  be applying an older batch at that moment, so the command applied at once waits for a witness from every thread
  of the memory that took a batch before it: such a thread ends its batch with a one-byte `write` to its socket, a
  host call guest code cannot make, which the embedder reads before it sends the command. Built with parallel
  threads, not in W1 or W2; until then a batch needs no witness.
- Dropping translations while other threads of a memory run (code changed, a range unmapped) stops those threads
  first, as the VM form's group stop does (docs/threads.md sections 7.5 and 10); slot table and code area are memfds
  the kernel's side writes directly.

### 4.3 The system call and its return

- Each guest thread has two pages of its stub's **thread-pages memfd**: the **control page**, read-only in the
  stub (3.7), holding the answer word, the `Resume`, the queued commands and a placement request; and the
  **register page**, read-write, holding the request word, the exit, the register block and the thread's
  exit-clear word (4.1). The pages of thread index t (0 to `THREADS_PER_PROCESS` - 1, with `THREADS_PER_PROCESS` =
  2^15) sit at offsets t·4 KiB (control) and 2^27 + t·4 KiB (register) of that memfd. Both ranges are mapped whole
  when the stub starts, before its filter (control `PROT_READ`, register read-write, shared, at fixed addresses in
  our area), with its thread's signal stack (16 KiB at a fixed place per index, anonymous, `MAP_NORESERVE`), and
  the fd is then closed: a new thread needs no mapping call, which the filter would refuse in our area, and untouched
  pages cost nothing. The embedder maps each stub's thread-pages memfd whole too (256 MiB of address space, one host
  mapping per stub), outside the window cache of 4.2, so a page a runner waits on is never evicted under it. A
  `thread_new` that finds no free index answers `CpuError::OutOfMemory`, which the kernel gives the guest's `clone`
  as it gives any other failure to make a thread (counted "guest threads per process exhausted", warned). The runner (`Runner::run` on an embedder thread) writes the control page, wakes the
  answer word's futex, and waits on the request word with a time-out of the slice or 100 ms, whichever is sooner
  (4.1); the guest thread's exit writes the `Exit` and wakes it back. A futex wait needs only read access to its
  word, so the stub waits on a word it cannot write (W0 probe run 5: 2.36 to 2.40 µs back to back against 2.33 to
  2.36 µs with the page writable on Graviton4, 1.93 to 1.98 against 1.88 to 1.94 µs on the Xeon, the read-only
  run always first; with guest work between calls no difference). Every wait passes the value it compared as the futex's
  expected value, so a wake between the compare and the wait is never lost (run 2's probe re-read the word there
  and hung). The request word is the guest's to forge (a sibling thread can write it): a forged value only ends
  that thread's own turn early with an exit the embedder validates (3.7).
- **The wait is the plain futex, no spin and no yield.** Spinning on one CPU only adds its bound (cloud arm and
  intel rows), and across CPUs it burns a core per waiting side. Yielding for a bounded time before the futex sleep
  is the cheapest transition measured on one idle CPU (costs-cloud.json's intel row, 1.21 µs against the futex's
  1.72; the third check: 1.88 to 1.89 against 2.35 to 2.37 µs on Graviton4, 1.49 to 1.50 against 1.91 to 1.93 µs
  on the Xeon, 20 to 22% less), but on a CPU that another program also uses each yield hands that program its
  slice, about 3.0 ms per round trip on both machines (third check). Not taken: that loss lasts until placement
  notices the program (up to a sampling window, below), so one such window costs what about 6,400 yields save
  (3 ms against 0.47 µs each); a yield only on CPUs found free is a second wait beside the futex for a gain that
  W1's measurements may show worth revisiting.
- **Placement**: a runner and the guest thread it drives share one CPU, which the runner chooses so that no other
  busy program shares it if any CPU of the set is free of one. The runner sleeps while the thread runs, so the two
  never compete, and each transition is a direct switch on that CPU. The rule, all of it:
  1. The host-process host (one per embedder process, 4.1) keeps a table over the process's allowed set
     (`sched_getaffinity` at its start, read again when a pin is refused, so a cpuset the embedder or its container
     gives is honoured): for each CPU, how many runners hold it and when it was last found contended. At its first
     `run`, a runner takes the CPU held by the fewest runners that is not marked contended in the last second
     (lowest number on a tie; if every CPU is marked, the fewest held), pins itself there and holds it until it is
     dropped. So the runners of all containers spread over the set, never all on CPU 0.
  2. A guest thread is moved by its own stub: when a turn brings it to a runner on another CPU than its last, the
     control page carries the CPU and the stub calls `sched_setaffinity` on itself before resuming (its filter
     allows that call only for the calling thread). The embedder never pins a stub thread by an id the stub
     reported, so a forged id cannot make it pin another program's thread.
  3. Every 10 ms of the runner's busy time (time inside `run` not spent waiting for the kernel to have work), the
     runner reads the CPU time of its own thread (`CLOCK_THREAD_CPUTIME_ID`) and of each stub process it gave a turn
     to in that time (`clock_getcpuclockid` of the stub's process id, which the zygote reported, readable without
     any privilege), a few calls per 10 ms. Their sum over the busy time is the CPU's share: 1.000 measured on a
     free CPU, 0.50 to 0.56 on a CPU that one spinning program also uses (W0 probe run 4). Under 0.75 the runner
     marks its CPU contended and moves to the CPU that step 1 would give now; if that is its own, it stays.
  As built (W1 run 4): the runner is the thread that calls the executor, pinned from its first turn until the
  executor ends; a guest thread's move is a `Place` command of its own turn (`sched_setaffinity` of the calling
  thread with a 1,024-bit set, the only form the filter allows), applied with the other commands. As built (W1
  round 5 run 1): a stub's clock is read just before the stub ends and its time kept for the window (before, a
  window holding a stub that ended decided nothing, so a runner of short-lived processes never moved); a runner
  moves only to a CPU not found contended in the last second and at most once a second (step 1's fallback to the
  CPU held by the fewest is for a runner's first CPU alone; with it a runner moved every window, about 50 times a
  second, when every CPU of its set was contended; now 10 moves in 10 s, W1 round 5 run 1).
  That is the whole rule. With several guest threads of one stub running on different runners at once (not before
  the kernel allows it, 3.9), a stub's process clock mixes them and overstates a share, so a contended CPU can go
  unnoticed (never a needless move); that case is measured again when parallel threads are built. If the CPU
  clocks are refused (a sandbox), the runners keep step 1 only, counted ("placement without samples") and warned.
  Measured (W0 probe run 4, guest work of 10.8 µs on Graviton4 and 17.4 to 18.1 µs on the Xeon between calls,
  section 11): on a CPU shared with a spinning program the rule moves once (21 to 22 µs / 118 to 137 µs for the
  move) and then runs at 13.37 to 13.39 µs per call on Graviton4, the free CPU's own 13.38 to 13.41 µs (18.1 to
  18.3 µs on the Xeon, against 21.0 to 21.9 µs on a free CPU in the same runs, whose clock speed varies); left
  pinned there it costs 27.76 to 27.83 µs (36.7 to 37.1 µs). With every CPU of the set contended it moves twice in
  a run of about 1.1 s (about 1 ms per move, waiting for the busy CPU)
  and costs 1.4 to 1.8% more than staying (Graviton4).
  The reason for one CPU rather than two or the scheduler's own choice is measured too (W0 probe run 3), with the
  lost-wakeup fault of run 2's probe fixed:
  - idle CPUs: one CPU 2.41 to 2.46 µs on Graviton4 and 1.98 to 1.99 µs on the Xeon, against 4.6 to 4.8 µs and 4.2
    to 4.5 µs on two CPUs and 4.5 to 4.9 µs and 4.4 to 4.9 µs unpinned (no stalls: 1 of 504,000 unpinned round trips
    over 1 ms on Graviton4, none on the Xeon). One CPU halves the round trip;
  - CPUs busy with another program (a spinning process of normal priority on each CPU used): one CPU still gives
    a median of 3.6 to 3.9 µs and a mean of 6.0 to 7.0 µs on Graviton4 (2.2 to 3.1 / 4.7 to 5.7 µs on the Xeon),
    about one round trip in a thousand waiting one time slice (3.3 to 4.0 ms); two CPUs wait for the other program's
    time slice on almost every round trip, about 2 ms each on both machines, and so does the scheduler's own
    placement on four busy CPUs of Graviton4 (1.0 to 2.0 ms median; 2.3 to 2.4 µs on the Xeon, where it put both
    on one CPU).
  So one CPU it is, free of other programs where the set has such a CPU. Where it has none, P still runs on one
  CPU, and pays per round trip 6.1 to 6.2 µs on Graviton4 (4.3 to 5.1 µs on the Xeon) more than the same guest
  work alone on that CPU, which is what I's guest work pays there (W0 probe run 4); section 7 uses that.
- Measured round trips for this exact shape (futex slot, ppid-like request), bare metal (W0 probe run 2, no timing
  per round trip): Graviton4 one CPU 2.29 to 2.37 µs, two CPUs 4.58 to 4.83 µs; Xeon 8488C one CPU 1.89 to 1.96 µs,
  two CPUs 4.02 to 4.16 µs; run 3's figures above include two clock reads per round trip. Cloud rows (median, one
  CPU / two CPUs): Graviton4 c8g 2.20 / 5.58 µs, Granite Rapids 1.72 / 4.18 µs, Zen 4 5.14 / 8.46 µs. The stub's turn as built (W1 run 2: a turn with no commands, the embedder's thread and the stub
  pinned to one CPU, alternating with W0's slot in the same run, three rounds of 21 batches of 10,000): 2.70 to 2.72
  µs against the slot's 2.35 to 2.37 µs on Graviton4 (load 7 to 12), 2.24 to 2.25 µs against 2.02 to 2.04 µs on the
  Xeon (load 0.2 to 0.3), 1.10 to 1.15 times the slot.
- Rejected: seccomp user notification on an untranslated `svc` (as fast on one CPU and 0.4 µs cheaper across CPUs,
  but it gives only the number and six arguments, so exits that need the whole register file would need a second
  path; and it is not offered by every sandbox); pipes, sockets and ptrace (all dearer, measured).
- Everything the guest reads that the kernel answers is a round trip in P, the time counter included (our vDSO has
  only `__kernel_rt_sigreturn`, so `clock_gettime` is a system call): this is what the evaluator counts as kernel
  entries.

### 4.4 Fork, exec, shared mappings, signals

- Fork while another thread of the parent is in a system call, and a third runs: `memory_duplicate` is made during
  the forking thread's kernel entry and applies the parent's write protection at once (3.5, 4.2), after every older
  queued command (4.2's order), before it returns;
  the thread in the other system call waits in its stub and is unaffected (its call completes in the kernel, whose
  `Cpu` calls on the parent's memory come before or after the duplicate as the kernel orders them); the running
  thread's writes land before the protection's instant or fault after it. The child holds only the forking thread,
  as on Linux, whose host thread is the child stub's first thread. A parent that forks twice before a page is copied,
  and a child that forks before copying, are 3.5's rules (one owner per page, one copy per event).
- A shared mapping written by two guest processes: one memfd mapped in both stubs (3.5).
- Guest signals are the kernel's: it writes the frame and resumes the thread `FromRegisters`; nothing host-level.
- A host signal arriving inside the transition: 3.6 (the entry finishes from the frame; a stop during the queued
  commands waits until they are all applied; the resume window leaves as a stop; the wait restarts).
- A host signal in the stub's own code that 3.6 does not name is a fault of ours: the stub reports it if it can and
  ends; the process ends as killed, counted ("stub fault").

### 4.5 What is saved at a guest thread switch, and by whom

The host kernel, lazily, at every switch of host threads (3.8). At every exit the floor saves the whole register
file to the register page's block (3.3); `Cpu::registers` and `set_registers` read and write the block, never the
thread; a thread resumed `FromRegisters` loads everything from it, and one resumed with `Return` or `Continue`
loads what the exit stored. A stub thread never uses SVE (the guest has none, 3.2, and the stub's routines use
none), so the host's lazy SVE handling never adds to its wakes.

### 4.6 How the kernel's runners drive it

As built (W1 round 3 run 2): the kernel's loop (`wcontainers_runtime_single_kernel::drive`) runs P through the
runtime's `Execution` trait, as it runs the WebAssembly output and the VM form's inside; that is the one module
that implements an executor trait. With linked pieces a counted turn cannot stop at its exact count (no stepping
without ptrace): it ends at the first exit at or after the count, or when a slice of one nanosecond per unit of its
limit sends the stop, counted "turn limit passed". The exact executor of 3.6 ends it exactly.

`Kernel::new(config, parallelism)` gives N runners; the embedder runs each on a thread of its own with an executor
handle of its own (section 8). A runner's `run` asks the kernel for the next thread under the kernel's lock, lets the
lock go, hands the turn to that thread's host thread through its control and register pages,
blocks until it comes back, takes the lock
again and gives the kernel the `Exit`, and repeats. With today's kernel one thread runs at a time (3.9).
Today `KernelRunner::run` is `unimplemented!` (crates/kernel/implementation/linux/executing.rs: no executor call
runs a thread; section 8 item 1 asks for one). Until the owner rules, W1 and W2 drive the kernel the way the Unicorn
executor does (crates/executor/implementation/unicorn/driver.rs): the started kernel's `with_parts` gives the kernel
and the executor under the kernel's lock, and one loop calls `kernel.next()`, runs the turn through the executor's
own public `run_thread` (the item-1 call, on the crate's type until the trait has it) and gives the result to
`kernel.exit`. That holds the lock for the turn, which is what one thread at a time means anyway; the loop above
replaces it when item 1 exists.

## 5. Form I: the kernel in process

### 5.1 Processes and threads

- Every guest memory is a slot of the embedder process's one pool (3.1), whichever I container holds it; every
  guest thread is a register block. Runners are the embedder's threads, each running whichever guest thread the
  kernel gives it: the kernel's
  own model (a call that cannot finish parks its thread and returns), with no host thread per guest thread.
- Fork, exec and thread creation make no host process or thread: a slot and a memfd, or a block.

### 5.2 Memory

- The guest's view is its memfd mapped into its slot's window with the guest's permissions; host page tables
  enforce the guest's permissions inside the window, the check of 3.2 keeps every access inside it. A fault inside
  the window is a host SIGSEGV whose address less the slot base is the guest address.
- Mapping commands are made at once in the same process (no queue). Copy-on-write and code pages are 3.5 and
  3.4 (written marks and watches are the kernel's, 3.5), with the faults handled in process (1.31 µs measured for the handler's round trip, plus the copy).
- Exact against wrapping checks: built exact (3.2 item 1): every access Linux faults on faults here at the same
  address with the same registers. The wrapping form (the same access cut to the window, `[base, wA, uxtw]`,
  without the check before it: R32) measured cheaper (+9.2% against +19.4% on Graviton4, +1.8 to +4.3% against
  +11.0 to +15.2% on Granite Rapids, both with the stack exempt) but sends an access outside the window to the
  process's own memory at that address modulo 2^32, where Linux faults: a behaviour difference no counter can see.
  Both are safe against speculation by the same cut. It is the owner's to allow (section 12).

### 5.3 The system call and its return

A plain call: `bl entry` (3.2 item 3) saves the whole register file (16 ns measured for a save and load with q0 to
q31 on Graviton4; section 1), switches back to the runner's own host stack, whose pointer the resume saved in the
runner's block (the embedder thread's stack, never guest memory: the guest's stack is guest memory), swaps the
thread register, and returns `Exit::Syscall` to the runner's loop as a function
return; `Resume::Return` is a return into the call site with x0 set and the file loaded. No host system call is made
on the path.

### 5.4 Fork, exec, shared mappings, signals

As 4.4 with every step in one process: the forking runner protects the parent's view itself before returning (3.5);
a racing sibling on another runner writes before that instant or faults after it. A shared mapping is one memfd
mapped into both windows. A host signal inside the transition: 3.6. A stop signal arriving while the runner is in the
kernel or the embedder's code returns at once (3.6) after marking the runner's current turn to end at its next
entry into guest code, which the resume sequence reads from the runner's own block (out of the guest's reach).

### 5.5 What is saved, by whom; the runners

The floor, eagerly, at every guest thread switch (3.8). The runners are `Runner::run` on the embedder's threads,
each calling its executor handle's run-a-thread call in a loop, as 4.6 (section 8), and until item 1 of section 8
exists the `with_parts` loop of 4.6.

## 6. What each form needs from the host

Both (embedder's process): `memfd_create` (or 3.4's `O_TMPFILE` files through `openat`), `ftruncate`, `fallocate`
with `FALLOC_FL_PUNCH_HOLE`, `mmap` (fixed addresses, `MAP_FIXED_NOREPLACE`, `MAP_NORESERVE`, shared file mappings,
read-execute), `munmap`, `mprotect`, `futex` (wait with time-out, wake; shared futexes on memfd mappings),
`tgkill`, `rt_sigaction`, `rt_sigprocmask` (at each `Runner::run` start and end, and once per handled signal that
does not return), `sigaltstack`, `clone` for threads, `fstatfs` (3.4's fallback only); on x86-64 `membarrier`
(optional, 3.10).
- P adds: in the embedder, `fork` (or `clone` without namespace flags) for the zygote, `socketpair`, `sendmsg`,
  `ppoll` (a stub's socket, 4.1), for placement (4.3) `sched_getaffinity`, `sched_setaffinity` on its own runner
  threads and `clock_gettime` of its own thread's CPU clock and of a stub's process CPU clock, `read` of a stub's socket
  (parallel threads only, 4.2), `wait4` of the zygote after its end; in the zygote, `mmap` (its code and stack),
  `close`, `getpid`, `rseq` (unregister the inherited area, then register and unregister its own, 4.1),
  `set_robust_list`, `set_tid_address`, `close_range`, `munmap`, `recvmsg`, `sendmsg`, `fork`, `wait4` (on a process
  id the embedder names), `kill` (SIGKILL to its own unreaped child, 4.1), `exit_group`; in each stub, `prctl` (`PR_SET_PDEATHSIG`, `PR_SET_NO_NEW_PRIVS`),
  `getppid`, `set_tid_address`, `seccomp(SECCOMP_SET_MODE_FILTER)`, `dup3` (fds to their fixed numbers, before the
  filter), `rt_sigaction` (the stop's handler, and the fault handler for SIGSEGV, SIGBUS, SIGILL and SIGTRAP), with
  the mappings of 4.1 and 4.3. The stub's
  own filter then allows: `futex`; `mmap`, `munmap`, `mprotect` bounded as 3.7 (`mmap` of the memory's memfd at
  the offset equal to the address, or of a shared block's memfd, which is the lowest descriptor, 0, at any offset,
  and of the code area's memfd, received the same way, `PROT_EXEC` alone over exactly the code area from offset 0);
  `clone` with 4.1's thread flags; `recvmsg` on its socket with `MSG_DONTWAIT` (a block's memfd, sent before the
  turn that maps it, so it never waits) (and `write` on it with parallel threads); `close` of descriptor 0 (the
  block's, once mapped); `sched_setaffinity` on the calling
  thread; `rt_sigprocmask`; `sigaltstack`; `exit`; `exit_group`; on ARM64 `rt_sigreturn`, used only where a signal
  frame holds no FP/SIMD registers (gVisor, 3.6); on x86-64 `membarrier`, and
  `arch_prctl` (`ARCH_SET_GS` only) where the host lacks FSGSBASE (3.10).
- I adds: `timer_create`, `timer_settime` and `timer_delete` for slices, `rt_sigreturn`; `arch_prctl` on x86-64
  hosts without FSGSBASE. No process is ever made, so I runs where `fork` is refused.
- Neither needs: ptrace, user namespaces, mount, chroot, capabilities, `process_vm_readv/writev`, `userfaultfd`,
  `perf_event_open`, /proc (only /proc/sys/vm/max_map_count is read, once, 4.2), `execve`. Docker's default seccomp profile allows every call
  listed (it withholds only namespace flags of `clone`, which are not used; it allows `process_vm_readv` too, which
  is simply not needed); W1 and W2 run their suites inside a container started with
  Docker's default profile, and under gVisor's runsc (`runsc do`, systrap platform). Found under gVisor (release
  20261109.0, W1 run 5): `rseq` answers ENOSYS (the zygote then has no registration to drop, 4.1); `lseek`'s
  `SEEK_DATA` on a memfd answers EINVAL (a fork then lends every private page of the guest area, a slow path counted
  "memory extents unknown"); `fallocate`'s hole punch answers EOPNOTSUPP (zeros are then written, 64 KiB at a time,
  counted "hole punch refused"); on x86-64 gVisor maps its own `[usertrap]` page at 0x63000, inside the guest area, which it refuses to
  unmap, so the zygote reports `UnmapRefused` and P is unavailable unless runsc runs with
  `--systrap-disable-syscall-patching` (which x86's use of the GS base needs too, 3.10); the systrap platform kills a
  process past about 4,096 threads (measured: SIGKILL at the 4,097th), so under gVisor a guest process holds at most
  that many threads; gVisor accepts `sched_setaffinity` but does not pin, so placement (4.3) has no effect there.
  On ARM64 (W1 run 10, scratch w1/run10): a signal frame holds no FP/SIMD record and the handler starts with those
  registers zeroed, so P returns through the frame for them (3.6, decision 14); an execute-only mapping can be read
  (not written), so translated code can read its code area there, which Linux refuses
  (`translated_code_can_neither_read_nor_write_the_code_area` fails under gVisor for that read). On x86-64 its frames
  hold the FP state as Linux's do (`fpregs` set, the interrupted xmm0 there; scratch w1/run10/gframe-x86.c), so
  x86-64's floor (3.10) needs no return route.
- P makes one host process per guest process, so a host limit on processes (a pids cgroup, `--pids-limit`) is also
  the guest's; I makes none.
- Host pages larger than 4 KiB (16 KiB on Apple silicon, 64 KiB on ARM64 Linux kernels built so, such as RHEL's
  kernel-64k). The owner's ruling of 2026-11-18 (brief-native.md): "A back end gives the guest 4 KiB pages on
  whatever the host has; underneath it may use larger pages wherever a whole range has the same permissions". Both
  forms follow it, so a guest always sees `wcontainers_abi::PAGE_SIZE`:
  - a host page whose guest pages all have the same permission and the same backing (private memory is one memfd
    at identity offsets, so neighbours share it) is mapped as one, with that permission;
  - a **split** host page (its guest pages differ in permission or in marks, or map different files, as a shared
    block at an address that is not a multiple of the host page, or a mapping beside a hole) is mapped with the
    intersection of its guest pages' permissions, or not at all where their files differ. A host fault there goes
    to the floor, which decides by the faulting guest page's own permission and marks: refused, it is the guest's
    fault exactly as on Linux; allowed, the floor performs that one instruction against the kernel's view (I: its
    second mapping; P: a round trip and the embedder's window) through the translator's single-instruction path
    (the one `execute_single_instruction` uses, so its semantics are the reference's) and resumes after it. Counted
    "access in a split host page", warned;
  - copy-on-write units, written marks, code pages and watches cover at least a host page, each fault on a
    neighbour resolved by its own guest page as above; I's guards and `LOWEST_ADDRESS` are already 64 KiB.
  W1 and W2 refuse to start on such a host (the start-up probe reads the page size; the form is then unavailable,
  named "host page larger than 4 KiB, not yet built"); W4 builds it (section 13).
- macOS: the VM form (Hypervisor.framework) works on every Mac with no entitlement beyond an ad hoc one, so P and I
  are not planned there; on Apple silicon they would also need W4's split host pages (16 KiB).
  Intel Macs would need `os_sync_wait_on_address` for the futex (macOS 14.4 on; measured, confinement
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
  with a fixed piece of work between them on the CPU 4.3's step 1 gives, less the same work alone, the mean, not
  only the median) and that CPU's share (4.3's step 3); whether any CPU of the set was free; I's check cost, from a
  fixed translated loop run in both forms (or the per-CPU figure of section 11).
- The container's configuration: its memory limit, and the embedder's policy on speculation attacks (3.7): a line
  between guest processes of one container for guest code is P's alone; a line between containers needs an
  embedder process per container in either form, because our kernel serves every container of its process in both.
- What earlier runs of the same workload recorded, as the embedder defines "the same" (image digest and command):
  kernel entries per second of guest CPU time, processes started per second, pages copied after forks per second,
  peak live processes (I holds 2047 per embedder process, 3.1), peak address space per process (I holds 4 GiB;
  runtimes that reserve more than they use, such as V8's sandbox reservation or a Java heap's, exceed it whatever
  their use, which is one more reason the default without history is P).
  Our kernel does not report kernel entries yet (section 8).

The crossover. Per second of guest CPU time, P adds k·c (k kernel entries per second, c the round trip less I's
call) and I adds f (its checks' share of the time). I beats P when k > f / c. f is the sound form's (section 11:
X32-e plus the stack pointer's guard; its access is already the speculation-safe one, 3.2), and it still leaves out
I's other costs: the extra instruction per jump through a register, the eager register switch at guest thread
switches, the one more instruction of the base-only access forms (exclusives,
acquire and release, atomics, structures: 3.2 item 1), and 3.2's rule (f) for sp-based register offsets, which the
harness left unchecked (5 such instructions in 737,566 of python3.12, counted by the second check); W2 measures
them.
When no CPU of the set is free of other programs, both forms' guest work runs at the CPU's share (it takes s times
its free time, s about 2 with one other program: 21.60 to 21.65 against 10.78 to 10.79 µs on Graviton4, W0 probe
run 4), I's check with it, while P's round trip there costs c_busy more than the work alone. Then I beats P when
k > s·f / c_busy.
With P's placement and the measured f:
- Graviton4: f = 0.214 (X32-p, section 11; 0.211 when run 5 re-measured it, which moves no figure below by more
  than 1.5%). On a free CPU, c = 2.29 to 2.37 µs back to back (run 2) -> 90,000 to
  94,000 entries per second; with 10.8 µs of work between calls c = 2.60 to 2.63 µs (run 4) and 2.50 to 2.53 µs
  (run 5, the stub's answer word on its read-only control page) -> 81,000 to 86,000.
  With every CPU contended, s = 2.00 and c_busy = 6.11 to 6.23 µs (run 4) -> 69,000 to 70,000.
- Xeon 8488C: f is not measured on this CPU; with Granite Rapids's f with the stack exempt, 0.110 to 0.152 by
  layout, plus the stack guard's share measured on Graviton4 (none within the pairs' spread). Free CPU, c = 1.89 to
  1.96 µs back to back -> 56,000 to 80,000; with work between calls c = 2.9 to 4.5 µs (run 4, at a load rising
  to 10; this CPU's clock speed varies from run to run, the same work alone taking 17.4 to 18.1 µs) -> 25,000 to
  52,000, and c = 2.02 to 2.18 µs (run 5, idle, work alone 15.80 to 15.82 µs) -> 50,000 to 75,000. Every CPU
  contended: s = 1.76 to 1.88, c_busy = 4.3 to 5.1 µs -> 38,000 to 66,000.
- Zen 4 (c7a, cloud rows): f = 0.100 to 0.254 by layout with the stack exempt, c = 5.14 µs (median) -> 19,000 to
  49,000.
P alone also pays, per new guest process, a process creation: 80 µs (Graviton4) or 95 µs (Xeon), twice for a fork
and an exec (W0 probe), against a slot and a memfd in I; and per unit of pages a fork child touches before it owns
them, a round trip plus a copy and a mapping (about 3.5 µs expected, 3.5) against I's in-process fault (1.31 µs
plus the copy). Both go into the comparison with the recorded rates: P's extra time per second is k·c + n·(process
creation) + m·(c + 1.15 µs), I's is f + m·1.31 µs, for n processes and m copied units per second, the copies
themselves being the same in both. P also shares one zygote per embedder process, whose end ends every P guest
process of that embedder (4.1); I has no such process.
The workload split measured on the laptop (confinement-costs.json, "measure workload-mix"): a shell loop starting
2,000 programs makes about 256,200 system calls and 2,000 processes in 1.53 s of wall time (about 167,000 calls
per second of wall time). Its user time of 0.73 s is not used here: that file's own re-run found that rusage
charges short-lived processes' kernel time to user time and that about 82% of the time is in the host kernel, so
the guest's own CPU time is at most the 0.73 s and probably far less, which only raises its rate per second of
guest CPU above 350,000. Either way it is I's by a factor (and P pays two process creations per program). A Python
start with 70 imports makes about 3,460 calls in 0.9 s (about 4,000 per second of wall time): P (under 1% against
I's f); a compute benchmark makes 9: P. These are expected outcomes from the arithmetic; W2 measures them on both
forms.
Rule given to the embedder (it chooses, not us): with no history, P (the owner's default for agentic workloads);
with history, the form whose arithmetic above is lower at the recorded rates on this host's measured c and f (s
and c_busy when the probe found no free CPU); P whatever the arithmetic when the container needs the line between
its guest processes for guest code (3.7) or its recorded peaks exceed
I's limits (2047 live processes in the embedder process, 4 GiB per process). The form is chosen per container at
start and fixed for its run. Changing it at an exec of a guest process (the kernel is
in the embedder's process in both forms, so one executor could hold memories of both) is possible later and not
built (section 12).

## 8. The interface question

These executors implement, as they stand:
- `wcontainers_executor::Executor` (one instruction, `execute_single_instruction_in_place`): each translates the
  instruction at look-ahead none and runs it in a scratch memory (P: a scratch stub; I: a scratch slot) with the
  given processor state and memory deltas, through the executor conformance suite.
  As built (W1 round 3 run 3): not implemented by P. The suite's only state generator
  (wcontainers-instruction-input-domain-arm64) puts every state's instruction at 0x1000 and its data at 0x2000 to
  0x4000, below the host's `vm.mmap_min_addr` (32,768 on Graviton4, 65,536 on the Xeon), which an unprivileged
  process cannot map, and P runs guest addresses as host addresses; the helper every `Cpu`-based executor uses
  (`execute_on_cpu`) is in the WebAssembly executor's crate, which P would carry whole. The owner's new
  `Executor::execute` with a `MemorySpace` replaces this trait; P conforms to that when it is merged.
- `wcontainers_abi::Cpu`, which `wcontainers-kernel-linux` requires of its executor (`C: Cpu + Executor`), every
  method as 3.5, 4.1 and 5.1 say.
- Not `PerThreadWatch` (either trait) in W1 and W2: threads of one host process share one set of host page tables.
  Watches per process (`Process::watch_memory`) work through the guest view's protections. Not closed off: P can
  give per-thread watches later by making each guest thread of a watched process its own host process over the same
  memfds (its own page tables, mapping commands applied in each), at a process creation (80 to 95 µs) per thread
  creation; I cannot (one process). Listed for the owner (section 12).
- `Runtime`, `Container`, `Interrupter` are the runtime's: the embedder picks the form by which runtime it makes,
  and the runtime's `Interrupter` reaches a running thread through the kernel and item 3 below.

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
   handle touches only that thread's state (its pages or block); everything shared is behind the executor's own
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
3. Stopping a running thread from outside its runner. Nothing in the reviewed interfaces can: `Cpu` has no stop
   call, `Executor` has only `execute_single_instruction*`, item 1's `run_thread` stops only at its own slice, and
   `Kernel::suspend_tasks`, `Process::suspend_threads`, a signal sent to a thread running on another runner, a group
   stop and the runtime's `Interrupter::interrupt` (which has no implementation yet) all need a running thread to
   leave guest code now, from a host thread that holds no `&mut` to the runner's executor. The executor knows the
   host thread (P) or the runner (I) but has no call to be told. The smallest change, for every executor (the VM
   form raises its interrupt, WebAssembly writes its flag word, Unicorn stops its emulation, as native.md R2 asked):
   ```rust
   pub trait Executor {
       // ...as today...
       /// A handle, usable from any host thread, that makes this executor's running threads leave guest code.
       type Stopper: ThreadStopper;
       fn stopper(&self) -> Self::Stopper;
   }

   pub trait ThreadStopper: Clone + Send + Sync + 'static {
       /// Ends the thread's current turn, or its next one if none runs, with `Exit::Stop` at an instruction boundary.
       fn stop(&self, thread: ThreadId);
   }
   ```
   A stop applies to the turn running or the next one, so a stop sent while a runner is between taking the thread
   from the kernel and starting it is not lost; it is cleared when a turn ends. Here: P records it in the
   embedder's record of the thread (out of the guest's reach), which `run_thread` reads before it resumes, and
   sends 3.6's signal to the host thread if a turn runs; I does the same with the runner's block and the runner's
   host thread. The kernel takes the stopper from its executor at `Kernel::new` and calls it from the places above.
4. Nothing else for running: `Cpu::address_space` carries the ceiling, `ProcessInfo::address_ceiling_distance` and
   `on_process_outgrown` cover growth, `CpuError::OutOfMemory` covers slots running out.
5. Making an executor. The executor interface has no constructor, and these two have process-wide parts (3.1, 4.1):
   one zygote and window cache (P), one reservation, slot pool, slot table and set of signal handlers (I). By the
   owner's convention (`StandaloneClock`), the smallest addition is a host that is made once and makes executors:
   ```rust
   /// What an executor's process-wide part is made from and makes executors of.
   pub trait StandaloneExecutorHost: Sized {
       type Executor: Executor;
       type Config;
       type Error;

       /// Makes the one host of its kind in this process, or says why it cannot.
       fn new(config: Self::Config) -> Result<Self, Self::Error>;
       /// Makes an executor for one container, sharing this host's process-wide part.
       fn executor(&self) -> Self::Executor;
   }
   ```
   `Error` names "a host of this kind exists in this process", a refused reservation and a refused call of
   section 6. Until the owner rules, each crate has the smallest public constructor of its own host with the same
   two calls, so that tests can run.
Considered and not asked: a clock the system publishes for the floor to read without a kernel entry (as Linux's
vDSO does) would remove P's round trip per time read, but it changes what the system decides (every read is the
system's answer today); listed for the owner in section 12 instead.

## 9. Named differences from the WebAssembly output and Linux

- P: a guest load or store to our area in its stub (stores to its register pages and signal stacks; loads from
  those, its control pages, and the code area and slot table where the host has no execute-only) succeeds where
  Linux and the other outputs fault. Only a guest that uses an address it never mapped can see it; it changes
  nothing outside its own guest process (3.7). This extends the VM form's accepted
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
- P's ceiling is 2^44 against the VM form's 2^45; I's is 2^32 - 64 KiB per process on ARM64 (2^32 on x86-64 at
  first), and I holds at most 2047 live guest processes per embedder process (255 on x86-64): a fork past that gets
  ENOMEM where Linux would succeed (counted, 3.1).
- I: guests of one embedder process are not separated from each other or from our code against branch target
  injection and cache timing channels on a shared core (3.7); P's guest code is, by the host kernel's mitigations
  between processes. In both forms our kernel serves every container of its embedder process, so a speculation
  gadget in it is not bounded by either form; an embedder process per container is (3.7).
- P: at most `THREADS_PER_PROCESS` (2^15) live threads per guest process, where Linux's own limits are usually
  higher (4.3); a parent that ends while its fork child still borrows its pages pays their copy (3.5).
- P, defence in depth: if translated code were escaped, the stub could write the pages it lends to its fork
  children before they copy them, and shared blocks past others' code-page and watch protections (3.7).

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
x86-64 machine (Xeon Platinum 8488C, 96 processors, bare metal, Linux 7.0.0-1018-aws) at load 0.05 to 0.13. The
figures come from run 2 of W0, except the futex slot's placement, the signal mask and the windows, which come from
run 3 (w0/run3: slot.c, run3.sh, arm3-slot.out and x863-slot.out; Graviton4 at load 3.8 to 7.4, Xeon at 0.7 to
1.6). Run 2's probe.c has a lost-wakeup fault in the guest side's futex wait (and the same in its zygote test's
wait): it passed the word re-read, not the value compared, so a wake landing between the two hangs the run. Its
runs that finished are valid; its runs that "did not finish" were that hang, not slow round trips. Run 1's
outputs (probe-*.out, probe2-*.out, probe3-*.out, probe4-arm.out) are kept beside it but no figure is taken from
them.
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
- The futex slot between two processes, ppid-like, median of the batch means, run 2 (finished runs only, no
  timing per round trip): Graviton4 one CPU 2.29 µs, two CPUs 4.58 µs (the full probe); repeated one CPU 2.36 to
  2.37 µs and two CPUs 4.66 to 4.83 µs (3 runs each, CPUs chosen anew each time), unpinned 4.63 µs; Xeon one CPU
  1.89 to 1.96 µs (4 runs), two CPUs 4.02 to 4.16 µs (4 runs), unpinned 4.39 to 4.56 µs. The second check
  reproduced one CPU at 2.40 to 2.46 µs (Graviton4) and 1.98 to 2.02 µs (Xeon) with its own timing.
- The futex slot, run 3 (slot.c, waits fixed, two clock reads per round trip included). Idle, 6 rounds of 42,000
  round trips per placement, CPUs chosen anew each round: Graviton4 one CPU 2.41 to 2.46 µs (worst 24 µs), two CPUs
  4.60 to 4.80 µs, unpinned 4.54 to 4.82 µs and within a set of 4 CPUs 4.57 to 4.89 µs (1 of 504,000 unpinned
  round trips over 1 ms, worst 1.6 ms); Xeon one CPU 1.98 to 1.99 µs, two CPUs 4.22 to 4.54 µs, unpinned 4.43 to
  4.86 µs (none over 1 ms, worst 169 µs). Every run finished. Busy, 3 rounds of 4,200 round trips, one spinning
  process of normal priority on each CPU named: one CPU shared with a spinner, median 3.71 to 3.73 µs, mean 6.0 to
  6.4 µs, 4 over 1 ms each (Graviton4), 2.72 to 2.96 / 4.9 to 5.7 µs, 3 to 4 over 1 ms (Xeon); the same inside a
  set of 4 busy CPUs, 3.75 to 3.87 / 6.5 to 7.0 µs and 2.68 to 3.07 / 4.7 to 5.7 µs; two CPUs, both busy: 1.6 to
  2.0 ms median of batch means (Graviton4) and 1.98 to 1.99 ms (Xeon); unpinned inside 4 busy CPUs: 1.4 to 2.0 ms
  (Graviton4) and 2.6 to 3.0 µs (Xeon); unpinned inside 4 CPUs of which 2 busy: 4.7 to 4.8 µs and 5.3 to 6.0 µs.
  Hence P's placement (4.3).
- Run 3, other: `rt_sigprocmask` unblocking one signal (raw call) 125 ns (Graviton4) / 87 to 89 ns (Xeon); a 2 MiB
  memfd window mapped over a reservation, one byte touched, and replaced by `PROT_NONE` 8.5 to 8.6 / 4.8 to 5.2 µs,
  without the touch 3.2 / 2.0 to 2.1 µs.
- Run 4 (w0/run4: place.c, sve.c, run4.sh; outputs arm4-place.out and x864-place.out; 2026-11-23 11:52 to 11:53
  UTC, Graviton4 at load 7.4 to 8.2, Xeon at 2.7 to 10.4, the latter rising from other tracks during the run; gcc
  13.3 -O2 -static). The futex slot with guest work between calls (15,000 steps of a multiply-add chain), against
  the same work alone in one thread, 3 rounds, CPUs chosen each round as the least busy over 2 s, "spinner" a
  normal-priority spinning process on the named CPU. Per call, Graviton4: work alone on a free CPU 10.78 to
  10.79 µs, with the round trip 13.38 to 13.41 µs (so c = 2.60 to 2.63 µs), back to back 2.41 to 2.44 µs; with a
  spinner on that CPU work alone 21.60 to 21.65 µs (share 0.50), with the round trip 27.76 to 27.83 µs (c_busy
  6.11 to 6.23 µs; 82 to 84 of 20,000 calls over 1 ms), back to back mean 6.03 to 6.14 µs. Xeon: alone 17.44 to
  18.08 µs, with the round trip 21.0 to 21.9 µs, back to back 2.33 to 3.52 µs; with a spinner alone 31.9 to
  32.8 µs, with the round trip 36.7 to 37.1 µs, back to back 4.86 to 5.16 µs. The CPU share of the two sides,
  from their CPU clocks: 1.000 on every free CPU, 0.505 to 0.512 with work and a spinner, 0.537 to 0.559 back to
  back with a spinner. 4.3's rule (10 ms windows, 0.75, a 1 s mark), 40,000 calls: on a free CPU with a free second
  CPU, no move in any round; started on a CPU with a spinner and a free second CPU, one move each round (21 to
  22 µs / 118 to 137 µs), second half 13.37 to 13.39 µs per call (Graviton4) and 18.10 to 18.29 µs (Xeon); both
  CPUs with spinners, two moves of about 1 ms each, 28.14 to 28.32 µs (Graviton4) and 36.8 to 41.3 µs (Xeon, the
  last round's second CPU slower: its work alone took 36.3 µs).
- Run 4, SVE at a system call (Graviton4, vector 16 bytes): p0 and FFR set all true and z0 to a pattern, read back
  without a call: all kept; after `getppid`: p0 and FFR zero, z0's 16 bytes kept.
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
- Run 5 (w0/run5: check5.c, run5.sh, spcost5.sh; outputs arm5-check5.out, x865-check5.out, spcost-results/;
  2026-11-23 12:07 to 12:20 UTC; Graviton4 at load 16 to 21 for check5 and 5.8 to 6.7 for the campaign, Xeon at
  0.1 to 0.4; gcc 13.3 -O2 -static). P's round trip with the stub's answer word on a page it maps `PROT_READ` and its
  request word on a read-write page, both of one memfd the embedder maps read-write, against both read-write, 3
  rounds of 21 batches, embedder and stub on one least-busy CPU. Graviton4: back to back 2.36 to 2.40 µs (read-only)
  and 2.33 to 2.36 µs; with 15,000 steps of work between calls 13.24 to 13.27 / 13.23 to 13.26 µs against the work
  alone 10.73 to 10.74 µs, so c = 2.50 to 2.53 µs. Xeon: 1.93 to 1.98 / 1.88 to 1.94 µs; with work 17.82 to 17.99 /
  17.85 to 18.03 µs against 15.80 to 15.82 µs, so c = 2.02 to 2.18 µs. The stub's filter rule of 3.7 on both
  machines: `mmap` of the fixed fd at offset 0 or 4096 to 2^40 refused, at offset 2^20 to 2^20 mapped, at 2^20 to
  2^32 + 2^20 (only the low words equal) refused, `mprotect` of a read-only shared page to read-write refused,
  and the same `mprotect` without the filter done. I's check cost again with the harness above, unchanged, 3
  rounds of 6 alternating pairs on one CPU (the fault program was not built in this copy; output and status equal
  the plain build's for all 9 programs): X32-e +21.8% wall (+19.2% with calls), X32-p +21.1% (+18.6%),
  instructions +60.3 / +60.4%; per program within run 2's ranges (sort +34.0 / +34.4%, matrix +76.1 / +70.5%).
  So f = 0.211 to 0.214 for X32-p.
- So by section 7's arithmetic, with f = 0.214 (X32-p, 8 programs): Graviton4 c = 2.29 to 2.37 µs on one CPU gives
  90,000 to 94,000 kernel entries per second back to back, 81,000 to 86,000 with work between calls (runs 4 and 5),
  and 69,000 to 70,000 when every CPU is contended (run 4, I's work slowed as P's); the Xeon 8488C (Granite Rapids's
  f, 0.110 to 0.152) 56,000 to 80,000 at c = 1.89 to 1.96 µs, with work between calls 50,000 to 75,000 idle (run 5)
  and 25,000 to 52,000 at a load rising to 10 (run 4), 38,000 to 66,000 with every CPU contended. The X32 variants' access is `[base, wA, uxtw]`, the speculation-safe one of 3.2, so f needs no
  further instruction for it.
- W1 run 5 (2026-11-23 16:00 UTC, Graviton4 at load 18 to 19, runner and stub pinned to CPUs 40 and 41; ignored
  test `copy_unit_cost`, scratch w1/run5/unit.sh, outputs unit-arm.out and unit2-arm.out, 3 rounds each): a parent
  writes 16,384 pages and forks; the child stores once to each page in order, then a new child to every 16th page in
  a seeded order. Microseconds per store, in order / scattered: u = 0 19.6 to 23.8 / 24.1 to 26.2; u = 1 11.9 to
  13.6 / 26.3 to 27.7; u = 2 9.0 to 10.8 / 29.4 to 33.8; u = 3 7.7 to 8.0 / 35.0 to 38.6; u = 4 6.6 to 6.8 / 50.6 to
  52.7; u = 5 5.6 to 6.1 / 39.5 to 42.7; u = 6 5.4 to 5.5 / 35.5 to 35.9; u = 9 5.2 to 5.3 / 30.7 to 31.0. The
  scattered child holds 2,054 host mappings for u up to 3, 8 from u = 4. Each store is a turn here, so the
  figures carry a round trip that translated code will not.
- W1 run 6 (2026-11-23 17:00 UTC, Graviton4 at load 4.6, one CPU; scratch w1/run6/specarm.c run by spec.sh, output
  spec-arm.out): I's accesses under speculation, the bounds-check bypass of 3.2 as a host program, ahead of the same
  test on translated code at W2. Twelve access shapes (a register offset, byte and Q; an immediate offset; a pair;
  `LDARB`, `LDAPRB`, `LDXRB`, `LDADDB`, `SWPB`, `LD1`, `LD1R` through `add t, base, wA, uxtw`; sp written by 3.2
  (d) then read at `[sp]`), each after I's own check (`ubfx`/`cbnz` on the address) and after a check whose bound is
  a flushed load (a window of about 140 ns), each with its access cut (`uxtw`) and, as the control, uncut (a 64-bit
  offset). A call is given an outside address at random one time in four; the byte it reads speculatively picks one
  line of a probe array, and one line is timed per round: the secret's, the cut address's or another. Seeds 1 and
  2, 3,000 rounds per gadget (about 1,000 per line kind). Behind the slow check, uncut: the secret's line cached in
  938 to 1,038 of 959 to 1,054 rounds for the nine shapes that load speculatively; cut: 0 or 1 of about 1,000 (as
  for lines no one touched, 0 or 1), and the cut address's byte's line in 954 to 1,053 of 960 to 1,065, so the
  speculation ran and read inside the window. Behind I's own check, cut or uncut: at most 2 of about 1,000 for
  every line kind, so on this core the check on the address register leaves no window the harness can use.
  `LDXRB`, `LDADDB` and `SWPB` read nothing speculatively even uncut behind the slow check, so for them the test
  cannot show the cut; their address is formed by the same `add` as the others'. Stores are not measured (a
  speculative store is seen only through a later load of its own address, which the cut keeps in the window). The
  harness had to avoid three things these cores do: their branch predictors learn the original proof's fixed
  pattern of five in-window calls and one outside (a single load behind the slow check then filled its line 9 times
  in 2,000, against 1,976 with outside calls at random); a prefetcher fills most of 256 flushed probe lines when
  several are timed in one round (90% in a fixed order, about 40% in a random one); and with the same two bytes in
  every round the secret's line was filled in up to 60% of rounds of a cut gadget, which a new secret and in-window
  byte each round ended.

## 12. Decisions for the owner

1. Names: the two crates (`host-process`, `in-process`) and the form value's variants.
2. I's wrapping check (5.2): about 10 points faster on Graviton4, and a guest access outside its window lands in
   its own memory instead of faulting. Both it and the exact form are safe against speculation (3.2).
3. P's reachable stub pages (9).
4. PAuth and MTE under P and I (9).
5. A clock the system publishes for reads without a kernel entry (8).
6. Changing the form at a guest exec (7).
7. The interface changes of section 8: running a thread (item 1, a handle method on `Executor` or a handle type in
   `Runner::run`), the count of kernel entries (2), the stopper (3) and the executor host (5).
8. Per-thread watches, and so `(Parallel, Exact)`, under P by a host process per guest thread (8): wanted, and when.
9. I's runner threads holding off the embedder's SIGCANCEL and SIGSETXID while they run (9).
10. I's window of 4 GiB per guest process on ARM64, cut for free by the access's addressing mode, against a larger
    one (up to 2^36 per process, fewer slots) that pays one more instruction per access, not measured.
11. I's guests sharing one process without a line against branch target injection and cache timing (9).
12. P's defence in depth behind translation (3.7): accept that an escaped stub can write the pages it lends to its
    fork children before they copy them and the shared blocks it maps, or close the first with a sealed file per
    fork (the parent's view remapped at every fork, overwritten pages kept until the file closes).
13. The order of the work: hosts with pages larger than 4 KiB follow the 2026-11-18 ruling by split host pages
    (section 6), built at W4, after both forms work on 4 KiB hosts.
14. P under gVisor on ARM64 (3.6, W1 run 10): a stop or fault there returns through its signal frame to get the
    FP/SIMD registers back, and the frame lies on the thread's signal stack, which the guest's other threads can
    write (item 3). A sibling thread that rewrites the frame's pc between the handler's store and the host's read
    in `rt_sigreturn` (a few microseconds under systrap) runs at a pc of its choosing in its stub: an escape from
    translation, bounded by the stub's filter and by gVisor itself. Accept it there, or make P unavailable under
    such a host (the evaluator then chooses I). Linux hosts never take this route.

## 13. Milestones

**W1: form P on ARM64 Linux.** Needs the translator's `HostProcess` form and the `stop` crate with a parameter for
the slot table's base (10), and, for the comparisons of addresses, memory size and the count with the WebAssembly
output, the kernel's K2 (3.1), which is in the kernel: the kernel test gives both outputs the WebAssembly output's
ceiling and memory size and compares wait status, output, root files, addresses, memory size and the turns of
one-thread programs. `differential` is built (tests/differential.rs, W1 round 7 run 2): the VM form's differential
test, every presented form's layer 1 cases with the bar's own edges, the words layer 1 leaves out, the float status
words and the exclusive pairs, each run under P's two look-aheads (the exact executor's lone pieces and the ordinary
executor's full look-ahead) and compared whole with the WebAssembly output, with the count against the counting rule;
P has no translation without the count, so "with and without the count" is the VM form's alone. `count_at_every_stop` is built on 3.6's exact executor (pieces of one instruction, as the VM
form's look-ahead none) and compares the returns and the hash of their positions (count and pc) with the VM executor's,
which shares the native output and the count's definition (the WebAssembly output ends a limited turn at its own stop
checks, another quantity): one test per program of the VM form's set, bench, calls and jumptables. Also kept: turns of
the ordinary executor limited to one counted event change nothing (`turns_limited_to_one_counted_event_change_nothing`). Does not need the owner's interface items (section 8): the kernel is driven by 4.6's
`with_parts` loop, the executors are made by their crates' own constructors, and the kernel is not yet given the
stopper. Builds: the shared layout, floor, code area, memory with copy-on-write, stops (section 3), the zygote and
stubs, the control and register pages, placement and the runners (section 4), the start-up probe and the seccomp
filter. Pass bar, every item on the rented ARM64 machine:
- the executor conformance suite and the abi `Cpu` behaviour the kernel relies on (the kernel test crate's
  programs: hello, the fork, exec and signal program, and busybox's `text` and `files` commands, as N3's bar) give
  the reference's output and status, and the same verdicts as the WebAssembly output; once K2 lands, exits,
  registers and memory equal the WebAssembly output's at every look-ahead, with and without the count
  (`differential`, the VM form's differential test, seeded, with its case knob; cases run recorded in the status
  file);
- the count at every stop is identical to the reference's (`count_at_every_stop`, a stop at every counted event,
  under the exact executor, against the VM executor's returns and hash of positions);
- the zygote survives its own migration between CPUs (the fourth check's case: moved 200 times, answering after
  each), and no registration of restartable sequences is left in it or in a stub;
- the adversarial cases, each a kept test: a fork while a sibling thread is in a system call and another writes a
  page in a loop, words rising by one per pass (seeded, case knob): in every child's copy the words fall at most once,
  as a copy taken at one instant gives; a parent that forks twice before the page is copied and a child that forks
  before copying, each process's contents checked against the reference; a shared mapping written by two guest
  processes, checked against the reference; a stop signal landing at every instruction of the floor's entry, resume,
  wait and handler-tail windows (forced by tracing the thread from the test and stepping it to each instruction in
  turn, W1 run 8, tests/step.rs; the handler's own instructions by a second stop sent at each of them, which must
  land just after the handler's unblock in the window it chose, W1 run 9), and
  during a resume's queued commands, a `clone` among
  them (the thread is made exactly once); a guest that stores to its control pages (each store a SIGSEGV for the
  guest at that address, as on Linux) and writes forged exits, thread ids and registers into its threads' register
  pages, after forking twice: A forks B and C, A writes a page so that B owns it and C borrows from B, then B's
  guest forges every exit a fault on that page could give and writes the page again (C's copy equals the page at
  the fork's instant, B's own writes stay B's, and no host code runs that we did not translate); a stub
  that tries a host call outside its filter is killed; a stub killed from outside ends its process as SIGKILL within
  100 ms of its death, and the zygote holds no zombie once the embedder has asked it to reap; the embedder killed
  leaves no stub or zygote behind; threads made and ended in a seeded loop (case knob) with stops sent at random
  moments, a new thread's start included, each thread's registers its own and no index reused before its
  exit-clear word is zero; a process that makes `THREADS_PER_PROCESS` threads and one more gets its `clone`
  refused, counted; a guest that maps one shared block 10,000 times holds no fd for it in its stub;
  64 guest processes each mapping 2^43 bytes and touching pages across all of it, run by one embedder, whose own
  mappings stay under the window cache's bound (counted evictions recorded); one P container and two I containers
  (W2) running at once in one embedder; the register-group isolation tests of 3.8, and the start-up probe's SVE
  test of 3.8;
- placement: runners of two containers in one embedder take different CPUs; with a spinning program on a runner's
  CPU and a free CPU in the set, the runner moves once and its guest's getppid loop with work between calls runs
  within 5% of the same loop on a free CPU, measured in the same run; with every CPU of the set contended, at most
  one move per second;
- the suites pass inside a container started with Docker's default seccomp profile;
- I's bounds-check bypass on ARM64 (brief-native.md's last section), one gadget per access shape as a host program
  of I's own sequences, each beside its uncut control, which must read the byte outside the window: nothing read
  through a cut access (section 11, W1 run 6); the same test on translated code is W2's;
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
  just outside the window's edges, tagged and untagged, every access form, sp-based ones with register offsets and
  register post-indexes included, each faulting exactly where the reference does with the reference's registers;
  sp moved out of the window by every form of 3.2 item 1 (immediates up to 16 MiB, writebacks, register writes),
  then read, moved back and accessed, each matching the reference, the displaced counter recorded; slot exhaustion
  gives ENOMEM at fork, counted; two I containers and a P container in one embedder, each guest reading and writing
  at every edge of its window, none seeing another's bytes;
- speculation: a bounds-check-bypass test (a guest trains the check's branch, then gives an address in another
  slot, and times its own cache lines to read the byte there), seeded with a case knob: it reads nothing under I,
  and the same gadget built as a host program without the cut reads the byte, which shows the test can see a leak;
- speed: the translator's benchmark set under I against the same programs run plainly, alternating pairs, within
  the sound form's figure (X32-p, section 11) plus 3 points on the same machine (re-measured at W2 start); the
  getppid entry under 100 ns median;
- the crossover measured with both forms built on the ARM64 machine: the compute benchmark, a Python start, and
  busybox `sh` starting 1,000 programs, each in both forms with its kernel entries per second, and the measured k at
  which the two cost the same, against section 7's arithmetic.

**W3: both forms on x86-64 Linux.** Needs the x86 native output with its `Way` (XN1, XN2). Builds 3.10's parts.
Pass bar: W1's and W2's bars on the rented x86-64 machine, the crossover there included, the x86 plan's own way
P and way I tests, and a handler installed without `SA_RESTORER` caught by a test of the start-up path.

**W4: hosts with pages larger than 4 KiB.** Builds section 6's split host pages in both forms. Pass bar: W1's and
W2's suites on an ARM64 Linux machine with a 64 KiB-page kernel (a rented machine booted with one, such as Ubuntu's
`linux-image-generic-64k`), each guest seeing 4 KiB pages; a seeded random test (case knob, count recorded) of
mappings, protections, shared blocks and forks at 4 KiB granularity inside one host page, every access matching the
reference; the split-page counter and its cost recorded for busybox, Python and a Rust program.

