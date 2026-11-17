# x86 track status

Plan: docs/phase-2-guest-cpu-and-x86.md. Branch: phase-2-x86. Brief:
pyro-parked/wcontainers-scratch/orchestrator/brief-x86.md.

## Where things stand

- X0 (the plan, and the x86 reference machine proven): done in run 1, 2026-11-17; the first independent
  check's findings fixed in run 2, the second check's in run 3, the third's in run 4, the fourth's in
  run 5, the fifth's in run 6, the sixth's in run 7, the seventh's in run 8, the eighth's in run 9 and
  the ninth's in run 10, the same day (below).
- X1 (the interface, with ARM64 moved onto it): run 1 moved ARM64 and the translator onto the interface;
  run 2 (2026-11-17) fixed what the independent check of X1 found, above all building the whole of section
  3 with the translator's side of every item. See "X1, run 2" for where it stands.

## X1, run 2 (2026-11-17)

The check's findings and what was done about each:
- Blocking, interface built in part: every item of plan section 3 now exists (`ID`, `MAX_LENGTH`,
  `bit_reg`, `initial_block` in `Personality`, `probe_module`, `host_from_probe`, `host_key`, `offer_key`,
  a CPU's `Reason`/`REASONS` from 64, `TrapKind` 4 to 7, `check`'s `which`, `check_range`, `warn`,
  `raise_stop`, `RETURN_ADDRESS`, `RED_ZONE`, `CODE_CHANGES`, `STEPS`/`step`/`StepAt`/`Step`,
  `HookType::registers`, `Index::BitOffset`/`Lanes`, `AccessBytes::Counted`/`Masked`, `Class::Segment`,
  the `before` walks), and the translator does its side of each: `PASSED_IN` and `PASSED_OUT` may differ
  (the compile-time equality check is gone), check plans per (offset, access), groups keyed by segment
  too, red-zone frames, a `Stack` return address moved past in `ret` and expected back after `call`
  (emitter and frame analysis), hook registers written back and reloaded, `check_range`, `warn` with
  sites by number, `raise_stop` with a stop check before the next instruction, and the walk back for
  `built_address` and `jump_table`. The runners count a CPU's reasons by number, probe through
  `Guest::probe_module`, start blob threads from `initial_block`, and stop with x86's faults as
  `Stopped::Fault`. `tests/test_guest.rs` in the translator is a guest made for the test, shaped like
  x86-64 where ARM64 differs (1 to 3 byte instructions, return address on the stack, red zone 128,
  passed in [r7, r6, sp] and out [r0, sp], an `inc` keeping C, an FS base, a trap with pc after it, two
  accesses in one instruction, a range check, its own slow path, a hook on registers, a raised stop, a
  jump into the middle of an instruction, a switch found by walking back); its translations at none,
  bounded 1, 5, 256 and full, both settings, both offers, and its thread module validate.
- Blocking, the float hook 1.5 times slower: `HookType::state` names the bytes a `Cpu` hook works on
  (ARM64's float hook: 36 bytes, FPSR through FPSR tracking); the host runner registers the hook with
  `func_new_unchecked`, arguments and state in fixed arrays sized by the largest hook, the state copied
  with one `copy_nonoverlapping` each way; the browser copies only those bytes too. Timed with
  `tests/float_hook.rs` (`programs/rounding.c`, 200,000 turns a mode, three modes, output equal to the
  native run), in the 2274314 copy and the checkout alternating, one run each per round, on the Mac:
  - first fix (36-byte copy, 1 KiB buffer zeroed per call), 3 rounds, load 27.8 to 28.6: best old none
    0.6884 s, full 0.9012 s; new 0.7506 s, 0.9948 s (about 1.09 and 1.10).
  - with the buffer sized to 36 bytes and the copy by `copy_nonoverlapping`, 3 rounds old first (load 23.1
    to 23.9): old 0.7250 / 0.9355 s, new 0.7309 / 0.9577 s; 4 rounds new first (load 34.9 to 41.8): old
    0.7505 / 0.9836 s, new 0.7812 / 1.0015 s. New over old 1.01 to 1.04 (none) and 1.02 (full), against
    1.5 before the fix.
  - an experiment registering the float hook with a typed `func_wrap` instead (not kept): old 0.7065 /
    0.9610 s, new 0.7590 / 1.0030 s (load 33.9 to 34.7), no better, so the registration is not the rest.
  What is left, about 8 ns of about 200 ns per hook call, is expected to be the generic hook path (36 bytes
  copied each way where the old code moved 16, and the arguments read by type); not measured further.
- Blocking, the browser did not ask `memory_fault`: the cdylib exports `cpu_memory_fault(address,
  length)`; runner.js copies the state and the instruction's bytes to `cpu_state` and stops with the
  answer (page fault, general protection or stack fault).
- `guest_free.rs` now allows only `wcontainers_abi::PAGE_SIZE` and `wcontainers_abi::Permissions`, by full
  path; a second test shows the check catches `RegisterBlock`, a `use` group and a bare `use`.
- `int 4` gets `TrapKind::Overflow` (7), so the kernel tells it from general protection (trap number 4,
  no RF); plan 3.2, 5.2, 6.
- The plan's contradictions fixed (header, 3.2's code block with `Personality`, the kernel's wording,
  `Class` and `Ty`), and this file's "X0, run 1" heading restored.
- `flags_needed_after` finds its place by binary search (a block's offsets are in order).
- The comparison command covers every look-ahead (none, bounded 1, 17, 64, 256, 4096, full), and for a
  binary each kind of start alone at full and bounded; both sides run the current `tests/output.rs` (for
  a commit before the interface its calls are spelled the old way by sed in the script).
- The move in one commit: history is not rewritten; run 2's work is in separate commits.
- zerocopy's licence recorded below.
- `TrapPc::After` and the `no_std` cores: the test guest's break leaves pc after it; `tests/no_std.rs` in the
  test crate builds both cores for `wasm32v1-none` with `translate` off.

## X1, run 1 (2026-11-17)

Done:
- Merged origin/phase-1-translator (22 commits) first.
- `crates/wcontainers-translator-test/same-output.sh <commit>` with `tests/output.rs`: one command that
  builds the earlier commit from its own copy (scratch), writes every module's length and BLAKE3 hash on
  both sides (benchmark set: whole program and the 8 workloads; the 9 M5 binaries; full and bounded
  look-ahead 256; speed and size; the thread module) and compares the lists with `cmp`. Committed before
  any interface change (2274314), which is the "before" of every comparison below.
- The X0 review's open findings settled in the plan: `TrapPc` (a trap may leave pc after the instruction:
  x86 `int3`, `int 3`, `int1`, `int 4`); the `no_std` core and `translate` feature layering of the
  interface and of each CPU crate (plan 3.1, 6); rdpmc kernel only and VMX designed SIGILL and excluded from
  layer 1 (5.2, 7.4); `int 4` a trap with rip after (5.2, 6); segment loads write the FS/GS base (5.5,
  3.4); `sgdt`, `sidt`, `lsl 0x7b` designed answers and excluded (5.10, 7.4); the recorder runs with the
  translation's host and offer (3.1, built so); X1's dependency check (valid.rs moved, 9).
- `crates/wcontainers-guest-cpu`: core (`Personality`, `Registers`, `Bit`, `TrapKind`, `MemoryFault`)
  `no_std`; `Guest`, `Emitter`, the plain types, `Reason` and the generic effects recorder behind the
  `translate` feature (default on).
- `wcontainers-arm64`: `Arm64` with its `Personality` in a `no_std` core; `Guest` in `src/guest.rs`
  (shapes, completing, stack moves, switch tables, built addresses, loop-proof facts, `run_hook` for the
  float hook); meanings bound on `Emitter<Arm64>`; FPSR tracking and exclusive-monitor offsets moved here
  from the contract.
