# wcontainers: the native back end

Status: plan, 2026-11-18 (N0). Nothing of it is built.

Words used here:
- **native output**: the translator's second output, machine code of the guest's own architecture.
- **virtual machine**: one hardware virtual machine made through KVM (Linux) or Hypervisor.framework (macOS).
- **inside**: the one program we run in the virtual machine's privileged level: our kernel, the runtime's
  loop, the native back end's own part and the layer under them. **floor**: that layer under them (entry,
  page tables, interrupts, timer, the link out).
- **monitor**: the host program's part that makes the virtual machine, runs its virtual CPUs and answers
  what the inside asks of the host.
- **link**: the one way the inside asks the host for something (section 5.6).
- **count**: the progress count of runtime.md section 9, defined on the guest's own instructions.
- **position**: a (count, pc) pair.
- **reference**: real Linux of the guest's architecture, as in runtime.md.

Paths are shortened: `gcpu/` is crates/wcontainers-guest-cpu/src, `arm64/` crates/wcontainers-arm64/src,
`tr/` crates/wcontainers-translator/src, `rt/` crates/wcontainers-runtime/src, `abi/` crates/wcontainers-abi/src,
`kernel/` crates/wcontainers-kernel/src. Lines are at 68013b31 (this branch). Experiments are under
pyro-parked/wcontainers-scratch/native/experiments and are listed with their results in docs/status-native.md.
Every figure says whether it was measured (and where) or is only expected.

## 1. What the native back end delivers

A guest of the host's own architecture runs inside a virtual machine at full speed, translated (same
architecture in and out), with our kernel inside the same virtual machine, so a system call is answered
without leaving it. Platforms: KVM on Linux for x86-64 and ARM64 hosts, Hypervisor.framework on macOS
(ARM64). Where the machine does not let an ordinary program use virtualization, the WebAssembly back end runs
as today; which one runs is chosen at start by probing the host (section 5.8).

Done means:
- the same suites the WebAssembly back end passes pass on the native back end, with the same results
  (section 8);
- a run recorded on either output replays on the other, to the same positions and the same memory and
  registers (section 8);
- translated code runs within the bar of section 8 against the same program run plainly on the host.

## 2. Rules

All rules of runtime.md section 2 hold, and the owner's rulings in the brief (brief-native.md). The ones
that shape this plan most:
- Unmodified guest code never runs. Every page of guest memory is mapped without execute permission for the
  guest's level; only translated code runs.
- Identical behaviour to the WebAssembly output, instruction for instruction as the guest sees it. Where the
  hardware would answer differently from what the WebAssembly output presents, the instruction is routed
  (section 3.4). Section 7.2 lists the places where identity needs an owner's decision.
- One loop, one kernel crate driven through `Cpu` and `Host`, one execution interface. Nothing here adds a
  second loop or scheduler.
- No stop check in translated code. One add per counted event. A check only at a replay target.

## 3. The translator's native output

### 3.1 Shared with the WebAssembly output, and new

The translator today is one crate whose finding and emitting are both in it:
- Shared as it is, from the guest interface: `Personality` (gcpu/lib.rs:24-38), `Guest::decode`, `flow`,
  `accesses`, `stack_move`, `built_address`, `jump_table`, `plain`, `SYSCALL`, `CODE_CHANGES`, `trap_kind`,
  `memory_fault`, `REASONS` and the guest's offer (gcpu/translate.rs:11-224). None of these writes
  WebAssembly.
- Shared after a move inside the translator crate: what finds code, which writes no WebAssembly either:
  `Code` and `Decoded` (tr/code.rs:520-566), the `Finder` with `functions`, `stretches`, `explore` and the
  `Means` of every start (tr/full.rs:228-605, tr/code.rs:85-120), `LookAhead` and `Optimise`
  (tr/code.rs:21-45), and the piece list a translation hands back (`CodeModule::pieces`, tr/code.rs:122-130).
  Today the finder is `pub(crate)` and built inside `plan` together with the WebAssembly plans
  (tr/full.rs:44-113). Change T1 (section 6.2) moves it into a module of its own with no wasm-encoder in it.
- WebAssembly only, unchanged: `Guest::emit` and the `Emitter` trait, which hand meanings a wasm-encoder
  `InstructionSink` (gcpu/translate.rs:158, :246-316), the plans (registers as locals, flags as the last
  comparison), the contract, the thread module and the state memory (tr/contract.rs, tr/thread.rs,
  tr/state.rs, tr/layout.rs).
- New: the native emitter, generic over `G: Guest + Native`, and a `Native` trait each guest crate
  implements only for a guest that can be a host (ARM64, x86-64). `Native` says, per decoded instruction,
  how it is carried over (section 3.4); it never describes what the instruction computes, since the
  hardware computes it.

Why not implement `Emitter` for native code: `Emitter` is WebAssembly's shape (a value stack, locals,
`sink()` returning wasm-encoder's sink, gcpu/translate.rs:248), and every meaning is written as WebAssembly
(for example arm64/meaning.rs:1440-1460). Native output of the same architecture copies instructions; going
through meanings would re-implement every instruction in host code, which is slower and is exactly what the
hardware does for free. Rejected too: Cranelift or LLVM as the native emitter (an IR compiler in the path of
code that is already the host's own; slower to translate, and the guest's exact instruction would be lost to
the compiler's choices).

The place where a guest memory access is emitted is one function of the native emitter (`access`), which
today copies the instruction as it is, because the virtual machine's page tables enforce the guest's
permissions. An in-process form (guest memory rewritten) would change only that function (brief).

### 3.2 Pieces, the code area and linking

- **The code area.** Translated code lives in the inside's half of the address space (ARM64: the TTBR1 half;
  x86-64: the upper canonical half), mapped executable for the guest's level and, on ARM64, not readable
  by it (execute-only: stage 1 AP[2:1] = 00 with UXN = 0). Guest memory (the TTBR0 half, the lower canonical
  half) is mapped never-executable for the guest's level (ARM64 UXN = 1; x86-64 NX = 1). So unmodified code
  cannot run, and code we made cannot be read by the guest on ARM64. Execute-only for EL0 on Hypervisor.
  framework is to be confirmed by N1's first test (not measured yet).
