# Translator status

Plan: docs/phase-1-translator.md. Branch: phase-1-translator.

## Where things stand

M0 (skeleton) and M1 (hello world at look-ahead none, in wasmtime and Chrome) are built, tested and
pushed, including the fixes from the independent check of M1 (third M1 run). M2 has not started.

## Done

- `wcontainers-arm64`: our own `Instruction` type; `decode` wraps yaxpeax-arm 0.5.0 and settles from the raw
  word what its operands leave open (shifted against extended add, register-offset index fields, system
  register identity) and the reserved encodings it accepts (32 bit shift amounts of 32 or more, extend
  amounts above 4, reserved logical bit masks, EXTR lsb of 32 or more in 32 bit). `emit` holds the meaning of
  every base integer instruction once, as WebAssembly, against the `Emitter` trait. `Instruction::flow`
  says where control can go. `forms` reads Arm's list of every A64 form (`arm/a64-forms.txt`) and evaluates
  each form's condition (features and field limits) against the CPU we present (`OFFERED_FEATURES`: FP,
  AdvSIMD, CRC32, LSE). `decode` asks the forms first: a word in no form of that CPU is `Undefined`
  before yaxpeax-arm sees it (yaxpeax-arm accepts some words that are in no form, such as MOVZ W with
  hw=2 and conditional selects with wrong fixed bits). A word of that CPU the decoder does not translate
  is `Unsupported`. The forms are indexed by bits 31 to 21, built once on first use. RPRFM is the one form
  Arm carved out of a hint space, so without its feature it is a PRFM hint and does nothing.
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
    match the abi's HWCAP; random words from the whole 32 bit space: every word we translate is of a form
    layer 1 covers (or the RPRFM hint carve-out); 12 words in no form that yaxpeax-arm accepts are undefined.
  - Layer 1 also runs random words in no form of Arm's list (for any feature) on the CPU and through ours:
    both raise illegal instruction.
  - Stand-in write and writev read the permission table: a buffer (or writev's vector array) with any byte
    on a page that does not allow reading gives -EFAULT and writes nothing, as Linux does for a pipe (Docker
    gives the reference a pipe). writev takes its vector count as 32 bits, as Linux does, and gives -EINVAL for
    more than 1024 vectors and for a length that is negative as a signed number. `programs/buffers.c` checks all of these against Linux, in wasmtime and
    in Chrome (`flat.ld` now defines `__blob_end`, so it can reach the page after the blob).
  - Stop hook stand-in: a flag raised from outside is cleared, counted ("stopped by the flag"), warned, and
    the thread carries on (hook returns 0), as the plan says. The runners raise the flag themselves to make
    the thread leave after exit or exit_group (and after every system call in layer 1); that is not counted.
    `tests/smoke.rs` raises it before a run and checks the result and the count.
  - `tests/programs.rs`: `programs/integers.c` and `programs/tables.c` (only zeroed data, a jump table)
    built at -O0, -O1, -O2, -O3, -Os as flat blobs; output and exit status identical to real ARM64 Linux.
    zig cc gives the same bytes at -O1, -O2 and -O3 for both, so that is 3 distinct blobs per program, 6 in
    all. The linker script follows zeroed data with one byte so a flat copy keeps it.
  - M1, `programs/hello.c`: printf through the C library (musl as zig builds it, its own crt1.o `_start`,
    init and fini arrays placed by `programs/flat.ld`), exit status 3. Built at all five levels (-O1, -O2,
    -O3 give the same blob, so 3 distinct). Stdout, stderr and exit status byte-identical to real ARM64
    Linux in wasmtime (`tests/programs.rs`) and in Chrome (`tests/browser.rs`, which also runs integers and
    tables at every distinct level).
  - Starting stack (`layout::initial_stack`): argc 1, argv "blob", empty environment, auxv with AT_PAGESZ,
    AT_HWCAP (= abi HWCAP, checked by a test), AT_HWCAP2 0, AT_RANDOM, AT_EXECFN. Shared by both runners and
    the native reference, so all three start the blob identically.
  - The C library reaches SIMD and floating point loads and stores and some vector data processing
    (54 forms: MOVI, FMOV, ORR, DUP, CMEQ, UMAXV, EXT, CNT and others; the list is `VECTOR_FORMS` in
    `src/layer1.rs`). These, and every SIMD and floating point load and store form in the load/store groups
    layer 1 covers, are translated now (`wcontainers-arm64/src/vector.rs`), and layer 1 carries v0 to v31
    both ways. Layer 1 covers 399 forms in all.
  - Hello uses exactly these system calls: set_tid_address and ioctl (answered "not implemented", counted
    and warned, as the plan says; musl carries on), writev, exit_group.
  - Chrome runner: `browser/` is the translator built for wasm32 (its own workspace), `browser/runner.js` a
    module worker doing what the wasmtime runner does (hooks, stand-in system calls, counters, warnings,
    translation at run time in the page), every name and number read from the translator's contract
    except where the thread block and permission table sit in the state memory, which the runner chooses.
    `browser/run.mjs` drives the installed Chrome headless through playwright-core from the Nix shell,
    serving the files from https://wcontainers.test with cross-origin isolation headers.
    `src/browser.rs` builds it and reads the result. Ran in Chrome 157.

Found and fixed in the second run: the misaligned SP and jump faults above; `translate` overflowed on a
branch whose target wraps below the blob (found by layer 1, regression test in `tests/valid.rs`); flat blobs
lost zeroed data when a program had no initialised data.

Found and fixed in M1: tests failed now and then with empty output from the native side. Cargo replaces
the built reference binary on every build, even with nothing to do, and every native run called cargo, so
a container started while another test's build was running could find no binary. The reference is now
built once per test process and copied to `target/reference/run/<pid>/`. Builds also work under names of
their own and each native run reads its own copy of the blob. After the fix: 15 runs of
`tests/programs.rs` in a row passed (it had failed three times in about 25 runs before).

Found and fixed in the third M1 run (from the independent check): yaxpeax-arm accepted about 0.45% of all
32 bit words that are in no Arm form as base integer instructions, and we ran them where the CPU raises
SIGILL (fixed by asking the forms first). The stand-in write and writev ignored the permission table and
wrote out unreadable buffers; a length of -1 crashed the wasmtime runner. The stop hook counted every
exit as a stop and always made the thread leave. The description section had a hand-written LEB128
encoder (now wasm-encoder's `Encode`). The permission byte was built with a fixed shift (now from the
contract's bits, in both runners).

## How to run

- Everything: `nix develop -c cargo test -p wcontainers-arm64 -p wcontainers-translator -p wcontainers-translator-test`
  (includes the Chrome test; it needs Google Chrome installed and Docker running).
- Lint the two side packages too: from `crates/wcontainers-translator-test/browser`,
  `nix develop ../../.. -c cargo clippy --target wasm32-unknown-unknown --target-dir ../../../target/browser -- -D warnings`;
  from `.../reference`, `nix develop ../../.. -c cargo clippy --release --target-dir ../../../target/reference -- -D warnings`.
- Layer 1, long: `WC_LAYER1_CASES=10000 WC_LAYER1_SEED=<n> nix develop -c cargo test --release -p wcontainers-translator-test --test layer1 -- --nocapture 2>&1 | grep -v "^wcontainers warning"`
  (defaults: 64 cases, seed 1). Replay a failure: same seed and cases, plus `WC_LAYER1_FORMS=<form name>`.
  The filter drops the runner's warnings (memory faults and stops), one per case.
- Decoding against Arm's forms: `WC_DECODE_CASES` (default 64 per form), `WC_DECODE_WORDS` (random words
  from the whole space, default 200,000), `WC_DECODE_SEED` (default 1).
- Words in no form against the CPU: part of the layer 1 command; `WC_LAYER1_CASES` words.
- Translator proptest: `PROPTEST_CASES`, `PROPTEST_RNG_SEED` (default seed 1).

## Cases actually run

- Layer 1: 261 forms x 64 cases, seed 1: 0 mismatches.
- Layer 1: 261 forms x 1000 cases, seed 2: 0 mismatches (261,000 cases). This was before SP could be
  misaligned in cases; the misaligned SP bug was there and not found.
- After the fixes: 266 forms x 64 cases, seed 1: 0 mismatches. 5 newly covered forms x 500 cases, seed 1:
  0 mismatches.
- Decoding: 4429 forms x 64 words, seed 1, and 4429 x 2000 words, seed 2: 0 failures.
- M1, with the vector forms and the UNKNOWN-after-fault rule: 399 forms x 1000 cases, seed 2, release
  build: 0 mismatches (399,000 cases).
- Third M1 run, after the decoder asks the forms first: random words, 40,000,000, seed 2026, release:
  every translated word is of a form layer 1 covers. Words in no form on the CPU: 2000, seed 1, and 1000,
  seed 3: all illegal on both sides. The same test with the old decoder failed within 2000 words.
- Third M1 run, after the decoder change: layer 1, 399 forms x 1000 cases, seed 3, release build: 0
  mismatches (399,000 cases). This confirms the second run's seed 2 result with a new seed.
- Pass bar of the plan (10,000 per form) not yet run. One command for it is under "How to run".

## Measurements

- Layer 1, 261 forms x 64 cases (debug build, opt-level 1): 93 s at load 26 to 34, about 5.6 ms per case.
- Layer 1, 261 forms x 1000 cases: 1334 s at load 39 to 73, about 5.1 ms per case.
- Layer 1, 399 forms x 1000 cases, release build: 780 s at load 22 to 29, about 2.0 ms per case. At that
  rate the 10,000 per form pass bar is about 2.2 hours.
- Layer 1, 399 forms x 1000 cases plus 1000 words in no form, release build: 480 s at load 9 to 18 (other
  tests ran beside it), about 1.2 ms per case.
- Look-ahead none, every instruction a start, release build, best of 5, load 53 to 88 (gzip at its default
  level; the ratio is gzipped WebAssembly over gzipped code):
  remeasured in the third M1 run, load 7.4 to 8.6:
  - integers -O0: 2896 bytes of code (1674 gzipped), translated in 0.50 ms (5.8 MB/s), 122,224 bytes of
    WebAssembly (42.2 per byte), 14,277 gzipped (8.5), wasmtime compile 20 ms.
  - integers -O2: 3428 bytes (1953), 0.52 ms (6.6 MB/s), 106,915 bytes (31.2), 15,495 gzipped (7.9), 25 ms.
  - tables -O0: 2368 bytes (1289), 0.43 ms (5.5 MB/s), 97,480 bytes (41.2), 11,619 gzipped (9.0), 15 ms.
  - tables -O2: 1552 bytes (1204), 0.25 ms (6.2 MB/s), 46,621 bytes (30.0), 8,036 gzipped (6.7), 10 ms.
  - hello -O2 (with the C library): 19,752 bytes (12,748), 3.0 ms (6.6 MB/s), 683,475 bytes (34.6),
    89,761 gzipped (7.0), 132 ms.
  - Sizes are unchanged by the third run's fixes. Code modules have imported every vector global since M1
    (468 bytes each). Before M1 integers and tables were 468 bytes smaller (121,756 / 106,447 / 97,012 /
    46,153); integers grew from 113,270 / 105,767 (first run) mostly by the SP alignment checks.
- The forms index (built on the first decode): about 10 ms once, then about 22 ns per word for "is this
  word an instruction of the CPU we present" (1,000,000 random words in 22 ms), release, load 7.5.
- hello in wasmtime translates at run time 10 times per run (pieces reached through registers); those 10
  take 0.58 to 0.79 s in all in a release build, load 21 to 31. That is wasmtime compiling one small
  module per piece; look-ahead (M2, M4) is what removes it.
- Chrome test (hello, integers, tables, 9 distinct blobs, a fresh headless Chrome for each): 23 to 29 s in
  all over 6 runs, load 22 to 33. With buffers too (12 blobs): 28 s, one run, load 4 to 8.
- Run time against native and against elfconv: not measured. The M0 programs run for far less than a
  millisecond natively, so a comparison means nothing yet; it starts with the benchmark set at M2. elfconv
  is not set up.

## Next

1. Before M2: decide the alignment exit with the orchestrator (see "Shared crate").
2. M2: full look-ahead (section 10 of the plan). The benchmark set (pyro-parked/translator-bench/src/bench.c)
   becomes test programs; hello and the benchmark set must give the same results as at none, in wasmtime
   and Chrome. The Chrome runner already takes any flat blob.

## Known gaps (behave as the plan says: illegal instruction, counted, warned)

- Not translated yet (M3): floating point arithmetic, vector forms outside `VECTOR_FORMS`, CRC32, LSE
  atomics, exclusives, LDAR/STLR family, system instructions (SYS, DC ZVA, IC IVAU, MSR to PSTATE), system
  registers other than NZCV and TPIDR_EL0 (FPCR and FPSR included).
- Layer 1 does not carry FPSR and FPCR yet; nothing translated so far reads or writes them.
- Flat blobs have no program headers, so thread local data is not supported in test programs.
- Only 64 bit guest memory. 32 bit memory hosts need the high-bit check (section 6).
- The WebAssembly stack overflowing on deep guest recursion is not handled.
- Stand-in brk never moves the break.
- A stop flag raised from outside is tested in wasmtime only (`tests/smoke.rs`), and only raised before a
  run, not by a second thread while the guest runs. The Chrome runner has the same hook code.
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
  (Programs with the C library are built for zig's default CPU, as the library is.)
- Vector instructions brought forward from M3: the C library's printf path reaches SIMD loads and stores
  and the 54 vector data processing forms above, so hello cannot run without them. Each is proven by
  layer 1 like the base forms.
- Words of a translated vector form whose arrangement Arm marks reserved decode as undefined.
- Layer 1, on a load or store that faults on both sides: the registers the reference loaded and the
  memory it stored count as UNKNOWN (Arm's rule), and are taken as ours, provided ours changed no memory.
  The reference CPU writes part of a 32 byte pair that crosses onto a faulting page; ours writes none.
- The browser translator is a separate package (`browser/`, own workspace and Cargo.lock) because it
  builds for wasm32. It reuses `wcontainers-translator` unchanged.
- Flake: added `WC_PLAYWRIGHT_CORE` = nixpkgs `playwright-driver`, so Node can drive the installed Chrome
  without npm. Chrome itself is the one installed on the Mac (channel "chrome").
- Stand-in write and writev give -EFAULT for a buffer that is only partly readable. That is what Linux
  does for a pipe within a page; for a pipe write that spans pages, or a regular file, Linux writes the
  readable start and returns a short count. The stand-in's output is a pipe in the reference, and the
  real kernel replaces the stand-in.
- The runners make the thread leave after exit, exit_group (and, in layer 1, after any system call) by
  raising the stop flag themselves; the stop hook then returns 1 without counting. A flag raised from
  outside is counted and the thread carries on.
- A word of the CPU we present that yaxpeax-arm rejects or that we do not translate is `Unsupported`, also
  where Arm's decode pseudocode makes it UNDEFINED (that is not in the forms list): both raise illegal
  instruction, but such a word is counted as "unsupported instruction reached" too.
- Vector registers, FPSR and FPCR at the start of a flat blob: the runners and the reference start v0 to
  v31 at zero, as Linux does at exec. The reference leaves FPSR and FPCR as its own process had them.

## Shared crate (wcontainers-abi)

- Added `Exit::AlignmentFault { address }` (one new variant, additive): a load or store through an SP that
  is not a multiple of 16 (`pc` at the instruction, `address` = SP), or a jump to an address that is not a
  multiple of 4 (`pc` = `address` = the target). The kernel should deliver SIGBUS, BUS_ADRALN, si_addr =
  `address`, as Linux does. The kernel half must add a match arm for it. Needed because the independent
  check found the translator carrying on where Linux faults; without it the CPU side cannot report the
  fault. Later LDAR/STLR and exclusives on misaligned addresses will use it too.
- Seen, not changed: `cargo clippy -p wcontainers-abi --all-targets -- -D warnings` fails on the abi's own
  layout test (`assertions_on_constants`, registers.rs line 79).