- `wcontainers-translator`: generic over `G: Guest`, depends on the interface only; blocks keep their
  instruction offsets (variable lengths), lookup and switch tables by `ALIGN`, hooks and type indices after
  the guest's hooks, trap pc from `TrapPc`. `tests/guest_free.rs` fails if its manifest or sources name a
  CPU crate.
- Test crate and browser cdylib name `Arm64`. Host runner: guest hooks registered by `HookBody`, the float
  hook through `Arm64::run_hook`; the memory fault hook asks `Arm64::memory_fault`. Browser: `cpu_state`,
  `cpu_hook`, `cpu_hook_result`, `cpu_hook_reason` exports, runner.js copies the state around them; the
  contract JSON lists the guest's hooks and `align`.
- `rust-toolchain.toml` gains the target `wasm32v1-none` (no standard library). Both cores build for it with
  `--no-default-features`; the control (interface with `translate`) fails on `can't find crate for std`.

Proof so far (machine: the Mac, Apple M5; load 18 to 42 while it ran):
- Byte for byte, at 7178588 (the move) against 2274314 (before it): `same-output.sh` gives "the same" for
  bench (37 modules: thread module, whole program and 8 workloads, full and bounded, speed and size),
  busybox, musl, python, rg, git, java, go (4 modules each), node (6) and bun (8); Node and Bun each alone.
  Each side of the "before" was also compared with itself first (same lists), so the output is
  deterministic. After merging origin/phase-1-translator again (9626b63, which changed only
  docs/status-translator.md) bench, busybox, musl and python were compared again: the same. The other
  binaries were not run again, as no input of the translator changed.
- No intentional change of ARM64's output in X1 (the stop check's raised branch changes at X6), so there is
  one comparison, across the whole move.
- Suite: `cargo test -p wcontainers-guest-cpu -p wcontainers-arm64 -p wcontainers-translator -p
  wcontainers-translator-test` at the tree of 7178588: 74 passed, 0 failed, 8 ignored (the measuring ones),
  22 test binaries, Chrome tests included (load 17.4 at start, 22.4 at end). Clippy with `-D warnings` on
  the four crates (tests and bins) and on the browser crate for wasm32: clean.