- **Pieces** are what the shared finder gives at the container's look-ahead, as for WebAssembly: one
  instruction (none), a stretch (bounded), or a function (full). A piece is the guest's instructions in
  order, each carried over by its route (section 3.4); branches inside a piece are direct.
- **Exits linked to each other.** A direct branch out of a piece goes to its target's piece once that is
  translated: a direct branch where the target is within reach (ARM64 128 MiB, x86-64 2 GiB; the code area
  is filled in that order so that most are), otherwise an absolute jump through a withheld register. Until
  the target is translated it goes to the target's entry slot (below), which faults to the inside and is
  then patched (one aligned 4-byte write on ARM64, a rel32 on x86-64, while no virtual CPU runs that code,
  then the architecture's instruction-cache maintenance).
- **Jumps through a register and returns.** ARM64 (fixed 4-byte instructions): the code area holds an entry
  mirror, a 16-byte slot for every 4-byte guest address G below 2^45, at `mask | G << 2` with `mask` the
  logical immediate 0xffff_8000_0000_0000, so the mirror fills the top half of the TTBR1 half and the inside
  and the pieces sit in its lower half. A slot holds its piece's address made in three moves and a `br`
  (it reaches any piece); a slot with no piece holds `udf`; a slot page not yet needed is not mapped. A
  jump through a register is `lsr t, xN, #45; cbnz t, bad; lsl t, xN, #2; eor t, t, #mask; br t` (t a
  withheld register, section 3.3; no memory read, no flags touched). A guest address at or above 2^45 goes
  to `bad`, which reports the fault at that address as the WebAssembly output does. A missing slot raises
  an exception at EL1, which asks for a translation from that address (bounded look-ahead, as the
  WebAssembly output does at run time), fills the slot and carries on. Calls can keep the return-address
  predictor right: a guest `bl` or `blr` at G is made through G's own slot, whose last instruction is a
  `blr` to a call stub, so the predictor holds the slot of G + 4; the stub turns x30 back into the guest's
  return address (`eor x30, x30, #mask; lsr x30, x30, #2`) and goes on to the callee. A guest `ret`
  becomes the jump above ending in `ret t`, which the predictor then foresees, and the slot of G + 4 goes
  back into the caller's piece. Entering at G itself (a resume after a signal, a jump to a call
  instruction) runs the same slot, so it is the same call. N3 measures this against plain calls (x30 set
  from the base word, a direct branch, every return through the mirror unforeseen) and keeps the faster
  exact one. The mirror needs every guest address below 2^45 (32 TiB); that is the owner's (section 10),
  and without it ARM64 uses x86-64's lookup below. x86-64 (instructions of any length, 47-bit addresses):
  a per-thread cache of (guest address, piece) pairs, direct mapped, read through the GS base, in front of
  a per-process table; a miss enters ring 0. The sequence keeps the guest's flags where they are live.
  N8 measures it against a plain per-process table lookup and keeps the cheaper exact one.
- **Addresses in emitted code.** Output never holds an absolute guest address, as for WebAssembly
  (runtime.md section 7): a guest pc-relative instruction (ARM64 `adr`, `adrp`, literal loads, branches;
  x86-64 RIP-relative operands) is re-emitted as the same operation on an address made from the piece's base
  word (one 8-byte word per loaded translation, read pc-relatively, holding where the guest range sits). So
  the same bytes serve every process that maps the range at any address, and only the base word differs.
