# Translator status

Plan: docs/phase-1-translator.md. Branch: phase-1-translator.

## Where things stand

M0 (skeleton) is built and tested, including the fixes from the independent check of M0 (second run). M1
has not started.

## Done

- `wcontainers-arm64`: our own `Instruction` type; `decode` wraps yaxpeax-arm 0.5.0 and settles from the raw
  word what its operands leave open (shifted against extended add, register-offset index fields, system
  register identity) and the reserved encodings it accepts (32 bit shift amounts of 32 or more, extend
  amounts above 4, reserved logical bit masks, EXTR lsb of 32 or more in 32 bit). `emit` holds the meaning of
  every base integer instruction once, as WebAssembly, against the `Emitter` trait. `Instruction::flow`
  says where control can go. `forms` reads Arm's list of every A64 form (`arm/a64-forms.txt`) and evaluates
  each form's condition (features and field limits) against the CPU we present (`OFFERED_FEATURES`: FP,
  AdvSIMD, CRC32, LSE). A word the decoder does not translate is `Unsupported` only if it is an instruction
  of that CPU, otherwise `Undefined`. RPRFM is the one form Arm carved out of a hint space, so without its
  feature it is a PRFM hint and does nothing.
- Alignment faults: a load or store with SP as its base and SP not a multiple of 16, and a jump to an
  address that is not a multiple of 4, stop with `Exit::AlignmentFault` (Linux: SIGBUS, BUS_ADRALN, at SP or
  at the target). Prefetches are not checked, as on the CPU.
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
    and translation at run time when control reaches an address in a loaded blob with no piece. Every slow
    path goes through one function: counted by reason, timed where the runner does the work (translation at
    run time), and warned once per address (or system call number) when its count there reaches
    `contract::WARNING_THRESHOLD` (1 for now). Warnings go to stderr and `Machine::warnings`; counters read
    as abi `CounterReading`s.
  - `reference/`: native reference runner (own small package, built with rust-lld for
    aarch64-unknown-linux-musl, run in Docker as `alpine:latest`). Enters a case through a rewritten signal
    frame, so every register is exact; catches system calls from the test page with seccomp.
  - `crates/wcontainers-arm64/arm/a64-forms.txt`: all 4429 A64 encodings from Arm's specification
    (2026-09 release), made by `cargo run -p wcontainers-translator-test --bin arm-forms -- <Instructions.json> <out>`.
  - Layer 1: `tests/layer1.rs`. 266 forms: every base integer form M0 covers, including the five whose
    condition only limits fields (LDRB/STRB/LDRSB register, PRFM register). SP is misaligned in a quarter of
    cases. A second test checks fixed alignment cases against the CPU and checks they reach the fault.
  - `tests/decoding.rs`: words of every form in Arm's list that the CPU we present does not have decode as
    undefined (pointer signing branches and loads, FlagM, SB, CSSC, MTE and the rest); the offered features
    match the abi's HWCAP.
  - `tests/programs.rs`: `programs/integers.c` and `programs/tables.c` (only zeroed data, a jump table)
    built at -O0, -O1, -O2, -O3, -Os as flat blobs; output and exit status identical to real ARM64 Linux.
    zig cc gives the same bytes at -O1, -O2 and -O3 for both, so that is 3 distinct blobs per program, 6 in
    all. The linker script follows zeroed data with one byte so a flat copy keeps it.

Found and fixed in the second run: the misaligned SP and jump faults above; `translate` overflowed on a
branch whose target wraps below the blob (found by layer 1, regression test in `tests/valid.rs`); flat blobs
lost zeroed data when a program had no initialised data.

## How to run

- Everything: `nix develop -c cargo test -p wcontainers-arm64 -p wcontainers-translator -p wcontainers-translator-test`
- Layer 1, long: `WC_LAYER1_CASES=10000 WC_LAYER1_SEED=<n> nix develop -c cargo test --release -p wcontainers-translator-test --test layer1 -- --nocapture 2>&1 | grep -v "^wcontainers warning"`
  (defaults: 64 cases, seed 1). Replay a failure: same seed and cases, plus `WC_LAYER1_FORMS=<form name>`.
  The filter drops the runner's warnings (memory faults and stops), one per case.
- Decoding against Arm's forms: `WC_DECODE_CASES` (default 64 per form), `WC_DECODE_SEED` (default 1).
- Translator proptest: `PROPTEST_CASES`, `PROPTEST_RNG_SEED` (default seed 1).

## Cases actually run

- Layer 1: 261 forms x 64 cases, seed 1: 0 mismatches.
- Layer 1: 261 forms x 1000 cases, seed 2: 0 mismatches (261,000 cases). This was before SP could be
  misaligned in cases; the misaligned SP bug was there and not found.
