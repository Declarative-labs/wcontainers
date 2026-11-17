# x86 track status

Plan: docs/phase-2-guest-cpu-and-x86.md. Branch: phase-2-x86. Brief:
pyro-parked/wcontainers-scratch/orchestrator/brief-x86.md.

## Where things stand

- X0 (the plan, and the x86 reference machine proven): done in run 1, 2026-11-17; the first independent
  check's findings fixed in run 2, the second check's in run 3 and the third check's in run 4, the same
  day (below).
- Next: X1, the interface crate `wcontainers-guest-cpu` with ARM64 moved onto it and its output proven byte
  for byte the same (plan section 9). No code of this track exists yet.

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

## How to run

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
  stepper, stepper3, stepper4, cases.sh, cases2.sh, cases4.sh, cases5.sh, approx, approx_dump,
  approx_formula, approx_mxcsr and their .out files); the independent checks' files are in
  `~/wcontainers-x86/check`, `check2` and `check3`. Installed in runs 3 and 4: nothing.
- Not always idle: another track's atomic-cost measurement (`~/ordering-cost`, wasmtime) runs there at
  times. The independent check saw load about 1.04. Run 2 saw 0.07. Record `/proc/loadavg` with every
  timing there.

## Cases actually run

- X0: 22 single-instruction cases on the reference CPU in run 1, 6 more in run 2, 21 more in run 3
  (hand-written, no seed); in run 3 also every one of the 2^32 inputs of `rcpss` and of `rsqrtss`, three
  times (analysis, formula in MXCSR 0x1F80, formula in 0xFFC0); in run 4, 21 more hand-written cases.

## Measurements

All in the "X0, run 1", "run 2" and "run 3" sections above.

## Licences of what this track uses

- iced-x86 1.22.0: MIT. Not yet a dependency of anything in the repository (X3 adds it, and as a
  build-dependency with `instr_info`).
- astro-float 0.9.6: MIT, with astro-float-num 0.3.7 and astro-float-macro 0.4.6 (both MIT), lazy_static,
  itertools, either (all MIT or Apache-2.0, already common). Planned for X7 (x87 transcendentals); not yet a
  dependency.
- rustc_apfloat 0.2.3: Apache-2.0 WITH LLVM-exception, already a dependency of wcontainers-arm64.
- sandsifter: BSD-3 (Battelle); read only, nothing copied.
- RISU: EPL-1.0; read only, nothing copied.
- gex: AGPL-3.0; not read, nothing used.

## Departures from the plan

None yet (no milestone of the plan has been built).

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
- `TrapKind` (and later the abi's `Exit`) grow divide error, general protection, float exception and single
  step.
- Alignment checking (rflags AC) is not provided: `popf` leaves AC clear, as on a CPU without it, counted
  and warned. A real x86 Linux would fault misaligned accesses while AC is set.
- A far transfer to Linux's 32-bit code segment (selector 0x23) gives general protection, counted, until
  32-bit mode is built.