- **Where translation happens.** On the host, in the monitor, with the translator as it is (std, rayon,
  the cache of runtime.md section 7). The inside asks through the link (section 5.6) with the range's bytes
  in guest memory, its starts and the settings; the monitor answers with the code placed in frames the
  inside gave. Why: translating takes milliseconds to seconds (runtime.md section 7, measured at the
  translator's M5) against an exit of 0.7 µs (M5) to 6.2 µs (old Intel) (brief), and the translator, its
  threads and the disk cache need the standard library. Rejected: the translator built `no_std` inside the
  virtual machine (one virtual CPU to translate on, no disk cache, and a large change to the translator).
- **The cache.** As runtime.md section 7, with the back end in the key ("native, ARM64 on ARM64" or "native,
  x86-64 on x86-64"), the guest's offer and, for native code, the host CPU features the emitted code relies
  on (none beyond the architecture's base, and the withheld registers' choice, which is part of the build
  identity). Files on disk are shared between runtimes of the same build (the other project's "translated
  code cached per backend as files", design.md section 6). Inside the virtual machine, a loaded translation
  is found by its key's hash, so a second process mapping the same bytes at the same address uses the same
  frames without asking the host; at another address it gets its own base word and the same code frames.

### 3.3 Registers the translated code keeps for itself

Translated code needs a few registers the guest cannot see: the count (section 4) and one or two scratch
registers for jumps through a register and for routed instructions. The guest's own values of those
registers live in a per-thread page of the inside's half (read and written through the code area's
private mapping) and every guest instruction that names one is re-emitted through a scratch register.
- x86-64: the count in one withheld register (`lea r, [r + 1]` keeps flags, measured as the cheapest exact
  count, progress-count-cost.json "measure native-x86-64"); the thread page through the GS base, which the
  guest does not use (its own GS base is the kernel's state, set through `arch_prctl`, and its `gs:`
  operands are re-emitted against it).
- ARM64: three withheld registers (count, scratch, thread page), the shape the round 3 prototypes used
  (costs-round-3/native-arm64/src/instrument3.py, x26 to x28). Which three is chosen by measurement, not
  now: compiled with three fewer registers clang lost nothing measurable (+0.0% instructions,
  summary-main-r0.txt "res3"), but a guest that names the withheld registers pays for each use. Measured
  here (static, not dynamic): over eight ARM64 Linux files (busybox, musl's loader, rg, xz, libpcre2,
  postgres, libcrypto, a Go program; 3.7 million instructions) the fewest instructions name x18 (1.3 per
  1000), x15 (2.9), x14 (3.9) and x13 (5.1); x26 to x28 are named by 13.7 to 22.4 per 1000, and the Go
  program names x27 by 58 per 1000 (experiments/regs.out). N2 counts dynamic uses on the translator's
  benchmark programs and real programs and picks the cheapest three.

### 3.4 How each instruction is carried over

`Native::route(instruction)` gives one of these, per decoded form (each guest crate's table):
- **Copy**: the bytes as they are (most instructions; every memory access, since the page tables check it).
- **Rebase**: a pc-relative instruction, made from the base word (section 3.2).
- **Transfer**: a branch, call or return, emitted as section 3.2 says, with the count's add where the count
  definition puts one (section 4).
- **Kernel**: the instruction becomes a call into the inside, which answers as the WebAssembly output's hook
  for the same instruction does. These are every instruction whose result depends on the host:
  - ARM64: counter reads (`mrs` of CNTVCT_EL0 and CNTPCT_EL0, the WebAssembly output's `Clock` hook,
    arm64/meaning.rs:1458), `ic ivau` (the `CodeChanged` hook, arm64/memory.rs:382), and store-exclusive
    (below);
  - x86-64: `rdtsc`, `rdtscp`, `cpuid`, `rdrand`, `rdseed`, `rdpid`, `xgetbv`, `syscall`, and what the x86
    plan routes to a hook;
  - every instruction the guest crate decodes as a system call (`Guest::SYSCALL`).
- **Constant**: a register read the WebAssembly output answers with a fixed value (arm64/decode_forms.rs:
  242-256: the counter frequency 1 GHz, CTR_EL0 0x8444_c004, DCZID_EL0 4, the identity registers, the
  read-only thread pointer). Measured on this M5 at EL0 (experiments/sysregs.out): CNTFRQ_EL0 reads 1 GHz and
  DCZID_EL0 4, the same as presented, while the round 3 virtual machine read a counter frequency of 24 MHz
  inside Hypervisor.framework (mac-tests.jsonl, "cntfrq"), and CTR_EL0 traps under macOS. So these are
  never left to the hardware: the presented value is the answer everywhere.
- **Masked**: a write the WebAssembly output keeps to the presented bits. ARM64 FPCR keeps only
  `FPCR_BITS` (0x07c0_0000, arm64/instruction.rs:491); measured on this M5, the bits 0x3fbf07 outside that
  set stick when written (experiments/fp.c and sysregs.out: alternative handling, the trap enables, FZ16 and others), so a guest
  could switch on host behaviour the WebAssembly output does not have. The write is re-emitted with the
  mask.
- **Trap**: what the WebAssembly output turns into a trap (an instruction outside the offer, which the guest
  decoder does not decode; `udf`; `brk`; x86's `ud2`, `int3`, privileged instructions) is a call into the
  inside with the same `TrapKind` (gcpu/lib.rs:133-154). The offer is the translator's, never the host's:
  ARM64 offers FP, AdvSIMD, CRC32 and LSE (arm64/forms.rs:8), and this M5 has 55 features (`sysctl
  hw.optional.arm`, SME and AFP among them), so an instruction the M5 runs but the offer does not include is
  never copied.
- **Exclusive**: ARM64 load-exclusive and store-exclusive keep the WebAssembly output's monitor
  (`ExclusiveBytes`, `ExclusiveAddress`, the value, arm64/guest.rs:408-411) in the thread page: a load
  exclusive records, a store exclusive compares and stores with LSE's compare-and-swap (in the offer). Why:
  a hardware monitor is cleared by any exception or interrupt taken between the two, so a store exclusive
  would fail where the WebAssembly output's succeeds, and the retry would change the count. Expected from
  the architecture's rules, to be shown by N5's replay test with interrupts between the pair.

Proof that routing matches the WebAssembly output: a test (in the guest crate's tests, not shipped) runs
every decoded form through `Guest::emit` with a recording emitter and checks that a form whose meaning
calls `hook`, `syscall`, `trap` or writes a fixed value is routed **Kernel**, **Trap** or **Constant** natively,
and that every **Copy** form's meaning calls none of them. A form added to a guest crate is then routed by
the same table or the test fails.

### 3.5 Code a program writes itself

As for WebAssembly (runtime.md section 7): ARM64 programs announce changed code with `ic ivau` (the
presented CTR_EL0 says so), which is routed to the inside and drops the pieces of that line; the kernel's
"code is here" notice drops a range. Dropping a piece unmaps its entry slots and turns every linked branch
into it back into a branch to the slot, then invalidates the code area's lines on every virtual CPU. x86-64
does not announce: as the x86 plan says (section 5.7 there), a page that has been translated carries the
"translated code" write lock, so the first write faults to the inside, which drops that page's pieces and
lifts the lock. A range whose bytes changed after it ran is translated at bounded look-ahead from then on,
counted "code changed after it ran", as runtime.md says. Native code never reads the guest's bytes after
translating them, so a change the guest does not announce (ARM64) runs the old translation, as the
WebAssembly output does.

### 3.6 Crates, and what must be ours

- Host side: `kvm-ioctls` 0.25.1 (Apache-2.0 OR MIT) and `kvm-bindings` 0.14.2 (Apache-2.0) for KVM;
  `applevisor` 1.0.0 (MIT OR Apache-2.0) for Hypervisor.framework. Alternatives: `hv` 0.1.2 (Apache-2.0,
  older), `xhypervisor` 0.3.0 (MIT, x86 macOS). `vm-memory` (Apache-2.0 OR BSD-3-Clause) is not needed: guest
  memory is one mapping we make.
- Inside: `aarch64-paging` 0.12.3 (MIT OR Apache-2.0) and `x86_64` (MIT/Apache-2.0) for page table entry
  types; `aarch64-cpu` 12.0.0 (MIT/Apache-2.0) for system registers; `talc` 5.1.1 (MIT) as the allocator.
  The page-table walk that copies, shares and write-protects is ours (section 5.2), since it is the speed
  of fork and of the write locks.
- Emitting: x86-64 re-encodes copied instructions at their new address with `iced-x86` 1.22.0 (MIT; already
  the x86 track's decoder), whose block encoder fixes relative branches and RIP-relative operands. ARM64's
  forms we emit (branches, `adr`/`adrp`, `movz`/`movk`, `add`/`sub` immediate, `eor` immediate, loads and
  stores with an immediate offset, `mrs`) are a small encoder of ours: fixed 4-byte fields, purpose-built.
  Rejected: `dynasmrt` 5.1.0 (MPL-2.0, a file-level copyleft licence, and an assembler driven by macros
  where we need a few encodings at run time).
Licences checked with `cargo info` on 2026-11-18 (docs/status-native.md).

## 4. The progress count

**One add.** The count is a withheld register (section 3.3). At each counted event the native output emits
one add (ARM64 `add c, c, #1`, which sets no flags; x86-64 `lea c, [c + 1]`). No compare, no branch. The
count is saved to the thread's register block at every exit to the kernel, with the count word runtime.md
section 9 places there, and loaded when the thread runs. So the kernel, saves and moving a thread see it as
on every back end.

**One switchable place.** Which events are counted is the orchestrator's choice, from measurement (brief).
It is one function of the shared finder: `counted(instruction, flow) -> Option<Count>`, saying whether an
instruction carries an add and where (before the transfer; at a loop head as round 3's "Hn"; at the
target), with the arithmetic that pays back an add made on a path that is not the counted event (round 3's
"Th" and "Mn", instrument3.py). Both emitters call it; neither has its own rule. Every candidate the
measurements have (T, E, Hn, Mn, round 3) is a value of that function. The kernel's own changes of pc are
counted by the loop on every back end (runtime.md section 9) and are not the emitter's.

**An interrupt between an add and its transfer.** An interrupt can land at any host instruction. Every
translation carries a position map: for each run of host code, the guest instruction it belongs to and how
far the count register is ahead of the count of completed events at that point (0 or 1, or, for a paid-back
add, a value the map names). The floor, at any interrupt or fault in translated code:
1. finds the run holding the host pc;
2. sets the guest pc to that guest instruction's address, and the count to the register less the map's
   amount;
3. undoes the instruction's partial work by restarting it: every host sequence that carries one guest
   instruction is made restartable from its start (it writes only withheld registers before its last
   instruction, which is the one that commits; a guest register it writes earlier it writes with a value
   that does not depend on itself, as `x30` before a call's branch).
So a stop is always at a guest instruction boundary with the count of events completed before it, which is
the same position the WebAssembly output reports for a thread stopped there. Round 3 used the same shape
(the `_gmap_corr` map, instrument3.py) and replayed 320 asynchronous stops twice each to the same (count, pc,
data) (progress-count-cost.json "measure native-arm64"). x86-64 instructions the hardware stops part-way
(`rep` string operations) are finished by the floor before the stop is reported (they are bounded by their
own work), unless the count definition counts their rounds, in which case the native output emits them as
the WebAssembly output does, one counted round at a time. Which one follows from the definition the
orchestrator adds and is settled at N8.

**Tested:** N4's every-offset test interrupts a thread at every host instruction of every piece of a
program once (seeded order) and checks that the reported position and registers equal a run stopped there
by a replay check.

## 5. The virtual machine

### 5.1 Who runs where

- ARM64: the inside runs at EL1, translated guest code at EL0. EL2 is not used: nothing inside is a
  hypervisor (Hypervisor.framework can offer EL2 to a guest on newer chips, `hv_vm_config_set_el2_enabled`
  in Apple's documentation for macOS 15; KVM only with nested virtualization; neither is wanted).
- x86-64: the inside runs in ring 0, translated code in ring 3, in 64-bit mode with 4-level paging.
- One virtual machine per container. The monitor is one host thread per virtual CPU and the host threads
  the host services need.

### 5.2 The inside

One `no_std` Rust program per architecture, built for `aarch64-unknown-none` or `x86_64-unknown-none`. It
holds:
- the floor: entry (ARM64: SCTLR_EL1, TCR_EL1 with a 4 KiB granule for both halves, MAIR_EL1, VBAR_EL1;
  x86-64: GDT, IDT, TSS, CR0/CR4/EFER), exception vectors, a frame allocator over guest-physical memory,
  the heap (`talc`), the timer (ARM64 the virtual timer through the GIC; x86-64 the local APIC's
  TSC-deadline timer), the interrupt controller, and the link. What the guest's level is allowed is set as
  the WebAssembly output presents it: on ARM64, SCTLR_EL1.A = 0 (unaligned ordinary accesses allowed) and
  SA0 as `Guest::SP_ALIGNMENT_CHECKED` says (gcpu/translate.rs:196), UCI set as Linux sets it (data cache
  maintenance a program does before `ic ivau` runs as on Linux; cache-type reads are routed anyway), DZE set (`dc zva` with the presented block
  size, which this M5 also has); on x86-64, XCR0 holding only the state the offer claims, CR4.TSD set so a
  stray `rdtsc` traps;
- the kernel crate unchanged. Measured: crates/wcontainers-kernel builds as it is for `aarch64-unknown-none`
  and `x86_64-unknown-none` with Rust 1.99.0 in release (experiment on the Linux laptop, 2026-11-18); it is
  `no_std` with `alloc` (kernel/lib.rs:9-11);
- the runtime's loop (`Stepper`, rt/lib.rs:189-244), the native `Execution` (section 6) and an `Outside`
  (section 6.2), all `no_std`.
The workspace's toolchain file gains the two targets (rust-toolchain.toml lists none of them today). The
monitor carries the inside as bytes made by a build script that runs cargo for the target.

**Page tables.** The kernel's `Cpu` side today is a software record per page, written through
`set_permissions`, `memory_new`, `memory_duplicate`, the shared blocks and `code_changed` (abi/cpu.rs:35-97).
Inside the virtual machine the native `Cpu` side writes real page tables instead, one set per guest
process (one address-space identifier each), 4 KiB granule, so each guest page has its own permissions,
as the brief rules. A guest page is a 4 KiB guest-physical frame; the host backs guest-physical memory in
its own pages. On a 16 KiB-page Mac that changes nothing for the guest: measured, two neighbouring 4 KiB
guest pages inside one 16 KiB host page took different permissions in Hypervisor.framework, and a write
to the read-only one faulted alone with the right address (costs-round-3/virtual-machine
results/mac-tests.jsonl "perm4k", three runs). The kernel's permissions are the page table bits less the
back end's write locks (runtime.md section 8), made by one function from the record, as today.
Measured costs in the round 3 prototypes: a first-touch fault handled inside, 224 ns with the frame already
backed, 584 ns when the inside zeroes it (Hypervisor.framework, mac-tests.jsonl "guest_fault"); one page's
permission changed through a system call inside, 558 ns with the invalidation Linux uses, against
323 ns for the host's own `mprotect` (same file, "guest_permission_change", "native_mprotect_16k").

**Memory from the host.** Guest-physical memory is one large reservation the monitor maps into the virtual
machine at start; the host gives it pages only when touched. The inside gives memory back through the link
(the host then drops those pages: measured with `madvise`-style reuse 5.4 ms per GiB and with unmapping and
mapping again 14.7 ms, mac-tests.jsonl "hvf_memory"). "More memory" past the reservation is a link call
that maps another region (brief: leaving for more memory).

### 5.3 Processes, fork, threads and virtual CPUs

- **A process** is one set of page tables and its record. `memory_new` makes an empty set.
- **Fork** (`memory_duplicate`): the child gets a copy of the parent's tables in which every writable
  private page is read-only in both and marked copy-on-write; the first write by either faults to the
  inside, which copies the 4 KiB frame (or takes it back if it is the last holder) and carries on. This is
  Linux's own method; no host call is made. Shared blocks (`shared_map`) map the same frames in each process,
  which a 4 KiB granule allows at any 4 KiB boundary, so the copying method runtime.md section 8 needs for
  16 KiB host pages is not needed here.
- **Threads** are register blocks plus the thread page (section 3.3); a switch saves the withheld
  registers and the count into the block and loads the next.
- **Virtual CPUs**: one at first, so one guest thread runs at a time, as on every back end today. More than
  one is built only after the owner's ruling on parallel guest threads (the x86 memory ordering question is
  the same ruling); the design allows it (one monitor thread per virtual CPU, interprocessor interrupts
  through the interrupt controller for stops and for TLB shootdown when the kernel changes a table, as
  tr/contract.rs:17-27 asks of any runner).
- **Dirty pages and point-in-time copies** (the other project's design.md section 7): a mark write-protects
  every writable private page, the first write records it; a point-in-time copy also copies the frame aside
  first. Both are the write locks of runtime.md section 8 in real page tables.

### 5.4 Host calls leave and return

The kernel calls `Host` (abi/host.rs) from inside. The inside's `Outside` answers each method either
inside or through the link:
- inside: the monotonic clock (from the virtual counter or the TSC, scaled to nanoseconds with the offset
  the monitor gave at start), randomness from a pool the monitor fills through the link in batches,
  warnings counted inside and sent in batches;
- through the link: `output`, `backend_send` (files), every network call, the wall clock, more memory, and
  translation (section 3.2).
Events from outside (`Event`, abi/host.rs:57) arrive on host threads, which write them into a ring in
guest memory and raise an interrupt (section 5.5); the inside's `Outside::arrived` takes them from the ring
at the next exit, as the shipped outside's would.

### 5.5 Stopping a running guest

- **A turn's end** is the inside's own timer interrupt, armed by `Outside::turn` for the slice. Nothing
  leaves the virtual machine. Measured: Hypervisor.framework's virtual timer through its GIC costs 290 ns a
  tick at a 10 µs period with no exit to the host (mac-tests.jsonl "hvf_guest_timer", mode gic); on the
  Linux laptop the local APIC timer under KVM costs 7.8 µs a tick at 100 µs with 3.1 exits per tick
  (my run, experiments in the status file).
- **A turn that must end at an exact count.** With no check at counted events, nothing can stop a thread
  when its count reaches a number at a pc not known in advance. The native back end therefore ends a turn
  exactly only at a position (section 5.7). `Turn::Counted { limit }` (rt/lib.rs:25-31), whose limit is a
  count alone, cannot be honoured natively; R4 (section 6.2) makes a turn end at a position or when
  stopped, and the test outside's seeded runs are checked by recording on one output and replaying on the
  other (section 8). Until R4, the native back end answers a counted turn with a limit as `Failure`
  (named "turn limited by a count alone"), never by running past it.
- **An event from outside** is a real interrupt raised from a host thread: Hypervisor.framework's
  `hv_gic_set_spi` (Apple's documentation, macOS 15 and later; the round 3 prototype made the same GIC with
  `hv_gic_create`, hvfrun.c, but raised only its timer through it), KVM's `KVM_IRQFD` or `KVM_IRQ_LINE` with
  the in-kernel interrupt controller (Documentation/virt/kvm/api.rst). Neither is tried yet; N1 and N7 do,
  and measure the time to arrival. It lands in the inside's vector at an instruction boundary, and section
  4's map makes the position exact.
- **The host stopping the whole machine** (a save, ending the container): `hv_vcpus_exit` or a signal to the
  virtual CPU's thread with `immediate_exit`. Measured: 420 stops with Hypervisor.framework, pc always where
  it should be (mac-tests.jsonl "hvf_stop_by_hv_vcpus_exit"); on KVM 420 stops each way, pc always right,
  9.9 to 11.8 µs median from the decision to the registers read, with a tail to 4 ms (my run).

### 5.6 The link

One call shape: the inside writes a request in a page shared with the monitor and rings a doorbell; the
monitor answers in the same page and resumes the virtual CPU. The doorbell is the floor's one
platform-specific instruction: `hvc` on Hypervisor.framework (an exception exit, used by the round 3
prototype, hvfrun.c and guest.s), an `out` to a port on KVM x86-64 (`KVM_EXIT_IO`, used by vmrun.c), and a
store to an unmapped guest-physical page on KVM ARM64 (`KVM_EXIT_MMIO`, from KVM's documentation,
Documentation/virt/kvm/api.rst; not yet tried: no KVM ARM64 machine is at hand). Requests are typed messages
(the `Host` calls, translate, more memory, give back, idle). Measured exit cost 0.7 µs on the M5 and 5.6 to
6.2 µs on the Intel laptop (brief).

### 5.7 Replay to a logged position

To stop at (count N, pc P): the inside asks for P's piece again with a check at P (the translator's one
operation, "translate from this address", with a stop point added to the request; the stop point is in the
cache key, so such translations are kept apart), or patches the first host instruction of P's run into a
branch to a check stub when P starts a run. The check compares the count with N without touching flags (ARM64: N
moved into a withheld register, `sub` and `cbz`; x86-64: a form that keeps the guest's flags, chosen at
N8) and branches to the inside when equal; elsewhere nothing changes. Since between two counted events pc
only rises, P is reached with count N exactly once. Once there, the original piece comes back.

**Across outputs.** Both outputs count the same events with the same function (section 4), on the guest's
own instructions, and report positions at guest instruction boundaries. So:
- a position the WebAssembly output logged is a boundary, which the native output reaches by the check at
  that pc;
- a position the native output logged (an interrupt anywhere) is also a boundary after section 4's
  roll-back. The WebAssembly output reaches it the same way: its translator, asked for P's piece with a stop
  point, emits a check before P's instruction (at look-ahead none every instruction is its own piece, so
  this always exists). This is the same request in both outputs, which runtime.md section 12 left as "step
  at look-ahead none until pc matches"; that needs the count to be reachable with a limit first, and the
  stop point needs nothing else.
Asked of the translator track: the stop point in a translation request, and the count with the shared
`counted` function (T2, T3).

### 5.8 Making, saving and bringing back a virtual machine

- **Making**: the monitor reserves guest-physical memory, creates the machine and one virtual CPU, maps the
  inside at its link address and starts it at its entry with the start arguments in a page (measured to make
  a machine: 0.6 to 0.8 ms on KVM, 0.1 ms on Hypervisor.framework, brief).
- **Saving**: with every virtual CPU stopped, the guest-physical memory (the kernel's heap, every process's
  frames and page tables, every thread's block) and each virtual CPU's registers. The kernel's whole state is
  then one memory, as the kernel plan wanted; translated code is not saved (the code area's frames are
  left out and the inside drops its loaded translations on bringing back).
- **Bringing back**: a new machine of the same architecture, the memory put back, the registers set, then the
  inside's restore path, which makes every thread run from its registers (`Resume::FromRegisters`, the
  kernel's `restore`) and re-arms its timer from the monitor's clock offset.
- **Probing**: KVM is used when `/dev/kvm` opens and `KVM_CREATE_VM` works; Hypervisor.framework when
  `hv_vm_create` works (it needs the binary to carry com.apple.security.hypervisor, measured working with an
  ad hoc signature, brief).

### 5.9 One interface of ours over KVM and Hypervisor.framework

A trait in the monitor's crate, resolved at compile time per host (a host has one of the two):
`Hypervisor` with `Machine` (map and unmap guest-physical memory, raise an interrupt, make a virtual CPU) and
`VirtualCpu` (run until an exit, read and write registers, be stopped from another thread). Exits are one
enum of ours: doorbell, fault the inside could not take, stopped. KVM ARM64 and KVM x86-64 differ only in
registers and the doorbell; Hypervisor.framework differs in the call names. Each method has one body per
platform; nothing above the trait names a platform.

## 6. The runtime's execution side, without a second loop

### 6.1 Where the loop runs

The loop is `Stepper::step` (rt/lib.rs:196-225), which calls `cpu.run` with an answer that calls
`kernel.exit`, `outside.arrived` and `kernel.next` at every exit (rt/lib.rs:263-289). With the kernel inside
the virtual machine, the loop must run there too, or every system call would leave the machine (0.7 to
6.2 µs against 21 to 32 ns inside, brief). So the same `Stepper` runs inside, built `no_std`, with the native
`Execution` and an `Outside`. On the host, the container's `step` and `run` (rt/container.rs:147-163) enter
the virtual machine and come back with the `Step` the loop inside returned; `Idle` comes back to the host
only when the inside has nothing to wait on itself (with a timer armed, the virtual CPU halts inside).

### 6.2 What changes, and who owns it

- **R1 (runtime track): the loop and its traits `no_std`.** `Turn`, `Carry`, `Leaving`, `Answer`,
  `Execution`, `Outside`, `Step`, `Stepper` move into a part of the runtime crate built without the standard
  library (with `alloc`); `StopFlag`'s `Mutex` and `Arc` (rt/lib.rs:35-89) stay in the std part. No behaviour
  changes.
- **R2 (runtime track): the container steps a loop that may be elsewhere.** `Container<B, O>` holds the
  kernel, the back end and the outside and calls `Stepper` on them (rt/container.rs:137-163). The change
  makes the place a type the back end names: here (today's fields), or a virtual machine whose `step` enters
  it. `BackEnd` (rt/container.rs:19-33) keeps its methods; `new` of the native back end makes the machine.
  The user surface (`Runtime::container`, `step`, `idle`, `run`) does not change.
- **R3 (runtime track): outsides that can run inside.** The `Outside` runs where the loop runs. The shipped
  outside (rt/system.rs:230-302) splits into its policy (the turn's deadline, `ran`, `arrived`, `idle`,
  which are a few lines each) built `no_std`, and the host services under it (output, files, network,
  randomness, sleeping, the wall clock) behind one trait the policy calls: called directly on the host for
  WebAssembly, through the link inside the virtual machine. The test outside (kernel test crate) splits the
  same way, so the same seed gives the same turns and events on both back ends. Rejected: forwarding every
  `Outside` call to the host (one exit for `ran`, `arrived` and `turn` at every system call).
- **K1 (kernel track): nothing in the kernel's interface.** It is driven by the same `Cpu` and `Host` traits.
  The kernel's clock method asked for in runtime.md section 4 (`Context::monotonic` made public) is used by
  the inside's counter reads as by the WebAssembly back end.
- **K2 (kernel track, only if the owner agrees, section 10): an ARM64 container's address-space size
  2^45** on every back end, in place of the 2^48 runtime.md section 8 gives it, for the entry mirror of
  section 3.2. The same value everywhere keeps the outputs identical; it differs from the reference only for
  a program that asks for a fixed address at or above 2^45.
- **T1 (translator track): the finder apart from WebAssembly** (section 3.1), so the native emitter uses it
  without wasm-encoder, and a WebAssembly-only build has no native emitter, nor a native build the
  WebAssembly one unless it asks.
- **T2 (translator track): the shared `counted` function** and the count in the WebAssembly output from it.
- **T3 (translator track): a stop point in a translation request** (section 5.7).
- **X1 (x86 track): `Native` for x86-64** follows the x86 decoder's forms; this track writes it with the x86
  track's routing table as its source, as `Native` for ARM64 follows arm64's.
- **R4 (runtime track): a turn ends at a position or when stopped.** `Turn::Counted { limit }` becomes a
  turn that ends at a (count, pc) or has no end but a stop; the progress handed with each exit stays. The
  WebAssembly back end reaches a position with the same stop point (T3). The test outside's seeded turns
  become seeded stops whose positions are recorded, so a seed still repeats on one back end and the record
  repeats on every back end. This changes an interface: the owner's (section 10).
- **The other project's views of kernel state** (de-requirements-map.json, "Borrowed views of kernel
  state"): with the kernel inside a virtual machine, a host program cannot borrow its data. Whatever reads
  views must be built into the inside or receive copies through the link. Nothing here forbids either;
  it is the owner's to know (section 10).
- **A1 (this track): the toolchain targets** `aarch64-unknown-none` and `x86_64-unknown-none` in
  rust-toolchain.toml.

### 6.3 The native `Execution`

`run(thread, resume, turn, answer)` (rt/lib.rs:132-138) inside: applies the resume to the block, loads the
withheld registers, arms the timer for the turn, drops to EL0 or ring 3 at the thread's piece and takes every
exception that comes back:
- a system call (the routed instruction or `svc`/`syscall` itself) is `Exit::Syscall`, handed to
  `answer.exit` at once, with no exit from the virtual machine; "carry on" returns to EL0 at the next
  instruction, as the WebAssembly back end's hook returns;
- a page fault is settled by the back end's write locks (copy-on-write, tracking, translated code) and
  carried on, or is `Exit::MemoryFault` with the guest's address and access;
- an instruction abort in the code area is a translation miss (section 3.2); one in the guest's half is
  `Exit::MemoryFault` with `Access::Execute`;
- the timer or a raised interrupt is `Exit::Stop`;
- a routed trap is the `Exit` of its `TrapKind`; alignment faults are `Exit::AlignmentFault`.
Registers are exact at every exit (section 4), so `Exit::MemoryFaultWithoutRegisters` never happens here.
`restored()` makes the same inside state over the saved memory with no loaded translations.

## 7. What is refused, not built, or the owner's

### 7.1 Not built, by name

- Running any guest instruction untranslated.
- Protection keys (brief).
- The in-process native form and the separate-process form (brief): not built and not ruled out; the one
  `access` function is where they would differ.
- Windows Hypervisor Platform (not in the ruling).
- Devices, firmware, a second operating system in the virtual machine: the inside is the only program.
- Large guest pages: the guest has 4 KiB pages only (brief); the host may back memory as it likes.
- Features beyond the translator's offer, on any host (section 3.4).
- 32-bit guests (AArch32, x86-32).
- More than one virtual CPU until the owner rules on parallel guest threads.
- Saving translated code with a machine (it is a cache).
- Nested virtualization inside the guest; KVM inside the container.

### 7.2 Where native behaviour could differ from the WebAssembly output

- **Reading the code area on x86-64.** x86-64 has no execute-only pages without protection keys, so a
  guest load from an address in the code area (upper canonical half) reads our code where the WebAssembly
  output reports a memory fault, as Linux would. The thread page (ARM64 and x86-64) is likewise reachable by
  a guest that addresses the inside's half on purpose; it holds only that thread's own state. No
  correct program does this; it is still a difference, so it is the owner's (section 10).
- **Store exclusive** (section 3.4): handled, to be proven at N5.
- **Floating point**: native ARM64 on ARM64 computes what Arm defines, which the WebAssembly output also
  produces (bit-exact not-a-numbers, translator plan); the native output never changes FPCR outside the
  presented bits.
- **A program reading the counter in a loop** sees the kernel's clock on both outputs.

## 8. Tests and the pass bar

- **The same suites.** The kernel's layer 2 and layer 3 (the Linux Test Project) with the runtime and the
  native back end behind the boundary, with the results the WebAssembly back end has, on the Mac
  (Hypervisor.framework, ARM64) and on the Linux laptop (KVM, x86-64, once the x86 guest passes its own
  bar); the kernel's layer 1 random sequences over the native `Cpu` side built for the host (its page-table
  writer and walker are plain Rust over a byte buffer standing for guest-physical memory, so they need no
  virtual machine), in place of the buffer CPU; the translator's layers run with the native output against
  the reference.
- **Routing.** The recording-emitter test of section 3.4, for every form of each guest crate.
- **Positions.** The every-offset test (section 4). Seeded: the seed and the number of cases are arguments,
  printed with every failure.
- **Replay across outputs.** Seeded programs and the translator's benchmark programs are run with seeded
  events delivered at seeded interrupts (native) or stop checks (WebAssembly), the outside recording every
  input and position. Each record is replayed on the other output and on the same one, and at every event
  the registers, the count and a hash of every process's memory must be equal. One command, with the seed
  and the number of runs as arguments.
- **Speed.** The translator's benchmark programs (the eight bench.c workloads and calls.c of the cost
  rounds) and real programs from the kernel's layer 2 (busybox, python, git), each run plainly on the host
  and through the native back end, alternating, 15 pairs or more, instructions retired and time, with the
  machine and its load recorded. The guest's own computing inside a virtual machine was measured equal to
  plain native (round 3, 4 KiB pages: instructions 1.000 to 1.004, time within noise,
  mac-summary-main.json), so what the bar tests is the translation and the count. Proposed bar: geometric
  mean time within the count's own measured cost plus 10 points (round 3 measured the count forms at about
  +15% time on the M5, summary-main-r0.txt), and system calls answered inside at under 100 ns. The bar is the
  owner's (section 10).

## 9. Milestones

Each milestone ends with its bar run and recorded in docs/status-native.md.

- **N1, the floor on Hypervisor.framework.** The inside (floor only) boots at EL1 under a monitor using
  `applevisor`; 4 KiB-granule tables; an EL0 program (hand-written, not translated) makes doorbell calls;
  timer and raised interrupts land in the inside; execute-only EL0 pages and never-executable guest pages
  fault as section 3.2 needs. Bar: a test binary (signed with the hypervisor entitlement) runs each of these
  and checks the exception syndrome and address of every fault; the doorbell round trip and an interrupt's
  arrival are measured.
- **N2, the first real program.** The kernel and the `no_std` loop inside (R1, A1, done by this track if the
  runtime track agrees, or waiting on it), the native ARM64 output at look-ahead none (copy, rebase, route,
  count), translation on the host through the link. Bar: busybox `echo` and `sh -c` from the kernel's layer 2
  run inside Hypervisor.framework with the reference's output; the dynamic register uses of section 3.3 are
  counted and the withheld registers chosen.
- **N3, linking and speed.** Bounded and full look-ahead, linked exits, the entry mirror, the call stub, the
  cache with the native key and disk files. Bar: the speed method of section 8 on the Mac, recorded per
  program, against plain native.
- **N4, the execution side.** `Execution` and `BackEnd` for native (R2, R3), processes, fork with
  copy-on-write, threads on one virtual CPU, shared blocks, write locks, code a program writes (Node and Java
  from layer 2), the every-offset test. Bar: kernel layers 2 and 3 on the Mac with the WebAssembly back end's
  results; layer 1 over the native `Cpu` side.
- **N5, replay.** The replay check, the record and replay outside, store exclusive under interrupts. Bar: the
  cross-output replay test both ways, with the number of runs recorded.
- **N6, save and bring back.** Saving the machine and bringing it back in a new process. Bar: the kernel tests
  that save (procsignals and the rest of layer 2's saving tests) pass with saves of the whole machine.
- **N7, KVM x86-64 floor.** N1 again on the Linux laptop with `kvm-ioctls`, ring 0 and ring 3.
- **N8, x86-64 native output.** `Native` for x86-64 (X1), `iced-x86` re-encoding, the lookup cache, `rep`
  and the count. Bar: N2 to N5's bars on the laptop, once the x86 guest passes its own bar.
- **N9, KVM ARM64.** The ARM64 inside under KVM. Bar: N4's bar on an ARM64 Linux machine with KVM (one must be
  found or rented; none is at hand).
- **N10, the other project's needs.** Dirty pages and point-in-time copies as `Execution` methods once a user
  exists (runtime.md section 12), and the change of back end for a running process (stop, save registers and
  memory, continue on the other output).

## 10. Decisions for the owner

1. Where the `Outside` runs: inside the virtual machine with the loop (R3), with the shipped and test
   outsides split into a `no_std` policy and host services? Recommended: yes; the alternative leaves the
   machine at every system call.
2. Names: the monitor crate (`wcontainers-hypervisor`?), the inside program (`wcontainers-inside`?), the
   native back end (`wcontainers-native`?), and whether the native emitter is a module of
   `wcontainers-translator` behind a cargo feature (recommended) or a crate of its own.
3. The translator split into a finder and two emitters behind features (T1): agreed? Recommended: yes.
4. x86-64's readable code area and the reachable thread page (section 7.2): accept as a named
   difference, or allow protection keys for this one purpose on x86-64? Recommended: accept and record it on
   the kernel's written list of differences; no correct program sees it.
5. Licences: `kvm-ioctls` (Apache-2.0 OR MIT), `kvm-bindings` (Apache-2.0), `applevisor` (MIT OR
   Apache-2.0), `aarch64-paging` (MIT OR Apache-2.0), `x86_64` (MIT/Apache-2.0), `aarch64-cpu`
   (MIT/Apache-2.0), `talc` (MIT), `iced-x86` (MIT). `dynasmrt` (MPL-2.0) rejected. Agreed?
6. The speed bar of section 8: the count's measured cost plus 10 points, and system calls under 100 ns?
7. A guest of the other architecture on a native host (section 11): which option?
10. Turns that end at a position or when stopped, in place of a count limit (R4)? Recommended: yes; it is
   what the ruling of no check at counted events implies for every back end.
11. The other project's borrowed views of kernel state: built into the inside, or copied out through the
   link, when the native back end is used? Recommended: built into the inside, as the kernel's own code.
8. Windows Hypervisor Platform: left out, as the ruling names only KVM and Hypervisor.framework? Recommended:
   left out until asked.
9. An ARM64 container's address space 2^45 (32 TiB) on every back end (K2), so a jump through a register
   costs five instructions with no memory read, or 2^48 as the reference, with a table lookup on every jump
   through a register and every return? Recommended: decide after N3, which measures both; our kernel places
   mappings low first, so 2^45 is expected to bind only programs that ask for high fixed addresses.

## 11. A guest of the other architecture on a native host

Options, with what each takes:
1. **The WebAssembly output under wasmtime, as today.** Takes nothing. Speed is the WebAssembly output's.
   Recommended until one of the others is asked for.
2. **A cross-architecture native output** (x86-64 to ARM64 and ARM64 to x86-64), run in the same virtual
   machine with the same kernel. Takes a real compiler back end per host: the meanings would be compiled to
   host code (Cranelift, Apache-2.0 WITH LLVM-exception, is the established crate; our own register
   allocation and flag handling otherwise), the count and position map as here, and, for x86-64 guests on
   ARM64, the x86 memory ordering question the owner has not ruled on. Identity with the WebAssembly output
   follows from compiling the same meanings. The largest option; the only one that keeps every rule.
3. **The WebAssembly output compiled by wasmtime inside the virtual machine**, with our kernel there: the
   guest code is the existing WebAssembly, compiled ahead on the host by wasmtime for the host and loaded
   into the code area. Takes running wasmtime's compiled code without its runtime (its traps, its stack
   checks and its imports become the inside's), and the hooks and memory checks stay. Keeps identity by
   construction, gains the in-machine system call (about 30 ns against the WebAssembly back end's measured
   10.5 ns hook plus the kernel's 116 to 120 ns, runtime.md section 10), but not compute speed.
4. **An existing user-mode emulator as a guest program** (FEX-Emu or Box64, x86-64 on ARM64; their
   licences to be checked before any trial): the
   x86 program runs under the emulator, an ordinary ARM64 Linux program on our kernel. Takes nothing of ours,
   but the guest is then the emulator, not the x86 program: its offer, its system calls and its timing are
   the emulator's, so behaviour is not the x86 WebAssembly output's and replay records the emulator.
   Recommended against for anything but a trial.
5. **Rosetta or QEMU**: refused. Rosetta for Linux is Apple's, reached only through a
   Virtualization.framework share into Apple's own virtual machines, not ours to ship (from Apple's
   documentation of that framework, not checked further); QEMU's translator is GPL-2.0.
