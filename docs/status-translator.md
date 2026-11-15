# Translator status

Plan: docs/phase-1-translator.md. Branch: phase-1-translator.

## Where things stand

M0 (skeleton) is built and tested. M1 has not started.

## Done

- `wcontainers-arm64`: our own `Instruction` type; `decode` wraps yaxpeax-arm 0.5.0 and settles from the raw
  word what its operands leave open (shifted against extended add, register-offset index fields, system
  register identity) and the reserved encodings it accepts (32 bit shift amounts of 32 or more, extend
  amounts above 4, reserved logical bit masks, EXTR lsb of 32 or more in 32 bit). `emit` holds the meaning of
  every base integer instruction once, as WebAssembly, against the `Emitter` trait. `Instruction::flow`
  says where control can go.
- `wcontainers-translator`: `translate(blob, code range, starts, LookAhead::None)` and `thread_module(slots)`.
  `contract` is the module contract: import names, hooks, trap kinds, warning reasons, permission table bits,
  description section. Look-ahead none follows direct control flow from the starts; every instruction reached
  is its own piece.
- Piece convention: params x0 to x3, sp, expected return address; results x0 to x3, sp. Everything else in
  thread module globals. Returns check x30 against the expected address, else leave through "go to address"
  (an exception the bottom loop catches).
- Lookup: one table slot per 4 bytes of code (design one of the two). The second design is not built.
- Permission check (one byte per page, with next-page bits) and stop check are already in look-ahead none.
- `wcontainers-translator-test`:
  - `runner`: wasmtime runner, one thread, the hooks, the stand-in for write, writev, exit, exit_group, brk,
    and translation at run time when control reaches an address in a loaded blob with no piece.
  - `reference/`: native reference runner (own small package, built with rust-lld for
    aarch64-unknown-linux-musl, run in Docker as `alpine:latest`). Enters a case through a rewritten signal
    frame, so every register is exact; catches system calls from the test page with seccomp.
  - `arm/a64-forms.txt`: all 4429 A64 encodings from Arm's specification (2026-09 release), made by
    `cargo run -p wcontainers-translator-test --bin arm-forms -- <Instructions.json> <out>`.
  - Layer 1: `tests/layer1.rs`. 261 forms (every base integer form M0 covers).
  - `tests/programs.rs`: `programs/integers.c` built at -O0, -O1, -O2 as a flat blob; output and exit
    status identical to real ARM64 Linux.

## How to run

- Everything: `nix develop -c cargo test -p wcontainers-arm64 -p wcontainers-translator -p wcontainers-translator-test`
- Layer 1, long: `WC_LAYER1_CASES=10000 WC_LAYER1_SEED=<n> nix develop -c cargo test --release -p wcontainers-translator-test --test layer1 -- --nocapture`
  (defaults: 64 cases, seed 1). Replay a failure: same seed and cases, plus `WC_LAYER1_FORMS=<form name>`.
- Translator proptest: `PROPTEST_CASES`, `PROPTEST_RNG_SEED` (default seed 1).

## Cases actually run

- Layer 1: 261 forms x 64 cases, seed 1: 0 mismatches.
- Layer 1: 261 forms x 1000 cases, seed 2: see below (result added when the run finished).
- Pass bar of the plan (10,000 per form) not yet run.

## Measurements

- Layer 1, 261 forms x 64 cases (debug build, opt-level 1): 93 s at load 26 to 34, about 5.6 ms per case.
- integers.c, look-ahead none, every instruction a start, release build, best of 5, load 64 to 77:
  - -O0: 2896 bytes of code, translated in 0.51 ms (5.6 MB/s), 113,270 bytes of WebAssembly (39.1 per byte),
    wasmtime compile 32 ms.
  - -O2: 3428 bytes of code, translated in 0.65 ms (5.3 MB/s), 105,767 bytes (30.9 per byte), compile 39 ms.
- Compressed size, run time against native and against elfconv: not measured yet.

## Next

1. M1: hello world with a C library at none, in wasmtime and Chrome (the Chrome runner does not exist yet).
2. Before M2: decide the alignment exit with the orchestrator (see "Shared crate").

## Known gaps (behave as the plan says: illegal instruction, counted, warned)

- Not translated yet (M3): floating point, vectors, CRC32, LSE atomics, exclusives, LDAR/STLR family, system
  instructions (SYS, DC ZVA, IC IVAU, MSR to PSTATE), system registers other than NZCV and TPIDR_EL0 (FPCR and
  FPSR included).
- SP alignment: Linux faults (SIGBUS) on a load or store through a misaligned SP; the translator does not
  check it. Layer 1 keeps SP 16-aligned.
- Only 64 bit guest memory. 32 bit memory hosts need the high-bit check (section 6).
- The WebAssembly stack overflowing on deep guest recursion is not handled.
- Stand-in brk never moves the break.

## Departures from the plan

- Two kinds of module per thread: a thread module (register globals, lookup table, unwind tag, bottom loop,
  save and load state) and code modules (pieces only, importing the thread module's globals and table). The
  plan has every module export run and save/load; here only the thread module does, so code loaded later
  and split modules are all the same kind.
- A second memory, "state", shared, holds thread blocks (stop flag at 800) and the permission table, so the
  guest cannot reach them.
- The permission check and the stop check are in look-ahead none from M0, so layer 1 can compare faults
  exactly. In none every piece checks the flag at entry (atomic load) and after every system call.
- Top Byte Ignore is part of the meanings: data addresses ignore the top byte when bit 55 is clear (Linux
  turns it on for user space); branch targets through registers get the same treatment. Costs a few
  instructions per access; to be measured at M2.
- LDAR/STLR are reported unsupported rather than translated as plain accesses: they need an alignment fault.
- Layer 1 excludes: CONSTRAINED UNPREDICTABLE encodings (write back to a transfer register, load pair into
  one register), branches to themselves (never finish), and hints whose meaning differs on a CPU with pointer
  signing or later features (the reference CPU has them, the CPU we present does not; we run them as
  nothing, as the plan says). System register moves are limited to NZCV and TPIDR_EL0.
- A jump to a misaligned address: the CPU raises SIGBUS at the target; the translator goes there and finds no
  piece. Layer 1 counts that as the same outcome; the CPU side must turn it into the signal.
- Arm's site refuses automated downloads (403/404), so the specification came from the kernel.org mirror of
  Arm's BSD release (kernel.googlesource.com/pub/scm/linux/kernel/git/maz/AARCHMRS, release 2026-09), kept in
  pyro-parked/wcontainers-scratch/translator/arm-mrs. Only the derived forms list, with Arm's notice, is
  committed.
- Layer 1's knobs are WC_LAYER1_CASES, WC_LAYER1_SEED and WC_LAYER1_FORMS (one proptest runner per form).
- Test programs are built with `-mcpu=generic-neon-fp_armv8`: zig ignores `-mgeneral-regs-only`.

## Shared crate (wcontainers-abi)

- No changes.
- Needed later, for the orchestrator: an exit for alignment faults (Linux SIGBUS, BUS_ADRALN): jumps to
  misaligned addresses, LDAR/STLR and exclusives on misaligned addresses, and misaligned SP. `Exit` has no
  variant for it today.