- `cargo tree -p wcontainers-translator`: wcontainers-abi and wcontainers-guest-cpu only.
- Run time, wasmtime, every check, best of 5 with old (2274314's copy) and new (the checkout) alternating
  one run each (scratch `x1/bench-alternate.sh 5`, outputs in `x1/bench/`; Mac load 16 to 27): fib 2.307 /
  2.170 s, sieve 0.640 / 0.642, hash 0.580 / 0.578, sort 1.107 / 1.081, interpret 0.530 / 0.535, matrix
  0.238 / 0.233, walk 2.262 / 2.240, search 0.245 / 0.232 (old / new). New over old 0.94 to 1.01; the
  modules are the same bytes, so these differences are the machine's noise.
- The move landed as one commit (7178588) after the comparison tool's own commit: the translator could not
  be made generic piece by piece and still build, so the steps were checked in the working tree (byte for
  byte on bench, busybox and musl as soon as it built) rather than committed one by one.

Left for X1 after run 1: nothing of the pass bar as it then stood. The independent check of X1 then found
the interface built only in part, a slower float hook and the browser not asking `memory_fault`; run 2
below.

## X0, run 1 (2026-11-17)

Done:
- Wrote the plan from the brief and the seam map. Appendix A maps every kind b and c place of the seam
  map's translator and tests areas (41 and 60 places) to the section that covers it.
- iced-x86: 1.22.0, MIT (crate `Cargo.toml` and `cargo info`). Builds for wasm32-unknown-unknown from a
  `#![no_std]` library with `default-features = false` and `no_std`, `decoder`, `instr_info`, `encoder`,
  `op_code_info`. Size it adds to a stripped wasm32 release cdylib: decoder only 227,228 bytes raw, 60,705
  gzip -9, 47,058 brotli -q 11; with `instr_info` 353,374 / 82,721 / 64,947. Today's browser translator
  (ARM64) for scale: 2,170,386 / 412,059 / 312,302. Measured on the Mac (Apple M5), load 15 to 28.
- Fused multiply-add: WebAssembly's relaxed madd may fuse or not; the specification's rule quoted in the
  relaxed SIMD overview makes the choice fixed per environment, so the phase-one probe can rely on it. No
  scalar form. Plan: FMA claimed only with the AVX setting, software where the host does not fuse.
- How others stop an x86 CPU after one instruction: sandsifter (trap flag, and page-end placement), RISU
  (a UD1 operation after each instruction), debuggers (trap flag). Chosen: the trap flag set in the signal
  frame the reference runner returns through.
- Experiment 1: `hello.c` built with `zig cc -target x86_64-linux-musl -static` on the Mac, copied with
  scp, run over SSH on the x86 machine; output and exit status 7 came back.
- Experiment 2: `stepper.c` runs one instruction from given registers by the trap flag and reads every
  register back. 22 cases, all as the Intel manual says (plan section 2 lists them and what they teach
  the reference runner).
- Measured: a new SSH connection about 344 ms (Mac load 57). The figure of 410 ms for 1,000 stepper
  processes recorded here had no method written down and could not be repeated; withdrawn in run 3 (see
  there).

Scratch (not in the repository), all under pyro-parked/wcontainers-scratch/x86:
- `iced-size/` size builds (base, dec, info, nostd); `dis/` an iced disassembler used to check encodings;
  `reference-proof/` hello.c, stepper.c, cases.sh, cases2.sh, cases.out, cases2.out; `sources/` the
  relaxed SIMD overview and sandsifter's injector.c and licence; `target-browser/` and `browser-build.log`
  the browser translator size build.

## X0, run 2 (2026-11-17): the independent check's findings, all fixed in the plan

- Blocking, the bottom loop's marker: a matched `ret` could reach `UNREACHABLE_RETURN & !FLAGS_WANTED`
  on x86 and RV64. Now each CPU makes it unreachable before the compare: ARM64 by its existing untag, x86
  by a canonical check in the meanings of `ret` and of every jump or call through a register or memory
  (four operations each, cost measured at X4), RV64GC by marker 1 (no `jalr` target has bit 0 set). Plan
  3.2, 3.4 (T14), 3.5, 5.3, 12.
- Blocking, interface not in full: added `Guest::Offer` and `offer_key` (the AVX setting), passed to
  `translate` and read through `Emitter::offer()`; added `Guest::run_hook`, `HookBody` and `HookDone`, so
  the software float runner (ARM64 `exact::run`, x86 float and x87) ships behind the interface and no
  runner names a CPU function; the browser exports `cpu_hook`. Added appendix B: every interface surface
  item of the translator (24) and tests (25) areas and where it is covered.
- `mov ss` holds the trap flag's stop back one instruction; the runner reads a `SI_KERNEL` stop at CODE +
  length + 1 as "landed at CODE + length" (plan 2, 7.3).
- `int 0x80` runs a 32-bit system call on the reference; this phase gives general protection, as a kernel
  without 32-bit emulation does (expected from Linux's source, not seen), counted and warned, excluded
  from layer 1 with that reason. `int1` maps to `TRAP_BRKPT` (plan 5.2, 6, 7.3, 7.4).
- The `syscall` experiment's description corrected (no `TRAP_TRACE` at all; the filler stopped it).
- Lookup array figure: bun's 52,050,800 bytes of code need about 208 MB of slots (was a wrong 38 MB).
- Write protection of translated pages now follows the permission contract: other threads stopped first
  (free with one host thread), pieces recheck after calls, and a page in a live stack frame makes the
  threads concerned give up their WebAssembly frames, counted "code translated from a live stack frame".
- Decoder only has no `encoding()` or `cpuid_features()`: a build script makes a per-`Code` table (claim
  group, VEX) from iced's own `instr_info` as a build-dependency, checked by a test.
- X1 pass bar: the builder adds `WC_BENCH_DUMP` to tests/benchmarks.rs first, so the benchmark modules can
  be compared with `cmp`.
- CPUID: `rdpid` is not claimed (the i7-8750H lacks it); the "asked about AVX" counter firing in nearly
  every glibc program is stated as intended, with the reason.
- x87: precision control by rustc_apfloat semantics of our own (precision 66, 53, 24, extended exponent
  range) and rounding to odd; transcendentals with `astro-float`.
- The x86 machine is shared at times (another track's `~/ordering-cost` wasmtime job, load about 1.0 seen);
  Rust was already on it.

Checked by looking, run 2:
- Repeated on the reference (x86 load 0.07): `mov ss` (SI_KERNEL, CODE+3), `int 0x80` (ran, CODE+3),
  `int 0x81` (SIGSEGV SI_KERNEL), `int1` (TRAP_BRKPT, CODE+1), `int 3` (SI_KERNEL, CODE+2), `syscall`
  (ran, SI_KERNEL CODE+3).
- astro-float 0.9.6, MIT (`cargo info`). It builds for wasm32-unknown-unknown with std. (Run 2 wrote that
  it does not build without std; run 3 found it does once `lazy_static`'s `spin_no_std` is on, see
  there.) What it adds to a stripped wasm32 cdylib (opt-level s,
  LTO; sin, atan, log2, exp, add at 128 bits) against the same cdylib without it: 142,688 bytes raw (160,136
  against 17,448), 40,947 brotli -q 11 (46,678 against 5,731). Mac load 16.6 to 20.7.
- rustc_apfloat 0.2.3's `ieee::Semantics` is a public trait that can be implemented outside the crate.
  Semantics with 15 exponent bits and precision 66 and 24 work. Rounding to odd then once to 24 bits gave
  the right results in both directions (scratch `apfloat-pc`).
- iced 1.22.0: `Code` has 4,936 values (`iced_constants.rs`); `encoding()` and `cpuid_features()` are behind
  `instr_info`.

## X0, run 3 (2026-11-17): the second independent check's findings, all fixed in the plan

Each finding was first repeated on the reference (x86 load 0.00 to 0.89, all results as the check
reported), then fixed:
- Blocking, `call` to a non-canonical target: the CPU writes the return address at rsp - 8 first, then
  gives general protection with rsp unchanged; a page fault on the push wins; `call [m]` reads its target
  before the push. Also seen for a direct `call rel32` from code mapped at 0x7fffffff8000 (`stepper3.c`).
  The plan now has the meaning do the CPU's steps in the CPU's order (read target, push, canonical check,
  move rsp) and the emitter's `call` only makes the call (plan 3.3, 3.4 T14, 4 decision 2, 5.3, 5.4).
- r11 after `syscall` carries the stepping's TF (0x3C3 from 0x2C3): added to the per-case masks (7.2).
- An access running from a canonical start into the non-canonical range is general protection: new
  `Guest::memory_fault(address, code) -> MemoryFault`, which the runner calls before telling the kernel;
  x86 answers from the access's size, no cost on any access (3.2, 3.4, 5.4, 6).
- `rcpps`, `rcpss`, `rsqrtps`, `rsqrtss`: matched bit for bit by a closed formula in f64 vector
  arithmetic, no table and no slow path. Found and checked on the reference over all 2^32 inputs of each,
  zero differences, in MXCSR 0x1F80 and 0xFFC0 (5.8).
- `div` and `idiv` of a 128-bit dividend: inline fast path when rdx is rax's extension, inline long
  division otherwise, divide error rules; not a slow path, cost measured at X3 (5.2).
- The "AVX instruction reached" counter is keyed on the AVX setting's claim group, not on VEX; the build
  table no longer needs a VEX bit (2, 5.2, 5.10, X8).
- Round trips re-measured with the method written down (2 item 3, 7.3).
- FPU claimed by default is now stated in the plan as a reading of the brief, with the reason (5.10),
  and listed below for the owner.
- astro-float and `no_std`: hooks are a runner's job, never the kernel's, so the CPU crates stay `std`
  and the kernel never depends on one (3.1, 3.2, 3.4, 6). astro-float does build without std once
  `lazy_static`'s `spin_no_std` is on (scratch `x87-nostd`: a `no_std` cdylib with its own panic handler
  links with it and fails on a duplicate `panic_impl` without it).
- Interface gaps: `zerocopy` added to the interface's dependencies; `jump_table` returns `Vec<u64>`;
  `Value`, `Induction`, `Comparison`, `Relation`, `Extend`, `NarrowWrite`, `MemoryFault` defined;
  `counting_exit` and `unsigned_bound` became one `branch_comparison`; `narrow_load` became `loaded_max`;
  `index_extension` dropped (the index's `Extend` is in `MemoryAccess`); `NARROW_WRITE` is
  `[Option<NarrowWrite>; 3]`; the effects recorder and `Registers` live in the interface crate (3.1, 3.2,
  3.4, appendix B).
- RISU checked in its source (mirror github.com/rth7680/risu, commit eed2249; git.linaro.org gave 504):
  `risu_i386.c` marks operations with `UD1 r, eax` and steps over its 3 bytes. Licence EPL-1.0, read only.

Measured in run 3:
- New process on the x86 machine: `bash -c 'time (for i in $(seq 1000); do ./stepper 90; done >
  /dev/null)'` 0.93, 1.25, 1.25 s; the same over `./hello` 0.86, 0.88, 0.94 s (x86 load 0.89, governor
  powersave). About 1 ms a process.
- New SSH connection from the Mac, `time ssh tom@192.168.2.240 true`, five runs: 0.24 to 0.32 s (Mac load
  9.2).
- Exhaustive runs on the x86 machine: `approx` 24 s, `approx_formula` 73 s (x86 load 0.02),
  `approx_mxcsr` 145 s (load 0.66).

## X0, run 4 (2026-11-17): the third independent check's findings, all fixed in the plan

- Blocking, code written without announcement missed two write paths: the kernel's own writes into
  guest memory (`Cpu::write`, `zero`, `shared_write`, which ignore permissions) and writes through another
  mapping of a shared page (memfd or shm mapped twice, .NET's W^X double mapping, another process). The
  plan now keeps code marks per backing page in the CPU side (which owns memories, permissions and shared
  blocks), protects every alias of a shared page, and has those three `Cpu` methods take the same
  drop-and-unprotect step before writing. No kernel change: the kernel stays CPU-free. Built at X10 with
  three tests; until then x86 runs only under the stand-in, which maps nothing twice (plan 5.7, 6, 9, 12).
- Blocking, DF among the lazily tracked flags made the per-shape liveness test never true. DF (and ID)
  now live in a control `Rflags`; `FLAGS` is the six arithmetic flags; `flags_needed_after` becomes a
  test per flag (`pending` set), which on ARM64 gives exactly today's answers because every ARM64 setter
  sets all four. "Plain computation" is a CPU function `Guest::plain` (ARM64 today's list), not derived
  from effects, because a derived rule would let ARM64 look past `crc32` and change its output (plan
  3.2, 3.4, 3.5, 5.1, 5.9).
- Blocking, `GuestTest` had no Rust shape. Section 7.2 now gives the trait in full (names, forms, `cases`
  as one strategy per CPU so ARM64's seeds give the same cases, `presented`, `excluded`, `no_form`,
  `offered`/`told`, `designed`, `FIELDS`, `to_state`/`from_state`, `may_differ`, `reference_stop`,
  `auxiliary`, `STAND_IN`, `PROGRAMS`, the corpora, layer 2's `MODEL_REGISTERS`, `Extra` and `assemble`),
  its plain types, the reference runner's per-CPU `Machine` trait, the wire format, the transport
  (`Reference::<G>`, Docker or SSH, same records) and the browser contract's per-CPU numbers.
- Address limit: the x86 personality now has `TASK_SIZE_MAX` 0x7ffffffff000 (plan 5.4, 6, X10).
- Interface gaps: `locals_of` (inverse of `with_locals`) and `complete`; `Bit` has its bit number;
  `Target`, `Access`, `Locals` defined; the flags word holds only the tracked flags, so bits 1 and 9 need
  no interface item; `jump_table` takes the jump's offset and a `before` walk, and ARM64 keeps reading its
  window from the blob exactly as today.
- Far transfers designed (plan 5.3) from new reference runs; `memory_fault` now takes the block and picks
  the faulting access by the reported address, since one instruction's two accesses may differ in size.
- x87 `fsqrt`: rustc_apfloat 0.2.3 has no sqrt (its lib.rs "Future work"), ieee-apsqrt 0.1.1 only
  32/64/128-bit IEEE; now astro-float's sqrt at 128 bits toward zero, exactness by squaring at 256 bits,
  round to odd at 66 bits, then once to 24/53/64 (plan 5.8).
- The reference runner now writes the x87 component too (`xstate_bv` bit 0), proven by `stepper4`.
- Found while checking, added to the plan: with AC set Linux gives `SIGBUS BUS_ADRALN` on a misaligned
  user access. The plan does not provide alignment checking: `popf` and `iretq` leave AC clear (the 386
  test reads it as absent), counted and warned (plan 5.1).

Checked on the reference in run 4 (x86 load 0.00 to 0.09; 36.5 from another job during the AC cases,
nothing timed):
- `cases4.sh` (8 x87 cases with `stepper4`), `cases5.sh` (9 far-transfer cases), and 4 AC and `pushfq`
  cases (`cases6.out`, run as `./stepper3 488b07 rdi=0x20000101 rflags=0x40202` and so on), all as
  recorded in plan section 2, run 4.

## X0, run 5 (2026-11-17): the fourth independent check's findings, all fixed in the plan

Each was first repeated or widened on the reference (x86 load 3.6 to 5.4 from another job, nothing timed):
- Blocking, encodings past 15 bytes: they give general protection, not `SIGILL`, whatever the body is
  (valid, an invalid opcode, an invalid lock): `cases7.sh`. iced reports them as `INVALID` length 15, like
  a 15-byte invalid one. Plan 5.2 now decides it: for `INVALID` at length 15, `decode` drops the prefix
  bytes that change no length (keeps the last 66, 67, F2/F3 and a REX before the opcode), decodes the
  rest and adds the count; over 15 is `GeneralProtection`, otherwise `Undefined`. iced's count after an
  opcode it calls `INVALID` is not the CPU's (`cases8.sh`: `0F 24` counts no ModRM, `0F 38 FF` counts
  one), so the x86 crate gets a table of what the CPU counts after each such opcode, started from a sweep
  of 1,024 opcodes (`sweep15.sh`, 24 count nothing, 539 at least one byte) and completed at X3 (plan 5.2,
  9). `no_form` and 7.4 now say so; X3's pass bar gains 10,000 no-form cases.
- `built_address` now takes `at, i, blob, code, before` like `jump_table`; ARM64 reads the word at
  `at - 4` from the blob whenever it lies in code, exactly as full.rs:561-581, so its output cannot
  change; RV64GC walks `before` (plan 3.2, 3.4).
- `clflush`: a checked one-byte read, then nothing (reference: page fault unmapped, general protection
  non-canonical; `prefetchnta` never faults) (plan 5.10).
- `Stop::Breakpoint(si_code)`, with `GuestTest::breakpoint_code(detail)` for the translated side; ARM64's
  `brk` gives `TRAP_BRKPT` (1), seen in Docker with `brk.c` (plan 7.2, 7.3).
- `Locals` holds 9: x86's shift by cl keeps value, count, result and six old flags, one local each, which
  keeps the per-flag liveness (plan 3.2, 5.1).
- Header now says four checks; 7.3's connection cost now quotes what was measured. Run 5 measured five
  new SSH connections at 0.92 to 1.19 s (x86 load 6.9 from another job, Mac load 27.6) and then 0.27 to
  0.48 s (Mac 28.2); the fourth check saw 0.24 to 0.40 s at Mac load 34.
- This file's list of what is on the x86 machine is corrected (the cases outputs are only on the Mac).

## X0, run 6 (2026-11-17): the fifth independent check's findings, all fixed in the plan

Each was first repeated and widened on the reference (`cases9.sh`, `cases10.sh`, x86 load 0.00 to 0.12,
nothing timed):
- Blocking, the stack fault: a non-canonical access through the stack segment is #SS, which Linux sends
  as `SIGBUS` `SI_KERNEL` address 0, not general protection. Widened: only base rsp or rbp counts (r12
  and r13 give general protection), `fs:` and `gs:` make it general protection, `cs:`, `ds:`, `es:` and
  `ss:` change nothing, every implicit stack access (push, pop's read, call, ret, pushfq, push fs, enter,
  leave) counts, `pop [rbx]` gives either by which address is bad, and an alignment check comes first
  (a misaligned non-canonical `movaps [rsp]` is general protection). Plan: `MemoryFault` gains `Stack`;
  x86 `memory_fault` picks it by one test on the faulting access (no segment, base rsp or rbp); `Stop`
  gains `StackFault`; section 6's signal list, 7.3's mapping, 5.3's call and ret steps, 4 decision 16,
  layer 1's operand drawing and the X2 and X3 pass bars carry it. `TrapKind` does not grow: no meaning
  raises a stack fault (the reason is in plan 3.2).
- Found while fixing it: `Base::Segment` could not express `fs:[rdx + rax * 4]` (a segment base with a
  base register). `MemoryAccess` now has its own `segment: Option<R>` and `Base` is `Register` or `Pc`;
  the check analysis keys on (segment, base), which for ARM64 (segment always `None`) is today's key.
- MMX writes set the x87 register's sign and exponent to 0xFFFF (seen for four writes; reads leave
  them). Plan 5.8: eight i32 controls `X87High(n)` over bytes 8 to 11 of each slot, written with one
  `set_control` by every MMX meaning that writes mmN.
- The longest valid rest after the dropped prefixes is 15 bytes, not 14 (plan 5.2).
- iced's sizes are now given as totals, with the 257 / 232 / 178-byte base subtracted for what it adds
  (plan 2; the two stripped files measured again).
- `SHAPES` for ARM64 is today's ten shapes in `shapes()` order, and the flags-word functions keep the
  generic "flags in locals" one first at `F_FLAGS_WORD` (plan 3.4, 3.5).
- Cross-references fixed (RISC-V is 3.5, the reasons are 3.4) and section 13 lists all five checks'
  folders on the x86 machine.

## X0, run 7 (2026-11-17): the sixth independent check's findings, all fixed in the plan

Each was first repeated and widened on the reference with `stepper5` (`cases11.sh`, `cases12.sh`,
`cases13.sh`, x86 load 0.00, nothing timed):
- Blocking, NT: `popf` and `iretq` keep NT (bit 14) in user mode, `pushf` stores it, `syscall` and fault
  frames keep it, and `iretq` with NT set is general protection before it reads its frame. Plan 5.1:
  `Control::Rflags` holds DF, NT and ID; `iretq` tests NT first (5.3); the kernel's personality puts NT
  in signal frames and keeps NT and ID at sigreturn as Linux does (6).
- Found while checking: `iretq` reads its frame out of order (rflags, then CS, SS before rsp), so a page
  fault names the slot the CPU names. Plan 5.3: five checked reads in the CPU's order; where rip's read
  falls is settled at X5 before the meaning is written.
- Entering NT and ID: proven by setting them in the handler's own rflags just before `rt_sigreturn`,
  which keeps non-`FIX_EFLAGS` bits from its caller. Plan 7.3 (`Machine::enter`, the one-instruction
  steps) and 7.4 (starting rflags: six flags, DF, NT, ID drawn; AC, TF, IOPL, RF, VIF, VIP clear).
- MXCSR and lazy binding: `ldmxcsr` turns tracking on; `fxrstor`, `xrstor` and sigreturn restore without
  it, so glibc's lazy binding trampoline keeps the fast path. `float_slow` is mode bits not 0x1F80 or
  tracking (3.4, 5.8). Confirmed on the reference: the loader has `fxsave`/`fxrstor` and
  `xsavec`/`xrstor`.
- Reading MXCSR's flags while not kept: `stmxcsr` and the `fxsave`/`xsave` family call the float hook
  with `ReadStatus`, counted "MXCSR's exception flags read while not kept" (5.8). `may_differ` now says
  what ARM64's layer 1 does today: untracked default-mode flags must equal the case's starting ones; the
  x87 clause is gone (x87 status is always kept). A signal frame's MXCSR is not counted, as ARM64's FPSR
  today; listed in plan 12.
- VEX or EVEX after 66, F2, F3 or REX: the CPU counts the whole VEX or EVEX instruction (16 bytes give
  general protection, 15 `SIGILL`, also EVEX on this CPU without AVX-512). Plan 5.2 drops those prefixes
  too when the next byte is C4, C5 or 62 (67 stays); 7.4's no-form cases draw such runs.
- `sysenter`: the reference filter now also traps every `AUDIT_ARCH_I386` call wherever made (proven:
  `SIGSYS` with rip in the vDSO). Its meaning is general protection like `int 0x80` (a kernel without
  32-bit emulation loads a null `IA32_SYSENTER_CS`; expected from Linux's source, not seen); SEP claimed;
  excluded from layer 1 with `int 0x80`'s reason and checked by the designed answers (2, 5.2, 5.10, 6,
  7.2 to 7.4, X2, 12).

## X0, run 8 (2026-11-17): the seventh independent check's findings, all fixed in the plan

Each was first repeated and widened on the reference with `stepper6` (scratch `run8/`: `cases14.sh`,
`cases15.sh`, `smc.c`, `copyuser.c`; x86 load 0.00 to 0.16, nothing timed):
- Blocking, code written on its own page: `smc` prints `eax 1 sum 20100` (the check's result). Plan 5.7:
  the step also sets a one-shot mark; the bottom loop runs the faulting instruction once from a
  look-ahead-none piece that is not entered in the lookup and protects no page, and the next instruction
  is translated afresh. Any stop clears the mark; each repeat clears one page's mark, so it ends. Decision
  4.6 and the X6 pass bar (three such programs, among them `smc.c`) say so.
- Blocking, `bt`/`bts`/`btr`/`btc` and `xlat`: `MemoryAccess::index` is now `Index` (`Register` with
  `Extend` from 8 to 64 bits, `BitOffset { reg, width }`, `Lanes` for VSIB). Seen: offsets read signed at
  the operand's width (64, 32, 16), bits above it ignored, rbp base gives the stack fault. The check
  analysis and loop proofs use only accesses ARM64 has today (index `None` or a 32/64-bit `Register`,
  `Fixed` bytes); every other access is checked by its meaning each time (plan 3.2, 3.3, 3.4, 5.4).
- Blocking, gathers and masked moves: `AccessBytes` gains `Masked { element, count }` (and `Counted`
  names its element). Seen and designed (plan 5.4): gathers go in lane order, a fault leaves the earlier
  elements gathered and their mask lanes clear and names the faulting element's first byte not allowed;
  `vmaskmov`/`vpmaskmov` never fault on masked-off elements, are all or nothing, a load reports its first
  byte not allowed and a store the first selected byte when it is not allowed, else the last selected byte
  (two one-byte checks); `maskmovdqu` ignores the mask for faults, stores nothing on a fault, and reports
  as if it checked rdi + 8 first, then rdi. `memory_fault` works out each access's real range.
- `Machine::unfinished` now takes the instruction's bytes and steps on only for a rep string instruction
  or a gather (which under the trap flag stops once, with RF, before its fault), up to `Machine::limit`;
  a branch to CODE is finished (plan 5.6, 7.2, 7.3).
- gzip figures: run 1's were Apple gzip storing the file names (12 bytes more than the check's names).
  The plan now gives `/usr/bin/gzip -9 -n -c`: decoder 60,682, with `instr_info` 82,697, base 208,
  browser 412,037; GNU gzip 1.14 `-n`: 60,831 and 82,541. Brotli is Homebrew's 1.2.1, figures unchanged
  (Mac load 20.4).
- Kernel side, copy routines: each CPU gets its own measured model. x86 (Linux 7.3.4, ERMS, no FSRM):
  pipe `read`, `/dev/zero` `read` and `uname` write every byte up to the page not allowed, for buffers
  ending 1 to 389 bytes before it (plan 6, X10 repeats it).

## X0, run 9 (2026-11-17): the eighth independent check's findings, all fixed in the plan

Repeated and widened on the reference first where the finding was about the CPU (scratch `run9/`:
`cases16.sh` 39 cases and `xacquire.out` 2, with run 8's `stepper6`; x86 load 0.02 to 0.06, nothing timed):
- Blocking, features not claimed whose bytes run on a CPU without them: the whole reserved-NOP space
  (`0F 0D`, `0F 18` to `0F 1F`: `endbr64`, `endbr32`, `rdssp`, `cldemote`, MPX, `prefetchw`,
  `prefetchwt1`, `prefetchit0`) runs as a NOP on the reference with any address, never faulting; a lock
  prefix there gives SIGILL. Plan 5.2: iced decodes with `FORCE_RESERVED_NOP`, which makes all of them
  `Reservednop_*` Codes (feature `MULTIBYTENOP`, claimed) with one meaning that does nothing; ignored
  prefixes (`xacquire`, `xrelease`, `bnd`, `notrack`) are ignored, as seen. 5.10 states the rule. Layer 1
  (7.4, 7.2 `presented` and the new `REFERENCE_HAS`) splits forms by the decoded Code's features and runs
  the not-offered list on the reference too: compared where the reference lacks the feature (TSX, WAITPKG,
  CLWB, PTWRITE, SERIALIZE, MOVDIRI all SIGILL there), designed answer where it has it (AES, PCLMULQDQ,
  RDRAND, RDSEED, CLFLUSHOPT, XSAVEOPT, XSAVEC, the AVX group when off). ARM64's tests stay as they are.
- `Guest::shape` now returns `Option<Self::Shape>`, as today's `Shape::of` (3.2, 3.4).
- Code written on its own page: the plan now reuses phase one's exact path (`UNWIND_GO_TO_EXACT`,
  `EXACT_SLOT`, `load_code(pc, 1)`). At X6 the memory fault hook returns the unwind payload (ARM64's
  runners always `UNWIND_LEAVE`; ARM64's output loses one `i32.const` per fault site), and the CPU side
  keeps a per-thread exact mark that makes that one `load_code(A, 1)` translate without protecting (5.7,
  decision 6, X6).
- Berkeley SoftFloat: considered and not used, with the reason in plan 5.8. softfloat-sys 0.1.4 and
  softfloat-rs 0.1.0 do not build here: their build scripts test the building machine with `cfg!` and
  panic off x86-64 ("build rules are not implemented"), and softfloat-sys's `c99` dependency runs CMake,
  which fails with the Nix shell's CMake. The pure-Rust crates lack 80-bit precision control.
  SoftFloat's C does compile for wasm32 with zig (`extF80_sqrt.c`, seen), so vendoring it is possible
  but would put C into the Rust project's x86 crate and every build of it.
- `cpu_hook`'s buffer: the cdylib exports `cpu_state() -> *mut u8`, `threadStateSize` bytes allocated once;
  runner.js's per-call copy, index and results are spelled out (3.4 Hooks).
- Code area: `FILLER` and a new `CODE_SPAN` are per guest in `layout.rs`; the translated side writes the
  filler over the code area when it is not 0 (x86: 0xCC over 64 KiB, ARM64 nothing), and layer 1 draws
  rip-relative operands into it (7.2, 7.4).
- RISU's operation number is in ModRM's r/m field (plan 2, checked in scratch `sources/risu/risu_i386.c`).
- iced without std: proved by a `no_std` cdylib with its own panic handler that links (scratch
  `run9/iced-strict-nostd`), with a control using `std` that fails on a duplicate `panic_impl`
  (`run9/iced-strict-std`) (plan 2).

## X0, run 10 (2026-11-17): the ninth independent check's findings, all fixed in the plan

Repeated and widened on the reference first (scratch `run10/`: `tf.c`, `rf.c`, with `stepper6` for
`umip.out` and `branch-unmapped.out`; x86 load 0.00 to 0.24, nothing timed):
- Blocking, the trap flag had no state: TF set by a program now behaves as seen with `tf.c` (popf or
  iretq setting TF: the next instruction runs, then `TRAP_TRACE`; popf clearing TF still stops after
  itself; `syscall` with TF: the instruction after it runs before the stop; `int3` and a fault give only
  their own signal; TF kept across `rt_sigreturn`; handlers start with TF clear; `pushfq` stores TF).
  Plan 5.1: TF is bit 8 of `Control::Rflags`; while it is set the stop flag stays raised and a new
  `Guest::step` (default `Off` for ARM64 and RV64GC) runs each instruction alone through phase one's exact
  path and answers `Trap` at the next stop check. New `Emitter::raise_stop` (only x86's `popf` and `iretq`
  call it), `StepAt` and `Step` types, x86 private words `Step`, `StepAt`, `StepHeld` (`mov ss`). The stop
  hook returns the unwind payload from X6, like the memory fault hook. `SingleStep` is no longer a
  `TrapKind` (no meaning raises it). rep string instructions take the bulk path only with DF and TF clear.
  One known difference recorded: a gather faulting part way under TF (designed-answers case at X8).
  Section 6: TF from the sigreturn frame goes into the control and steps; RF is put in fault frames only
  (seen with `rf.c`) and not kept at sigreturn (no program can see it). Tests: X6 trap-flag blobs compared
  with the reference (the blob runner now records a fatal signal's rip and registers, 7.3); X10 runs
  `tf.c` and `rf.c` against the reference's output.
- `CODE_SPAN`'s comment: a branch beyond the code area stops by `TRAP_TRACE` at the target before any
  fetch (repeated: `jmp rel32`, `jmp rax` to two unmapped addresses), so on x86 it matters only for fall
  through and rip-relative operands.
- SSE3 (`fisttp`) and CMOV (`fcmovcc`, `fcomi`, `fcomip`, `fucomi`, `fucomip`) contain x87 instructions:
  plan 5.10 extends the FPU reading to them, with the reason.
- The claim lists now name iced's own `CpuidFeature` values (plan 5.10 gives both lists), with the
  baseline names `INTEL8086` to `INTEL486`, `X64`, `CPUID`, `PAUSE`, `MULTIBYTENOP`, and `FPU287`,
  `FPU387`, `CLFSH`, `CMPXCHG16B`, `SEP`, `SYSCALL`. The claim table gains a fourth value, "kernel only",
  for every `is_privileged()` Code (plan 2, 5.2). Counted in `run10/feat.out`. `sldt`, `str`, `smsw`
  (baseline `INTEL286`) run in user mode on the reference (no UMIP).
- Layer 1's forms now cover every encoding but MVEX (and the `KNC` VEX Codes, which iced decodes only with
  its `KNC` option), split into default, AVX setting, never-claimed and kernel-only lists by the decoded
  Code's claim group; the never-claimed list (EVEX, XOP, 3DNow and the rest) runs on the reference at 100
  cases a form; X5's pass bar includes it (plan 7.4, 9).
- The plan's header now counts nine checks.

## How to run

- X1's byte-for-byte proof: `WC_OUTPUT_ROOT=<scratch>/x1/same nix develop -c
  crates/wcontainers-translator-test/same-output.sh 2274314 bench,busybox,musl,python,rg,git,java,go`, then
  the same with `node` and with `bun`, each alone (many GB). The earlier commit's lists are kept under
  `<root>/lists/<commit>`, so later runs only translate the current tree; delete them when
  `tests/output.rs` changes. `WC_OUTPUT_LOOK_AHEADS=full,bounded` (labels of `LOOK_AHEADS`) limits the
  look-aheads. The binaries come from `target/binaries` (copied from the translator worktree's M5 files).
- The float hook's time: `nix develop -c cargo test --release -p wcontainers-translator-test --test
  float_hook -- --ignored --nocapture` (knobs `WC_FLOAT_STEPS`, `WC_FLOAT_RUNS`); old against new by
  running the same file in a copy of the earlier commit (`<root>/trees/<commit>` of the comparison).
- The test guest: `nix develop -c cargo test -p wcontainers-translator --test test_guest`.
- The suite of what this track owns: `nix develop -c cargo test -p wcontainers-guest-cpu -p wcontainers-arm64
  -p wcontainers-translator -p wcontainers-translator-test` (Chrome and Docker needed), and the browser
  crate's lint as in docs/status-translator.md.
- The no-std check is `tests/no_std.rs` of the test crate (part of the suite); by hand: `nix develop -c
  cargo build -p wcontainers-guest-cpu -p wcontainers-arm64 --no-default-features --target wasm32v1-none
  --target-dir <scratch>/x1/nostd-target`.

- Run 10's experiments: `nix develop -c zig cc -target x86_64-linux-musl -static -O1 -fno-stack-protector
  -o <scratch>/run10/tf <scratch>/run10/tf.c` (and `rf` likewise), copy both to
  `~/wcontainers-x86/proof10/` and run each there; `umip.out` and `branch-unmapped.out` are `stepper6` in
  `~/wcontainers-x86/proof8` with `0f00c0`, `0f00c8`, `0f01e0`, and `e900000010`, `ffe0
  rax=0x700000000000`, `ffe0 rax=0x7ffffffff000`. iced's feature names: `nix develop -c cargo run
  --release --manifest-path <scratch>/run10/feat/Cargo.toml --target-dir <scratch>/run10/feat/target`.

- Run 9's experiments: copy run 8's `stepper6` and `run9/cases16.sh` to `~/wcontainers-x86/proof9/` and
  run `./cases16.sh` there. Decoding: `nix develop -c cargo build --release --manifest-path
  <scratch>/run9/dis/Cargo.toml --target-dir <scratch>/run9/dis/target`, then `dis9 <hex> ...`. iced
  without std: `cargo build --release --target wasm32-unknown-unknown` of `run9/iced-strict-nostd` (links)
  and `run9/iced-strict-std` (must fail). SoftFloat: the same build of `run9/sf` (fails, as recorded).

- Run 8's experiments: build `stepper6.c`, `smc.c`, `copyuser.c` in scratch `run8/` like the steppers
  below, copy them and `cases14.sh`, `cases15.sh` to `~/wcontainers-x86/proof8/` and run each there.
  `NOTF=1`, `STEPS=n` and `DUMP=offset` in the environment change how `stepper6` runs (its header says
  how). gzip sizes: `/usr/bin/gzip -9 -n -c <file> | wc -c` on scratch
  `iced-size/target/wasm32-unknown-unknown/release/size_*.stripped.wasm`.

- Run 7's experiments: build `stepper5.c` like the steppers below, copy `stepper5`, `cases11.sh`,
  `cases12.sh` and `cases13.sh` to `~/wcontainers-x86/proof/` and run each there. `SECCOMP=1` in the
  environment installs the filter with its arch test.

- Run 6's experiments: copy `stepper4`, `cases9.sh`, `cases10.sh` from scratch `reference-proof/` to
  `~/wcontainers-x86/proof/` and run each there (`ssh tom@192.168.2.240 'cd ~/wcontainers-x86/proof &&
  ./cases9.sh'`).

- Run 5's experiments: copy `stepper4`, `cases7.sh`, `cases8.sh`, `sweep15.sh` from scratch
  `reference-proof/` to `~/wcontainers-x86/proof/` and run each there. `sweep15.iced` adds to each line
  of `sweep15.out` iced's decoding of the opcode followed by eight `cc` bytes (scratch `dis` tool);
  `sweep15.classes` keeps the `INVALID` ones, as "more" where the boundary gives general protection and
  the opcode alone does not, else "none". `brk.c`: `nix develop -c zig cc -target aarch64-linux-musl
  -static -O2 -o brk brk.c`, then
  `docker run --rm --name wc-x86-brk -v <scratch>/reference-proof:/p:ro alpine:3.22 /p/brk`.
- Run 4's experiments: `python3 -I <scratch>/scripts/make_stepper4.py <scratch>/reference-proof/stepper3.c
  <scratch>/reference-proof/stepper4.c`, build it as below, copy `stepper4`, `cases4.sh`, `cases5.sh` to
  `~/wcontainers-x86/proof/` and run the two scripts there.
- Run 3's experiments: build `stepper3.c`, `approx.c`, `approx_dump.c`, `approx_formula.c`,
  `approx_mxcsr.c` the same way (without `-fno-stack-protector` for the approx ones), copy to
  `~/wcontainers-x86/proof/`, run each (`CODE_BASE=0x7fffffff0000 ./stepper3 e8fb7f0000` for the direct
  call). astro-float without std: `nix develop -c cargo build --release --target wasm32-unknown-unknown
  --manifest-path <scratch>/x87-nostd/Cargo.toml --target-dir <scratch>/x87-nostd/target`.
- Experiments of X0: build `nix develop -c zig cc -target x86_64-linux-musl -static -O2
  -fno-stack-protector -o <scratch>/reference-proof/stepper <scratch>/reference-proof/stepper.c` from the
  checkout root; copy `stepper` and `cases.sh` to `tom@192.168.2.240:wcontainers-x86/proof/`; run
  `ssh tom@192.168.2.240 'cd ~/wcontainers-x86/proof && ./cases.sh'`.
- Size of the decoder: `nix develop -c cargo build --release --target wasm32-unknown-unknown
  --manifest-path <scratch>/iced-size/<base|dec|info>/Cargo.toml --target-dir <scratch>/iced-size/target`,
  then `wasm-tools strip` and gzip -9 / brotli -q 11.

## The x86 machine

- Intel i7-8750H, 6 cores / 12 threads, 15 GB, AVX2, FMA, FSGSBASE, no AVX-512; Ubuntu 20.04.3 with a
  mainline Linux 7.3.4 kernel; reached as `ssh -o BatchMode=yes tom@192.168.2.240`.
- Installed on it by this track: nothing. Already there before this track and not used by it: Rust
  (`~/.rustup` dated 2026-11-14, rustc 1.100.0). Files of this track: `~/wcontainers-x86/proof/` (hello,
  stepper, stepper3, stepper4, stepper5, the scripts cases.sh, cases2.sh, cases4.sh, cases5.sh, cases7.sh,
  cases8.sh, sweep15.sh, cases9.sh, cases10.sh, cases11.sh, cases12.sh, cases13.sh, and approx,
  approx_dump, approx_formula, approx_mxcsr with approx.out, approx_formula.out and approx_mxcsr.out) and
  `~/wcontainers-x86/proof8/` (stepper6, smc, copyuser, cases14.sh, cases15.sh and their outputs) and
  `~/wcontainers-x86/proof9/` (stepper6, cases16.sh, cases16.out) and `~/wcontainers-x86/proof10/` (tf,
  rf, tf.c, rf.c, tf.out, rf.out). The
  cases scripts' outputs before run 8 are kept only in the Mac's scratch `reference-proof/`. The
  independent checks' files are in `~/wcontainers-x86/check` and `check2` to `check9`. Installed in runs
  2 to 10: nothing.
- Not always idle: another track's atomic-cost measurement (`~/ordering-cost`, wasmtime) runs there at
  times. The independent check saw load about 1.04. Run 2 saw 0.07. Record `/proc/loadavg` with every
  timing there.

## Cases actually run

- X0: 22 single-instruction cases on the reference CPU in run 1, 6 more in run 2, 21 more in run 3
  (hand-written, no seed); in run 3 also every one of the 2^32 inputs of `rcpss` and of `rsqrtss`, three
  times (analysis, formula in MXCSR 0x1F80, formula in 0xFFC0); in run 4, 21 more hand-written cases; in
  run 5, 21 hand-written cases (`cases7.sh`, `cases8.sh`) and the 1,024-opcode sweep twice over (each
  opcode at the boundary and alone, 2,048 runs), plus one ARM64 `brk` in Docker; in run 6, 46 hand-written
  cases (`cases9.sh` 38, `cases10.sh` 8); in run 7, 45 hand-written cases (`cases11.sh` 15, `cases12.sh`
  18, `cases13.sh` 12); in run 8, 41 hand-written cases (`cases14.sh` 24, `cases15.sh` 17), one run of
  `smc` and 42 copy cases (`copyuser`: 14 buffer placements for each of three calls). `cases14.sh` ran
  twice: the first run did not pass its environment settings to `stepper6` and was discarded; in run 9,
  41 hand-written cases (`cases16.sh` 39, the two `xacquire` runs 2); in run 10, one run each of `tf`
  (8 trap-flag scenarios in 9 runs, 20 stops) and `rf` (8 signal frames), and 6 `stepper6` cases.

## Measurements

All in the "X0, run 1" to "run 10" sections above (runs 9 and 10 timed nothing).

## Licences of what this track uses

- Added at X1, all already in the workspace: `blake3` (CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH
  LLVM-exception) as a dev-dependency of the test crate; `wasm-encoder` (Apache-2.0 WITH LLVM-exception OR
  Apache-2.0 OR MIT) as a dependency of the test crate and of the interface crate; `zerocopy` 0.8
  (BSD-2-Clause OR Apache-2.0 OR MIT) as a dependency of the interface crate and of the browser crate;
  `wasmparser` 0.263 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT) as a dev-dependency of the
  translator (the test guest's validity test).

- iced-x86 1.22.0: MIT. Not yet a dependency of anything in the repository (X3 adds it, and as a
  build-dependency with `instr_info`).
- astro-float 0.9.6: MIT, with astro-float-num 0.3.7 and astro-float-macro 0.4.6 (both MIT), lazy_static,
  itertools, either (all MIT or Apache-2.0, already common). Planned for X7 (x87 transcendentals); not yet a
  dependency.
- rustc_apfloat 0.2.3: Apache-2.0 WITH LLVM-exception, already a dependency of wcontainers-arm64.
- sandsifter: BSD-3 (Battelle); read only, nothing copied.
- RISU: EPL-1.0; read only, nothing copied.
- gex: AGPL-3.0; not read, nothing used.
- Looked at in run 9 and not used: softfloat-sys 0.1.4 (BSD-3-Clause, bundles Berkeley SoftFloat 3,
  BSD-3), softfloat-rs 0.1.0 (BSD-3-Clause), softfloat 1.0.0 (MIT OR Apache-2.0), machina-softfloat 0.1.2
  (MIT), era2m_softfloat 0.2.1 (BSD-3-Clause-Clear). Nothing copied.

## Departures from the plan

X1 (each also written into the plan's text in the same commit):
- The kernel's side: the plan had the interface crate need std and the kernel never depend on a CPU crate,
  against the brief ("the kernel is written once against the interface"). As the orchestrator ruled, the
  interface and each CPU crate now have a `no_std` core (`Personality`: register block and offsets now, the
  rest of the personality at X10) and a translation half behind the default feature `translate`; the kernel
  will depend on both with it off (plan 3.1, 6). `X86Block` lives in x86's core, not the abi.
- Each CPU's test support (`GuestTest`, plan 7.2) lives in the test crate, not in the CPU crate, though the
  brief says each CPU crate supplies everything specific to its CPU: it needs test-only dependencies
  (wasmtime, Docker and SSH transport, dynasm, iced's assembler) that a shipped CPU crate must not carry,
  and it is used only by the tests. The CPU crate still supplies everything the translator, runners and
  kernel use.
- Run 1 built only the interface items ARM64 uses and put the rest off; run 2 built them all (plan 3.2,
  "Built whole at X1"), as the plan reviewed before X1 had it. Three uses that change ARM64's output stay
  with the milestone whose x86 code needs them, each its own commit with its own byte-for-byte comparison:
  the description section's guest and offer bytes (X3) and the stop and memory fault hooks returning the
  unwind payload (X6, which the runners' `step` and code-change handling need). The first frame check also
  covering a `Stack` return slot waits for X4, where it is measured (gated on `ReturnAddress::Stack`).
- No `NARROW_WRITE` item: nothing would read it. The proofs use a 32-bit induction only through a 32-bit
  read, where zero and sign extension agree, and x86's merging writes are never inductions (plan 3.2).
- `HookType` gains `state: StateBytes`, the bytes of the thread state a `Cpu` hook works on, and
  `run_hook` gets only those, so a hook costs its work and not a copy of the whole state (plan 3.2, 3.4).
- `Guest::STEPS` (default false) beside `step`, so runners copy nothing for a CPU that cannot step.
- `TrapKind::Overflow` (7) for x86 `int 4` (plan 3.2, 5.2, 6).
- `initial_block` is in `Personality`, not `Guest`: the kernel makes threads too.
- A CPU's own slow paths are numbered from `GUEST_REASONS` (64); the translator's `Sites` has room for
  16 of them; `Reason::name_of::<G>` names any number.
- The runners' stops for faults the abi does not name yet are the test crate's `Stopped::Fault`; the
  abi's `Exit` grows them at X10 (the abi is not this track's until then).
- Signatures changed while building, to keep ARM64's bytes: `trap_kind` returns `TrapPc` too;
  `temporaries(flags)` added (logic takes no temporaries today); `run_hook` takes the hook's index;
  `data_address` and `branch_target` push rather than work in place; `branch_comparison` gets each path
  instruction's effects and the branch's offset; `loaded_max(i)` returns the register and the bound;
  `built_address` and `jump_table` get no `before` walk until x86 needs it (X4). The runtime's `Reason`
  list moved into the interface (the contract re-exports it and `TrapKind`).
- The comparison is one command (`same-output.sh`) writing lengths and BLAKE3 hashes of every module, not
  the `WC_BENCH_DUMP` knob and stored files: Bun alone is about 4 GB a variant, and the lists cover full and
  bounded look-ahead too. Equality of two 256-bit hashes is taken as byte for byte equality.
- `tests/valid.rs` moved from the translator crate to the test crate (it draws ARM64's forms), so the
  translator has no dependency on a CPU crate at all, dev-dependencies included (X0 finding).
- Run times: wasmtime only, old and new alternating, best of 5 (the orchestrator's X1 task), not wasmtime
  and Chrome best of 3 as the plan had.
- The X1 pass bar gains a test guest (`tests/test_guest.rs`), the `no_std` build as a test, the float hook
  timing and the wider comparison (plan 9).
- No intentional change of ARM64's output at X1: the stop check's raised branch changes at X6 (plan 5.1),
  so the whole move is compared byte for byte against one "before".

## Decisions taken in the plan that the owner may want to hear about

- The AVX setting claims the whole x86-64-v3 level (AVX, AVX2, FMA, F16C, BMI2, XSAVE), not AVX alone.
- CPUID claims vendor GenuineIntel with the reference CPU's family and model, and a brand string of our own.
- FPU is claimed by default although x87 always runs in software (counted). That is the plan's reading
  of the brief's claim rule ("only features every instruction of which maps onto WebAssembly without a
  software slow path"), not its letter: x86-64 has no CPU without x87, the ABI puts `long double` there
  without asking CPUID, and the brief itself plans x87 as a counted slow path (plan 5.10). MMX is
  offered (baseline).
- Every x86 `call` to a non-canonical target writes the return address before its general protection,
  because the i7-8750H does so (Intel's pseudo-code says otherwise).
- 64-bit `div` and `idiv` with a full 128-bit dividend run an inline long division that is not counted as
  a slow path; its cost is measured at X3.
- `rcpps` and `rsqrtps` reproduce Intel's approximations exactly by a formula found on the i7-8750H
  (other CPUs are expected, not seen, to give other approximations; the reference decides).
- The lookup array costs four bytes of state memory per byte of x86 code.
- Switch tables with absolute entries (non-PIE code) and tables over the br_table limit go through the
  lookup, counted; revisited only if X9 shows them common.
- The legacy vsyscall page is not provided.
- `int 0x80` (the 32-bit system call gate) gives general protection, as a kernel without 32-bit
  emulation does, until 32-bit mode is built.
- Every x86 `ret` and every jump or call through a register or memory pays a canonical-address check.
  It is about four operations each.
- astro-float adds about 41 KB brotli to the x86 browser build (scratch measurement).
- `TrapKind` (and later the abi's `Exit`) grow divide error, general protection and float exception; the
  abi's `Exit` also gains single step (from the stepping of section 5.1, not a `TrapKind`), and it and the
  runners' memory faults gain x86's stack fault (SIGBUS), which the
  i7-8750H gives for a non-canonical access through rsp or rbp.
- Alignment checking (rflags AC) is not provided: `popf` leaves AC clear, as on a CPU without it, counted
  and warned. A real x86 Linux would fault misaligned accesses while AC is set.
- A far transfer to Linux's 32-bit code segment (selector 0x23) gives general protection, counted, until
  32-bit mode is built.
- `sysenter` also gives general protection (as `int 0x80`), while SEP is claimed: the instruction exists,
  the kernel has no 32-bit calls behind it, as on Linux without 32-bit emulation.
- Code written on the page that writes it costs one fault, one step and two translations per write (the
  faulting instruction runs once from an unprotected one-instruction piece), counted with "code written
  without announcement".
- Accesses that are not base plus scaled index (`bt` with a register offset, gathers, masked moves, 8-
  and 16-bit indexes such as `xlat`'s) are checked every time by their own meanings and never grouped or
  proven; expected to cost little, measured at X5 and X8.
- The kernel's copy routines get a measured model per CPU; x86's writes every byte up to the fault.
- Glibc programs without `BIND_NOW` fire the counter "MXCSR's exception flags read while not kept" at each
  lazy binding (the loader's `fxsave`), warning once; a state restore never moves a program off the fast
  path, only `ldmxcsr` or a non-default mode does.
- x86 instructions of features not claimed do what they do on a CPU without the feature: the reserved-NOP
  space (CET's `endbr64` and `rdssp`, `cldemote`, MPX, `prefetchw`) runs as NOPs, so CET-built programs run;
  the rest gives illegal instruction.
- Berkeley SoftFloat is not used for x87 precision control and `fsqrt`, though it does exactly that job:
  no crate wrapping it builds for these targets, and vendoring its C would bring C into the x86 crate. The
  plan keeps rustc_apfloat and astro-float with a little glue.
- At X6 the memory fault hook returns the unwind payload, which changes ARM64's output by one instruction
  per fault site (behaviour unchanged).
- x86 layer 1's code area is 64 KiB around the instruction, filled with `int3` on both sides; ARM64's stays
  256 MiB of zeros.
- A program's own trap flag is held and stepped exactly: each instruction then runs alone through the exact
  path, counted ("trap flag set by the program" and "left to address"). From X6 the stop hook returns the
  unwind payload, which changes ARM64's output inside each stop check's raised-flag branch (behaviour
  unchanged). One known difference: a gather faulting part way under TF gets no extra SIGTRAP first.
- SSE3 and CMOV are claimed although each holds a few x87 instructions (`fisttp`; `fcmov`, `fcomi`), which
  run in software like all x87, under the same reading as FPU.
- Layer 1 runs the never-claimed forms (EVEX, XOP, 3DNow and the rest) on the reference at 100 cases a
  form, not 1,000, since an instruction the CPU lacks stops before touching its operands.
