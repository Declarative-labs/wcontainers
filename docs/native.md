# wcontainers: the native back end

Status: plan, 2026-11-18 (N0 and its five checks; corrected at the start of N1 for the last check's findings
and the orchestrator's rulings: the count is E, K2 carries the memory size, routing is the default for
host-model results, the owner's names). The names the owner has chosen are used: `Executor` (today's
`BackEnd`), `System` (today's `Outside`), `LocalSystem` (today's shipped outside), `GuestMachine`,
`TranslatorSettings`, `TaskId`. Lines cited in code are at 68013b31, where the old names still stand.

Words used here:
- **native output**: the translator's second output, machine code of the guest's own architecture.
- **virtual machine**: one hardware virtual machine made through KVM (Linux) or Hypervisor.framework (macOS).
- **inside**: the one program we run in the virtual machine's privileged level: our kernel (one instance per
  container), the runtime's loop, the native back end's own part and the layer under them. **floor**: that
  layer under them (entry, page tables, interrupts, timer, the link out).
- **monitor**: the host program's part that makes the virtual machine, runs its virtual CPUs and answers
  what the inside asks of the host.
- **link**: the one way the inside asks the host for something (section 5.6).
- **count**: the progress count of runtime.md section 9, defined on the guest's own instructions.
- **position**: a (count, pc) pair.
- **reference**: real Linux of the guest's architecture, as in runtime.md.
- **slot address**: where the entry mirror (section 3.2) holds the slot of a guest address.

Paths are shortened: `gcpu/` is crates/wcontainers-guest-cpu/src, `arm64/` crates/wcontainers-arm64/src,
`tr/` crates/wcontainers-translator/src, `rt/` crates/wcontainers-runtime/src, `abi/` crates/wcontainers-abi/src,
`kernel/` crates/wcontainers-kernel/src, `ktest/` crates/wcontainers-kernel-test/src, `x86 plan` docs/
phase-2-guest-cpu-and-x86.md on origin/phase-2-x86 at 96c49dbb. Lines are at 68013b31 (this branch).
Experiments are under pyro-parked/wcontainers-scratch/native (`experiments/` mine, `check-n0/` the
independent check's) and are listed with their results in docs/status-native.md. Every figure says whether
it was measured (and where) or is only expected.

## 1. What the native back end delivers

A guest of the host's own architecture runs inside a virtual machine at full speed, translated (same
architecture in and out), with our kernel inside the same virtual machine, so a system call is answered
without leaving it. Platforms: KVM on Linux for x86-64 and ARM64 hosts, Hypervisor.framework on macOS
(ARM64). Where the machine does not let an ordinary program use virtualization, or the host lacks a feature
the guest is offered, the WebAssembly back end runs as today; which one runs is chosen at start by probing
the host (section 5.8).

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
  (section 3.4). Section 7.2 names what still differs.
- One loop, one kernel crate driven through `Cpu` and `Host`, one execution interface. Nothing here adds a
  second loop or scheduler.
- No stop check in translated code. One add per counted event. A check only at a replay target (and, if the
  owner chooses option B of decision 10, at counted events of a container whose turns end at a count).
- Every number this plan names (the mirror's mask and shift, the slot size, the address width, register
  numbers, offsets into the thread page) is, in code, one named constant derived from the fact it describes
  (the brief's hard rule); the figures here are for the reader. The guest's page size is
  `wcontainers_abi::PAGE_SIZE` (abi/memory.rs:6), its one definition: the floor's translation granule, the
  frame size, the copy-on-write unit and every "4 KiB" below are that constant in code, so a later 16 KiB
  mode changes it there (section 5.2 says what the floor does then).
- Every slow path of the native back end has a name, a counter and a warning (section 3.7).

## 3. The translator's native output

### 3.1 Shared with the WebAssembly output, and new

The translator today is one crate whose finding and emitting are both in it:
- Shared as it is, from the guest interface: `Personality` (gcpu/lib.rs:24-38), `Guest::decode`, `flow`,
  `accesses`, `stack_move`, `built_address`, `jump_table`, `plain`, `SYSCALL`, `CODE_CHANGES`, `trap_kind`,
  `memory_fault`, `REASONS` and the guest's offer (gcpu/translate.rs:11-224). None of these writes
  WebAssembly.
- Shared after a move inside the translator crate: what finds code: `Code` and `Decoded` (tr/code.rs:
  520-566), the `Finder` with `functions`, `stretches`, `explore` and the `Means` of every start
  (tr/full.rs:228-257 and :299-604; `flags_word_function` at :258-264 and `flags_word_functions` at
  :269-296 build WebAssembly functions and are not part of it), `LookAhead` and `Optimise`
  (tr/code.rs:21-45), and the piece list a translation hands back (`CodeModule::pieces`,
  tr/code.rs:122-130). Today the finder is `pub(crate)` and built inside `plan` together with the
  WebAssembly plans (tr/full.rs:44-113). Change T1 (section 6.2) moves it into a module of its own that both
  emitters call.
- The finder is not free of WebAssembly, and stays so: `Code::decode` gives every instruction its `effects`
  (tr/code.rs:543-548), which `wcontainers_guest_cpu::effects` finds by running the instruction's meaning
  into a recording emitter (gcpu/effects.rs:43-64, `G::emit` at :62). So a native build carries the meanings
  and wasm-encoder (the guest crate's `translate` feature, crates/wcontainers-guest-cpu/Cargo.toml:18), at
  build time only: no WebAssembly is made or run on the native path. Rejected: a second, native description
  of what each instruction reads and writes (two definitions of one fact, which would drift apart).
- `effects` takes `G::Host`, the probed behaviour of the WebAssembly engine (gcpu/translate.rs:124-131;
  for ARM64 whether relaxed fused multiply-add fuses and whether not-a-number results follow Arm's rules,
  arm64/host.rs:18-24, read by the float meanings, arm64/float.rs:235-249). The native finder passes the
  value that describes the hardware itself (ARM64: fuses, Arm's not-a-numbers), which is what the native
  output computes. Effects are registers, flags, calls and system calls (gcpu/effects.rs:6-18), and the host
  value changes only whether a float meaning computes by its hook or directly, not what it reads and writes.
  Whatever both outputs must agree on, the count above all, is decided from each guest instruction, its
  `flow` and its own address alone, never from `effects` and never from what else the finder has found
  (section 4); T2's test runs `counted` under every `G::Host` value and requires the same answer.
- WebAssembly only, unchanged: `Guest::emit` and the `Emitter` trait, which hand meanings a wasm-encoder
  `InstructionSink` (gcpu/translate.rs:158, :246-316), the plans (registers as locals, flags as the last
  comparison), the contract, the thread module and the state memory (tr/contract.rs, tr/thread.rs,
  tr/state.rs, tr/layout.rs). The meanings stay the definition of what every instruction does; the native
  output's routing is proven against them (section 3.4).
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
today copies the instruction as it is (or puts the alignment test of section 3.4 in front), because the
virtual machine's page tables enforce the guest's permissions. Every route that emits a guest access emits it
through `access`: copied loads and stores, the Aligned route's accesses, the Rebase route's literal loads,
the Exclusive route's load, `cas`, `casp` and `stadd`, the Float status route's copied save and restore, and
every x86-64 instruction re-encoded with a stand-in register (section 3.3), whose memory operand `access`
emits. Only translated code's accesses to the thread's own pages of ours (section 3.3: the stand-in moves,
the lookup cache) are not guest accesses and do not go through it. An in-process form (guest memory
rewritten) would change only that function (brief).

### 3.2 Pieces, the code area and linking

- **The code area.** Translated code lives in the inside's half of the address space (ARM64: the TTBR1 half;
  x86-64: the upper canonical half), mapped executable for the guest's level and, on ARM64, execute-only for
  it (stage 1 AP[2:1] = 00 with UXN = 0, PXN = 1). Guest memory (the TTBR0 half, the lower canonical half) is
  mapped never-executable for the guest's level (ARM64 UXN = 1; x86-64 NX = 1). Measured under
  Hypervisor.framework on this M5 (check-n0/xo.out): EL0 runs an execute-only page (case 1), a load from it
  faults with a permission fault, both pc-relative and through a register (cases 2 and 3, ESR_EL1 EC 0x24
  FSC 0xf), a store to it faults (case 5), and EL0 cannot run a UXN page (case 4, EC 0x20). The floor reports
  any guest fault in the inside's half to the kernel as a fault at an address where nothing is mapped, which
  is what the WebAssembly output and Linux report there.
- **Pieces** are what the shared finder gives at the container's look-ahead, as for WebAssembly: one
  instruction (none), a stretch (bounded), or a function (full). A piece is the guest's instructions in
  order, each carried over by its route (section 3.4); branches inside a piece are direct.
- **A loaded translation** is the code for one range of guest bytes at one guest address. On disk (the cache)
  a translation does not depend on the address: it carries a list of the places that hold a guest address.
  Loading it into the code area writes those places for the address the range sits at (ARM64: `movz`/`movk`
  immediates; x86-64: 64-bit immediates), so emitted code reads no data of ours to know where it is. Its
  frames are shared by every process of the container that maps the same bytes at the same address (a fork's
  child; the same program run twice, since our kernel places mappings repeatably, top-down from a fixed
  mmap base, kernel/memory.rs:260 and :407). Rejected: one base word per loaded translation read
  pc-relatively (an execute-only page cannot be read, check-n0/xo.out case 2) or from a page the guest's
  level can read (one more page of ours the guest could read where Linux faults, section 7.2).
- **Exits linked to each other.** A direct branch to a piece of the same loaded translation is patched into a
  direct branch once the target is translated (one aligned 4-byte write on ARM64, a rel32 on x86-64, while no
  virtual CPU runs that code, then the architecture's instruction-cache maintenance). A branch to a guest
  address outside the loaded translation goes through the process's own lookup (the mirror below, or x86-64's
  cache), since another process sharing these frames may map something else there.
- **Jumps through a register and returns, ARM64.** The entry mirror holds a 16-byte slot for every guest
  address G below 2^45, at the slot address `mask | (G << 2)`, `mask` being the logical immediate
  0xffff_8000_0000_0000; the mirror fills the top half of the TTBR1 half and the inside and the code area sit
  in its lower half. A slot holds its piece's address made in three moves and a `br` (it reaches any piece);
  a slot with no piece holds `udf`; a slot page not yet needed is not mapped. The mirror is per process
  (section 5.3). A jump through xN is:
  `ubfx t, xN, #45, #11; cbnz t, bad; ubfiz t, xN, #2, #45; bfxil t, xN, #0, #2; eor t, t, #mask; br t`
  (t a withheld register, section 3.3; no memory read, no flags touched; assembled and run on this M5 for
  aligned, misaligned, tagged and out-of-range targets, experiments/n0r2/mirror.out). Behaviour against the
  WebAssembly output, which untags every branch target and traps a misaligned one (arm64/guest.rs:736-738,
  arm64/meaning.rs:543-557, tr/thread.rs:220-242):
  - Tag: the top byte (bits 56 to 63) is never looked at, so a tagged target lands where its untagged
    address does, as the WebAssembly output's `untag` and Linux's Top Byte Ignore make it. A target with bit
    55 set goes to `bad`, as does one at or above 2^45; `bad` reports a fault at the target as `untag` gives
    it (the whole address when bit 55 is set), which is where the WebAssembly output faults.
  - Alignment: `bfxil` keeps the target's low two bits in the slot address's low two bits, so a misaligned
    target makes the final `br` go to an address that is not a multiple of 4, and the hardware raises a PC
    alignment fault there. Measured under Hypervisor.framework on this M5 (experiments/n0r2/branch.out,
    cases B and C): a `br` at EL0 to a target 1 or 2 bytes off gives ESR_EL1 EC 0x22 with FAR_EL1 and
    ELR_EL1 equal to that target. The floor takes the guest address back from the slot address with
    `ubfx g, s, #2, #45` and reports `TrapKind::Alignment` there, as tr/thread.rs:220-242 does.
  - A missing slot (an unmapped slot page or `udf`) raises an exception at EL1, which finds the piece among
    the process's loaded translations or asks for a translation from that address (bounded look-ahead, as
    the WebAssembly output does at run time), fills the slot and carries on.
- **Calls, ARM64.** A guest `bl` or `blr` at G is made through G's own slot, whose last instruction is a
  `blr` to G's call stub, so the return-address predictor holds the slot address of G + 4. The stub makes x30
  the guest's return address with one instruction, `ubfx x30, x30, #2, #45`, and goes on to the callee (for
  `blr xM`, through the jump above on xM). The slot's `blr` is the call's commit point: section 4 says how
  an interrupt on either side of it is reported. A guest `ret` becomes the jump above ending in `ret t`, which
  the predictor then foresees, and the slot of G + 4 goes back into the caller's piece. Entering at G itself
  (a resume after a signal, a jump to a call instruction) runs the same slot, so it is the same call.
  `blr x30` is made in the plain form (the target taken into t by the jump above, then x30 built in place as
  G + 4 by `movz`/`movk`, then `br t`), since the slot's `blr` would overwrite the target; the first `movz`
  into x30 is the commit point, and section 4's constant fix-up finishes x30 if an interrupt lands among
  the `movk`s. N4 measures this against plain calls for every call
  and keeps the faster exact one.
- **The mirror's reach.** The mirror covers guest addresses below 2^45 (32 TiB), so the native ARM64 back
  end's address ceiling (`Cpu::address_space`, abi/cpu.rs:34-35) is 2^45, under the container's width of 2^48,
  as the brief's ruling on ceilings allows (section 6.2, K2; decision 9).
- **Jumps through a register and returns, x86-64** (instructions of any length, 47-bit addresses): a
  per-thread cache of (guest address, piece) pairs, direct mapped, in the thread's read-only page (section
  3.3), in front of a per-process table; a miss enters ring 0, which fills the cache. A misaligned target
  does not exist on x86-64 (any byte may start an instruction). The x86-64 call, return and indirect-jump
  sequences are not given in this plan; N9 designs and measures them. What is known now, which any
  sequence must meet: a guest `call` puts its 64-bit guest return address on the guest stack, which `push
  imm32` cannot (it sign-extends), so it is `mov t, imm64` then `push t`, the push being the commit point;
  the guest's flags at an unknown target cannot be shown dead, so the lookup keeps them always, which needs
  a flag-free compare (`jrcxz` through a stand-in, `lahf` and `sahf` around a `cmp`, or `lea` arithmetic
  ending in a `jrcxz`; N9 measures which); and the host's return-address predictor cannot be used, since a
  host `ret` would read a host address from guest memory. N9 measures the lookup against a plain
  per-process table lookup and keeps the cheaper exact one.
- **Only our own host addresses are ever jumped to.** Every host address translated code goes to is a
  direct branch the translator wrote, a mirror slot computed from the guest address (ARM64; a slot's own
  `br` to a piece), or a cache or table entry the inside wrote. None is read from memory the guest's level
  can write (section 3.3), so a guest store cannot steer a jump into the middle of our code, and no host
  instruction runs that was not emitted for a guest instruction's route.
- **Where translation happens.** On the host, in the monitor, with the translator as it is (std, rayon,
  the cache of runtime.md section 7). The inside asks through the link (section 5.6) with the range's bytes
  in guest memory, its starts and the settings; the monitor answers with the code placed in frames the
  inside gave. Why: translating takes milliseconds to seconds (runtime.md section 7) against an exit of
  0.7 µs (M5) to 6.2 µs (old Intel) (brief), and the translator, its threads and the disk cache need the
  standard library. Rejected: the translator built `no_std` inside the virtual machine (one virtual CPU to
  translate on, no disk cache, and a large change to the translator).
- **Written code is made visible before it runs.** Every write of host code or of a slot, by the inside or
  by the monitor into frames the inside gave, is followed, before any virtual CPU may run it, by the
  inside's maintenance over the written range: on ARM64 `dc cvau` for each data cache line, `dsb ish`,
  `ic ivau` for each instruction cache line, `dsb ish`, `isb` (line sizes from CTR_EL0 as the virtual CPU
  reports it; `ic ialluis` in place of the lines for a range larger than a bound N4 measures), broadcast so
  that every virtual CPU sees it; on x86-64 the exit that brings the answer back, a serializing return, on
  the virtual CPU that asked, and an interprocessor interrupt to the others once more than one runs one
  process. This covers every kind of write named here: loaded translations from the monitor, load-time
  addresses (section 3.2, above), slots filled or emptied, linked exits, and replay's translations (section
  5.7). Why: Hypervisor.framework does not make the guest's instruction cache coherent with the host's writes.
  The independent check measured it (check-n0c/icache_late.out: code the host wrote into fresh guest memory
  after the guest had run code a few lines away ran as the stale zeros, taken as `udf` at EL1, in 18 of 24
  cases; 0 of 24 when written before the first run), and a stale `udf` in a slot would be taken for a missing
  slot again and again.