- After the fixes: 266 forms x 64 cases, seed 1: 0 mismatches. 5 newly covered forms x 500 cases, seed 1:
  0 mismatches.
- Decoding: 4429 forms x 64 words, seed 1, and 4429 x 2000 words, seed 2: 0 failures.
- Pass bar of the plan (10,000 per form) not yet run.

## Measurements

- Layer 1, 261 forms x 64 cases (debug build, opt-level 1): 93 s at load 26 to 34, about 5.6 ms per case.
- Layer 1, 261 forms x 1000 cases: 1334 s at load 39 to 73, about 5.1 ms per case.
- Look-ahead none, every instruction a start, release build, best of 5, load 53 to 88 (gzip at its default
  level; the ratio is gzipped WebAssembly over gzipped code):
  - integers -O0: 2896 bytes of code (1674 gzipped), translated in 0.53 ms (5.4 MB/s), 121,756 bytes of
    WebAssembly (42.0 per byte), 14,151 gzipped (8.5), wasmtime compile 31 ms.
  - integers -O2: 3428 bytes (1953), 0.51 ms (6.7 MB/s), 106,447 bytes (31.1), 15,366 gzipped (7.9), 30 ms.
  - tables -O0: 2368 bytes (1289), 0.43 ms (5.5 MB/s), 97,012 bytes (41.0), 11,497 gzipped (8.9), 22 ms.
  - tables -O2: 1552 bytes (1204), 0.30 ms (5.2 MB/s), 46,153 bytes (29.7), 7,918 gzipped (6.6), 14 ms.
  - integers grew from 113,270 / 105,767 bytes (first run) mostly by the SP alignment checks.
- Run time against native and against elfconv: not measured. The M0 programs run for far less than a
  millisecond natively, so a comparison means nothing yet; it starts with the benchmark set at M2. elfconv
  is not set up.

## Next

1. M1: hello world with a C library at none, in wasmtime and Chrome (the Chrome runner does not exist yet).
2. Before M2: decide the alignment exit with the orchestrator (see "Shared crate").

## Known gaps (behave as the plan says: illegal instruction, counted, warned)

- Not translated yet (M3): floating point, vectors, CRC32, LSE atomics, exclusives, LDAR/STLR family, system
  instructions (SYS, DC ZVA, IC IVAU, MSR to PSTATE), system registers other than NZCV and TPIDR_EL0 (FPCR and
  FPSR included).
- Only 64 bit guest memory. 32 bit memory hosts need the high-bit check (section 6).
- The WebAssembly stack overflowing on deep guest recursion is not handled.
- Stand-in brk never moves the break.
- Slow paths taken inside translated code (left to an address) and hook calls that do no work are counted
  with zero time; only the runner's own work is timed.
- Layer 1 checks only words the CPU we present has. Whether undefined words really raise SIGILL on a CPU
  without the feature is checked against Arm's list, not against a CPU (the reference CPU has newer
  features than the one we present).

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
- The alignment check for jumps is in the thread module's bottom loop and in each code module's dispatcher,
  where a target with no piece is handled, so it costs nothing on the normal path.
- A slow path warns at its first occurrence per address (`WARNING_THRESHOLD` = 1); no measurement yet says
  a higher threshold is needed.
- The forms list moved to `wcontainers-arm64` (it is ARM64 knowledge the decoder now uses);
  `wcontainers-translator-test` keeps the tool that makes it.
- Arm's site refuses automated downloads (403/404), so the specification came from the kernel.org mirror of
  Arm's BSD release (kernel.googlesource.com/pub/scm/linux/kernel/git/maz/AARCHMRS, release 2026-09), kept in
  pyro-parked/wcontainers-scratch/translator/arm-mrs. Only the derived forms list, with Arm's notice, is
  committed.
- Layer 1's knobs are WC_LAYER1_CASES, WC_LAYER1_SEED and WC_LAYER1_FORMS (one proptest runner per form).
- Test programs are built with `-mcpu=generic-neon-fp_armv8`: zig ignores `-mgeneral-regs-only`.

## Shared crate (wcontainers-abi)

- Added `Exit::AlignmentFault { address }` (one new variant, additive): a load or store through an SP that
  is not a multiple of 16 (`pc` at the instruction, `address` = SP), or a jump to an address that is not a
  multiple of 4 (`pc` = `address` = the target). The kernel should deliver SIGBUS, BUS_ADRALN, si_addr =
  `address`, as Linux does. The kernel half must add a match arm for it. Needed because the independent
  check found the translator carrying on where Linux faults; without it the CPU side cannot report the
  fault. Later LDAR/STLR and exclusives on misaligned addresses will use it too.
- Seen, not changed: `cargo clippy -p wcontainers-abi --all-targets -- -D warnings` fails on the abi's own
  layout test (`assertions_on_constants`, registers.rs line 79).