- **The cache.** As runtime.md section 7, with the back end in the key ("native, ARM64 on ARM64" or "native,
  x86-64 on x86-64"), the guest's offer, the withheld registers' choice and the inside's build identity.
  The emitted code relies on no host feature outside the offer except LSE on ARM64 (its own compare-and-swap,
  section 3.4), and the probe (section 5.8) refuses a host without them, so host features are not in the
  key. Files on disk are shared between runtimes of the same build (the other project's "translated code
  cached per backend as files", design.md section 6). Inside the virtual machine a loaded translation is
  found by its key's hash and its guest address (above).

### 3.3 Registers the translated code keeps for itself

Translated code needs a few registers the guest cannot see: the count (section 4), a scratch register for
jumps through a register and routed instructions, a place for the thread's stop word (section 5.7) that the
guest's level reads and only the inside writes, and a way to the thread's pages of ours, in the inside's
half, mapped only in the thread's own process (section 5.2):
- the **stop word**. ARM64: the hardware register TPIDRRO_EL0, which EL0 can read and only EL1 can write.
  The WebAssembly output answers every guest read of it with a fixed 0 (arm64/decode_forms.rs:246), so a
  guest read is the Constant route (section 3.4) and the hardware register is free; the floor writes the
  running thread's stop word into it at every switch. Measured under Hypervisor.framework on this M5
  (experiments/n0r4/tpidrro.c, tpidrro.out, three values): an EL1 `msr tpidrro_el0` does not exit to the
  host, an EL0 `mrs` reads the value, and an EL0 `msr` to it is undefined (ESR_EL1 EC 0) and leaves it
  unchanged. x86-64 has no such register, so its stop word is in the thread page below.
- the **thread page**, which the guest's level may read but not write: x86-64's stop word, lookup cache and
  the guest's kept GS base; on ARM64 only option B's limit word (decision 10), so ARM64 has no thread page
  under options A and C. The inside writes it through its own mapping of the frame. ARM64 has no stage 1
  encoding for "EL0 reads, EL1 writes" (AP[2:1] = 11 is read only at both levels; Arm Architecture Reference
  Manual, stage 1 data access permissions), and x86-64 with CR0.WP set refuses a ring 0 write to a read-only
  page (Intel SDM volume 3, section 4.6), so on both the inside writes through its own read-write mapping of
  every frame;
- the **register page**, which the guest's level reads and writes: the guest's own values of the withheld
  registers and ARM64's exclusive monitor (section 3.4), which translated code writes as the guest runs.
  Nothing read from it is ever used as a host address or decides where host code goes (section 3.2); a
  guest store to it changes only the guest's own registers or monitor (section 7.2).
Every guest instruction that names a withheld register is re-emitted through a stand-in, one rule on both
architectures. The guest value of the withheld register lives in the register page. The stand-in is the
scratch register t when the instruction names one withheld register and its route needs no scratch of
its own (t is loaded from the register page, the instruction runs on t, and t is stored back if the
instruction writes it: one load and at most one store). Otherwise the stand-in is a guest register the
instruction does not name: its own value is saved to the register page, the withheld register's guest
value loaded into it, the instruction run on it, then the result stored back and the stand-in's own value
reloaded (four moves). Either way the re-emitted instruction is the sequence's commit point, and every
move before or after it only relocates a guest value, which the position map records (section 4), so an
interrupt anywhere in the sequence is reported with every guest register exact. Counted at translation
time per site, as the WebAssembly output counts its slow-path sites (`CodeModule::sites`,
tr/code.rs:124-127), under "withheld register named by the guest" (section 3.7).
- x86-64: two withheld registers, the count and one scratch; the pages are reached through the GS base, so
  they need no register. The count is added to with `lea c, [c + 1]`, which keeps the flags. Round 2
  counted with it (costs-round-2.json, "measure native-x86-64", form E: a `lea` per counted event, +47%
  instructions together with that form's sparse checks); the count alone with no check is not measured on
  x86-64 yet (N9 does). A guest instruction naming a withheld register is re-emitted (with `iced-x86`) by the
  stand-in rule above, its moves being `mov`s with `gs:` operands (no flag touched). A stand-in can replace
  only a register the instruction names in its encoding, so both withheld registers are chosen among r8 to
  r15, which no form names implicitly except `syscall` and `sysret` (r11), both routed. Which two is chosen
  by measurement, by the same rule as ARM64: the fewest dynamic uses on the translator's benchmark programs
  and real programs (N9).
  Measured here (static, not dynamic): over eight x86-64 Linux files on the laptop (busybox, glibc 2.31,
  python3.8, git, libcrypto 1.1, xz, libpcre2, cargo; 5.3 million instructions), the fewest instructions name
  r11 (15.4 per 1000), r10 (18.7), r9 (21.5) and r8 (30.7); the other registers 40 to 211 per 1000
  (experiments/regs86.py, regs86.out). So with r10 and r11 withheld, about 34 instructions in 1000 name one
  (static), each costing two to four moves: this is expected to cost far more than ARM64's withholding, and N9
  measures it against one withheld register (the count) with the lookup's scratch saved to the register page
  around each lookup instead, keeping the cheaper exact one. The guest's own GS base is thread
  state the x86 plan's meanings read and write (x86 plan lines 1832-1838: `rdgsbase`, `wrgsbase`, and
  loading GS with `mov gs, r16` or `pop gs`), so natively it is kept in the thread page and every form that
  reads or writes it is routed (section 3.4), and `gs:` operands are re-emitted against the kept base. The
  guest's FS base is the hardware's own (copied `fs:` operands, `rdfsbase` and `wrfsbase` as they are), loaded
  from the block when the thread runs and saved at every exit.
- ARM64: three withheld registers (count, scratch, register page), the shape the round 3 prototypes used
  (costs-round-3/native-arm64/src/instrument3.py, x26 to x28). Which three is chosen by measurement, not
  now: compiled with three fewer registers clang lost little (geometric mean +0.3% instructions over the
  eight bench.c workloads, +2.7% on sort; summary-main-r0.txt line 114 and the sort row at line 55, "res3"),
  but a guest that names the withheld registers pays for each use. Measured here (static, not dynamic): over
  eight ARM64 Linux files (busybox, musl's loader, rg, xz, libpcre2, postgres, libcrypto, a Go program; 3.7
  million instructions) the fewest instructions name x18 (1.3 per 1000), x15 (2.9), x14 (3.9) and x13 (5.1);
  x26 to x28 are named by 13.7 to 22.4 per 1000, and the Go program names x27 by 58 per 1000
  (experiments/regs.out). N4 counts dynamic uses on the translator's benchmark programs and real programs
  and picks the cheapest three. Re-emitting a use follows the stand-in rule above.

### 3.4 How each instruction is carried over

`Native::route(instruction)` gives one of these, per decoded form (each guest crate's table):
- **Copy**: the bytes as they are (most instructions, and plain loads and stores, since the page tables check
  them). Only a form whose host instruction does exactly what its meaning does, for every input, may be
  copied; the two tests at the end of this section prove it per form, and the behaviour probe (section 5.8)
  checks on each host the cases that depend on the host's own model.
- **Split**: a load or store the hardware carries out as several accesses that a fault can stop part-way.
  The WebAssembly output checks the whole access first and then changes nothing when it faults: it writes
  no memory and loads no register (docs/status-translator.md:2594-2596, where the translator track records
  that the reference writes part of a 32-byte pair crossing onto a faulting page, Arm's rule leaving those
  bytes UNKNOWN, and that the WebAssembly output writes none). The hardware does not, measured on this M5 at
  EL0 (experiments/n0r5/partial2.out and partload.out, three runs of every boundary offset; the independent
  check's check-n0d/partial.out): every store or load of more than 16 bytes crossing onto a page that
  refuses it changes part before faulting, in whole 16-byte registers: `stp q`, `stnp q`, `st1` and `st4`
  of four registers write the registers wholly in the first page, and `ldp q`, `ld1` and `ld4` of four
  registers load some destination registers (`ld4` all four, partly). Every store and load of 16 bytes or
  fewer (`str` and `stp` of w, x, d and q, `ldr`, `ldp x`, `st1` of one register or two 8-byte ones) changed
  nothing, and nothing was written when the first page refused. So on ARM64 every form that moves more than
  16 bytes (`ldp`, `ldnp`, `stp`, `stnp` of q registers; `ld1` to `ld4` and `st1` to `st4` of more than 16
  bytes) takes `access`'s page test: `add t, xN, #offset` (left out when the offset is 0), `and t, t,
  #(PAGE_SIZE - 1)`, `add t, t, #(size - 1)`, `tbnz t, #log2(PAGE_SIZE), cross`, no flags touched. An
  access within one page is copied as it is (one page refuses all of it or none). On `cross` a probe of
  each of the two pages comes first (a load: `ldrb wzr` at the access's first and last byte; a store:
  `staddb wzr` there, which needs write permission and changes nothing, measured in experiments/n0r3/
  stadd.out), so a refused page faults before anything changes, and then the copied instruction, which can
  no longer fault, is the commit point. The floor reports such a fault at the address the WebAssembly
  output's exact check gives for that access (tr/code.rs:797-830), worked out from the guest instruction;
  N2's differential test compares it. Cost: three or four instructions in front of each such form,
  expected to be small and measured at N4; the crossing path is counted at translation time as "access that
  may cross a page probed" (section 3.7). The forms above are the ones this M5 was measured changing part of;
  until the owner rules on decision 14, every other load and store that can cross a page takes the same
  test too, on both architectures (Kernel route, host-model results), so nothing rests on a host's model.
  x86-64 would need no such route on the reference model: measured on the
  Linux laptop (i7-8750H, experiments/n0r5/partial86.out), `mov` of 8 bytes, `movdqu`, `vmovdqu` of 32
  bytes, `fxsave64` and `xsave64` crossing onto a page that refuses writes wrote nothing in the first page,
  and `movdqu` and `vmovdqu` loads crossing onto an unreadable page left xmm0 unchanged, at every offset;
  that is what decision 14's option (a) would rest on.
- **Nothing**: a form the WebAssembly output decodes as doing nothing (`I::Nothing`, whose meaning is only
  `e.next()`, arm64/meaning.rs:1425) is emitted as no instruction at all (none of them is counted under E, section 4). On ARM64 that is the whole hint space (arm64/decode.rs:403-405: pointer
  signing and authentication hints, branch target marks, `wfe`, `wfi`, `sev`, `sevl`, `yield`, `chkfeat` and
  any hint added later) and `prfm`, `prfum`, `dmb`, `dsb`, `isb`, `ssbb` and `pssbb` (arm64/decode.rs:
  704-706). `clrex` is not among them: the forms decoder takes it first (arm64/decode.rs:13,
  decode_forms.rs:63) as `ClearExclusive`, whose meaning forgets the monitor (arm64/memory.rs:357-360), so
  it is the Exclusive route's. Why not copy: the M5 has FEAT_PAuth, FEAT_PAuth2 and FEAT_BTI (`sysctl
  hw.optional.arm`), and the independent check measured a copied `xpaclri` at EL0 in the virtual machine,
  with SCTLR_EL1.EnIA and EnIB clear, stripping x30's bits above the address size (check-n0c/pmu_fpsr.out
  case H: 0x0000008000001000 became 0x1000, 0x0012340000002000 became 0x2000), where the WebAssembly output
  leaves x30 alone; a copied `wfi` or `wfe` would idle the virtual CPU. The barriers change
  nothing one guest thread can see; when the owner rules on parallel guest threads, their route is decided
  with the WebAssembly output's.
- **Aligned**: a copy with an alignment test in front: `and t, xN, #(size - 1); cbnz t, trap` (no flags
  touched), raising `TrapKind::Alignment` at the address as the WebAssembly output's `exact_address` does
  (arm64/memory.rs:36-47). For ARM64's atomic, exclusive and acquire/release forms (the LSE atomics, `cas`,
  `casp`, the load and store exclusives, `ldar`, `stlr` and their sizes; all take a bare base register). Why:
  on a host with LSE2 (this M5: `hw.optional.arm.FEAT_LSE2: 1`) these complete when misaligned inside one
  16-byte block, at EL0 inside the virtual machine too with SCTLR_EL1.A = 0 and nAA = 0 (check-n0/xo.out case
  6), while the offer has no LSE2 and the WebAssembly output faults. No floor setting restores the fault.
- **Rebase**: a guest pc-relative instruction (ARM64 `adr`, `adrp`, literal loads; x86-64 RIP-relative
  operands), re-emitted as the same operation on the guest address written at load (section 3.2). ARM64:
  `adr` and `adrp` build the address in place in their destination with `movz` and up to two `movk`s (guest
  addresses are below 2^45 natively, section 3.2); the first `movz` is the commit point and section 4's
  constant fix-up finishes the destination. A literal load builds the address in t and loads through
  `access` from it; the load is the commit point. x86-64: no RIP-relative operand can be kept, since the code
  area is in the upper canonical half and guest memory below 2^47, never within the ±2 GiB a disp32 reaches.
  So `lea r, [rip + d]` becomes one `mov r, imm64` (its own commit point), and every other RIP-relative
  operand becomes `mov t, imm64` and the instruction re-encoded on `[t]`. Measured here (static): over the
  same eight x86-64 Linux files as section 3.3 (5.3 million instructions), 23.6 per 1000 are a RIP-relative
  `lea` and 19.3 per 1000 another RIP-relative use (experiments/n0r5/rip86.py, rip86.out; from 0.7 in
  libpcre2 to 61.9 in xz). So about 19 instructions in 1000 gain one `mov` of 10 bytes, and 24 in 1000
  grow from 7 bytes to 10; the run-time cost is expected to be small and is measured at N9. Rejected:
  placing the code area within 2 GiB of guest code (it would take guest addresses Linux gives programs).
- **Transfer**: a branch, call or return, emitted as section 3.2 says, with the count's add for the transfer (every transfer this route carries that can go backward is counted, section 4).
- **Kernel**: the instruction becomes a call into the inside, which answers as the WebAssembly output's hook
  or meaning for the same instruction does. These are every instruction whose result depends on the host or
  on state the native output keeps elsewhere, except the host-model cases the behaviour probe covers
  (listed after this route):
  - ARM64: counter reads (`mrs` of CNTVCT_EL0, the WebAssembly output's `Clock` hook, arm64/meaning.rs:1458;
    the decoder maps only CNTVCT_EL0 to `SystemRegister::Counter`, arm64/decode_forms.rs:247, so a read of
    CNTPCT_EL0 is `Instruction::Undefined`, decode_forms.rs:260, and is the Trap route's) and `ic ivau` (the
    `CodeChanged` hook, arm64/memory.rs:382);
  - x86-64: `rdtsc`, `rdtscp`, `cpuid`, `rdrand`, `rdseed`, `rdpid`, `xgetbv`; every form that reads or writes
    the GS base (`rdgsbase`, `wrgsbase`, `mov gs, r16`, `pop gs`); every write of a segment register (the x86
    plan's meanings decide which values Linux allows, x86 plan lines 1840-1842); and the descriptor and
    segment reads that run in user mode on the reference, `sgdt`, `sidt`, `sldt`, `str`, `smsw`, `lar`,
    `lsl`, `verr`, `verw`, whose meanings give the x86 plan's designed or measured answers (x86 plan lines
    2162-2170), never the inside's own tables; and the two instructions that write rflags bits beyond the
    arithmetic flags at CPL 3, `popf` and `iretq` (x86 plan lines 1446-1451), whose meanings take TF, DF, NT
    and ID and leave AC clear (below, "x86-64's trap flag and alignment checking"); and the x87
    transcendental instructions `fsin`, `fcos`, `fsincos`, `fptan`, `fpatan`, `f2xm1`, `fyl2x` and
    `fyl2xp1`, whose meanings give the correctly rounded result worked out with `astro-float` (x86 plan
    lines 2103-2111), where Intel's hardware is documented only within one unit in the last place: the
    inside calls the same function the meaning's answer comes from (`astro-float` 0.9.6, MIT, whose `std`
    feature is optional, so it is expected to build `no_std`; checked at N9) and writes the result and the
    status word into the thread's x87 state; counted "x87 transcendental worked out inside" (section 3.7);
  - every instruction the guest crate decodes as a system call (`Guest::SYSCALL`).
  Results that depend on the host's model and that the meanings fix to the reference's are routed, on
  every host, until the owner rules on decision 14 (the brief's letter: "instructions whose result depends
  on the host go to our kernel"; the orchestrator's ruling: route them, do not copy them on the strength of
  a probe): x86-64's approximate reciprocals `rcpps`, `rcpss`, `rsqrtps` and `rsqrtss` (and their VEX
  forms), which the meanings give as Intel's by a closed formula checked over all 2^32 inputs on the
  reference (x86 plan lines 1987-1995), while other vendors' approximations differ
  (https://robert.ocallahan.org/2021/09/rr-trace-portability-diverging-behavior.html), go to the inside,
  which computes the meaning's formula; the instructions after which Intel's manual leaves flags undefined
  (`mul`, `imul`, shifts, rotates, `bsf`, `bsr`, `bt` and others), whose meanings give the flags as the
  reference leaves them (x86 plan lines 1424-1428), are copied and followed by the meaning's flag
  computation emitted inline (no call when the flags are dead, which the finder knows within a piece, and
  a call into the inside otherwise; N9 chooses and measures); and on both architectures every load and
  store that can cross a page and is not already the Split route's takes the Split route's page test and
  probes (section 3.4, Split), so that no fault part-way is left to the host's model. The cost is named:
  the page test in front of most loads and stores (three or four instructions, measured at N4) and the
  flag work after the undefined-flag forms (measured at N9), counted per site at translation time as
  "host-model result routed". Decision 14 asks the owner whether to lift it on hosts whose behaviour
  probe matches (section 5.8).
- **Constant**: a register read the WebAssembly output answers with a fixed value (arm64/decode_forms.rs:
  242-256: the counter frequency 1 GHz, CTR_EL0 0x8444_c004, DCZID_EL0 4, the identity registers, the
  read-only thread pointer; x86-64 reads of cs, ss, ds and es give Linux's user values, x86 plan line 1840).
  Measured on this M5 at EL0 (experiments/sysregs.out): CNTFRQ_EL0 reads 1 GHz and DCZID_EL0 4, the same as
  presented, while the round 3 virtual machine read a counter frequency of 24 MHz inside
  Hypervisor.framework (mac-tests.jsonl, "cntfrq"), and CTR_EL0 traps under macOS. So these are never left to
  the hardware: the presented value is the answer everywhere.
- **Masked**: a write the WebAssembly output keeps to the presented bits. ARM64 FPCR keeps only `FPCR_BITS`
  (0x07c0_0000, arm64/instruction.rs:491); measured on this M5, the bits 0x3fbf07 outside that set stick when
  written (experiments/fp.c and sysregs.out: alternative handling, the trap enables, FZ16 and others), so a
  guest could switch on host behaviour the WebAssembly output does not have. The write is re-emitted with the
  mask (one `and` on a scratch register), inside the Float status route below.
- **Float status**: every instruction that reads or writes the float exception flags or the float mode:
  ARM64 `mrs` and `msr` of FPSR and `msr` of FPCR; x86-64 `ldmxcsr`, `vldmxcsr`, `stmxcsr`, `vstmxcsr`,
  `fxsave`, `fxrstor`, `xsave`, `xsavec`, `xsaveopt` and `xrstor` (x86 plan lines 2008-2031), and the x87
  environment saves `fnstenv`, `fstenv`, `fnsave` and `fsave`. Every save of x87 state among them stores the
  x87 last-instruction pointers and opcode (FIP, FDP, FOP), which the hardware fills with addresses of our
  translated code (or, on some vendors, leaves as they were), while the WebAssembly output writes its own
  values there, which the x86 plan lets differ from the reference (x86 plan line 2432; the reference runner
  zeroes them, line 431): the second call writes them in the saved image, through `access`, exactly as the
  x86 meaning of that save writes them (fixed at X1 from the meanings), and the floor does the same in every
  x87 state it saves into a thread's block, so no host address of ours reaches guest memory. x87's status
  word itself needs nothing of this route: both outputs always keep it (x86 plan line 2035), so x87
  arithmetic is copied except the transcendentals above. The WebAssembly
  output keeps the cumulative exception flags only while they are kept: once the program has written them
  (an FPSR write, arm64/meaning.rs:1480-1489; `ldmxcsr` or `vldmxcsr`), or while the mode is not the default
  (FPCR not 0, the `float_slow` test, arm64/guest.rs:416-420; MXCSR's mode not 0x1F80). Otherwise a float
  operation takes the fast path and adds no flags, and a read gives the flags as last written
  (arm64/meaning.rs:1444-1457, the `ReadStatus` hook adding nothing, arm64/exact.rs:196-198; x86 plan lines
  2024-2031), 0 for a fresh thread; a thread the kernel makes starts not kept (docs/status-translator.md
  lines 2411-2413). ARM64's QC is the exception: saturating instructions always set it (arm64/simd.rs:89-95).
  The hardware always accumulates: the independent check measured, at EL0 in the virtual machine with FPSR 0
  and FPCR 0, `fdiv` of 1 by 3 then `mrs x0, fpsr` giving 0x10 (IXC) in 9 of 9 runs, where the WebAssembly
  output gives 0 (check-n0c/pmu_fpsr.out case F). So natively the thread's float status is the block's,
  with the same controls as the WebAssembly back end (Fpsr, Fpcr and FloatTracking; Mxcsr and
  MxcsrTracking) and the same starting values, and the floor keeps it by one rule at every entry into the
  inside: when tracking is on or the hardware's mode is not the default, the block takes the hardware's
  flags; otherwise the block keeps its own (on ARM64 adding the hardware's QC). Every return to the guest's
  level loads the hardware from the block. The route wraps the instruction in two calls into the inside: one
  before it, which applies that rule and reloads the hardware, so the hardware holds exactly the guest's
  flags; then the instruction copied (FPCR's with its mask; the memory forms through `access`); then one
  after it, in which the block takes the hardware's whole status and tracking turns on after an FPSR write,
  `ldmxcsr` or `vldmxcsr`, as the meanings turn it on (`fxrstor`, `xrstor` and FPCR writes do not). The copied
  instruction is the commit point: an interrupt before it rolls back (the first call changed nothing the
  guest sees), and after it rolls forward with the second call's work as the fix-up (section 4). Signal
  frames, saves and the kernel's register reads take the float status from the block, which every entry
  keeps current. Counted by kind, as the Kernel route is, and a read while not kept is counted by the inside
  under the WebAssembly output's names ("exact floating point requested"; "MXCSR's exception flags read while
  not kept"). Cost: two entries into the inside per such instruction, which programs make in their float
  environment calls and, on x86-64, in glibc's lazy binding trampoline, once per lazily bound symbol (x86
  plan lines 2017-2021). Rejected: copying them (a read would show flags the WebAssembly output does not
  keep); a test of tracking in front of every float operation (a check on the hottest float path).
- **Trap**: what the WebAssembly output turns into a trap (an instruction outside the offer, which the guest
  decoder does not decode; `udf`; `brk`; x86's `ud2`, `int3`, privileged instructions) is a call into the
  inside with the same `TrapKind` (gcpu/lib.rs:133-154). The offer is the translator's, never the host's:
  ARM64 offers FP, AdvSIMD, CRC32 and LSE (arm64/forms.rs:8), and this M5 has 55 features (`sysctl
  hw.optional.arm`, SME and AFP among them). An instruction outside the offer is not decoded, so it is a
  trap and never copied; the words the decoder does decode as doing nothing (the hint space, where the M5
  gives pointer authentication and branch marks a meaning) are the Nothing route's, never copied either.
- **Exclusive**: ARM64 load-exclusive, store-exclusive and `clrex` keep the WebAssembly output's monitor
  (`ExclusiveBytes`, `ExclusiveAddress`, `ExclusiveLow`, `ExclusiveHigh`; arm64/meaning.rs:35-41, kept at
  the offsets of arm64/guest.rs:408-411) in the register page, in the meaning's order (arm64/memory.rs:
  258-291 and :292-345). A load exclusive passes its alignment test, puts its untagged address in t (`ubfx
  t, xN, #0, #56`, so a load into xN itself loses nothing), then loads into the guest's registers through
  `access` with one host instruction, the commit point (`ldr` of the size; `ldp` of the pair, with `xzr` or
  `wzr` in place of the first register when both name the same one, which gives the meaning's result, the
  second word); then it writes the monitor: the address from t, the size, and the value. If an interrupt
  lands after the commit point and before the monitor is whole, the floor writes it whole (section 4's
  monitor fix-up: the address from t, the size from the form, the value read again from guest memory at
  that address, which nothing has changed since, as one virtual CPU runs the container's threads, section
  5.3). `clrex` is one store of zero to `ExclusiveBytes` (arm64/memory.rs:357-360), its own commit point. A
  store exclusive first passes its alignment test and then always makes a write access at its
  address, as the WebAssembly output checks alignment and write permission there before it compares anything
  (`exact_address` with `Access::Write`, arm64/memory.rs:300-301, before the comparison at :304-307): when the
  remembered address and size match, it compares and stores with LSE's `cas` or `casp` against the recorded
  value; when they do not, it makes `stadd` of the zero register (of the access's size) at the address, which
  needs write permission and changes nothing, and writes status 1. Measured at EL0 on this M5 (experiments/
  n0r3/stadd.out, three runs; the independent check's casro.out): `cas`, `casal` and `casp` fault on a
  read-only page whether the compare succeeds or fails, and so do `staddb`, `staddh`, `stadd` of `wzr` and
  of `xzr`; `stadd xzr` and `staddb wzr` on a writable page leave the bytes unchanged; on an unmapped page
  `stadd xzr` faults (SIGSEGV). A fault there is reported with `Access::Write`, taken from the position map's
  guest instruction, not from the exception's write bit. The address and size test reads the register page
  into t and compares with `eor` and `cbnz` (no flags touched). `cas` needs the expected value in a
  register it overwrites: t, loaded from `ExclusiveLow`. `casp` needs two even-numbered register pairs, so
  the 64-bit pair form makes four guest registers stand-ins (section 3.3: saved to the register page, loaded
  with the recorded value and the new one), and puts them back after. The `cas`, `casp` (or the `stadd`) is
  the commit point; after it, as the meaning does (arm64/memory.rs:341-344), the monitor is forgotten
  (`ExclusiveBytes` zero) and the status register is written last, so a status register that is also the
  data or base register gets the status, as in the WebAssembly output. Section 4 says how an interrupt after
  the commit point is reported. Three cases in detail:
  - The 32-bit pair store exclusive (`stxp w, w`): the meaning compares and swaps the combined 8-byte value
    with `ExclusiveLow` (arm64/memory.rs:322-330, the new value from `pair32`). Natively it is one `cas x`:
    t holds the expected value (loaded from `ExclusiveLow`), and a stand-in (a guest register the
    instruction does not name, saved to the register page first, section 3.3) is built as rt | rt2 << 32
    by `mov` and `bfi`; the base is the guest's own register; the stand-in is restored after. The `cas` is
    the commit point and t holds the value read, as for the single form.
  - A load exclusive pair whose two registers are the same (`ldxp x1, x1, [x0]`): the meaning remembers
    both words (arm64/memory.rs:268-277) and the register gets the second. The commit point is `ldp xzr,
    x1, [t]`, so the first word is in no register; after it the sequence stores t into
    `ExclusiveAddress` and the size, then reads the first word again with `ldr t, [t]` (it cannot fault:
    the same page was just read and nothing changes tables while this virtual CPU runs the thread) and
    stores it into `ExclusiveLow`, and stores x1 into `ExclusiveHigh`. The monitor fix-up of section 4
    reads both words again from guest memory in the same way.
  - Untagging: `ubfx t, xN, #0, #56` clears the top byte, while the WebAssembly output's `untag` keeps the
    whole address when bit 55 is set (arm64/meaning.rs:543-557). An address with bit 55 set is never a
    mapped guest address on either output, so every access there faults on both; the floor reports such a
    fault at the address `untag` gives, worked out from the guest instruction and its base register (which
    nothing has changed before the commit point), as the Split route does, never at the hardware's fault
    address. N2's differential test seeds base registers with bit 55 set and compares the fault address.
  Why
  not the hardware's monitor: it is cleared by any exception or interrupt taken between the pair, so a store
  exclusive would fail where the WebAssembly output's succeeds, and the retry would change the count.
  Expected from the architecture's rules; N6's replay test puts interrupts between the pair.

**Encodings Arm leaves CONSTRAINED UNPREDICTABLE.** The decoder decodes some register collisions the
architecture leaves to the implementation, and the meanings give them a result: a load or store with write
back whose transfer register is its base (`ldr x0, [x0], #8`), and a load pair into one register (`ldp x1,
x1, [x0]`). The meaning does the access with the base's old value and writes back after it
(arm64/meaning.rs:628-636), so a load's write back wins and a store stores the old base. The hardware does
not run them: the independent check measured `ldr x0, [x0], #8`, `ldr x0, [x0, #8]!`, `ldp x1, x1, [x0]` and
`str x0, [x0], #8` raising SIGILL on this M5 (check-n0d/unpred.out, three runs), and another host may do
something else. So `Native` routes these encodings by their register fields, not only by form: the
colliding transfer register is replaced by t, which makes the host instruction defined and gives the
meaning's result in one instruction, the commit point (a load: `ldr t, [x0], #8`, the loaded value
discarded, x0 written back; a load pair: `ldp t, x1, [x0]`, x1 the second word; a store: `mov t, x0` then
`str t, [x0], #8`, storing the old base). The Exclusive route's own sequences keep the meaning's order for
its collisions (above). The differential test draws each form's register fields as well as their values,
with every collision among them (transfer and base, the two transfer registers, status and data or base),
so each such encoding is run against the WebAssembly output (N2).

Float arithmetic is copied. The WebAssembly output's `Hook::Float` (arm64/float.rs:5-8, called at
float.rs:216, :643 and :700) is its own way, in software, to compute what the hardware computes natively for
the presented FPCR modes; with FPCR masked to the presented bits the hardware computes what Arm defines,
which is what the hook computes. The exception flags it raises are visible to the guest only through the
Float status route above, which gives exactly the flags the WebAssembly output keeps. On x86-64 a float
exception the guest has unmasked in MXCSR raises #XM in hardware, which the floor reports as the meaning's
`TrapKind::FloatException` (gcpu/lib.rs:150; x86 plan line 2023).

**x86-64's trap flag and alignment checking.** The WebAssembly output holds TF exactly and steps as the
reference does (x86 plan lines 1463-1513: an instruction that starts with TF set stops right after it; `popf`
or `iretq` setting TF does not stop itself; after a system call or any return from the kernel with TF set the
first instruction runs before the stop; a fault gives only its own signal; `mov ss` holds the stop back one
instruction; with TF set a `rep` string instruction stops after each element, rip on the instruction, x86
plan lines 1848-1849 and 2975-2976). It leaves AC clear: `popf` and `iretq` never set it, so it reads back 0,
counted "alignment checking asked for, not provided" (x86 plan lines 1514-1520). Natively:
- `popf` and `iretq` are routed to the inside (above), which applies their meanings: the six flags, TF, DF,
  NT and ID taken, AC left clear and counted as the meaning counts it, TF set counted as "trap flag set by
  the program". So the hardware's AC in ring 3 is never set, and CR0.AM is clear as well (section 5.2), so
  no alignment-check exception can arise at any level; a copied `pushf` then stores AC clear, as the
  WebAssembly output's `pushf` does.
- The guest's TF is the hardware's TF in ring 3, which only the inside sets or clears (routed `popf` and
  `iretq`, and every return from the kernel to a thread with TF, through the frame the inside returns with),
  so a copied `pushf` stores it as the guest sees it. With TF set the hardware single-steps host
  instructions, not guest ones. The floor takes every single-step debug exception and uses the position map
  (section 4): at a guest instruction boundary (the first host instruction of a run, nothing of it done) it
  reports the step there, as `Guest::step`'s `Trap` does; at a `rep` string instruction it reports the step
  with the hardware's state after the element, which is the reference's (and the x86 plan's meaning's)
  partial state; anywhere else it resumes. Since a return to ring 3 with TF set in the frame runs one
  instruction before trapping (Intel SDM volume 3, chapter 18, single-step exception condition; the
  reference behaves so after `popf` and system calls, x86 plan lines 397-404), the first guest instruction
  after the inside runs before the stop, as on the reference. A routed instruction that started with TF set
  (a `popf` clearing TF, a system call) is answered with the step as the meaning's `StepAt::Kernel` rule
  says, and the x86 plan's `StepHeld` rule for `mov ss` (a routed segment write) is kept by the inside the
  same way.
- The cost falls only on a thread with TF set: one ring 0 exception per host instruction, counted "single
  step through host instructions" (section 3.7). KVM is expected to intercept #DB and #AC for every guest
  (its fixes for CVE-2015-8104 and CVE-2015-5307), so each such exception may also pass through the host's
  kernel; not measured, N8 measures it. Rejected: copying `popf` and filtering in the floor (a copied `popf`
  could set AC in hardware, and the flag test in front of it would change the guest's flags before the
  commit point); stepping by translating the stepped thread's code with a stop at every instruction (a second
  translation mode for one flag).

**Proof that routing matches the WebAssembly output**, two tests:
- A static test (in each guest crate's tests, not shipped) runs every decoded form through `Guest::emit`
  with a recording emitter, which records the hooks the meaning calls (`hook`), the thread state it touches
  (`get_control`, `set_control`), and whether it calls `syscall`, `trap` or `raise_stop` (x86 plan lines
  971-976), and the float hook's operation (the constant the meaning passes as the hook's first argument,
  arm64/meaning.rs:1449). Each guest crate's `Native` gives every hook kind and every control, read and
  written apart, a native class: held by the hardware as presented (ARM64 `ThreadPointer`; reads of `Fpcr`
  and `FloatTracking`, which only choose the WebAssembly output's float path; writes of `Fpsr`, which are the
  flags a float operation or a saturation adds and the hardware adds too; the `Float` hook for an
  arithmetic operation; x86-64 the FS base; reads of `Mxcsr` and `MxcsrTracking` by arithmetic; DF, NT and
  ID read through `Rflags`), the WebAssembly output's own bookkeeping with nothing to keep natively (x86-64
  the step machine's `Step`, `StepAt`, `StepHeld`), kept by the native output (the exclusive monitor;
  x86-64 the GS base; the float status: writes of `Fpcr`, `FloatTracking`, `Mxcsr` or `MxcsrTracking`, and
  the `Float` hook with `ReadStatus`, which is what every read of the flags makes while they are not kept;
  x86-64's classes are fixed against its meanings at X1, which tell its arithmetic's flag updates apart
  from the status writes in the same way),
  or answered by the inside (`Clock`, `CodeChanged`; x86-64 any write of `Rflags` and any `raise_stop`,
  which are TF's and AC's). A form may be Copy or Aligned only if everything it touches is of the first two
  classes; a form touching a kept or answered one must be routed to the route that class names; a form
  whose meaning touches nothing at all (no register, flag, control, memory or hook) must be Nothing. A form
  added to a guest crate is then routed or the test fails.
- The static test cannot see a form whose meaning gives a fixed answer where the hardware gives another
  (`sgdt`, `lsl`, a CTR_EL0 read). So a differential test runs every decoded form natively, as routed, at the
  guest's level inside the virtual machine, and through the WebAssembly output, from the same seeded
  registers, flags and memory, and compares every register, flag, control, memory byte and trap. Seeded,
  with the seed and the number of cases per form as arguments; N2 (ARM64) and N9 (x86-64) run it. Registers
  are seeded over all 64 bits (so a stripped or changed high bit shows), each form's register fields are
  drawn too, with every collision the encoding allows, and every load and store also runs placed across a
  page boundary with the first, then the second page refusing it. Every form also runs after a float
  operation that raises seeded flags, with tracking on and off and the mode default and not, so a float
  status form shows any flag the hardware kept and the WebAssembly output did not. For x86-64 the seeded
  states include TF and AC set in the values `popf` and `iretq` take, and a stepped run of each form
  compares the stops as well.

### 3.5 Code a program writes itself

As for WebAssembly (runtime.md section 7): ARM64 programs announce changed code with `ic ivau` (the
presented CTR_EL0 says so), which is routed to the inside; the kernel's "code is here" notice drops a range.
The process then leaves every loaded translation holding that line: its mirror slots for the translation's
range are emptied (section 5.3), so its next entry there is a miss and translates the new bytes. The
translation's frames stay while another process holds them and are freed when none does. x86-64 does not
announce: as the x86 plan says (section 5.7 there), a page that has been translated carries the "translated
code" write lock, so the first write faults to the inside, which leaves that page's translations the same way
and lifts the lock. A range whose bytes changed after it ran is translated at bounded look-ahead from then
on, counted "code changed after it ran", as runtime.md says. Native code never reads the guest's bytes after
translating them, so a change the guest does not announce (ARM64) runs the old translation, as the
WebAssembly output does.

### 3.6 Crates, and what must be ours

- Host side: `kvm-ioctls` 0.25.1 (Apache-2.0 OR MIT) and `kvm-bindings` 0.14.2 (Apache-2.0) for KVM;
  `applevisor` 1.0.0 (MIT OR Apache-2.0) for Hypervisor.framework. Alternatives: `hv` 0.1.2 (Apache-2.0,
  older), `xhypervisor` 0.3.0 (MIT, x86 macOS). `vm-memory` (Apache-2.0 OR BSD-3-Clause) is not needed: guest
  memory is one mapping we make.
- Inside: `aarch64-paging` 0.12.3 (MIT OR Apache-2.0) and `x86_64` 0.15.5 (MIT/Apache-2.0; the newest stable;
  0.16.0-rc.0 is a pre-release and is not used) for page table entry types; `aarch64-cpu` 12.0.0
  (MIT/Apache-2.0) for system registers, with `arm-sysregs` 0.5.2 (MIT OR Apache-2.0) for the fields it lacks
  (SCTLR_EL1's, CTR_EL0's, PMCR_EL0's); `arm-gic` 0.11.0 (MIT OR Apache-2.0, with `safe-mmio` 0.4.0) for the
  interrupt controller; `spin` 0.10.1 (MIT) for the lock over frames; `talc` 5.1.1 (MIT) as the allocator.
  The monitor reads the inside's ELF image with `object` 0.37.3 (Apache-2.0 OR MIT) and maps memory with
  `libc`. The page-table walk that
  copies, shares and write-protects is ours (section 5.2), since it is the speed of fork and of the write
  locks.
- Emitting: x86-64 re-encodes copied instructions at their new address with `iced-x86` 1.22.0 (MIT; already
  the x86 track's decoder), whose block encoder fixes relative branches; RIP-relative operands are not left
  to it, since none would reach guest memory from the code area, and are rebuilt by the Rebase route
  (section 3.4). ARM64's
  forms we emit (branches, `movz`/`movk`, `ubfx`/`ubfiz`/`bfxil`, `and`/`eor` immediate, `add`/`sub`
  immediate, `cbz`/`cbnz`, loads and stores with an immediate offset, `cas`, `mrs`) are a small encoder of
  ours: fixed 4-byte fields, purpose-built. Rejected: `dynasmrt` 5.1.0 (MPL-2.0, a file-level copyleft
  licence, and an assembler driven by macros where we need a few encodings at run time).
Licences checked on crates.io on 2026-11-18 (docs/status-native.md).

### 3.7 The native back end's slow paths

Each is counted and warned through the kernel's warnings (sent to the host in batches, section 5.4), with
these names (suggested; the owner chooses names):
- "code not yet translated here": a missing mirror slot or x86-64 lookup miss that finds no loaded piece and
  asks the host for a translation (one link exit and the translation's own time);
- "jump target found in the process table": a missing slot or a cache miss answered inside from the
  process's loaded translations (an exception at EL1 or ring 0, no exit);
- "asked the host": every link exit, by request kind (section 5.6);
- "code changed after it ran", as runtime.md;
- "stop deferred to the end of a string instruction" (x86-64, section 4);
- "single step through host instructions" (x86-64, a thread with TF set: every host instruction a ring 0
  exception, section 3.4);
- "step handed to a virtual CPU": each `Container::step` a caller makes is one handoff to a pool thread and
  back (section 6.1), where `Container::run` hands over once and runs `drive` inside;
- "turn limited by a count" (only under option B of decision 10);
- "turn limited by a count alone" (until the owner rules on decision 10: such a turn is answered as
  `Failure`, never run past its limit, section 5.5);
- "address-space identifiers ran out" (a new generation and one whole invalidation, section 5.2);
- "guest-physical space full" (a request for memory fails at the bound of section 5.1);
- "address above the ceiling" (a request for an address at or above the native ceiling fails until moving
  exists, section 6.2, K2);
- "large entry split" (a 2 MiB entry broken into 4 KiB entries because one page of it changed, section 5.2);
- "back end refused by the probe", with the missing feature (section 5.8);
- the float status reads the WebAssembly output also counts ("exact floating point requested"; "MXCSR's
  exception flags read while not kept"), under its names (section 3.4);
- "withheld register named by the guest" (a stand-in's moves around one guest instruction, section 3.3),
  counted at translation time per site and warned when a loaded translation has any;
- "access that may cross a page probed" (the Split route's page test in front of a load or store of more
  than 16 bytes, section 3.4), counted at translation time per site;
- "x87 transcendental worked out inside" (section 3.4, Kernel route);
- "host-model result routed" (section 3.4, Kernel route: a load or store given the page test because the
  host's model is not relied on, an x86-64 reciprocal or undefined-flag form), counted per site at
  translation time;
- "waited for a virtual CPU" (a container ready to run while every virtual CPU the platform allows is
  busy, section 5.1).
Not slow paths, and still counted for the record: `bad` (a fault the guest made, reported as the
WebAssembly output reports it), the routed Kernel and Float status instructions by kind. The routed store
exclusive, the alignment tests and the FPCR mask are the native output's normal and only way for those
forms.

## 4. The progress count

**One add.** The count is a withheld register (section 3.3). At each counted event the native output emits
one add (ARM64 `add c, c, #1`, which sets no flags; x86-64 `lea c, [c + 1]`). No compare, no branch. The
count is saved to the thread's register block at every exit to the kernel, with the count word runtime.md
section 9 places there, and loaded when the thread runs. So the kernel, saves and moving a thread see it as
on every back end.

**The definition (the orchestrator's ruling, E).** One counted event for every executed instruction that
can transfer backward: a direct branch whose target is at or below its own address, taken or not; every
call; every return; every jump through a register. It is a function of a single instruction and its own
address, `counted(instruction, address, flow) -> bool`, one function of the shared finder that both
emitters call; neither has its own rule. So it is the same at every look-ahead, for code found at run time,
in any discovery order, and on both outputs. Round 3's head-pass and mixed forms (Hn, Mn) are not
definitions: whether an instruction carries an add there depends on other instructions the finder may not
have found yet. The kernel's own changes of pc are counted by the loop on every back end (runtime.md section
9) and are not the emitter's.

**Where the add sits is free, what is counted is not.** The definition fixes which executed instructions
count, not where their add is emitted. The native output may place the add for an event anywhere earlier
in the same straight-line run (the host instructions from the run's entry to the event, with no branch
out between them), and may merge the adds of every event of one run into one `add c, c, #k` at a point
every one of them passes, as long as the position map (below) says, for every host instruction of the
run, how far the count register is ahead of the events completed there. An interrupt that lands between an
add and its event is corrected by that map, exactly as an add before an untaken transfer already is. One
place decides the placement, beside `counted`, so measuring another placement changes one function. N2
measures E's cost on the M5 with this freedom used (one merged add per run, placed where it costs least),
against round 3's figures for E (+13 to +14% cycles on the M5, against +4 to +6% for the head-pass form,
one program swinging by layout), and the speed bar of section 8 uses that measured figure.

**Why it keeps positions unique.** A position is reached at most once in a run only if no path of the
guest's control flow comes back to the same pc without a counted event. Every path that comes back to a pc
goes through a transfer to a lower or equal address or through a transfer whose target is not known when
translating (a jump through a register, a return, a call's return path), and E counts every one of them;
the kernel's changes of pc are counted by the loop. Replay (section 5.7) and the uniqueness of logged
positions rest on this. A test of the shared finder checks it: over every piece of the translator's
benchmark programs and of layer 2's programs, at every look-ahead, no cycle of the piece's control flow
(including the exits through a register) is free of a counted instruction.

**The count is the same at every stop, whatever the look-ahead and the output.** A second test runs each of
the translator's benchmark programs and a seeded set of layer 2's programs to the end and stops them at
seeded positions under every look-ahead setting (none, bounded, full) and under both outputs (WebAssembly
and native), and requires, at every stop, the same count for the same pc and the same final count. The
WebAssembly side runs at T2 (the WebAssembly output counting by `counted`); the native side's look-aheads
at N4; both outputs together at N6. Seeded, with the seed and the number of programs and stops as
arguments.

**An interrupt between an add and its transfer.** An interrupt can land at any host instruction. Every host
sequence that carries one guest instruction has one commit point: the one host instruction at which the
guest instruction's effect on guest registers, memory or the exclusive monitor first happens, or at which pc
leaves the sequence. Before it, the sequence may write withheld registers, and may relocate guest values
(a stand-in's own value saved to the register page, a withheld register's guest value loaded into a
stand-in or into t, section 3.3), which moves a value without changing it; it changes no guest value and no
memory. Probes and tests that fault (the alignment tests, the Split route's page probes) fault before it.
The commit point itself is one instruction, so an interrupt is taken either before or after it, and when it
faults it has changed nothing: for a load or store this holds because every form that a fault can stop
part-way is the Split route's, which probes first (section 3.4), and until decision 14 every other load and store
that can cross a page takes the same test. Every translation, the mirror's slots and the call stubs carry a position map: for
each run of host code, the guest instruction it belongs to, which side of the commit point it is on, how far
the count register is ahead of the count of completed events there (it may be behind for an add that comes
after the commit, as in a call stub), and where every relocated guest value is held there. The floor, at
any interrupt or fault in translated code:
1. finds the run holding the host pc;
2. puts every relocated guest value back where the guest has it, as the map says for that run (the same on
   both sides of the commit point: before it the stand-in's own value may be in the register page; after
   it the instruction's result may still be in the stand-in or t);
3. before the commit point, rolls back: the guest pc is that instruction's address and, with step 2,
   nothing of it is done;
4. after the commit point, rolls forward: the guest pc is the instruction's successor (or its transfer's
   target), and the map names the fix-ups the rest of the sequence would have made, from a closed set:
   - a constant into a destination built in place (section 3.4's Rebase `adr` and `adrp`, x30 in the plain
     form of `blr x30`): the destination gets the constant the map holds;
   - a slot address to a guest address (`ubfx g, s, #2, #45`): x30 in a call stub before its first
     instruction has run; the target in t after a plain call has begun x30 (the pc is then that target);
   - the exclusive monitor: after a load exclusive's load, the monitor written whole (the address from t,
     the size from the form, the value read again from guest memory there); after a store exclusive's
     `cas`, `casp` or `stadd`, the monitor forgotten and the status written: 0 if the `cas` or `casp` read
     the recorded value (left in t, or in the `casp`'s stand-ins), else 1, and 1 after the `stadd`;
   - a float status write's second call (section 3.4): the block takes the hardware's status, tracking
     turns on where that instruction turns it on, and an x87 state save's pointer fields are written;
5. sets the count to the register less the map's amount.
A sequence that does not fit this shape is not emitted: the every-offset test below takes a stop at every
host instruction of every sequence and fails on it.
So a stop is always at a guest instruction boundary with the count of events completed before it, and with
the registers the guest has there, which is what the WebAssembly output reports for a thread stopped at
that position. Round 2 used the same shape for ARM64 (instrument3.py's `_gmap_corr` map in round 3; round 2's
"320 asynchronous stops each replayed twice to the same (count, pc, data state)", costs-round-2.json,
"measure native-arm64", headline).

x86-64 `rep` string instructions are stopped part-way by the hardware with rcx, rsi and rdi at an
iteration's end. E does not count their rounds (a `rep` is not a transfer), so the position there is not one the WebAssembly output reaches (its meaning runs the whole instruction between two boundaries), so a stop that
lands inside one is rolled forward by the hardware: the floor sets an execution breakpoint (DR0) on the next
instruction and resumes, the instruction finishes, and the breakpoint's debug exception reports the stop
there (Intel SDM volume 3, chapter 18, debug registers; not yet tried under KVM, N8 does). Counted as "stop
deferred to the end of a string instruction". No instruction is finished in software. A fault inside one is the reference's own partial state, which the x86 plan's meanings give too. A
thread with TF set is the exception the reference itself makes: there the hardware's stop after each
element is the guest's own step (section 3.4).

**Tested: the every-offset test (N5).** A timer cannot be aimed at one host instruction, so the test uses the
hardware's single step from the inside's level: ARM64 EL1 software step of EL0 (MDSCR_EL1.SS set, and
SPSR_EL1.SS set in the frame the floor returns with), x86-64 TF set by the floor in its return frame.
Measured under Hypervisor.framework on this M5 (experiments/n0r3/step.c, step.out; load 43): with MDSCR_EL1.SS
set from the host and SPSR_EL1.SS in the frame, each EL0 instruction gives a software step exception at EL1
(ESR_EL1 EC 0x32) with ELR_EL1 at the next instruction, a `br` steps to its target, an `svc` gives its own
exception (EC 0x15) instead, and a `br` to a target 2 bytes off steps there and then gives the PC alignment
fault (EC 0x22); the debug exceptions are taken in the virtual machine (Hypervisor.framework's
`hv_vcpu_get_trap_debug_exceptions` gives false by default), and a write of OSLAR_EL1 from EL1 exits to the
host (EC 0x18), so the floor leaves the OS lock as the virtual CPU starts it (unlocked: stepping worked
without the write). x86-64 is tried at N8. The test steps a program through, and at the first visit of each
host instruction of every piece, slot and stub it takes a real stop there through the floor's interrupt
path, records the position and every register, and resumes from the reported position. The run must still
end with the reference's output, and a second run stopped by a replay check at each recorded position (all,
or a seeded sample whose size is an argument) must have the same registers.

## 5. The virtual machine

### 5.1 Who runs where

- ARM64: the inside runs at EL1, translated guest code at EL0. EL2 is not used: nothing inside is a
  hypervisor (Hypervisor.framework can offer EL2 to a guest on newer chips, `hv_vm_config_set_el2_enabled`
  in Apple's documentation for macOS 15; KVM only with nested virtualization). The one thing EL2 would bring
  is a performance counter for decision 10's option C on Hypervisor.framework: with EL2 disabled every PMU
  register access exits to the host and the virtual CPU reports no PMU (hv_vm_config.h, the discussion of
  `hv_vm_config_set_el2_enabled`; the independent check measured PMUVer 0 and EC 0x18 exits for
  `pmcr_el0`, `pmccntr_el0` and `pmuserenr_el0` at EL1, check-n0c/pmu_fpsr.out case P), and with EL2 enabled
  the framework emulates PMU accesses only when ID_AA64DFR0_EL1.PMUVer is not 0 (same header). Measured here
  (experiments/n0r4/tpidrro.out, case E, two runs): with EL2 enabled the virtual CPU still reports PMUVer 0,
  and the host can set PMUVer to 1 to 9 with `hv_vcpu_set_sys_reg`, which reads back. N1 tried whether the
  emulated counter counts EL0 instructions and raises an overflow interrupt: it does neither (decision 10),
  so EL2 stays unused and the monitor's interface has no setting for it.
- x86-64: the inside runs in ring 0, translated code in ring 3, in 64-bit mode with 4-level paging.
- **One virtual machine per host process**, holding every container that process runs natively. Why:
  Hypervisor.framework allows one virtual machine per process (hv_vm.h: `hv_vm_create` "Creates a VM
  instance for the current process"; check-n0/twovm.out: a second `hv_vm_create` while the first exists
  returns HV_BUSY), and one runtime makes many containers in one process (`Runtime::container`,
  rt/container.rs:85; the kernel tests run several tests in one process, runtime.md section 11; the other
  project runs many containers per node). KVM allows many, but one shape on both keeps one primitive.
  Rejected: one virtual machine per container (only the first container of a macOS process would get one);
  one host process per container (a process handoff at every step and at every host call to the user's
  services, and containers stop being objects of the user's process).
- **Each container** has its own kernel instance, in its own heap region of the inside at the same inside
  addresses for every container (so a saved container comes back anywhere, section 5.8), its own guest
  processes and its own turns. The heap is mapped non-global, in the tables of each of the container's own
  processes (section 5.2), so it is switched with the process. Containers in one process already share one
  host process under the WebAssembly back end, where each kernel is an object in it; here they share the
  inside program.
- **Virtual CPUs** are a pool owned by the monitor, one host thread each. Hypervisor.framework binds a
  virtual CPU to the thread that made it (hv_vcpu.h: "Creates a vCPU instance for the current thread"), so
  `Container::step` and `Container::run` on the user's thread hand the work to a pool thread and wait for
  it (the handoff's cost is measured at N5; section 6.1 says which calls hand over how often). A virtual CPU
  runs one container's work at a time, and holds it only while that container has a thread to run: each
  container's loop runs on its own stack in the inside (in its heap region), and when the container has
  nothing to run (`idle`, section 6.1) the floor parks that stack (its EL1 or ring 0 registers saved in the
  container's region) and the virtual CPU goes back to the pool; whichever virtual CPU is free takes the
  container up again when the wait ends. So containers waiting cost no virtual CPU, and the pool grows to
  the number of containers that have a thread ready at once, as the WebAssembly back end has one user
  thread busy per such container and the host's scheduler shares the cores among them. The platform bounds
  the pool: 64 virtual CPUs per machine on this M5 (`hv_vm_get_max_vcpu_count`, experiments/n0r2/
  branch.out), and on KVM what `KVM_CAP_MAX_VCPUS` reports (Documentation/virt/kvm/api.rst; not read on the
  laptop yet, N8 does). A container ready to run while every virtual CPU is busy waits for one, counted
  "waited for a virtual CPU" (section 3.7), and gets the next one freed; at its slice's end (section 5.5) a
  container with waiting others gives its virtual CPU back, so none is starved. Parking is a switch of a
  few registers between containers' stacks, not a scheduler of guest threads: which guest thread runs stays
  the kernel's.
- **Guest-physical space** is shared by the containers of the process: at most 2^40 bytes on this M5
  (`hv_vm_config_get_max_ipa_size` 40 bits, the default 36, branch.out) and 2^39 on the Linux laptop (its
  CPU's 39 physical address bits, /proc/cpuinfo). Frames are taken as touched, so this bounds what the
  process's containers have touched at once; past it, a request for memory fails, counted and warned, as the
  brief rules for a back end's ceiling.

### 5.2 The inside

One `no_std` Rust program per architecture, built for `aarch64-unknown-none` or `x86_64-unknown-none`. It
holds:
- the floor: entry, exception vectors, a frame allocator over guest-physical memory, the heaps (`talc`, one
  per container), the timer (ARM64 the virtual timer through the GIC; x86-64 the local APIC's TSC-deadline
  timer), the interrupt controller, and the link. What the guest's level is allowed is set as the WebAssembly
  output presents it:
  - ARM64: TCR_EL1 with the granule of `PAGE_SIZE` (4 KiB) for both halves, TBI0 = 1 and TBI1 = 0 (Linux's
    Top Byte Ignore for user addresses, which copied loads and stores need to match the WebAssembly
    output's `untag`; measured, branch.out case E: a load through a tagged pointer works with TBI0 = 1
    and faults with TBI0 = 0), A1 = 0
    (the address-space identifier comes from TTBR0); SCTLR_EL1.A = 0 (unaligned plain accesses allowed), SA0
    as `Guest::SP_ALIGNMENT_CHECKED` says (gcpu/translate.rs:196; the monitor reads it from the guest's
    `Guest` and passes it to the inside's boot registers, since that trait is the translating half's), UCI set as Linux sets it, DZE set (`dc
    zva` with the presented block size, which the probe requires of the host), nTWI and nTWE clear (a `wfi`
    or `wfe` at EL0 traps; none is ever emitted, section 3.4's Nothing route), the pointer authentication
    enables and BT0 clear; MAIR_EL1, VBAR_EL1;
  - x86-64: GDT with Linux's user selectors, IDT, TSS, EFER; CR0 with WP set (read-only pages refuse ring 0
    writes too, section 3.3) and AM clear (no alignment-check exception at any level; the guest's AC is
    never set, section 3.4); CR4.TSD set so a stray `rdtsc` traps,
    CR4.FSGSBASE set (the offer presents FSGSBASE, x86 plan line 1834), CR4.UMIP set where the virtual CPU
    offers it, so a stray copy of `sgdt`, `sidt`, `sldt`, `smsw` or `str` faults instead of reading the
    inside's tables (routing already sends them to the inside; the laptop's CPU has no UMIP, /proc/cpuinfo,
    and whether its KVM offers it emulated is checked at N8); XCR0 holding only the state the offer claims;
    the syscall mask clearing TF, AC, DF and NT on entry;
- the kernel crate unchanged, one instance per container. Measured: crates/wcontainers-kernel builds as it is
  for `aarch64-unknown-none` and `x86_64-unknown-none` with Rust 1.99.0 in release (experiment on the Linux
  laptop, 2026-11-18); it is `no_std` with `alloc` (kernel/lib.rs:9-11);
- the runtime's loop (`Stepper`, rt/lib.rs:189-244), the native `Execution` (section 6) and the policy half
  of a `System` (section 6.2), all `no_std`.
The workspace's toolchain file gains the two targets (rust-toolchain.toml lists none of them today). The
monitor carries the inside as bytes made by a build script that runs cargo for the target, from N3, when
there is an inside program to ship (N1's test builds its own cases program).

**Page tables.** The kernel's `Cpu` side today is a software record per page, written through
`set_permissions`, `memory_new`, `memory_duplicate`, the shared blocks and `code_changed` (abi/cpu.rs:35-97).
Inside the virtual machine the native `Cpu` side writes real page tables instead, one set per guest process
(one address-space identifier each), 4 KiB granule, so each guest page has its own permissions, as the brief
rules. A guest page is a 4 KiB guest-physical frame; the host backs guest-physical memory in its own pages.
On a 16 KiB-page Mac that changes nothing for the guest: measured, two neighbouring 4 KiB guest pages inside
one 16 KiB host page took different permissions in Hypervisor.framework, and a write to the read-only one
faulted alone with the right address (costs-round-3/virtual-machine/results/mac-tests.jsonl "perm4k", three
runs). The kernel's permissions are the page table bits less the back end's write locks (runtime.md section
8), made by one function from the record, as today. Measured costs in the round 3 prototypes: a first-touch
fault handled inside, 224 ns with the frame already backed, 584 ns when the inside zeroes it
(Hypervisor.framework, mac-tests.jsonl "guest_fault"); one page's permission changed through a system call
inside, 558 ns with the invalidation Linux uses, against 323 ns for the host's own `mprotect` (same file,
"guest_permission_change", "native_mprotect_16k").

**Larger entries where the guest cannot tell.** Round 3 ran the eight bench.c workloads untranslated at EL0
inside Hypervisor.framework against plain native runs. Over those eight (geometric mean of each workload's
median ratio, computed from costs-round-3/virtual-machine/results/mac-summary-main.json and
mac-summary-16k.json by experiments/n0r4/vmcost.py, vmcost.out), the cycles cost +8.5% with 4 KiB stage 1
pages (instructions +0.9%; sieve +39.5%, walk +36.9%), +2.8% with 2 MiB blocks (instructions +0.7%; walk
still +20.5%) and +2.9% with a 16 KiB granule: the difference is the TLB's reach. So the floor maps a 2 MiB
aligned range of a process with one 2 MiB entry (ARM64 a level 2 block; x86-64 a 2 MiB page) whenever the
kernel's record gives every page of it the same permissions and kind (private memory, no write lock),
backed by a 2 MiB aligned run of guest-physical frames never used since the host last gave them fresh.
Such a run reads as zeros and costs the host nothing until touched (section 5.2, memory from the host), so
taking a whole block at the first touch zeroes nothing and holds no more host memory than the pages
touched. A change that leaves the whole range alike (a fork write-protecting it all) keeps the block; one
that reaches only part of it (a permission change, a copy-on-write copy, a tracking lock, an unmapping)
first splits it into 4 KiB entries over the same frames (on ARM64 by break-before-make: the entry made
invalid, invalidated by one TLB invalidation at its address, which covers the whole block, then the new
table written), counted "large entry split" and warned at its first time (built in N1's floor). The guest cannot
tell: permissions stay per 4 KiB page as the record says, and the kernel reports residency the same on
every back end (every mapped page as in memory, kernel/counters.rs:41-44). N4 also measures ARM64's
contiguous bit (16 entries as one) for ranges that cannot take a block. Rejected: 4 KiB entries only (+8.5%
cycles before any translation, most of section 8's margin).

A later 16 KiB guest mode (brief) changes `PAGE_SIZE` and nothing in this design's shape: on ARM64 the floor
takes the granule from it (TCR_EL1 TG0 and TG1, chosen from `PAGE_SIZE` in one place, which today knows
the 4 KiB granule only, the one aarch64-paging's entries are built for, and stops the build for any other
size; the M5's virtual CPU offers both granules, ID_AA64MMFR0_EL1
TGran4 0 and TGran16 1, experiments/n0r3/idregs.out), and x86-64, which has no 16 KiB pages, writes each
guest page as four consecutive 4 KiB entries with the same permissions. The mirror and the lookup are per
guest address and do not change.

**What is global and what is per process.** One rule: an entry is global only when it is the same in every
process of every container in the machine, which is the inside's own code and data (with its read-write
map of all guest-physical memory, EL1 or ring 0 only, through which it writes page tables, slots and
thread pages) and the code area (a loaded translation is reached only through its own process's mirror or
table). Everything else is
non-global, written into the tables of each process it belongs to, and matched by that process's
address-space identifier: the guest's own memory, the entry mirror (ARM64) or table (x86-64), the thread
and register pages of the process's threads (so no process can address another's), and its container's
kernel heap. Each process has its own top-level table for the inside's half too, whose global entries are
shared. Switching process writes TTBR0 and TTBR1 (ARM64) or CR3 (x86-64) with the process's identifier and
no invalidation, so a switch between containers needs none either. Identifiers are given out across all
containers of the machine: 16 bits on ARM64 (TCR_EL1.AS; the M5's virtual CPU offers 16-bit identifiers,
ID_AA64MMFR0_EL1 ASIDBits 2, idregs.out) and the 12-bit PCID on x86-64 (the laptop's CPU has `pcid` and
`invpcid`, /proc/cpuinfo; whether its KVM passes them on is checked at N8). When they run out, the floor
starts a new generation and invalidates the whole TLB once, as Linux does on both architectures, counted
"address-space identifiers ran out". A change the kernel makes to one process's tables is followed by the
invalidation of those pages for its identifier (ARM64 `tlbi vale1is` with the identifier, last level only,
since the tables above the page are unchanged; `tlbi vae1is`, every level, only where an entry's level
changes, as when a block is split into a table of pages; x86-64 `invpcid`), broadcast to the other virtual CPUs as section 5.3 says. Rejected: global heap entries with a full
invalidation at every switch between containers (a cost at every turn that changes container), and
different heap addresses per container (a save could then come back only where its addresses are free).

**Memory from the host.** Guest-physical memory is one large reservation the monitor maps into the virtual
machine at start; the host gives it pages only when touched. The inside gives memory back through the link,
in whole host pages, and the monitor frees them through `VirtualMachine::give_back` (section 5.9), whose
Hypervisor.framework body unmaps the range from the virtual machine, maps
fresh anonymous memory over it and maps it in again (`hv_vm_unmap`, `mmap` with MAP_FIXED, `hv_vm_map`;
costs-round-3/virtual-machine/src/arm64/hvfrun.c:228-240). Measured on the M5 (mac-tests.jsonl "hvf_memory",
three runs of each): that frees the memory (the footprint after giving back 1 GiB fell from 1030.7 MiB to
6.2 MiB, and the guest read zeros after) in 14.7 to 16.6 ms per GiB and 60.5 to 62.7 ms for 4 GiB. Rejected:
`madvise` with MADV_FREE_REUSABLE, which took 4.9 to 5.7 ms per GiB but freed nothing there (the footprint
stayed 1030.7 MiB and the guest still read its data). On KVM the monitor gives back with `madvise`
MADV_DONTNEED on the range, after which private anonymous memory reads as zeros (madvise(2)); not measured
yet, N8 measures that it frees and what it costs. "More memory" past the reservation is a link call that
maps another region (brief: leaving for more memory), up to the bound of section 5.1.

### 5.3 Processes, fork, threads and virtual CPUs

- **A process** is one set of page tables, its mirror (or x86-64 table) and its record. `memory_new` makes
  an empty set and an empty mirror.
- **Fork** (`memory_duplicate`): the child gets a copy of the parent's tables in which every writable
  private page is read-only in both and marked copy-on-write; the first write by either faults to the
  inside, which copies the 4 KiB frame (or takes it back if it is the last holder) and carries on. This is
  Linux's own method; no host call is made. The child shares the parent's loaded translations (same bytes
  at the same addresses) and its mirror's slot pages: the inside counts the holders of every mirror frame as
  it counts guest frames, and copies a mirror frame before it writes a slot in it while another process
  holds it. Shared blocks (`shared_map`) map the same frames in each process, which a 4 KiB granule allows at
  any 4 KiB boundary, so the copying method runtime.md section 8 needs for 16 KiB host pages is not needed
  here.
- **Leaving a translation** (code changed, section 3.5, or the range unmapped) happens in one process:
  that process's slots for the range are emptied (copying the mirror frame first if shared) and its count of
  holders of the translation drops.
- **Threads** are register blocks plus the register page and, where there is one, the thread page (section
  3.3); a switch saves the withheld registers, the count and the float status (section 3.4) into the block
  and loads the next, with its stop word (ARM64: into TPIDRRO_EL0).
- **More than one virtual CPU for one container** is built only after the owner's ruling on parallel guest
  threads (the x86 memory ordering question is the same ruling). The pool of section 5.1 runs different
  containers at once from the start. The design allows it for one container: interprocessor interrupts
  through the interrupt controller for stops and for TLB shootdown when the kernel changes a table, as
  tr/contract.rs:17-27 asks of any runner.
- **Threads, as agreed with the owner in discussion** (not built; nothing here closes it off). Every group
  of threads that shares memory has a setting the embedder may change at any stop: take turns; parallel
  with page ownership recorded; parallel optimistic; parallel unrecorded. Groups always run in parallel
  with each other. How each part is met here:
  - One virtual CPU per running guest thread: the pool of section 5.1 hands a virtual CPU to each thread
    that runs, not only to each container; a stop is an interrupt to that virtual CPU (a software
    generated interrupt through the GIC from inside, `hv_vcpus_exit` or a raised interrupt from the host).
  - Each guest thread has its own table root: a thread's record holds its own TTBR0 root (ARM64) or CR3
    (x86-64) and its own address-space identifier, which the floor loads at every switch. Threads of one
    group share every lower table whose entries are the same for all of them and have their own where a
    page's permission differs between them (a page held by another thread for writing is invalid in this
    thread's view, a page several threads read is read-only in each). A change to a shared page's entry is
    written into every view holding it and invalidated for each identifier, broadcast. Under take turns
    every thread of a process has the process's one root, as today. N1 builds and tests the per-thread
    root (two threads of one process, one page writable in one view and read-only in the other, switched
    back and forth with no invalidation).
  - The kernel inside is entered by one caller at a time for now: a lock in the inside around every kernel
    entry (a system call, a fault the kernel resolves, a stop); a virtual CPU that finds it held waits in
    the inside, not on the host.
  - Take turns: today's shape, one virtual CPU per group, the kernel choosing the thread.
  - Page ownership recorded: a touch of a page the thread does not hold faults in its own view (above), and
    the kernel resolves it (the holder's view loses the page or its write permission, invalidated and
    broadcast, the toucher's gains it) and logs the transfer with both threads' positions; replay replays
    the transfers in order.
  - Optimistic: only locking and atomic instructions and kernel entries are put in order. The atomic forms
    are already routed (Aligned and Exclusive routes, section 3.4), so ordering them is one change there:
    each takes a ticket from a counter in the inside's memory before its commit point, logged. Its cost is
    measured when the owner rules.
  - Unrecorded: plain parallel runs, nothing logged.
  - A page's holder may be elsewhere (another machine, through the embedder): its entry is invalid in
    every view here, so the touch faults to the inside, and the kernel's `page_request` goes out through
    the link (section 5.6); the embedder answers by `page_give` (the page's bytes into a fresh frame here),
    `page_take` (the inside unmaps the page and hands its bytes out) or `page_copy` (a read-only copy here),
    each one link message of one page.
  - The setting changes at a stop: the floor rebuilds the group's views from the kernel's record.
- **Dirty pages and point-in-time copies** (the other project's design.md section 7): a mark write-protects
  every writable private page, the first write records it; a point-in-time copy also copies the frame aside
  first. Both are the write locks of runtime.md section 8 in real page tables.

### 5.4 Host calls leave and return

The kernel calls `Host` (abi/host.rs) from inside. The inside answers each method either inside or
through the link to the container's services on the host (section 6.2):
- inside: the monotonic clock (from the virtual counter or the TSC, scaled to nanoseconds with the offset
  the monitor gave at start), randomness from a pool the monitor fills through the link in batches,
  warnings counted inside and sent in batches;
- through the link: `output`, `backend_send` (files), every network call, the wall clock, more memory, and
  translation (section 3.2).
Events from outside (`Event`, abi/host.rs:57) arrive on host threads, which write them into a ring in
guest memory and raise an interrupt (section 5.5); the policy's `arrived` takes them from the ring at the
next exit, as `LocalSystem`'s would.

### 5.5 Stopping a running guest

- **A free turn's end** (`Turn::Free`, rt/lib.rs:28-30, which `LocalSystem` gives, rt/system.rs:256-267)
  is the inside's own timer interrupt, armed by the policy for the slice, in place of `LocalSystem`'s
  deadline thread and stop flag. On Hypervisor.framework nothing leaves the virtual machine: its virtual
  timer through its GIC costs 290 ns a tick at a 10 µs period with no exit to the host (mac-tests.jsonl
  "hvf_guest_timer", mode gic). On KVM x86-64 it does leave: on the Linux laptop the local APIC timer costs
  7.8 µs a tick at a 100 µs period with 3.1 exits per tick (experiments/kvm-laptop.out), so each turn's end
  costs about that there; at the kernel's slices of milliseconds that is expected to be well under 1%,
  measured at N8.
- **A turn that must end at an exact count** (`Turn::Counted { limit: Some(n) }`, rt/lib.rs:25-27). The test crate's
  `System` gives one whenever the kernel gives a slice (ktest/driver.rs:668-683; the slice is set when
  another thread can run, a deadline or a CPU timer is armed, kernel/task.rs:43-60). With no check at counted
  events, nothing stops a thread when its count reaches n at a pc not known in advance. How this is honoured
  is the owner's (decision 10): option A changes turns to end at a position or when stopped (R4); option B
  translates a container whose policy gives count limits with the replay check's compare at every counted
  event, against the thread page's limit word; decision 10 says what each option costs and option C, which
  needs an experiment. Until the owner rules, the native back end answers such a turn as `Failure` (named
  "turn limited by a count alone"), never by running past it. A turn with no limit (`limit: None`, which the
  test crate's `System` gives when the kernel gives no slice: a single thread with no deadline and no CPU timer) runs
  until it exits, so N3's single-process programs need no ruling; N5's suites wait for it (section 9).
- **An event from outside** is a real interrupt raised from a host thread: Hypervisor.framework's
  `hv_gic_set_spi` (Apple's documentation, macOS 15 and later; the round 3 prototype made the same GIC with
  `hv_gic_create`, hvfrun.c, but raised only its timer through it), KVM's `KVM_IRQFD` or `KVM_IRQ_LINE` with
  the in-kernel interrupt controller (Documentation/virt/kvm/api.rst). Neither is tried yet; N1 and N8 do,
  and measure the time to arrival. It lands in the inside's vector at an instruction boundary, and section
  4's map makes the position exact.
- **The host stopping a virtual CPU** (a save, ending the container, the user's stop): `hv_vcpus_exit` or a
  signal to the virtual CPU's thread with `immediate_exit`. Measured: 420 stops with Hypervisor.framework, pc
  always where it should be (mac-tests.jsonl "hvf_stop_by_hv_vcpus_exit"); on KVM 420 stops by signal, pc
  always right, 9.9 to 11.8 µs median from the decision to the registers read, with a tail to about 4 ms
  (experiments/kvm-laptop.out).

### 5.6 The link

One call shape: the inside writes a request in a page shared with the monitor and rings a doorbell; the
monitor answers in the same page and resumes the virtual CPU. The doorbell is the floor's one
platform-specific instruction: `hvc` on Hypervisor.framework (an exception exit, used by the round 3
prototype, hvfrun.c and guest.s, and by N1), an `out` to a port on KVM x86-64 (`KVM_EXIT_IO`, used by
vmrun.c), and a store to an unmapped guest-physical page on KVM ARM64 (`KVM_EXIT_MMIO`, from KVM's
documentation, Documentation/virt/kvm/api.rst; not yet tried: no KVM ARM64 machine is at hand; N10 adds
that page to the layout). Requests are typed messages, one `#[repr(C)]` type per request with its answer's
type, written over the link page's message words by field (`link::Message`), so the two sides share one
definition
(the `Host` calls, translate, more memory, give back, idle, the image's files at a container's start, and
the turn handoff of section 5.1). Measured exit cost 0.7 µs on the M5 and 5.6 to 6.2 µs on the Intel laptop
(brief).

### 5.7 Replay to a logged position

To stop a thread at (count N, pc P): that thread's stop word (section 3.3) becomes N (every other thread's
holds a value no count reaches), and the inside asks for P's piece again with a check at P: the
translator's one operation, "translate from this address", with a stop point added to the request (the
stop point is in the cache key, so such translations are kept apart). The new translation is loaded into
the target thread's process only (its slots, or its table on x86-64, are pointed at it, and the written code
made visible as section 3.2 says); other processes keep the original. The check compares the stop word with
the count without touching flags (ARM64: `mrs t, tpidrro_el0`, `sub t, t, c`, `cbz t, stop`, on withheld
registers with no memory read; x86-64: a form that keeps the guest's flags against the thread page's word,
chosen at N9) and enters the inside when equal; elsewhere nothing changes. Other threads of the same process
run the same code, and the check being against the running thread's own stop word is what makes only the
target stop. By the property of section 4, P is reached with count N at most once. Once there, the process's
slots go back to the original translation. Rejected: patching the first host instruction of P's run into a
branch to a check stub as a second way beside retranslation. It would save only the translation of one
piece per replay target (bounded look-ahead, milliseconds, and found in the cache the next time), and it
works only where P starts a run, so retranslation would still be needed for the rest: two mechanisms for one
job.

**Across outputs.** Both outputs count the same events with the same function (section 4), on the guest's
own instructions, and report positions at guest instruction boundaries. So:
- a position the WebAssembly output logged is a boundary, which the native output reaches by the check at
  that pc;
- a position the native output logged (an interrupt anywhere) is also a boundary after section 4's
  roll-back or roll-forward. The WebAssembly output reaches it the same way: its translator, asked for P's
  piece with a stop point, emits a check before P's instruction (at look-ahead none every instruction is its
  own piece, so this always exists). This is the same request in both outputs, which runtime.md section 12
  left as "step at look-ahead none until pc matches"; that needs the count to be reachable with a limit
  first, and the stop point needs nothing else.
The shared count is not enough on its own: the two outputs must also give the guest the same addresses
and the same memory size. With today's kernel they do not. It lays out each process top-down from 128 MiB
below `Cpu::address_space().end` (kernel/memory.rs:260, the end from kernel/loader.rs:503 and
kernel/task.rs:726), which is 2^45 natively on ARM64 and just under 4 GiB under the WebAssembly back end
(wcontainers-wasmtime/src/memory.rs:247-262), and it tells the guest that same end as its memory size
(kernel/container.rs:420, read by /proc/meminfo at fs/proc.rs:1374-1380 and halved for tmpfs limits at
container.rs:803). So every pointer, stack address and memory figure would differ between the outputs, and a
record from one could not replay on the other. Both are K2 (section 6.2): placement low first, never
depending on the ceiling, and the memory size given at start. Replay across outputs (N6) needs K2.
Asked of the translator track: the stop point in a translation request, and the count with the shared
`counted` function (T2, T3).

### 5.8 Making, saving and bringing back

- **Making**: at the first native container of a process the monitor reserves guest-physical memory,
  creates the machine, maps the inside at its link address and starts it at its entry with the start
  arguments in a page (measured to make a machine: 0.6 to 0.8 ms on KVM, 0.1 ms on Hypervisor.framework,
  brief). Each container then gets its heap region and its kernel instance inside, and the image's files go
  in through the link.
- **Saving a container**: with its threads stopped, its heap region (the kernel is plain data, "a container
  is saved by copying it with the guest memories", kernel/lib.rs:129, so its heap pages are the kernel), each
  guest process's pages by guest address (a frame shared by copy-on-write is saved once and its sharers
  named), and its threads' blocks with their registers. Translated code and the mirrors are not saved (they
  are a cache). The heap's pages hold addresses of the inside's code (trait objects), so a save names the
  inside's build identity and is brought back only by the same build.
- **Bringing back**: into any process's machine, the heap region at the same inside addresses (so every
  pointer in the kernel is still right), fresh frames for the guest pages with page tables made from the
  kernel's record, then `Execution::restored` and the kernel's `restore` (kernel/lib.rs:416-422), which makes
  every thread run from its registers and re-arms the deadlines on the clock offset the monitor gives.
- **Not by forking the host process.** The kernel tests save today by `fork`ing the whole test process in
  the test crate's `System`'s `stopped` and going on in the copy (ktest/driver.rs:730-771: `libc::fork` at :747,
  `cpu.restored()` at :765, `kernel.restore` at :770; runtime.md section 11). A copy made so cannot carry a
  virtual machine. The independent check measured it (check-n0d/forkvm.out and kvmfork.out, three runs
  each): on this Mac a forked child's `hv_vcpu_create` and `hv_vm_map` on the parent's machine fail
  (HV_BUSY, HV_BAD_ARGUMENT), and so do `hv_vm_create` and `hv_vcpu_create` of a new machine (HV_BUSY); on
  the KVM laptop the child's `KVM_CREATE_VCPU` on the parent's machine and `KVM_RUN` on the parent's virtual
  CPU fail with EIO, and only a new `KVM_CREATE_VM` works. The pool's threads are not in the copy either.
  So natively a save the policy asks for is made by the back end (R5, section 6.2): the policy's `stopped`
  decides, inside, that this turn saves; the native back end takes the container's image as above, ends the
  original (as the forking parent only waits), brings the image back as a new container in the same
  machine with a new services half whose clock reads differently, and the policy goes on there with
  `moved` and the kernel's `restore`, as the copy does today. N7 also brings an image back in a new host
  process.
- **Probing**, once per process: KVM is used when `/dev/kvm` opens and `KVM_CREATE_VM` works;
  Hypervisor.framework when `hv_vm_create` works (it needs the binary to carry com.apple.security.hypervisor,
  measured working with an ad hoc signature, brief). The probe also checks, inside a first virtual CPU, that
  the host's identity registers (ARM64 ID_AA64ISAR0_EL1 and the rest; x86-64 CPUID) include every feature
  of the offer and LSE, that DCZID_EL0 gives the presented block size, and that XCR0 can hold the offered
  state. A host that falls short runs the WebAssembly back end, warned with the missing feature's name.
  So every native node runs exactly the offer, which the other project needs (one feature set everywhere).
- **The behaviour probe**: only under decision 14's option (a) or (c), if the owner chooses one, are results
  that depend on the host's model copied where the host gives the meaning's (section 3.4). Until then they
  are routed, and the probe is not in the shipped path: N2 and N9 run it as a test and record, for the
  owner, whether this host would pass. It runs, at the guest's level in
  the first virtual CPU and compared with the meanings' answers: every load and store form not in the Split
  route, placed across a page boundary at every offset with each page refusing in turn (nothing may change);
  on x86-64 `rcpps`, `rcpss`, `rsqrtps` and `rsqrtss` over all 2^32 inputs (the meanings' formula was
  checked so on the reference, x86 plan line 1991), and every form whose flags Intel leaves undefined
  over every operand size and shift or rotate count with seeded values (the seed and the number of values
  named constants). Its verdict is kept in the translation cache's directory, keyed by the CPU's identity
  (ARM64 MIDR_EL1 and REVIDR_EL1; x86-64 CPUID's vendor, family, model and stepping, and the microcode
  revision), so it runs once per machine model; its time is measured at N2 and N9 (expected seconds for
  the 2^32 sweep). Under option (a), a host that differs anywhere would run the WebAssembly back end for
  that architecture, warned with the first differing form and input ("back end refused by the probe").
  Sampled inputs are not a proof: a host that matches every probed input and differs on another would
  differ from the WebAssembly output there, which is why routing is the default.

### 5.9 One interface of ours over KVM and Hypervisor.framework

A trait in the monitor's crate, resolved at compile time per host (a host has one of the two):
`Hypervisor` with `VirtualMachine` (the host's hardware virtual machine, never the guest's `GuestMachine`:
map guest-physical memory, give back the host pages of a range, raise an interrupt, make a virtual CPU,
give its interrupt numbers) and
`VirtualCpu` (run until an exit, read and write registers, be stopped from another thread). Exits are one
enum of ours: doorbell, fault the inside could not take, stopped. KVM ARM64 and KVM x86-64 differ only in
registers and the doorbell; Hypervisor.framework differs in the call names and in binding a virtual CPU to
its thread, which the pool of section 5.1 gives on both. Each method has one body per platform; nothing
above the trait names a platform.

## 6. The runtime's execution side, without a second loop

### 6.1 Where the loop runs

The loop is `Stepper::step` (rt/lib.rs:196-225), which calls `cpu.run` with an answer that calls
`kernel.exit`, `outside.arrived` and `kernel.next` at every exit (rt/lib.rs:263-289). With the kernel inside
the virtual machine, the loop must run there too, or every system call would leave the machine (0.7 to
6.2 µs against 21 to 32 ns inside, brief). So the same `Stepper` runs inside, built `no_std`, with the native
`Execution` and the `System`'s policy. On the host:
- `Container::run` (rt/container.rs:157-162, which is `Stepper::drive`, rt/lib.rs:227-244: step again and
  again, waiting in `idle` after `Step::Idle`) hands the virtual CPU one call that runs `Stepper::drive`
  itself inside, so a switch between guest threads (a pipe between two processes, say) never leaves the
  machine. The policy's `idle` (R3: its decision half) is a link request: the floor parks the container's
  stack and frees the virtual CPU (section 5.1), the services' waiting half waits on the user's thread that
  called `run`, and when it has waited it hands what arrived to the pool, where a free virtual CPU takes the
  parked stack up and `drive` carries on as if `idle` had returned. `run` returns when `drive` does, with
  `Finished` or `Ended`. A stop the host asks for (a save, the user's stop) is an interrupt that ends the turn, and the
  policy's `stopped` ends the run as it does today (rt/lib.rs:216-219). So there is one driver, `drive`, and
  one step, `Stepper::step`, on every back end; only the waiting half of `idle` is on the other side of the
  link.
- `Container::step` (rt/container.rs:145-150) keeps its meaning, one step, for a caller with its own event
  loop: one handoff to a pool thread and back per step, running `Stepper::step` once inside, including every
  `Step::Ran`. That is a pool-thread handoff (measured at N5) plus a doorbell exit (0.7 µs on the M5, 5.6 to
  6.2 µs on the Intel laptop, brief) per step, where a system call answered inside costs 21 to 32 ns; counted
  "step handed to a virtual CPU" (section 3.7), so a caller that steps natively is told to use `run` or
  accept it. When nothing can run, that step gives `Step::Idle` (rt/lib.rs:175) back to the caller, who waits
  as it likes, exactly as with the WebAssembly back end; `Container::idle` then waits in the host services
  and hands what arrived to the policy at the next step.

### 6.2 What changes, and who owns it

The runtime's user surface changes; these are interface changes, the owner's (decision 1):
- `Executor: Execution` (today's `BackEnd`, rt/container.rs:19) cannot hold: the native back end's `Execution` and `Cpu` side
  live inside the virtual machine, and the host side holds only a handle.
- `Runtime::container<O: System>` (today's `Outside`, rt/container.rs:85) takes a user's type, which cannot be compiled into
  the inside, a separate `no_std` program built for another target (section 5.2).
- `Container::outside` and `outside_mut` (rt/container.rs:169-176) return host references to a `System` that
  is no longer wholly on the host.
- `System::stop_flag` (today's `Outside::stop_flag`, rt/lib.rs:166) hands over a `StopFlag` that holds an `Arc` and a `Mutex` and writes a
  flag word (rt/lib.rs:35-89); natively a raise is an interrupt.

The changes:
- **R1 (runtime track): the loop and its traits `no_std`.** `Turn`, `Carry`, `Leaving`, `Answer`,
  `Execution`, `Step`, `Stepper` and the policy trait below move into a part of the runtime crate built
  without the standard library (with `alloc`). `StopFlag` stays in the std part and leaves the policy trait
  (R2). No behaviour changes for the WebAssembly back end.
- **R2 (runtime track): the container steps a place the back end names.** `Executor` (today's `BackEnd`) drops its `Execution`
  bound and its `stop_flag`, and names the place its loop runs: here (the kernel, the `Execution`, the policy
  and the `Stepper` as today's fields, rt/container.rs:137-163) or inside a virtual machine (a handle whose
  `step` hands one step to the pool and whose `run` runs `Stepper::drive` inside, section 6.1). It gives a
  stop handle whose `raise` writes the flag word (WebAssembly) or
  raises an interrupt (native), in place of `System::stop_flag`. `step`, `idle` and `run` keep their
  meaning, `Step` is unchanged, and `back_end` stays.
- **R3 (runtime track, and the kernel track for its test crate): the `System` in two halves.** `System` (today's `Outside`)
  (rt/lib.rs:146-167) splits into a **policy** (`turn`, `ran`, `arrived`, `stopped`, the decision half of
  `idle`, `cpu_warning`: what needs the kernel, `no_std`, run where the loop runs) and **services** (the
  `Host` calls the kernel makes for output, files, network, randomness, the wall clock, and the waiting half
  of `idle`: std, on the host, called directly by the WebAssembly back end and through the link by the
  native one). `Runtime::container` takes a policy and the user's services; `outside()` and `outside_mut()`
  become `services()` and `services_mut()`. `LocalSystem` (rt/system.rs:230-302) splits so, its
  deadline thread staying the WebAssembly back end's way to end a free turn. The test crate's `System` (ktest,
  `TestHost`) splits so too, its seeded `turn` and `ran` in the policy. A policy is a type the inside is
  built with: the runtime's inside carries the shipped policy and the test crate's; an embedding that
  wants its own policy builds its own inside program from the inside library with its type (one build
  script call). Rejected: forwarding every `System` call to the host (an exit for `ran`, `arrived` and
  `turn` at every system call, 0.7 to 6.2 µs each against 21 to 32 ns).
- **R5 (runtime track, and the kernel track for its test crate): a save the policy asks for is made by the
  back end.** Today the test crate's `System`'s `stopped` makes the copy itself with `libc::fork`
  (ktest/driver.rs:743-771), which no virtual machine survives (section 5.8). With R3 `stopped` is in the
  policy, inside the machine natively, where there is no host process to fork. So `stopped` only decides,
  and asks the place the loop runs (R2) to "save here and go on as the copy". The WebAssembly back end does
  that with the same `fork`, moved from the test crate into its std side with the rest of today's code
  around it (runtime.md section 11), until it saves to files (runtime.md section 11's final form), when it
  takes the image path too; the native back end takes the image, brings it back and ends the original
  (section 5.8). The test crate's half (the seed's moments, `moved`, the copy's report) stays its own. No
  behaviour changes for the WebAssembly back end. N7 needs it.
- **K1 (kernel track): nothing in the kernel's interface.** It is driven by the same `Cpu` and `Host` traits.
  The kernel's clock method asked for in runtime.md section 4 (`Context::monotonic` made public) is used by
  the inside's counter reads as by the WebAssembly back end.
- **K2 (kernel track, already asked by runtime.md section 8): placement low first, and the memory size
  given at start.** Two changes, both needed before N6. (a) The memory size the guest is told (MemTotal,
  `sysinfo`, tmpfs limits) comes from the container's start (runtime.md lines 408-410: "the runtime gives
  it in Start"), not from `Cpu::address_space().end` as today (kernel/container.rs:420), so a native ARM64
  guest is not told 32 TiB where the WebAssembly back end tells it about 4 GiB. (b) Placement, below. The
  native ARM64 back
  end's ceiling is 2^45 (section 3.2), under the container's width of 2^48, as the brief's ruling of
  2026-11-18 allows: "the container runs there while it stays under it and must move when it crosses; with
  nowhere to go, the request for memory fails, counted and warned". `Cpu::address_space` already is the back
  end's ceiling (abi/cpu.rs:34-35; the WebAssembly back end answers its own, wcontainers-wasmtime/src/lib.rs:
  2331-2335). What decides whether a container stays under 2^45 is placement. Today the kernel lays out
  every process inside `Cpu::address_space` and searches top-down from 128 MiB below its end
  (kernel/memory.rs:254-260, :407-430; the end from `cx.cpu.address_space().end`, kernel/loader.rs:503,
  kernel/task.rs:726). Natively that end is 2^45, so every address the kernel chooses is below it, but the
  addresses differ from those under a back end with another ceiling, as they already differ today between
  the stand-in and the WebAssembly back end (runtime.md section 8, lines 429-433). The placement runtime.md
  section 8 asks of the kernel track (low first, in tiers, never depending on the ceiling, lines 416-425)
  makes them equal: a process's highest address then stays small unless the program asks for an address
  (fixed, or as a hint) above the tier, and only a program that asks for an address at or above 2^45 crosses
  the native ceiling. That one then moves to a back end with a higher ceiling once moving exists (runtime.md
  section 12), or its request fails as the ruling says, counted "address above the ceiling". So the native
  back end needs no change of its own in the kernel; it needs the two changes already asked, and until
  they land its addresses and the memory size it reports differ from the WebAssembly back end's (section
  7.2), so N6 and N11 wait for K2. Rejected: the width
  2^45 for every ARM64 container (it would refuse fixed addresses and move hints above 2^45 that the
  reference maps, on every back end, to save a move only the native one needs); 2^48 natively with a table
  lookup on every jump through a register and every return (decision 9).
- **K3 (kernel track, for the other project): one process closure's kernel state extracted and merged**
  (de-requirements-map.json, "Extracting and merging one process closure's kernel state", open). The native
  back end adds only transport: the inside calls the kernel's extract and sends the result through the link,
  and a merge is received through the link and handed to the destination kernel, which may be under either
  back end. Nothing here closes it off.
- **T1 (translator track): the finder apart from the WebAssembly plans** (section 3.1), a module both
  emitters call, so a WebAssembly-only build has no native emitter. A native build still carries the meanings
  and wasm-encoder, since the finder's `effects` come from the meanings (section 3.1); it makes and runs no
  WebAssembly.
- **T2 (translator track): the shared `counted` function** and the count in the WebAssembly output from it,
  with the test of section 4's property.
- **T3 (translator track): a stop point in a translation request** (section 5.7).
- **X1 (x86 track): `Native` for x86-64** follows the x86 decoder's forms; this track writes it with the x86
  plan's meanings as its source, as `Native` for ARM64 follows arm64's, and the tests of section 3.4 check it.
- **R4 (runtime track, only under option A of decision 10): a turn ends at a position or when stopped.**
  `Turn::Counted { limit }` becomes a turn that ends at a (count, pc) or has no end but a stop; the progress
  handed with each exit stays. The WebAssembly back end reaches a position with the same stop point (T3). The
  test crate's `System`'s seeded turns become stops whose positions are recorded, so a record repeats on every back
  end, and a seed no longer fixes the turns on its own.
- **The other project's views of kernel state** (de-requirements-map.json, "Borrowed views of kernel
  state"): with the kernel inside a virtual machine, a host program cannot borrow its data. Whatever reads
  views must be built into the inside (as a policy is) or receive copies through the link. Nothing here
  forbids either; it is the owner's (decision 11).
- **A1 (this track): the toolchain targets** `aarch64-unknown-none` and `x86_64-unknown-none` in
  rust-toolchain.toml.
- **G1 (this track, in other tracks' crates, allowed by the orchestrator by name):** the `Native` trait in
  wcontainers-guest-cpu, its ARM64 implementation and the static routing test in wcontainers-arm64, and
  the native emitter in wcontainers-translator behind a cargo feature (N2). Each is its own commit, listed
  in the status file; the WebAssembly output and every existing test of those crates stay unchanged.

### 6.3 The native `Execution`

`run(thread, resume, turn, answer)` (rt/lib.rs:132-138) inside: applies the resume to the block, loads the
withheld registers, arms the timer for the turn, drops to EL0 or ring 3 at the thread's piece and takes every
exception that comes back:
- a system call (the routed instruction or `svc`/`syscall` itself) is `Exit::Syscall`, handed to
  `answer.exit` at once, with no exit from the virtual machine; "carry on" returns to EL0 at the next
  instruction, as the WebAssembly back end's hook returns;
- a page fault is settled by the back end's write locks (copy-on-write, tracking, translated code) and
  carried on, or is `Exit::MemoryFault` with the guest's address and access (any fault in the inside's half
  as a fault where nothing is mapped);
- an instruction abort or `udf` in the mirror is a missing slot (section 3.2); a PC alignment fault from the
  mirror is the guest's misaligned target (`TrapKind::Alignment`); an instruction abort in the guest's half
  is `Exit::MemoryFault` with `Access::Execute`;
- the timer or a raised interrupt is `Exit::Stop`;
- a Float status call (section 3.4) is answered inside and carries on; an x86-64 #XM is the `Exit` of
  `TrapKind::FloatException` (gcpu/lib.rs:150), as the meaning raises it;
- a routed trap is the `Exit` of its `TrapKind`; a failed alignment test is `Exit::AlignmentFault`.
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
- Large guest pages: the guest has 4 KiB pages only (brief); the host may back memory as it likes, and the
  floor's 2 MiB entries (section 5.2) are underneath, never pages the guest sees.
- Features beyond the translator's offer, on any host (section 3.4), and native runs on a host lacking any
  feature of the offer or whose behaviour probe differs from the meanings (section 5.8; decision 14).
- Running a native container's virtual machine across a host `fork` (section 5.8): saves are the back
  end's (R5).
- 32-bit guests (AArch32, x86-32).
- More than one virtual CPU for one container until the owner rules on parallel guest threads.
- Saving translated code with a container (it is a cache).
- Nested virtualization inside the guest; KVM inside the container.
- Finishing any guest instruction in software in the floor (x86-64 `rep` is finished by the hardware,
  section 4).

### 7.2 Where native behaviour still differs from the WebAssembly output

What of ours the guest's level can reach (the brief's addendum), by platform:
- ARM64: the code area and the mirror are execute-only at EL0, so a guest load or store there faults, as on
  the WebAssembly output (check-n0/xo.out cases 2, 3 and 5; the floor reports it as a fault where nothing is
  mapped). The inside's own memory and every container's heap are EL1-only. The stop word is in
  TPIDRRO_EL0, not in memory (section 3.3). What differs: each guest thread's register page (readable and
  writable at EL0: a guest load or store there succeeds, where Linux and the WebAssembly output fault),
  mapped only in the thread's own process (section 5.2), because translated code at EL0 uses it; and, only
  under option B of decision 10, the thread page holding the limit word (read-only at EL0: a load succeeds
  and a store faults).
- x86-64: as on ARM64 for the inside's memory (supervisor-only), and for the thread and register pages
  (the thread page read-only at ring 3). What differs besides: the code area (with the x86-64 per-process
  tables), which is readable at ring 3 since x86-64 has no execute-only pages without protection keys,
  which are never relied on (brief).
What a guest can do there, and no more: read our pages and its own translated code, and write its own
register page, which changes only its own withheld registers' values and its own exclusive monitor, as if
it had written those registers. It cannot change where host code goes (only addresses the inside or the
translator wrote are jumped to, section 3.2), cannot run a host instruction that was not emitted for a
guest instruction's route, and cannot change its stop word, limit word or lookup cache (in TPIDRRO_EL0 or
the read-only thread page), so replay and routing do not depend on the guest leaving these pages alone. In
every case the address is in the inside's half (ARM64 bit 55 set; x86-64 the upper canonical half), which
Linux never gives a user program; only a program that makes such an address on purpose sees it. It cannot
be counted without a check on every guest access, which the ruling's speed forbids, so it is named here and
carried on the kernel's written list of differences (decision 4). Replay across the two outputs rests on
positions, the inputs logged and the shared count, none of which uses these pages; a program that reads them
behaves differently on the two outputs from that access on, which the replay test reports as a mismatch at
that position, never silently. N1 tests that EL0 cannot write TPIDRRO_EL0, N8 that a store to the thread
page faults, and both that one process cannot address another's pages.

Addresses and memory size, until K2 lands (section 6.2). Today's kernel places every mapping top-down
from 128 MiB below the back end's ceiling and tells the guest that ceiling as its memory size, so a native
ARM64 guest gets addresses near 2^45 and is told of 32 TiB, where under the WebAssembly back end it gets
addresses under 4 GiB and is told of about 4 GiB. Every pointer, stack address, /proc/meminfo figure and
tmpfs limit differs, and replay across the outputs cannot match. K2 removes this difference entirely; N6
and N11 wait for it.

A permission change to a running function's own stack frame. The translator's contract lets the WebAssembly
output notice a change to a page of a running function's own frame, at or above the stack pointer, only at
the next function entry, not at once (tr/contract.rs:20-22). The native output's page tables apply every
change at once, as Linux does. So a program that changes the permissions of its own live frame (say with
`mprotect` making it read-only) and then writes to it before the next call faults natively and on the
reference, and not on the WebAssembly output. This is the WebAssembly output's difference from the
reference; natively nothing can delay the change without a check on every access. Replay across the two
outputs diverges at that access, and the replay test reports it as a mismatch there. Named here and on the
kernel's written list; decision 13 asks whether the translator track closes it.

Handled, and proven by tests rather than assumed:
- **Store exclusive** (section 3.4): the differential test (N2) for its faults and status, N6 under
  interrupts.
- **Misaligned and tagged branch targets, and misaligned atomics** (sections 3.2, 3.4): the differential test
  and the every-offset test (N2, N5).
- **x86-64's trap flag and AC** (section 3.4): the differential test's stepped runs (N9).
- **Floating point**: native ARM64 on ARM64 computes what Arm defines, which the WebAssembly output also
  produces (bit-exact not-a-numbers, translator plan); the native output never changes FPCR outside the
  presented bits; the exception flags the guest can read (FPSR, MXCSR, saved state, signal frames) are the
  ones the WebAssembly output keeps, by the Float status route (section 3.4), proven by the differential
  test's float status cases (N2, N9).
- **The hint space and the other forms decoded as nothing** (section 3.4's Nothing route): never copied,
  checked by the static test's rule and the differential test's full-width registers (N2).
- **A program reading the counter in a loop** sees the kernel's clock on both outputs.
- **Loads and stores that cross onto a refusing page** (section 3.4's Split route): change nothing, as in
  the WebAssembly output, checked by the differential test's page-crossing cases (N2, N9); every load and
  store that can cross a page is tested and probed first until the owner rules on decision 14.
- **Register collisions Arm leaves CONSTRAINED UNPREDICTABLE** (section 3.4): the meaning's result, checked
  by the differential test's drawn register fields (N2).
- **x87**: the transcendentals are worked out inside as the meanings do; the saved last-instruction
  pointers are the meaning's; x87 arithmetic and its status word are the hardware's, which the meanings
  match on the reference (N9's differential test).
- **Results that depend on the host's model** (x86-64's approximate reciprocals and undefined flags; loads
  and stores that can cross a page): routed on every host until the owner rules on decision 14 (section
  3.4), so nothing differs; checked by the differential test (N2, N9). Under decision 14's option (a) a
  residue would remain: the probe samples the undefined flags with seeded values, so a model that matches
  every probed value and differs on another would differ there.

## 8. Tests and the pass bar

- **The same suites.** The kernel's layer 2 and layer 3 (the Linux Test Project) with the runtime and the
  native back end behind the boundary, with the results the WebAssembly back end has, on the Mac
  (Hypervisor.framework, ARM64) and on the Linux laptop (KVM, x86-64, once the x86 guest passes its own
  bar); the kernel's layer 1 random sequences over the native `Cpu` side built for the host (its page-table
  writer and walker are plain Rust over a byte buffer standing for guest-physical memory, so they need no
  virtual machine), in place of the buffer CPU; the translator's layers run with the native output against
  the reference. Under option B of decision 10 the test crate's `System` gives the same turns on both back ends for a
  seed, so each test's output is compared, not only its verdict; but the test crate's `System` gives count limits, so
  those suites run the translations that carry the check at counted events, not the check-free ones the
  shipped policy runs. So under B the suites run a second time with the shipped policy (free turns) and
  their verdicts are compared, and the every-offset and replay tests run on check-free translations. Under
  option A the verdicts are compared and the records are replayed across.
- **Routing.** The static and the differential tests of section 3.4, for every form of each guest crate.
- **Count.** Section 4's two tests: no cycle free of a counted instruction (E's property), and the count at
  every stop identical across look-ahead settings and across the two outputs.
- **Positions.** The every-offset test (section 4). Seeded: the seed and the number of cases are arguments,
  printed with every failure.
- **Replay across outputs.** Seeded programs and the translator's benchmark programs are run with seeded
  events delivered at seeded interrupts (native) or stop checks (WebAssembly), the `System` recording every
  input and position. Each record is replayed on the other output and on the same one, and at every event
  the registers, the count and a hash of every process's memory must be equal. One command, with the seed
  and the number of runs as arguments.
- **Speed.** The translator's benchmark programs (the eight bench.c workloads and calls.c of the cost
  rounds) and real programs from the kernel's layer 2 (busybox, python, git), each run plainly on the host
  and through the native back end, alternating, 15 pairs or more, with the machine, its load and the share
  of each run on performance cores recorded. Measured per run: cycles and instructions retired of the whole
  host process (macOS `proc_pid_rusage`'s `ri_cycles` and `ri_instructions`, which count the virtual CPU's
  thread, as round 3 measured them, costs-round-3/virtual-machine/src/arm64/hvfrun.c:286-287; on Linux the
  same two counters from `perf_event_open`), and wall time for the record. Running inside a virtual machine
  costs something before any translation: round 3's untranslated runs over the same eight workloads cost
  +8.5% cycles with 4 KiB stage 1 pages and +2.8% with 2 MiB blocks (section 5.2, larger entries;
  instructions +0.9% and +0.7%), so the bar's margin pays for the virtual machine as well as the
  translation, and the floor uses larger entries to keep its share near the second figure. To know the
  shares, the method also runs each workload untranslated at EL0 on the floor (a test program, as round 3
  did; never shipped) and records its ratio beside the translated one's. The bar is in cycles, not
  wall time: under this Mac's load (55 to 141 in round 3) wall-time intervals spread tens of points
  (summary-main-r0.txt line 57: sort under En, wall +6.3% [-54.2 .. +916.8], cycles +7.2% [+4.4 .. +10.0]),
  so a wall-time bar could not be decided there. Proposed bar, two parts:
  - Compute: the geometric mean, over the eight bench.c workloads, of each workload's median cycles ratio
    within E's measured cycles cost plus 10 points, E's cost being the figure N2 measures with the add's
    placement free (section 4; round 3 measured E at +13 to +14% cycles on the M5, and the forms from +5.1%
    (Hn) to +17.2% (Tn), geometric means over the eight workloads, summary-main-r0.txt lines 115-118).
  - System calls: the back end's own share of a system call answered inside, at or under 100 ns. Method:
    a guest program makes 10^6 `getppid` calls in a loop, against the same program with each call replaced
    by an instruction that does nothing, as runtime.md section 10 measured the hook (15 alternating pairs or
    more, through the native back end, the per-call time being the difference of the two runs' wall times
    divided by 10^6; the load recorded). The kernel's own share of `getppid` is measured in the same run and
    on the same machine by runtime.md section 10's layer 1 method (116 to 120 ns there). The back end's
    share is the per-call time less the kernel's share's median; it includes the trap into the inside, the
    loop (`Stepper::step`, the policy's calls) and the return (expected about 30 ns plus the runtime's
    10 ns, from the brief's 21 to 32 ns and runtime.md section 10; not yet measured together).
  Decision rule, for each part: a 95% interval (for the geometric mean; for the median per-call time less
  the kernel's share) by a seeded bootstrap over the pairs (the seed and the number of resamples are
  arguments); pass when its upper end is at or under the bar, fail when its lower end is over it; otherwise
  run again with twice the pairs, up to 60, and if still undecided, report it undecided to the orchestrator
  with the figures, to be run on a quieter machine. The orchestrator has decided this bar (decision 6),
  with E's cost as N2 measures it.

## 9. Milestones

Each milestone ends with its bar run and recorded in docs/status-native.md, with the number of cases of
every seeded test. Every bar also includes the repository's automatic literal check passing (branch
phase-1-literals, once it has landed with a passing milestone and is merged here; until then the brief's
rule is applied by hand and what is left is listed in the status file). What each needs from others is named; nothing before N3 needs another track or an owner's
decision except the names of decision 2 (until the owner rules, the suggested names are used and renamed at
the ruling).

- **N1, the floor on Hypervisor.framework.** The inside (floor only) boots at EL1 under a monitor using
  `applevisor`; `PAGE_SIZE`-granule tables with per-process TTBR1 tops, global and non-global entries as
  section 5.2 says; an EL0 program (hand-written, not translated) makes doorbell calls; timer and raised
  interrupts land in the inside. Bar: one test binary (signed with the hypervisor entitlement) runs each case
  and checks its exception class, fault status, address and registers against the expected ones written in
  the test: execute-only code runs and refuses loads and stores; guest pages refuse execution; a misaligned
  `br` gives EC 0x22 at the target; tagged loads and branches behave as TBI0 says; TPIDRRO_EL0 written at
  EL1 is read at EL0 and an EL0 write to it is undefined (as experiments/n0r4/tpidrro.out); the register
  page is readable and writable at EL0; two processes (of two containers, with different identifiers) do not
  see each other's mirror, register pages or heap, after switching back and forth with no invalidation; two
  threads of one process, each with its own table root and identifier (section 5.3, threads), see one
  page writable and read-only respectively, switched back and forth with no invalidation; two adjacent
  4 KiB guest pages inside one 16 KiB host page with different permissions fault apart; a 2
  MiB block reads and writes at EL0, and after a permission change to one of its pages only that page
  refuses; code the monitor writes into a frame after the guest has run code next to it runs as written
  after the inside's maintenance (section 3.2; the independent check's late-write case, which ran stale code
  without it); EL1 software step of EL0 gives EC 0x32 at each next instruction (as
  experiments/n0r3/step.out); a timer interrupt and an `hv_gic_set_spi` interrupt each land; a second
  virtual CPU on another thread runs; memory given back through the link lowers the process's footprint.
  Every case passes; the doorbell round trip, an interrupt's arrival and the delay of a stop from another
  host thread (300 stops by default, seeded; the count and seed are settings of the test) are measured and
  recorded, and so
  is decision 10's option C case, recorded and not a pass condition: in a machine with EL2 enabled and
  PMUVer set by the host (section 5.1), whether the emulated counter counts EL0 instructions, whether its
  overflow interrupt reaches the inside, and how late. (Run at N1; once C was out its code had no use and
  was removed: the trial as run is at commit fe3a4bff, crates/wcontainers-inside/src/bin/floor-cases/cases.rs
  `counter_trial` and tests/floor.rs.) The cases program is a test program in the inside's package
  (`src/bin/floor-cases`), never part of the monitor: the test builds it for the bare-metal target itself,
  and its requests (reports, measurements, the stop test's and the rest) are its own set after the floor's,
  in one file the program and the test share. The floor's own set is what the floor sends (print, fatal) and
  give back. Every report names its case, and the test checks that every case of the bar reported exactly
  once.
- **N2, the native ARM64 output, instruction by instruction.** `Native` for ARM64, the small encoder, every
  route of section 3.4 at look-ahead none, the stand-in register for withheld ones, the position map, run at
  EL0 on N1's floor with no kernel: each case's state is set by the floor and read back by it. Needs no
  other track's work; its changes in other tracks' crates are this track's, as the orchestrator allows by
  name (section 6.2, G1): the `Native` trait in wcontainers-guest-cpu, its ARM64 implementation and the
  static routing test in wcontainers-arm64, and the native emitter in wcontainers-translator behind a cargo
  feature, each its own commit, with the WebAssembly output and every existing test unchanged. Also
  measures E's cost with the add's placement free (section 4) on the eight bench.c workloads, instrumented
  as round 3 did and run at EL0 on the floor, alternating with plain runs, with the load recorded; the speed
  bar uses that figure. Bar: the static routing test passes for every ARM64 form; the differential test
  passes for every ARM64 form at 1000 seeded cases per form or more (seed and count recorded), against the
  WebAssembly output under wasmtime on the same Mac; misaligned and tagged targets, misaligned atomics, the
  store exclusive's faults, `clrex` between a load and a store exclusive, the hint space with full-width
  x30, every float status form after a float operation that raised flags (tracking on and off, FPCR
  default and not), every load and store across a page boundary with each page refusing, and every
  register collision of section 3.4 are among its cases. The behaviour probe (section 5.8) passes on this
  M5, and its time is recorded.
- **N3, the first real programs.** The kernel and the `no_std` loop inside with the test crate's policy,
  translation on the host through the link, the count's adds from the shared `counted` (or, until T2, the
  same function in this track's code, moved at T2), one process at a time. Needs decision 1 and its changes
  R1 and R3 (by the runtime and kernel tracks, or by this track if they agree) and A1. Bar: every command of
  the kernel test crate's tests/busybox.rs `text` and `files` tests, run with the root file system in memory
  (the first of `compare`'s two runs, ktest/busybox.rs:169-175), gives the reference's output, exit status
  and files inside Hypervisor.framework at look-ahead none. Those runs are single-process with no timer and
  nothing owed by a test backend, so the test crate's `System` gives them turns with no limit (ktest/driver.rs:
  668-683, kernel/task.rs:43-60). The second run of `compare` (the root on the test backend, whose owed
  answers give count limits) waits for decision 10 and N5.
- **N4, linking and speed.** Needs T1 (the finder apart from the WebAssembly plans, section 6.2; today
  `plan` and `Finder` are private to the translator, tr/full.rs:44 and :228), by the translator track or by
  this track if it agrees. Bounded and full look-ahead, load-time addresses, linked exits, the entry
  mirror, the call stub, the cache with the native key and disk files; the dynamic register uses of section
  3.3 counted and the three withheld registers chosen; 2 MiB entries and their splitting (section 5.2), with
  the contiguous bit measured against 4 KiB entries for ranges that cannot take a block; the Split route's
  page test measured on the benchmark programs (the cycles of each against the same build with the test
  left out, recorded, not a pass condition). Bar: N3's bar still
  passes at every look-ahead, the count test of section 4 gives the same count at every stop under every
  look-ahead (native side), and the speed method of section 8 on the Mac meets the speed bar by its
  decision rule, every program's figures recorded beside the untranslated run's.
- **N5, the execution side.** Needs decisions 1, 10 and 12 and the changes they bring (R2, R3, and R4 under
  option A), done by their owners or by this track if they agree. `Execution` and `Executor` for native,
  `run` and `step` as section 6.1 says, the pool of virtual CPUs with parking at `idle` (section 5.1),
  several containers in one process (more of them in `run` at once than the platform's virtual CPUs, one
  case),
  processes, fork with copy-on-write, threads on one virtual CPU, shared blocks, write locks, code a program
  writes (Node and Java from layer 2), the every-offset test. Bar: kernel layers 2 and 3 on the Mac give the
  WebAssembly back end's results (as section 8 says for each option); layer 1 passes over the native `Cpu`
  side; the every-offset test passes with no mismatch over the translator's benchmark programs and layer 2's
  busybox, its seed and case count recorded; the handoff cost of `step` is measured and recorded.
- **N6, replay.** The replay check, the record and replay `System`, store exclusive under interrupts. Needs
  the translator track's T2 (the WebAssembly output counting by the shared `counted`) and T3 (the stop point
  in a translation request), for the WebAssembly side of the test and for this side's stop point; the
  kernel track's K2 (placement low first and the memory size given at start, section 6.2), without which
  the two outputs give the guest different addresses and memory sizes (sections 5.7, 7.2); and N5. Bar:
  the count test of section 4 across both outputs (the same count at every stop), and the cross-output
  replay test both ways, at least 100 runs each way, no mismatch, the number of runs recorded.
- **N7, save and bring back.** Saving a container and bringing it back in the same machine and in another
  process's machine. Needs R5 (section 6.2), by its owners or by this track if they agree, and N5. Bar: the
  kernel tests that save (procsignals and the rest of layer 2's saving tests) pass with native saves made
  as R5 says (no host `fork`), and a container saved in one process finishes in another with the output of
  an unsaved run.
- **N8, KVM x86-64 floor.** N1 on the Linux laptop with `kvm-ioctls`, ring 0 and ring 3: the same kinds of
  case in x86-64 form (NX, supervisor-only pages, the read-only thread page, GS base, CR0 and CR4 settings and
  whether UMIP and PCID are offered, `KVM_IRQFD`, the TSC-deadline timer's exits per turn, the DR0 breakpoint
  after a `rep`, TF single step from ring 0 of ring 3 and what each #DB costs, give-back by MADV_DONTNEED,
  and decision 10's option C case for ring 3). Bar as N1's.
- **N9, x86-64 native output.** `Native` for x86-64 (X1), `iced-x86` re-encoding, the withheld registers
  and their stand-ins, RIP-relative operands, the lookup cache, `rep`, single stepping by TF, the x87
  transcendentals inside (with `astro-float` built `no_std`, or the reason it cannot be), the x87 pointer
  fields of saves, the behaviour probe, and the count. Bar: N2 to N6's bars on the laptop, once the x86
  guest passes its own bar, with the behaviour probe passing on the laptop's i7-8750H and its time
  recorded.
- **N10, KVM ARM64.** The ARM64 inside under KVM. Bar: N5's bar on an ARM64 Linux machine with KVM (one must
  be found or rented; none is at hand).
- **N11, the other project's needs.** Needs K2 (equal addresses and memory size on both back ends, for
  bars 2 to 4) and N6. Dirty pages and point-in-time copies as `Execution` methods once a user
  exists (runtime.md section 12); moving a running process between the native and the WebAssembly back ends,
  and to a back end with a higher ceiling when it crosses 2^45 (section 6.2, K2); carrying K3's extract and
  merge through the link once the kernel has them. Bar: (1) over seeded programs writing seeded pages, the
  dirty set reported equals the set of pages whose bytes changed (every page hashed before and after), at
  least 100 runs; (2) a point-in-time copy taken at a position equals the memory the WebAssembly output has
  when replayed to that position; (3) once K3 exists (a native save is the inside's heap image, which only
  the same inside build brings back, section 5.8, so a move between back ends needs K3's structured
  extract and merge as its transport), a seeded program moved between the back ends at seeded positions,
  at least 100 runs, finishes with the reference's output and the memory hash of a run never moved; (4) once
  K3 exists, a process closure extracted inside and merged into a kernel under the WebAssembly back end, and
  the reverse, runs on to the same output.

## 10. Decisions for the owner

Settled by the orchestrator as internal (brief, 2026-11-18 evening): 3 (yes), 4 (accepted as a named
difference, with the two free measures), 6 (the bar as written, with E's cost as N2 measures it), 8 (left
out), 9 (option a), 11 (built into the inside), 13 (kept named); for 10, N1 tries option C. Still the
owner's: 1, 2 (the names chosen so far are used), 7, 12, 14 (routing is built until then), and 10's final
choice. The count's definition is E (section 4).

1. The runtime's interface for a back end whose loop runs elsewhere (section 6.2): `Executor` without
   `Execution`, a stop handle from the back end, and `System` split into a `no_std` policy built into the
   inside and host services that are the user's type (`Runtime::container` taking both, `services()` in
   place of `outside()`), and a save the policy asks for made by the back end rather than by the test
   crate's own `fork` (R5)? Recommended: yes; the alternative, every policy call through the link, leaves the
   machine at every system call, and no virtual machine survives a host `fork`.
2. Names: the monitor crate (`wcontainers-hypervisor`?), the inside program (`wcontainers-inside`?), the
   native back end (`wcontainers-native`?), the policy and services halves (`Policy`, `Services`?), the
   monitor's interface over the host's hypervisor (section 5.9: `Hypervisor`, `VirtualMachine` for the
   host's hardware virtual machine, kept apart from `GuestMachine`, which is what the guest is told it runs
   on; `VirtualCpu`, `Stopper`?), the slow paths of section 3.7, and whether the native emitter is a module of `wcontainers-translator` behind a
   cargo feature (recommended) or a crate of its own.
3. The translator split into a finder and two emitters behind features (T1), the native build still
   carrying the meanings and wasm-encoder for the finder's effects (section 3.1): agreed? Recommended: yes.
4. Our pages the guest's level can reach, only at addresses Linux never gives a user program (section 7.2):
   each thread's register page (readable and writable, its own withheld registers' values and exclusive
   monitor); on x86-64 also the thread page (readable) and the readable code area, and on ARM64 the thread
   page only under option B of decision 10 (otherwise the stop word is in TPIDRRO_EL0, section 3.3). Accept
   as a named difference on the
   kernel's written list? Recommended: yes; nothing closes it without a check on every access, and nothing
   in it lets a guest steer host code or replay.
5. Licences: `kvm-ioctls` (Apache-2.0 OR MIT), `kvm-bindings` (Apache-2.0), `applevisor` (MIT OR
   Apache-2.0), `aarch64-paging` (MIT OR Apache-2.0), `x86_64` 0.15.5 (MIT/Apache-2.0), `aarch64-cpu`
   (MIT/Apache-2.0), `talc` (MIT), `iced-x86` (MIT). `dynasmrt` (MPL-2.0) rejected. Agreed?
6. The speed bar of section 8: geometric mean cycles over the eight bench.c workloads within the chosen
   count definition's measured cycles cost plus 10 points, and the back end's own share of a `getppid`
   answered inside (the per-call time less the kernel's own share) at or under 100 ns, each decided by the
   bootstrap rule there? The margin pays for the virtual machine too (round 3: +2.8% cycles with 2 MiB
   blocks, +8.5% with 4 KiB pages only, before any translation). Recommended: yes, with larger entries as
   section 5.2 says, to be revisited once the count definition is chosen.
7. A guest of the other architecture on a native host (section 11): which option? Recommended: option 1
   until another is asked for.
8. Windows Hypervisor Platform: left out, as the ruling names only KVM and Hypervisor.framework? Recommended:
   left out until asked.
9. The native ARM64 back end's address ceiling (section 6.2, K2): (a) 2^45 as a back end ceiling under the
   container's width of 2^48, by the brief's ruling on ceilings, so a jump through a register costs six
   instructions with no memory read, and a program that asks for an address at or above 2^45 (fixed, or as
   a hint) moves or, until moving exists, has that request fail, counted and warned; it gives the same
   addresses as other back ends once the kernel places low first (asked already by runtime.md section 8),
   and until then natively places top-down from 128 MiB below 2^45 (kernel/memory.rs:260); (b) the width
   2^45 for every ARM64 container on every back end, which refuses fixed addresses and moves hints above 2^45
   that the reference maps, everywhere; or (c) 2^48 natively with a table lookup on every jump through a
   register and every return. Recommended: (a), the ruled route; N4 measures (c) against it, and if (c)
   costs nothing measurable it is taken instead.
10. Turns that end at an exact count (section 5.5), which the test crate's `System` gives:
   (A) R4: turns end at a position or when stopped on every back end; the test crate's `System`'s schedule is recorded
   and replayed, so a seed no longer fixes it alone. Costs an interface change and the test crate's `System`'s
   seeds; keeps one translation and no check.
   (B) A container whose policy gives count limits is translated with the replay check's compare at every
   counted event, against the running thread's limit word, counted as "turn limited by a count"; a policy
   that gives free turns (the shipped one) never carries it. Keeps the test crate's `System` and its seeds unchanged
   and repeatable across back ends, with no interface change. Costs: it is a check in translated code, which
   the ruling forbids for stopping; it makes two translation modes chosen by the policy, against the rule of
   one primitive (the same check as replay's, placed more widely, and in the cache key); and the suites,
   all run with the test crate's `System`, would test the check-carrying translations, not the check-free code the
   shipped policy runs, so section 8 runs them again with the shipped policy (verdicts only) and keeps the
   every-offset and replay tests on check-free code.
   (C) No check: the floor arms an instruction counter's overflow interrupt for the guest's level short of
   the limit (every counted event retires at least one instruction, its add, so n minus the count retired
   instructions cannot pass the limit; less a margin for the interrupt's lateness), then single-steps
   (section 4) to the exact count, as record-and-replay debuggers do (an idea, nothing copied). Keeps one
   translation and the test crate's `System`.
   Needs a guest performance counter with an overflow interrupt on both platforms. On Hypervisor.framework
   that means a machine with EL2 enabled, which section 5.1 otherwise has no use for: with EL2 disabled, as
   planned, every PMU access exits to the host and the virtual CPU reports no PMU; with EL2 enabled the
   framework emulates the PMU only when PMUVer is set, which the host can do (hv_vm_config.h; measured,
   section 5.1). Whether that emulated counter counts EL0 instructions and raises an overflow interrupt is
   untried. KVM documents a virtual PMU (Documentation/virt/kvm/api.rst), untried on the laptop. Costs a few
   interrupts and up to a margin's worth of steps per limited turn, on test runs only, and on
   Hypervisor.framework the floor entering at EL2 and running at EL1 below it.
   Recommended: let N1 and N8 try C (N1: EL2 enabled, PMUVer set by the host, an overflow interrupt after a
   given number of EL0 instructions, and its lateness; N8: the same for ring 3 under KVM); take C if it works
   on both, else B with its costs as above.
   N1's result (measured on this M5, macOS 27.2, three runs): C does not work on Hypervisor.framework. In a
   machine with EL2 enabled and PMUVer set to 1 by the host (it reads back), the virtual CPU enters at EL1
   and PMU accesses do not exit, but PMCR_EL0 reads 0x41: N, the number of event counters, is 0, and a write
   to PMEVCNTR0_EL0 reads back 0, so nothing counts instructions. The cycle counter does count (64 bits),
   and runs past its overflow with no overflow flag (PMOVSCLR_EL0 0) and no interrupt, while the virtual
   timer's interrupt arrives through the same GIC; so not even a cycle bound (instructions at most the core's
   retire width times cycles) can be armed. Also, in such a machine `hvc` goes to the guest's own EL2, so the
   doorbell must be a store to an unmapped page there. The obvious objection, that PMCR_EL0.N reads as the
   guest EL2's MDCR_EL2.HPMN, which starts at 0, was ruled out by N1's independent check
   (pyro-parked/wcontainers-scratch/native/check-n1/hpmn.c, hpmn.out): MDCR_EL2 reads 0 and ignores writes
   from both the host and the guest's EL2, PMCR_EL0.N is 0 even read at EL2, and event counter 0 counts
   nothing over a 10,000-round loop. By this decision's recommendation, C is out on this
   platform and the choice is between A and B; the owner rules.
11. The other project's borrowed views of kernel state: built into the inside, or copied out through the
   link, when the native back end is used? Recommended: built into the inside, as a policy is.
12. One virtual machine per host process holding all its native containers (section 5.1), on KVM too, so one
   shape serves both platforms? Recommended: yes; Hypervisor.framework allows no other shape short of a
   process per container.
13. The WebAssembly output's stack-frame exemption (tr/contract.rs:20-22; section 7.2): a permission change
   to a running function's own frame is noticed there at the next function entry, natively and on the
   reference at once. Keep it as a named difference between the outputs (and from the reference), or ask
   the translator track to apply such a change at once? Recommended: keep it named for now, since it is the
   translator's speed choice and only a program that changes its own live frame meets it; the replay test
   reports it where it occurs.
14. Results that depend on the host's model (section 3.4 and the behaviour probe of section 5.8): x86-64's
   `rcpps`, `rcpss`, `rsqrtps`, `rsqrtss`, the flags Intel leaves undefined, and on both architectures loads
   and stores a fault could stop part-way beyond the Split route's. (a) Copy them, and run the WebAssembly
   back end on a host model whose probe differs (other vendors' approximations differ, so such a host,
   another vendor's x86-64 for instance, would run x86-64 containers on the WebAssembly back end); (b) route
   them on every host (a call into the inside for each, on shifts, multiplies and the reciprocals: a cost on
   hot code everywhere; this is what is built until the owner rules, as the brief's letter and the
   orchestrator say); (c) route them only on hosts whose probe differs (two translations of the same
   forms, chosen by host). This decision changes the brief's letter if (a) or (c) is chosen, as decisions
   1, 10 and 12 change interfaces. Until the owner rules, (b) is built. Recommended for the owner to weigh
   once N4 and N9 have measured (b)'s cost: (a) costs nothing on a matching host, keeps one translation and
   keeps every native node on the one behaviour the other project needs, but leaves a residue (a model
   matching every probed input and differing on another, section 7.2) and departs from the letter.

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
   licences to be checked before any trial): the x86 program runs under the emulator, an ordinary ARM64
   Linux program on our kernel. Takes nothing of ours, but the guest is then the emulator, not the x86
   program: its offer, its system calls and its timing are the emulator's, so behaviour is not the x86
   WebAssembly output's and replay records the emulator. Recommended against for anything but a trial.
5. **Rosetta or QEMU**: refused. Rosetta for Linux is Apple's, reached only through a
   Virtualization.framework share into Apple's own virtual machines, not ours to ship (from Apple's
   documentation of that framework, not checked further); QEMU's translator is GPL-2.0.
