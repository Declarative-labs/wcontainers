# Translator status

Plan: docs/phase-1-translator.md. Branch: phase-1-translator.

## Where things stand

M0 (skeleton) and M1 (hello world at look-ahead none, in wasmtime and Chrome) are built, tested and
pushed. M2 (full look-ahead) is built and tested: every part of the plan's list is in, hello and the
benchmark set match none register by register and byte by byte, layer 2 runs, and size and speed numbers
are below. The second M2 run fixed the two faults the independent check found (a frame and the stack
alignment were relied on after a call that moved sp) and made checks cheaper by reading fixed values from
locals. The third M2 run fixed a hang the next check found (a frame or group check failing on a function's
first instruction looped for ever), added `tests/checks.rs`, and made layer 2 programs use stack frames of
every size, x19 to x28, accesses through sp and groups through other registers, and fault. Speed is still
far from the plan's hopes on call heavy and memory heavy code; see "Next".

S1 (run speed at full look-ahead) is in progress; see "S1" below for what is done, what was measured and
what comes next. The third run's work is under "S1, third run", the fourth's (measurements only) under
"S1, fourth run", the fifth's (the proof over a loop's range) under "S1, fifth run", and what is left under
"S1, what remains after the fifth run".

## S1: run speed at full look-ahead

Tools for it: `cargo run --release -p wcontainers-translator-test --bin machine-code -- bench -O2 <dir>
SCALE=1 WORKLOAD=<n>` writes the code module and wasmtime's compiled form (`objdump -d` it; function 8 is
the first piece, the tool prints which is which). For V8's code: `nix shell nixpkgs#nodejs_latest -c node
--print-wasm-code --no-liftoff --no-wasm-lazy-compilation -e 'new WebAssembly.Module(require("fs").readFileSync("<file>.wasm"))'`
(node 26, V8 14; Chrome's own build has no disassembler). The benchmarks take `WC_BENCH_VARIANTS`,
`WC_BENCH_REPEAT` (Chrome runs the program that many times in the same instances, each timed) and
`WC_CHROME_JS_FLAGS`.

Done so far, each measured (wasmtime unless said; times native, every check / no check; Apple silicon,
load in brackets; the machine is shared, so native times themselves moved by up to 50% between runs):
- Step 1, registers in the thread block. Registers that are not in locals are read and written at their
  fixed place in the thread block (`state.rs`), not in imported globals. The thread module has no
  registers, `save_state` and `load_state` are gone (the block always holds the registers when a hook runs
  or `run` returns), runners read and write the block. Machine code (wasmtime): the block's place plus the
  state memory's base sits in one register for the whole piece and every register is `ldr/str [xN, #off]`,
  no bounds check, nothing reloaded; V8: the same, the base reloaded from the instance once per loop turn
  (one load), no bounds check. Before / after (load 8 to 12): fib 9.4 / 8.2, sieve 3.2 / 2.2, hash 1.33 /
  0.98, sort 5.5 / 5.5, interpret 8.6 / 8.6, matrix 7.7 / 7.1, search 8.5 / 5.5. Chrome (second run in
  the same instance): fib 6.7 / 5.9, sieve 2.7 / 2.1, matrix 6.7 / 5.3, search 6.8 / 4.6.
- Calls, first part. Machine code of fib showed every piece saving all ten callee-saved general registers
  and eight vector registers on entry: in wasmtime a function containing `throw` does that. Pieces now
  call a small function of the code module that throws. Pieces also read the six imported values from the
  module's own immutable globals (one load each instead of two in wasmtime). Together with step 2's
  setting: fib 8.2 to 6.3, sort 5.5 to 4.1, sieve 2.2 to 2.0, search 5.5 to 4.75 (load 6 to 10).
- Step 2, guest memory. The runner's wasmtime engine has `memory_may_move(false)` (the contract now says
  the guest memory's base must never move): the base is loaded once per piece and the bound is a constant,
  so each access is an add, a compare and a select (wasmtime's Spectre guard) before it; before, each access
  loaded base and length from memory. V8 keeps base and size in registers already: one compare and branch
  per access. Measured and not kept: wasmtime with its heap Spectre guard turned off changed sieve, matrix
  and search with no checks by 0 to 6% (load 8 to 11; it is a host security setting, so left on). A 32 bit
  guest memory (addresses wrapped after the permission check, which already faults on anything above the
  table) removes the engine's check entirely: 10 to 27% faster in wasmtime (sieve every check 0.89 to 0.79
  s, matrix 0.66 to 0.43 s, search with no checks 0.24 to 0.17 s, fib 2.5 to 2.2 s; load 8 to 10). Not
  taken: the plan gives a process 64 bit memory where the host has one; a 4 GiB cap is the owner's call.
- Step 3, Chrome's compilers: confirmed. Running each workload several times in the same instances
  (load 8 to 11): matrix with no checks 11.9 times native the first run, then 2.0, 1.96, 1.88; search
  10.7 then 1.90; with every check 19.4 then 6.7 (matrix) and 19.7 then 6.8 (search). Sieve and fib
  were the same every run (their loops' functions are entered many times). With
  `--js-flags=--no-liftoff` the first run is as fast as later ones (matrix 1.85, search 1.63 with no
  checks). Tried and not working: a `metadata.code.compilation_priority` section (the compilation hints
  proposal) asking for every function optimized at once: no change, with no flag or with
  `--wasm-compilation-hints`; `--experimental-wasm-compilation-hints` is not a flag in Chrome 157.
  What an ordinary page can do, built: after the stop hook says carry on at a jump back, the thread goes
  on in a new call, which runs whatever code V8 has for the function by then. The page
  (`browser/run.mjs`) raises the stop flag at 1, 2, 4, ... 1024 ms after the guest starts. Result, every
  check, first run in Chrome (load 13 to 34): matrix 16.7 to 3.7 times native, search 17.0 to 3.8, hash
  2.9 to 1.0; later runs the same as the first. (With the stop checks left out there is nothing to
  re-enter, so "no check" still shows the old first run.) The first run's version entered the function
  itself again and went to the loop through the header of each loop around it (a word in the thread
  block's padding said which loop); in wasmtime those extra ways into loops cost matrix half its time.
  Second run, replaced by loop pieces (`full.rs`, `pieces`): for each loop header of a function, a piece
  that starts there and holds that loop and what follows it up to the header of any other loop, where it
  goes on in that loop's piece. Only a stop check that carries on at a jump back enters one (a direct
  `return_call`), so a loop piece is not in the lookup table and may rely on what its function knows at
  the loop (the frame and the alignment of sp); the function keeps a single way into each loop. Where a
  loop piece would need more entries to stay reducible, none is made and the stop check carries on in
  place, loading registers back. Measured (load 15 to 27, same session): wasmtime matrix every check 0.75
  s to 0.41 s (9.5 to about 5 times native), sieve 0.75 to 0.66 to 0.71, search 0.44 (unchanged); Chrome
  first run matrix 3.4, search 2.1, sieve 1.33 times native (load 25 to 45), the second run the same.
  Size, bench at -O2 SCALE=1: 141,853 to 329,880 bytes of WebAssembly (68 per byte of code), wasmtime
  compile 31 to 35 ms. A first version, whose loop pieces ran on to the end of the function, made
  1,306,873 bytes (one copy of the rest of the function per loop). The thread block's word 804 is no
  longer used. Found by the new test below: an entry check that went on in a new call never made progress
  when the flag was raised at every check, so the entry check carries on in place. Breaking the change
  on purpose (each stop going on in the loop piece of another loop) fails
  `stops_at_every_check_change_nothing`.
- Step 5, the stop check. In wasmtime the stop check now costs about 10% on sieve (1.71 against 1.55 with
  no stop check, load 10 to 23) where it doubled it before; matrix still pays more (0.59 against 0.34 s,
  same run; the second run traced this to the re-entry paths, see step 3). Carrying on no longer reloads
  registers anywhere but after a switch's jump back, at a loop with no loop piece, and after a system
  call: at loops the thread goes on in a loop piece (which loads what the loop needs at its entry), and
  at the entry check, which comes before any register but the passed ones is loaded, nothing is reloaded. This relies on the abi: after `Exit::Stop`, `Resume::Continue` says nothing has
  changed, and the kernel plan's uses of the hook (deliver a signal, switch threads) replace the registers
  (`Resume::FromRegisters`), which is the hook's "leave" answer.
- Step 6, permission checks over a whole loop. An access checked exactly inside a loop that makes no call
  and no system call keeps the page it last found allowed (the fast test: that page and the next allow
  the access); it checks again only when the access is on another page. Accesses of one kind through one
  base register in one loop share one kept page; it is forgotten when the loop is entered from outside,
  on re-entry, and when a stop check carries on in place. Faults stay exact (the full check runs on every
  new page). Measured (wasmtime, load 10 to 19): search 3.9 to 3.4, sieve 1.71 to 1.57, matrix 7.3 to
  6.2; interpret did not move (8.6 to 9.0, noise). The page is also forgotten on entering a loop piece.
  Since the third run a run of up to eight pages is kept instead of one page; see "S1, third run".
- Calls, second part. Before, a piece wrote its flags back at every return (packing NZCV from the lazy
  operands, about 12 instructions). Now the caller sets the low bit of the expected return address
  (`contract::FLAGS_WANTED`) when the flags may be read after the call (the rest of its block is looked
  at: anything but plain arithmetic before the next flag setting counts as reading); the piece writes the
  flags back at a return only then. `BOTTOM_RETURN` is now -2 (even) and the bottom loop passes it with
  the bit. fib, wasmtime, every check: 3.09 to 2.48 s, 6.9 to 6.25 times native (load 11 to 14). Writing
  flags back before a call is still needed: the callee may fault before it sets flags, and the fault must
  see the caller's flags.
- `Omit` and `translate_omitting` are public only with the translator crate's `measure` feature, which only
  the test crate and the browser translator turn on.
- New tests: `tests/programs.rs` `stops_at_every_check_change_nothing` (bench and integers at -O0 and -O2,
  the stop hook raising the flag again before it says carry on, so the thread stops at every check, about
  100,000 to 260,000 stops per program: output equal to Linux, registers and memory equal to none); layer
  2 runs every case a second time at full stopping at every check; `tests/checks.rs`
  `loops_running_onto_a_forbidden_page` (loads and stores with steps 1, 8, 4095, 4096, 4097 bytes running
  off the blob onto a page that allows nothing). Each was checked by breaking the code on purpose: a kept
  page two pages ahead fails the checks test; never asking for the flags after a call fails layer 2 within
  2000 cases (seed 9).

S1 measurements at the end of the first run (commit 413f126, `tests/benchmarks.rs`, best of 3, load 4 to 12;
times native; every check / no stop check / no permission check / no check; Chrome first run in the same
instance, the second run within 2% unless said):
- fib (native 0.40 s): wasmtime 6.38 / 5.52 / 5.69 / 4.96; Chrome 5.63 / 4.92 / 5.17 / 4.73.
- sieve (0.45 s): wasmtime 1.69 / 1.45 / 1.43 / 1.21; Chrome 1.38 / 1.33 / 1.03 / 0.94.
- hash (0.58 s): wasmtime 1.01 / 1.00 / 0.99 / 0.99; Chrome 1.00 / 1.82 then 0.99 / 0.99 / 1.78 then 0.98.
- sort (0.34 s): wasmtime 4.21 / 4.10 / 3.69 / 3.40; Chrome 3.39 / 3.37 / 3.06 / 2.95.
- interpret (0.15 s): wasmtime 8.79 / 8.90 / 2.79 / 2.87; Chrome 8.92 / 8.72 / 2.48 / 2.32.
- matrix (0.08 s): wasmtime 9.26 / 5.97 / 3.67 / 2.57; Chrome 3.29 / 14.5 then 4.06 / 2.17 / 12.7 then 1.91.
- search (0.12 s): wasmtime 3.46 / 4.27 / 2.03 / 2.08; Chrome 3.04 / 13.9 then 2.89 / 1.99 / 11.1 then 1.90.
- Size: bench at -O2: 138,761 bytes of WebAssembly (28.7 per byte of code), 31,094 gzipped (10.8).
- Experiments after it, not kept: (a) the stop checks without the paths into nested loops (every re-entry
  at the start, which is wrong but never taken in wasmtime): matrix 0.39 s against 0.75 s, sieve 0.70
  against 0.75 (load 4 to 9). So in wasmtime the paths into loops cost matrix about half its time. (b)
  re-entering only loops inside no other loop, a stop at a nested loop marking the piece to re-enter at
  the next outer jump back: wasmtime matrix 0.50 s, sieve 0.80, search 0.46; Chrome first run matrix 9.4,
  search 5.5, sieve 1.8 times native (load 11 to 14), much worse in Chrome, so reverted.

S1 measurements at the end of the second run (commit af2566e, `tests/benchmarks.rs` with
`WC_BENCH_VARIANTS=1,5`, best of 3, load 8 to 19; times native, every check / no check; Chrome first run
in the instance). Native times were 15 to 35% slower in this session than in the first run's (fib 0.54
s, sieve 0.54, hash 0.76, sort 0.46, interpret 0.19, matrix 0.10, search 0.12), so ratios may read low:
- fib: wasmtime 5.95 / 4.73; Chrome 4.31 / 3.88.
- sieve: wasmtime 1.44 / 1.19; Chrome 1.37 / 0.97.
- hash: wasmtime 0.99 / 0.97; Chrome 0.96 / 1.69 (no check has no stop to move to V8's optimised code).
- sort: wasmtime 3.92 / 3.64; Chrome 3.50 / 2.77.
- interpret: wasmtime 9.29 / 2.95; Chrome 9.13 / 2.34.
- matrix: wasmtime 4.88 / 2.60; Chrome 2.62 / 11.6 (as hash).
- walk: 0.6 to 0.85 everywhere. search: wasmtime 3.67 / 2.07; Chrome 2.91 / 11.0 (as hash).
- Size: 329,880 bytes of WebAssembly (68.2 per byte of code), 69,102 gzipped (24.1), wasmtime compile 37
  ms.
- Aims met with every check in this run: sieve and hash in both engines; matrix and search in Chrome. Not
  met: fib (aim 4), sort (2.5), interpret (3) in both; matrix and search (3) in wasmtime. For none of
  these does the status file show yet that the rest cannot be removed; see below.

S1, second run, what the machine code and experiments show (wasmtime's code from the `machine-code`
tool; A/B runs alternate the two builds in one session, because native times alone moved by up to 35%
between sessions at load 10 to 45):
- Matrix (wasmtime): with loop pieces the inner loop (7 guest instructions, two loads through x0 and x1
  with a register index) is about 45 instructions a turn. About 30 values are live in it: 19 guest
  registers, and of ours the permission table's place and length, two kept pages, the thread block, the
  guest memory's base, its bound and a zero for wasmtime's Spectre guard, and the operands kept for the
  flags. That is more than the 26 or so registers Cranelift hands out, so it spills: at the jump back 8
  stores and 3 loads to its own stack each turn. Registers the loop loads anew every turn (x6, x7) stay
  alive across the jump back only because the next turn's fault paths write them back. Not yet tried:
  making slow paths not need those registers in registers (e.g. writing such registers to the block as
  they are set inside the loop, or one shared exit per loop).
- Interpret: its stack accesses are `[x9, w10, sxtw #3]` through a copy of sp with a register index, so
  giving x9 the frame's knowledge would not drop them (the index is unknown). Each turn of the dispatch
  makes three kept-page checks (bytecode, jump table byte, stack), each a shift, a compare with a spilled
  kept page and a branch; the untagging of the switch's target and a 75 entry `br_table` are on the path
  too. Why removing the permission check takes it from about 9 to about 3 times native is still not
  explained; it needs an instruction-level profile (`sample` gives none; Instruments' counters or `perf`
  in a Linux VM).
- fib: per call about 150 instructions in wasmtime against 19 natively. In the code: Cranelift's own
  save of ten callee-saved registers, loads of the two imported memories' bases (two loads each) and the
  fixed values, the entry stop check, loading x19, x20, x29, x30 from the block and storing them back
  before every call, the frame check of the new frame (about 10), the alignment check, four guest stack
  stores and loads each with a 64-bit bounds check (add, compare, select), NZCV worked out and stored
  before the call (about 17), and the return check with x30 untagged (4). Measured, A/B, load 10 to 19,
  best of 3, seconds: leaving out the flags written before calls 2.39 / 2.64 (base) against 2.64 / 3.09;
  leaving out the frame check 2.42 / 2.80; comparing x30 before untagging it in the return check 2.73 /
  2.69 (new) against 3.04 / 2.73 (old). None measurable, so none kept. With no check at all fib is 4.7 to
  5.0 times native, so the aim of 4 needs a cheaper call even without checks: the likely cost is the
  chain of stores and loads through the thread block that every call makes for the guest's callee-saved
  registers (caller writes x19 to the block, callee reads it there to save it on the guest stack, and the
  same back). Next to try: pass x19 to x30 as parameters and results (the plan's tuning number); it
  changes the convention of every look-ahead, the dispatcher and both runners.
- Sort: the same costs per call as fib, plus a call through a register (the dispatcher's table lookup)
  to the comparison at every step.
- 64 bit guest memory: a 32 bit memory measured 10 to 27% faster in wasmtime (see step 2) and frees the
  bound and Spectre-guard registers in every loop; the owner's call.

S1, third run (commits 1f7dfee, 86f24d5):
- Interpret's permission cost found and removed. Counting kept-page misses (a counter in the thread
  block's padding, not committed) showed every one of interpret's 1.17 billion checked accesses missing:
  the kept page needed the page after it to allow the access too, and the interpreter's stack sits on the
  top page of the stack and its byte code and jump table on the last page of the blob, each followed by a
  page that allows nothing. So the fast test failed every time and the exact test ran. Matrix had 56
  million misses (its column walk moves to a new page every 5.3 turns), sieve 53 million.
- What is kept now (`code.rs` `keep_pages`, `full.rs` `check`): per kept key two locals, the first
  address of a run of pages that allow the access and the room in it. The check is `address - low <u
  room` (a subtract, a compare and a branch, as before); nothing kept is room 0, which nothing passes.
  After an access passes the full check (fast or exact), the run is that page and the pages after it that
  allow the same kind, up to eight, found with one 8 byte load of the table and a count of trailing zeros;
  room is the run's length less 31 (the longest access is 32 bytes), so an access that starts in it ends in
  it. Faults stay exact: anything outside the run takes the full check. The kept run is forgotten where the
  kept page was. Misses after it: interpret 4,003, matrix 11 million, search 14 thousand.
- Registers that are dirty when a loop is entered and that its own code neither reads nor writes are
  written back to the thread block just before the loop, so the loop's slow paths no longer keep them
  alive (`Plan::written_before`; the backward analysis counts them as needed on the edges that enter the
  loop, so the locals hold their true values there). Matrix 0.643 to 0.621 s, search 0.45 to 0.43 s, others
  within noise (A/B in one session, two rounds each, best of 2, load 8 to 11).
- The check of a kept run reads the table's place and length from the module's globals instead of the
  piece's locals, so loops do not keep them in registers for a check that rarely runs. A/B, two rounds,
  load 8 to 11: matrix 0.597 to 0.555 s, search 0.42 to 0.35 s, interpret and sieve unchanged.
- Tried and not kept: one kept page with the meaning "this page allows it" and a test that the access
  does not run past the page (one local): A/B against the runs, load 7 to 14, two rounds: matrix 0.48 / 0.55
  against 0.62 / 0.53 s, search 0.42 / 0.42 against 0.41 / 0.35, interpret 0.76 / 0.76 against 0.67 / 0.59,
  sieve 0.82 / 0.77 against 0.83 / 0.66. Writing the slow paths' pc from the load address global instead
  of the local: matrix 0.547 to 0.594 s, interpret 0.57 to 0.61 (load 6 to 10), so reverted.
- Checked by breaking on purpose: a run one page too long fails `tests/checks.rs`
  `loops_running_onto_a_forbidden_page`; leaving out the stores before loops (analysis unchanged) fails
  layer 2 within 300 cases.
- Benchmarks after 1f7dfee (`WC_BENCH_VARIANTS=1,5`, best of 3, load 6 to 18; times native, every check /
  no check; Chrome first run): fib wasmtime 6.67 / 5.12, Chrome 5.63 / 4.67; sieve 1.58 / 1.20, Chrome 1.39
  / 0.99; hash 0.96 / 0.93, Chrome 0.91 / 1.17; sort 4.11 / 3.33, Chrome 3.31 / 2.88; interpret 3.82 / 2.90,
  Chrome 3.87 / 2.04; matrix 7.74 / 2.65, Chrome 4.87 / 11.3; walk 0.8 to 1.0; search 2.61 / 1.55, Chrome
  2.12 / 7.26. Size: 340,588 bytes of WebAssembly (70.4 per byte of code), 70,878 gzipped.
- After 86f24d5, every check only (best of 3, load 24 to 50, so noisier): sieve wasmtime 1.61, Chrome 1.43;
  hash 1.00 / 1.00; interpret 4.12 / 4.25; matrix 7.26 / 3.99; search 3.00 / 3.06.
- Against the second run: interpret 9.3 to about 3.8 to 4.1 in both engines; search in wasmtime 3.7 to
  2.6 to 3.0; matrix worse, wasmtime 4.9 to about 7 and Chrome 2.6 to about 4 to 5 (native matrix 0.076 to
  0.080 s in both sessions). Matrix's inner loop spills (see the second run's notes); two locals per kept
  key instead of one add to it. The one-local variant above did not measure better for matrix within
  the noise, and is worse for interpret and search.
- Calls (fib, sort), why passing more registers cannot help in wasmtime: a test module
  (`pyro-parked/wcontainers-scratch/translator/s3/cc/cc.wat`, a function of 14 i64 parameters and 14
  results, compiled by wasmtime 49) shows Cranelift passes only the first four parameters in registers (x4
  to x7, after the two instance pointers in x2 and x3) and the rest on the stack, and returns six results in
  registers and the rest through memory. Our pieces already pass six (x0 to x3, sp, the expected return), so
  sp and the expected return already go through the stack on every call; x19 to x30 as parameters would
  be stores and loads to the stack instead of to the thread block, the same work. fib's remaining cost
  per call in wasmtime (machine code of `fib`, function 9): the engine's stack limit check (5
  instructions), its save and restore of ten callee-saved registers (12 instructions; they hold the
  instance, the thread block, the state memory's base, the load address, the permission table's place and
  length, the guest memory's base and the expected return, all live across the guest's call), the stack
  arguments, the four guest stack stores and loads each with a 64 bit bounds check (add, compare, select:
  24 instructions; a 32 bit guest memory would remove them, the owner's call), and x19, x20, x29, x30
  through the thread block (about 15 loads and stores). With every check off fib is 5.1 times native in
  wasmtime and 4.7 in Chrome in this run, so the aim of 4 is out of reach without a cheaper call
  convention in the engine or a 32 bit guest memory. Tried and not kept: in pieces that call, reading
  every fixed value from the module's globals at each use instead of copying them into locals. fib 2.42 /
  2.41 s against 2.41 / 2.46, sort 1.44 / 1.48 against 1.50 / 1.47 (A/B, load 70 to 160, so only a rough
  answer); the machine code shows why: the piece still saves all ten callee-saved registers, because
  Cranelift merges the reads of an immutable global into one value that lives across the calls anyway.

S1, fourth run (no code change kept; measurements and two experiments):
- Where each check's cost sits now, wasmtime, commit 5cac63e, best of 3, load 12 to 17 (every check / no
  stop check / no permission check / no return check / no check, times native): matrix 7.39 / 5.14 / 2.72 /
  7.62 / 2.66; interpret 3.78 / 3.69 / 2.88 / 4.09 / 2.80; fib 6.25 / 6.33 / 6.35 / 6.80 / 6.01; sort 4.05 /
  3.74 / 3.31 / 3.56 / 2.95; sieve 1.51 / 1.45 / 1.25 / 1.58 / 1.24 (native sieve 0.442 s). So in matrix and
  interpret almost all of the checks' cost is the permission check; in fib the checks cost about 4% and the
  call itself is the cost.
- Matrix in numbers: n = 96, 300 rounds, 265 million turns of the 6 instruction inner loop; native 0.29 ns a
  turn, no check 0.77 ns, every check 2.1 ns. Kept-run misses (11 million, one per 24 turns) account for
  about 2 instructions a turn; the rest is the two kept-run checks per turn and the spills they cause (the
  inner loop is about 56 instructions in wasmtime, 12 of them stores to its own stack and 10 loads, see the
  second run's notes). The way to the aim is the proof over the loop's range (below): with it the loop is
  the no-permission loop, 2.7 in wasmtime.
- Tried and not kept: (a) registers the header of a loop sets before reading them (x6, x7 in matrix) written
  back at every jump back and before the loop, so they are no longer alive across the jump back. The
  machine code shows the two stores and x6, x7 gone from the spills, but the loop still spills the same
  amount (other values took the registers). A/B, two rounds alternating, load 5 to 19, seconds: matrix 0.665
  / 0.554 against 0.609 / 0.552, sieve 0.884 / 0.672 against 0.756 / 0.676, search 0.444 / 0.350 against
  0.354 / 0.348, interpret 0.816 / 0.600 against 0.601 / 0.565. No gain. (b) The kept runs in mutable globals
  of the module instead of locals (so they are not loop values): matrix 0.529 s (unchanged), sieve 1.92,
  search 3.54, interpret 4.40 times native (all worse, load 6). Patch of (a):
  `pyro-parked/wcontainers-scratch/translator/s4/writeback.patch`. (c) The flags left out of the write back
  before calls (wrong, only to measure): fib every check / no check, seconds, wasmtime 2.75 / 2.14 against
  2.40 / 2.00 with them, Chrome 2.21 / 1.96 against 2.15 / 1.88; sort wasmtime 1.52 / 1.15 against 1.42 /
  1.15, Chrome 1.11 / 0.99 against 1.16 / 0.98 (one round each, load 17 to 26). No gain in either engine,
  so the flags are not where fib's time goes.
- fib against the toy, measured again today in wasmtime 50 CLI (toy modules from translator-bench/out,
  compiled at opt-level 2, process start included, best of 3, native 0.365 s, load 7.5 to 12): toy
  `args4+check` (return check only, 32 bit memory, registers in module globals, flags never carried across
  calls, no permission check) 1.25 s, 3.4 times native; `args4+check+flag@loops+callers` (adds the stop check
  at loops and at entries of callers) 1.48 s, 4.05; `locals+check` 1.53 s against `locals+check+mem64` 1.60
  s (the same with a 64 bit memory: 4% slower). So in wasmtime even the toy's call shape is at 4 once it has
  the stop check the plan requires, before a 64 bit memory, permission checks or flags kept across calls.
  Ours: 6.0 with no check, 6.25 with every check (this session). The toy figures in Chrome (2.55 to 2.72 times
  native, `out/poll-results.txt`, load 16) were not measured again.

S1, fifth run (commits dc7af11, 98f61b7, c7b40ab, 68cff43):
- The proof over a loop's range (`full/proof.rs`), the fourth run's plan for matrix. A loop that makes no
  call and no system call and holds no switch is looked at over its own blocks (not those of loops inside
  it). A register written there at one instruction only, by adding a constant or a register the loop does
  not write (add/sub, or pre/post indexing), moves by a fixed step at most once a turn. An exit at a block
  every turn passes, a compare (subs/cmp with an immediate or a register the loop does not write) or
  cbz/cbnz on such a register, bounds the turns: "not equal" with step 1 or -1 (modulo the width), or an
  ordered condition (lo, ls, hi, hs, lt, le, gt, ge) stepping toward the bound. Each access checked exactly or
  as a group whose base and index are such registers (one of them moving, or neither) covers a range of
  addresses over the turns. At the loop's entry the translated code works out the turns and every range,
  and looks up every page of each range in the permission table (eight table bytes at a time). When all
  allow the access it runs a copy of the loop with no check for those accesses; otherwise the loop runs with
  its checks, so faults stay exact. Anything that could wrap (a 32 bit index or counter passing its limit,
  steps above 2^24, more than 2^32 turns, addresses outside the table) makes the proof fail, never pass.
- Where the copy lives matters for wasmtime. With both copies in one function, matrix stayed at 4.5 to 5.4
  times native: Cranelift's register allocator moved values that live through the loop (x0 to x3, sp, the
  expected return, kept runs of outer loops) between registers and its stack at every turn (7 to 10 loads
  and stores a turn, seen in the machine code), even though the copy itself is 15 instructions. Trapping in
  the checked copy (only to measure) gave 2.88. So the checked copy is now only in the loop's loop piece: the
  function runs the proof and either the copy without checks or a tail call into the loop piece, which holds
  both copies. A stop at a jump back now writes back and goes straight on in the loop piece, whose entry
  check calls the stop hook (loop pieces always check the flag at entry), so the loop has no call that
  returns into it. Registers a loop neither reads nor writes are no longer kept in locals through it: they
  are loaded from the thread block on each edge leaving it (the dirty ones are already written back before
  it). In this case Cranelift forwarded the stored value past the loop anyway, so that part did not change
  matrix, but it is kept as the cleaner rule (it did not slow anything measured).
- Loops holding other loops are proven too (sieve's outer loop): only accesses and register changes in the
  loop's own blocks count; the copy is used only when every loop inside has a loop piece, so a stop
  anywhere inside carries on where a proof is made again.
- Measured, wasmtime, best of 3 (times native, every check): matrix 7.4 (fourth run) to 3.00 and 3.09
  (load 5 to 8; 2.63 with no check), sieve 1.69 to 1.42 with outer loops proven (load 8), search 2.9 to 1.75
  to 2.0, hash 1.0. Chrome (first run in the instance, load 5 to 9): matrix 2.11, search 1.73, sieve 1.32,
  hash 1.00.
- Full benchmark run at 98f61b7 (before outer loops were proven), best of 3, load 4 to 12, every check / no
  check: fib wasmtime 6.02 / 5.19, Chrome 5.16 / 4.71; sieve 1.69 / 1.25, Chrome 1.32 / 0.99; hash 0.98 /
  0.97, Chrome 1.00 / 1.59 (no check has no stop to move to V8's optimised code); sort 4.12 / 3.34, Chrome
  3.31 / 2.80; interpret 3.81 / 2.92, Chrome 4.18 / 2.19; matrix 3.00 / 2.63, Chrome 2.11 / 11.3; walk 0.92
  to 0.97; search 1.75 / 1.81, Chrome 1.73 / 9.2. Size: 348,702 bytes of WebAssembly (72.1 per byte of
  code), 74,679 gzipped, wasmtime compile 28 ms.
- Seen and not explained: search with only the permission check left out is 5.8 times native in wasmtime,
  also at the fourth run's commit 50908cb (so not from this run), while with no check at all it is 1.8. Only
  a measuring variant; its machine code for the inner loop is tight, so the time is elsewhere.
- Tests: layer 2 has a new chunk, a walk: a counted loop (every counter form above, 32 and 64 bit, the
  compare before or after the change) over memory through x18, moving its base by indexing or an add, or an
  index read as uxtw, sxtw or lsl, shifted or not, stepping by an immediate or a register, starting near
  2^32 or 2^31 so some wrap, in the area or walking forward onto the page after the blob. `programs/walks.c`
  (nested counted loops, one running onto the page after the blob) matches Linux at every level, in
  wasmtime and Chrome, and full matches none. Checked by breaking on purpose: a proof that always passes
  fails layer 2 and `walks.c`; a range one turn short fails layer 2 within 15 cases (seeds 41, 42).

- After c7b40ab (outer loops proven), every check, best of 3, load 7.5 to 10.6, wasmtime / Chrome first run:
  sieve 1.44 / 1.15, matrix 2.94 / 2.02, search 1.70 / 1.88, hash 0.97 / 0.97, interpret 3.74 / 3.89. Size
  350,392 bytes of WebAssembly (72.5 per byte of code).

S1, what remains after the fifth run (aims with every check, in both engines):
- Met in the last measurements (after c7b40ab, above): sieve, hash, matrix and search in both engines.
  Matrix is at its aim in wasmtime (2.94, 3.00, 3.09 in three sessions), so noise can put it just over.
- Interpret (aim 3): wasmtime 3.81, Chrome 4.18; no check 2.92 / 2.19. Its dispatch loop holds a switch
  (whose stop check carries on in place) and its accesses are data dependent (byte code through w12, the
  switch's byte table through the loaded opcode, the interpreter's stack through w10), so the proof does not
  reach them; three kept-run checks a dispatch remain. Not yet shown irremovable. Ideas: the switch's byte
  table load has a known bound (the jump table's own compare), so its range is known; the switch's jump
  back could hand over to a loop piece like other jump backs, which would let the dispatch loop be proven.
  Neither reaches the aim alone: with the permission check left out entirely interpret is 2.92 in
  wasmtime, so nearly every check would have to go, and two of the three depend on data in memory (the byte
  code and the interpreter's stack depth), which no proof at the loop's entry can bound.
- fib (aim 4): wasmtime 6.0, Chrome 5.2 with every check; 5.2 / 4.7 with none. The checks cost little; the
  call itself is the cost (see the fourth run: in wasmtime even the toy's call shape is 4.05 with the stop
  check). In Chrome the toy is 2.7, so the gap there is ours and not yet explained.
- Sort (aim 2.5): wasmtime 4.12, Chrome 3.31; with no check 3.34 / 2.80. Same call costs as fib.
- 32 bit guest memory: measured 10 to 27% faster in wasmtime; the owner's call.

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
  the thread block (since S1). Returns check x30 against the expected address, else leave through "go to
  address" (an exception the bottom loop catches). Since S1 the expected address's low bit
  (`contract::FLAGS_WANTED`) says whether the caller reads the flags after the call.
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

- M2, full look-ahead (`wcontainers-translator/src/full.rs`, `LookAhead::Full`):
  - Finding functions: the starts, targets of `bl`, addresses built by `adr` or by `adrp` with the `add`
    right after it (except those a jump table of the same function goes to), and extra entries that keep
    every function's control flow reducible (a block entered by a jump back from outside its loop becomes
    an entry of its own, tried again until none is left). A function is everything reachable from its
    entry without passing another entry; reaching another entry is a tail call. The same instruction may be
    in more than one piece.
  - One piece per function. Blocks are structured after Ramsey's "Beyond Relooper" (dominators from
    `petgraph`): loops for loop heads, blocks for merge points, everything else placed inline.
  - Registers (x4 to x30, v0 to v31) are locals; x0 to x3 and sp stay parameters and results. A forward
    analysis says which locals may be newer than their globals ("dirty"); a backward one says which must
    hold their true value, counting every register that may be dirty at a block's start as needed there, so
    no write back ever stores a stale local. Locals are loaded at the entry when needed, written back
    before calls, system calls, slow paths and leaving the piece, and loaded again after a call when
    needed.
  - Flags are kept as the last comparison: a flag setting the meaning makes once (`Effects::flags`) is kept
    as its operands in locals shared by every setter of the same shape, and a condition on it is one
    WebAssembly comparison (`condition_direct` in the arm64 crate: a subtraction gives eq, ne, hs, lo, hi,
    ls, ge, lt, gt, le straight from its operands; a logical operation gives the sign conditions). Where
    the flags are is known per point (flag locals, the globals, or one shape); where paths with different
    shapes meet, a local says which, and readers choose by it. After a call the flags are in the globals,
    so nothing is loaded.
  - Direct calls are WebAssembly calls; calls through a register and calls to code with no piece go through
    the dispatcher (the lookup). Jumps that leave the piece are tail calls, direct or through the lookup.
    Returns keep the check of M0.
  - Jump tables (`wcontainers-arm64/src/habits.rs`): a `br` whose target is a base built by `adr` plus a
    shifted entry loaded from a table built by `adr`/`adrp`, with the bound from the nearest flag setting
    subtraction of an immediate and `b.hi`/`b.hs` before it (looking up to 64 instructions back). The
    targets are read from the blob and become blocks of the function; the jump is a `br_table` on the
    address actually computed at run time, so a wrong guess or a changed table only takes the lookup. An
    unrecognised `br` leaves through the lookup and is counted ("jump through a register not
    recognised"), as is a recognised one going somewhere not in its table.
  - Stop check at every jump back and at the entry of every piece that calls or leaves to another piece. It
    reads the flag with a plain load (see "Departures"); when the hook says carry on, registers are loaded
    back from the globals just written rather than kept across the call. The check after a system call
    stays (it is how the kernel makes a thread leave).
  - Permission check with the drops the plan allows. The fast test is one clamped byte load and an AND
    (this page and the next both allow it); only when that fails is the exact test made (does the access
    really reach the next page, and where it faults). An address the table allows has a clear top byte, so
    untagging (Top Byte Ignore) moved into the check's slow path. Drops: (1) the stack frame: when the
    stack pointer moves down by a known amount, the pages of the new part are checked once for reading and
    writing, and accesses through sp inside the known frame have no check, across calls too; a forward
    analysis carries the frame between blocks. (2) A group: accesses through one base register in one
    block, with the base unchanged or moved by a known amount by its own pre/post indexing, and touching at
    most 4096 bytes in all, are checked once at the first of them. If a frame or group check fails, the
    thread leaves to the instruction with the unwind payload `UNWIND_GO_TO_EXACT`: the bottom loop counts
    "left to an address" and asks the load code hook for code at that address even though the table may
    hold a piece there (the failing one, when the instruction is a function's entry). The runner translates
    at none from there, unless it already did (it keeps the set of addresses it translated from), so the
    instruction runs from code that checks each access exactly, and faults land at the same instruction
    with the same registers as at none. Nothing proved before a call or system call is relied on after it,
    except the frame.
  - The stack alignment check is left out where the stack pointer is known to be a multiple of 16 (checked
    by an earlier access and moved only by multiples of 16 since); a new `Emitter::stack_aligned` asks.
  - A call after which a frame or the alignment is known compares the stack pointer the callee gives back
    with the one it was given. If they differ, the thread leaves to the instruction after the call ("left
    to an address") and carries on at none, which knows nothing about sp. Calls with nothing known after
    them have no compare.
  - The immutable imported values a piece uses (load address, thread block, permission table place and
    length) are copied into locals at its entry, so the engine can keep them in registers instead of
    reloading them through the import on every check (wasmtime loads an imported global through a
    pointer).
  - `translate_omitting` with `Omit { stop, permissions, returns }` leaves checks out, only to measure them
    (since S1 public only with the crate's `measure` feature, which only the test tools turn on).
- `wcontainers-arm64` additions: `Effects` and `Registers` (what an instruction reads and writes, found by
  running its meaning against a recording emitter, so there is no second copy of instruction logic),
  `Instruction::memory_access`, `habits::jump_table`, `condition_direct`, `untag` (now public; the check
  calls it), `Flags::Add::subtrahend` (the value subtracted, for direct comparisons).
- Tests for M2:
  - `tests/programs.rs`: integers, tables, buffers, hello and `programs/bench.c` at every distinct
    optimisation level, at none and full: output and status equal to Linux, and at full the final
    registers and every reachable byte of memory equal to none.
  - `programs/bench.c`: the toy's four workloads (fib, sieve, hash, sort through a callback) and four of our
    own (a byte code interpreter on a switch, a matrix product, a linked list walk, a byte search).
    `WORKLOAD` picks one, `SCALE=1` gives the toy's sizes for timing.
  - `tests/browser.rs`: the same programs (and bench) in Chrome at none and full, output equal to Linux.
  - `tests/layer2.rs` with `src/layer2.rs`: random programs assembled with `dynasm` (dynasmrt 5.1.0): loops
    with counters, calls with bounded depth, bounded recursion, tail calls, calls and jumps through
    registers, compiler-style jump tables, flags carried across instructions (cmp, ccmp, csel, mrs/msr
    nzcv), loads and stores in a 512 byte area. Each ends by writing its area, x0 to x15, x19 to x28 and
    NZCV. Full against none: registers, output and all reachable memory; the first few also against the
    real CPU. Breaking one direct comparison on purpose was caught within 218 cases and shrunk. Since the
    third M2 run: every function call (recursive ones too) makes a frame with sp, from 528 bytes to 1.5 MiB
    (some run off the 1 MiB stack, some reach below it but are used only at the top), at the instruction a
    recursive `bl` enters, so frame checks fail on entries; accesses through sp in the frame; groups of up
    to four accesses (every size, pairs, offset, pre- and post-indexing) through any register, from the
    area or from just before the end of the blob, which is followed by a page that allows nothing (the blob
    ends with a page and more of zeros so these reach no code). About 12% of programs fault (SIGSEGV),
    about 11% leave to an address at full. With the old leave-to (before this run's fix) a 500 case run
    hung (killed after 240 s); with the fix it passes in 14 s.
  - `tests/smoke.rs`: a flag raised by another host thread while a translated loop runs is seen inside the
    loop, at none and full.
  - `tests/programs.rs` also runs hand-written assembly programs (`programs/*.S`; a program may be C or
    assembly): callees that give sp back moved down the stack, moved onto a page that allows nothing, and
    no longer a multiple of 16. Each matches Linux (exit status 77, SIGSEGV, SIGBUS) at none and full, and
    full matches none in registers and memory. With the compare after the call taken out, the second gives
    77 at full where Linux faults, so the test catches it. A program killed by a fault now reports 128 plus
    the signal's number, as Docker does for the native run, instead of panicking.
  - `tests/checks.rs`: frame and group checks failing, on a function's first instruction too: recursion
    with 4 KiB frames made by the entry running off the stack (SIGSEGV), a function whose first two loads
    are a group reaching a forbidden page, a frame reaching below the stack used only in its mapped part
    (exit 0) and then once below it, groups crossing onto a forbidden page, an alloca loop running off the
    stack. Linux, none and full agree, and full matches none in registers and memory; the entry cases also
    run at full in Chrome. Before the fix the three entry cases hung at full.
  - `tests/benchmarks.rs` (ignored, one command, see "How to run"): native and wasmtime and Chrome timings
    with and without each check, and the translation's size. Native runs are timed by the reference itself
    (`wcontainers-reference time`), from making a child process to its end, so the reference's start-up is
    not counted (it was before, by busybox `time` in 10 ms steps). Matrix, the shortest: 0.093 s native
    either way, so the earlier ratios stand.

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
- Layer 2: `WC_LAYER2_CASES=1000000 WC_LAYER2_SEED=<n> WC_LAYER2_NATIVE=<k> nix develop -c cargo test --release -p wcontainers-translator-test --test layer2 -- --nocapture`
  (defaults 64 cases, seed 1, 8 of them on the CPU; each CPU case starts a container, about 0.3 s).
- Benchmarks: `WC_BENCH_RUNS=3 nix develop -c cargo test --release -p wcontainers-translator-test --test benchmarks -- --ignored --nocapture 2>&1 | grep -v "^wcontainers warning"`
  (`WC_BENCH_WORKLOADS=1,2` picks workloads, `WC_BENCH_CHROME=0` leaves Chrome out). About 15 minutes.

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
- Layer 2 (M2): 64 cases, seed 1, 8 on the CPU: 0 mismatches. 2000 cases, seed 7, 40 on the CPU, release:
  0 mismatches (22 s at load 3 to 4). Second M2 run, after this run's changes: 3000 cases, seed 11, 20 on
  the CPU, release: 0 mismatches (35 s at load 10 to 16). Pass bar (one million) not run.
- Layer 2, S1 (every case also run at full stopping at every check), release: 500 cases seed 3, 2000 seed
  5 (after kept pages), 2000 seed 9 (after the flags bit): 0 mismatches. Not yet the pass bar.
- Layer 2, second S1 run, after loop pieces: 2000 cases, seed 13, 10 on the CPU, release: 0 mismatches,
  308 faulted, 220 left to an address at full (37 s at load 22 to 57).
- Layer 2, third S1 run, after kept runs and the write back before loops: 2000 cases, seed 17, 10 on the
  CPU, release: 0 mismatches, 315 faulted, 211 left to an address at full (32 s at load 7 to 8).
- Layer 2, third M2 run, with frames, sp accesses, groups and faults: 500 cases, seed 1, 60 on the CPU,
  release: 0 mismatches, 61 faulted, 40 left to an address. 20,000 cases, seed 21, 40 on the CPU,
  release: 0 mismatches, 2949 faulted (SIGSEGV, same at none, full and on the CPU where run), 2131 left
  to an address at full; 265 s at load 17 to 19. Found on the way (a generator bug, not ours): a store
  with write back whose transfer register is its base is CONSTRAINED UNPREDICTABLE, and the reference
  CPU raises SIGILL for it; the generator now keeps such registers apart, as layer 1 does.

- Layer 2, fifth S1 run (with walks), release: 1000 cases seed 41, 40 on the CPU: 0 mismatches, 325
  faulted; 1000 seed 43 (after outer loops): 0 mismatches, 379 faulted; 500 seed 31 (before walks): 0
  mismatches; 5000 seed 47, 20 on the CPU, after outer loops: 0 mismatches, 1774 faulted, 380 left to an
  address at full (77 s, load 8 to 10). Not yet the pass bar.

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
- Run time against elfconv: not measured; elfconv is not set up.
- M2, full look-ahead, `tests/benchmarks.rs`, release build, best of 2, Apple silicon Mac, wasmtime 50
  (crate) and Chrome 157 headless; native is the reference runner in Docker timed by busybox `time`
  (10 ms steps). Load 3.4 to 8 during the run. Times are seconds, then times native, in the order: every
  check / no stop check / no permission check / no return check / no check.
  - fib(42), native 0.38: wasmtime 4.38 (11.5) / 3.10 (8.2) / 4.31 (11.3) / 4.15 (10.9) / 2.76 (7.3);
    Chrome 3.39 (8.9) / 2.85 (7.5) / 3.29 (8.7) / 3.28 (8.6) / 2.59 (6.8).
  - sieve, native 0.43: wasmtime 1.54 (3.6) / 0.73 (1.7) / 0.76 (1.8) / 1.59 (3.7) / 0.53 (1.2);
    Chrome 1.43 (3.3) / 0.73 (1.7) / 0.85 (2.0) / 1.26 (2.9) / 0.43 (1.0).
  - hash, native 0.55: wasmtime 1.04 (1.9) / 0.57 (1.0) / 0.58 (1.05) / 1.03 (1.9) / 0.55 (1.0);
    Chrome 1.79 (3.3) / 1.08 (2.0) / 1.67 (3.0) / 1.78 (3.2) / 0.98 (1.8).
  - sort, native 0.34: wasmtime 2.18 (6.4) / 2.20 (6.5) / 2.11 (6.2) / 2.22 (6.5) / 1.62 (4.8);
    Chrome 1.76 (5.2) / 1.66 (4.9) / 1.63 (4.8) / 1.73 (5.1) / 1.57 (4.6).
  - interpret, native 0.14: wasmtime 1.43 (10.2) / 2.06 (14.7, an outlier against the others) / 0.65 (4.6)
    / 1.43 (10.2) / 0.41 (2.9); Chrome 1.31 (9.3) / 1.17 (8.3) / 0.42 (3.0) / 1.30 (9.3) / 0.34 (2.4).
  - matrix, native 0.07: wasmtime 0.66 (9.4) / 0.38 (5.5) / 0.35 (5.0) / 0.68 (9.7) / 0.20 (2.9);
    Chrome 1.59 (22.7) / 0.99 (14.1) / 1.39 (19.8) / 1.58 (22.5) / 0.95 (13.5).
  - walk (memory latency), native 2.19: about 0.9 to 1.1 everywhere.
  - search, native 0.11: wasmtime 1.42 (12.9) / 0.48 (4.4) / 0.62 (5.6) / 1.35 (12.3) / 0.24 (2.2);
    Chrome 2.53 (23.0) / 1.65 (15.0) / 2.27 (20.6) / 2.52 (22.9) / 1.48 (13.5).
  - After keeping fewer flag operands (last commit of this run), wasmtime, load 5.7 to 7.8: sieve 3.8 /
    1.6 / 1.6 / 3.6 / 1.3, hash 2.1 / 1.1 / 1.1 / 1.9 / 1.0, search 9.3 / 3.8 / 5.2 / 9.3 / 2.0 times
    native. Within noise except search.
  - Against the toy's figures in the plan: the toy had loops at about 1.3 times native with registers in
    locals and no checks; we have 1.0 to 1.3 on sieve and hash with no check, so the shape holds. The toy's
    checks cost 5 to 15% (stop, tightest loop) and 0 to 24% (permissions, wasmtime) each; ours each alone
    cost 0 to 40% on sieve and hash, but together they double the loops in both engines. Call heavy code
    (fib, sort) is 5 to 7 times native even with no check; the toy's call figures were relative (about 20%
    better with 4 passed registers), not against native. Matrix and search in Chrome are 13 times native
    with no check, which wasmtime is not (2 to 3): unexplained.
  - Size, bench at -O2 SCALE=1 (all workloads), measured at commit 1852235 (before the last commit of the
    first M2 run): 4836 bytes of code (2867 gzipped), translated at full in 2.4 ms (2.0 MB/s), 88,244 bytes
    of WebAssembly (18.2 per byte of code), 16,400 gzipped (5.7 per gzipped byte), wasmtime compile 30 ms,
    load 7. The toy made about 5 bytes per byte of ARM64. Keeping fewer flag operands (b58212a) grew it to
    110,768 bytes (22.9 per byte), 26,896 gzipped (9.4); found by the independent check, not explained yet.
- Second M2 run, after the compare of sp after calls and the fixed values in locals (commit 9f4124e),
  `tests/benchmarks.rs`, best of 2, same order as above (every check / no stop / no permission / no
  return / no check), times native. Load 6.3 to 8.6 up to sort, then 10 to 25 (other work on the Mac), so
  the later workloads are noisier.
  - Size: 111,975 bytes of WebAssembly (23.2 per byte of code), 27,571 gzipped (9.6), translated in 2.6 ms,
    wasmtime compile 31 ms, load 6.6.
  - fib: wasmtime 12.2 / 8.4 / 10.1 / 10.2 / 8.2; Chrome 10.1 / 8.8 / 9.6 / 9.6 / 7.7. A run of the same
    code at load 4 to 7 gave 10.2 with every check in wasmtime, so fib did not change beyond noise.
  - sieve: wasmtime 3.5 / 1.6 / 1.4 / 3.4 / 1.3; Chrome 2.9 / 1.5 / 1.9 / 2.7 / 1.1. (3.2 with every
    check in a wasmtime-only run at load 7.)
  - hash: wasmtime 1.33 / 1.00 / 0.99 / 1.48 / 0.95; Chrome 2.9 / 1.9 / 2.7 / 2.9 / 1.8. Every check in
    wasmtime went from 1.9 to 2.0 times native down to 1.33 to 1.35 (three runs) with the fixed values in
    locals.
  - sort: wasmtime 7.0 / 7.4 / 6.5 / 8.2 / 5.1; Chrome 5.5 / 5.3 / 5.1 / 5.3 / 4.7.
  - interpret (load 16 to 25): wasmtime 9.5 / 9.9 / 4.6 / 10.8 / 3.3; Chrome 10.1 / 9.4 / 2.9 / 10.1 / 2.5.
  - matrix (load 13 to 21): wasmtime 9.8 / 6.5 / 5.5 / 8.6 / 2.9; Chrome 21.5 / 13.3 / 18.9 / 21.2 / 11.9.
    A wasmtime-only run at load 7 gave 7.7 with every check (10.2 before the fixed values in locals).
  - walk: 0.8 to 0.95 everywhere (memory latency; the native run is slower here, unexplained).
  - search (load 8 to 11): wasmtime 9.1 / 3.2 / 6.1 / 9.6 / 2.2; Chrome 20.4 / 14.4 / 17.7 / 19.8 / 10.5.

- Third M2 run, wasmtime, best of 3, load 14 to 17: sieve native 0.449 s, every check 3.23 times
  native, no stop check 1.49; matrix native 0.079 s, every check 8.06, no stop check 5.20. An experiment
  not kept, same conditions: with the registers not loaded back after the stop hook says carry on (kept in
  locals instead), every check gave sieve 2.90 and matrix 7.34 times native, about 11% faster. Not kept
  because the plan says the code loads its registers back after the hook; the gain says the stop check's
  cost is mostly elsewhere (the write back of every dirty register on its slow path, through one
  imported global pointer per register; see "Next").

## Next

1. Finish S1 (see "S1, what remains"): its aims are not met yet.
2. Measure the second lookup design (M5 says settle it; nothing built yet).
3. M3: every instruction.

## Known gaps (behave as the plan says: illegal instruction, counted, warned)

- Not translated yet (M3): floating point arithmetic, vector forms outside `VECTOR_FORMS`, CRC32, LSE
  atomics, exclusives, LDAR/STLR family, system instructions (SYS, DC ZVA, IC IVAU, MSR to PSTATE), system
  registers other than NZCV and TPIDR_EL0 (FPCR and FPSR included).
- Layer 1 does not carry FPSR and FPCR yet; nothing translated so far reads or writes them.
- Flat blobs have no program headers, so thread local data is not supported in test programs.
- Only 64 bit guest memory. 32 bit memory hosts need the high-bit check (section 6).
- The WebAssembly stack overflowing on deep guest recursion is not handled, at either look-ahead: every
  guest call is a WebAssembly call at none too. The engine stops the guest ("call stack exhausted"), the
  registers are lost and the runner panics; nothing is counted or warned. The independent check found it
  with recursion 50,000 deep in 16 byte frames (800 KB of a 1 MiB stack), which Linux runs. The plan's way
  out is the "stack is lost" path (M4): count call depth and, past a limit, leave to the bottom loop and
  carry on from the guest's own stack. Not built; its cost on calls is unmeasured.
- Not yet in the across-layer tests: pieces of different look-aheads mixed at random, injected rare events
  (stack lost, missing piece, flag raised at a random check), and faults with the table changed at random
  points (M4 per the plan's milestones). Layer 2 programs fault only on the page after the blob and below
  the stack, with a table fixed for the run.
- A test whose guest never ends hangs rather than fails (nothing bounds how long a translated run may
  take); the hang this run fixed showed as layer 2 and `tests/checks.rs` running for ever.
- The full piece of a function keeps every local it may need; very large functions could pass V8's 50,000
  locals or function size limits. Not checked yet (M5).
- Finding functions explores every entry again when entries are added; fine for the test programs, not
  measured on large binaries (M5).
- Stand-in brk never moves the break.
- A stop flag raised from outside is tested in wasmtime only (`tests/smoke.rs`, before a run and by a
  second thread mid-loop). The Chrome runner has the same hook code.
- Slow paths taken inside translated code (left to an address) and hook calls that do no work are counted
  with zero time; only the runner's own work is timed.
- Layer 1 checks only words the CPU we present has. Whether undefined words really raise SIGILL on a CPU
  without the feature is checked against Arm's list, not against a CPU (the reference CPU has newer
  features than the one we present).

## Departures from the plan

- Two kinds of module per thread: a thread module (lookup table, unwind tag, bottom loop) and code modules
  (pieces only, importing the thread module's table). The plan has every module export run and save/load;
  here only the thread module exports run, and since S1 there is no save or load state at all (see S1), so
  code loaded later and split modules are all the same kind.
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
- Full look-ahead finds functions only from the code (starts, `bl` targets, built addresses). It does not
  take a jump that leaves its function as a new entry unless something else makes it one; such code is
  copied into the jumping piece instead (correct, larger).
- Stop flag read with a plain `i32.load`, not an atomic load. An atomic load cost the same here, but a
  plain one is what the plan measured; an engine may in principle keep a plain load out of a loop, so
  `tests/smoke.rs` checks that a flag raised by another thread mid-loop is seen, at none and full, in
  wasmtime. Not yet checked mid-loop in Chrome.
- After a system call, and after the stop hook says carry on at a switch's jump back, registers are loaded
  back from the thread block instead of being kept across the call (it keeps registers out of
  callee-saved registers). Elsewhere since S1 a piece does not reload after carrying on: see S1.
- A frame or group permission check that fails leaves to the instruction and carries on at none from
  there ("left to an address"), instead of working out the exact fault in the piece. Exact either way.
  For this the contract has a third unwind payload, `UNWIND_GO_TO_EXACT`, and the load code hook may be
  asked for code at an address that has a piece (it puts one translated at none there, unless the one
  there already is). After that the table holds none code at that address, so calls through the table run
  it at none; direct calls still reach the full piece, whose check fails and leaves again each time
  (counted and warned each time; the runner does not translate again).
- The permission check of none changed shape too (fast test, then exact test), so both look-aheads share
  it; layer 1 passed after the change.
- Without the permission check (`Omit::permissions`, only for measuring) addresses are not untagged either,
  because untagging is part of the check's slow path now.
- The table byte after the last page must read as "nothing allowed"; the runners size the state memory for
  it. Pages past the table are clamped to that byte.
- A recognised jump table that reaches a target not in its table counts as "jump through a register not
  recognised" (it goes through the lookup like one).
- Every executed jump through a register that is not recognised calls the warn hook (counted per
  execution, as every slow path is). Tail calls through function pointers (common in the C library) pay a
  host call each time.
- Layer 2 compares against the real CPU only for the first `WC_LAYER2_NATIVE` cases, because the
  reference runs one blob per container; full against none is compared for every case.
- Layer 2 writes `mrs`/`msr nzcv` as raw words, because dynasm has no name for the register.
- Code translated at run time is at look-ahead none, and its pieces replace whatever the table held for
  every address it reaches (including full pieces). Correct; after a "go to address" later calls through
  the table may reach none pieces. So one "left to an address" (a group or frame check that fails, a
  tagged pointer in a group, a callee that moves sp, a jump through a table to a target not in it) runs
  the rest of that function, and what it reaches, at none. The independent check measured this (one
  unrecognised jump taken twice: 3 translations at run time). Left as is in M2: bounded look-ahead (M4)
  is the plan's answer, and its pieces will not need to replace full ones.
- Vector registers, FPSR and FPCR at the start of a flat blob: the runners and the reference start v0 to
  v31 at zero, as Linux does at exec. The reference leaves FPSR and FPCR as its own process had them.

- S1, approved by the orchestrator: registers that are not in locals live in the thread block in the state
  memory, at their places in `RegisterBlock`, not in the instance's globals (the plan: "registers live in
  the instance's globals"). There is no save or load state: the block always holds the registers when a
  hook runs or `run` returns, and a runner sets registers by writing it.
- S1: after the stop hook says carry on at a jump back, the thread goes on in a new call of a loop piece
  of that loop (or of the piece itself when the loop starts at its entry) instead of going on in place
  (the plan: "loads its registers back and continues"); at the entry check it goes on with nothing
  reloaded. Both rest on `Resume::Continue` meaning nothing changed. Loop pieces are pieces the lookup
  table does not hold (the plan has every piece in the table): only stop checks of their own function
  enter them, so they may rely on what that function knows at the loop. They are not listed in the
  description section either.
- S1: the browser page raises the stop flag at 1, 2, 4, ... 1024 ms after the guest starts, so V8's
  optimised code is used in long loops of functions entered once. A real browser host needs the same (or
  any other early raises); wasmtime needs none.
- S1: the plan's "whole function reasoning" for permission checks is a kept run of up to eight allowed
  pages per base register and kind in loops without calls (see step 6 and the third run), and, for counted
  loops whose accesses move by a fixed step, a proof over the loop's range at its entry that picks a copy
  of the loop without those checks (fifth run).
- S1: a proven loop's copy with its checks is only in its loop piece; the function tail calls into the loop
  piece when the proof fails. Loop pieces check the stop flag at their entry, and a stop check at a jump back
  to a loop with a loop piece writes back and tail calls it without calling the stop hook itself.
- S1: registers a loop neither reads nor writes are kept in the thread block through it and loaded on the
  edges leaving it (not for loops holding a switch).
- S1: registers dirty at a loop's entry that the loop does not touch are written back before it (the plan
  writes registers back only around calls, system calls, slow paths and the piece's edges).
- S1: the low bit of a piece's expected return address says whether the caller reads the flags after the
  call (`FLAGS_WANTED`); `BOTTOM_RETURN` changed from -1 to -2.
- S1: pieces throw through a small function of their module (wasmtime makes a function that throws save
  every callee-saved register), and read the imported values from the module's own globals.
- S1: the contract requires a guest memory whose base never moves (the runner's wasmtime engine has
  `memory_may_move(false)`).

## Shared crate (wcontainers-abi)

- Added `Exit::AlignmentFault { address }` (one new variant, additive): a load or store through an SP that
  is not a multiple of 16 (`pc` at the instruction, `address` = SP), or a jump to an address that is not a
  multiple of 4 (`pc` = `address` = the target). The kernel should deliver SIGBUS, BUS_ADRALN, si_addr =
  `address`, as Linux does. The kernel half must add a match arm for it. Needed because the independent
  check found the translator carrying on where Linux faults; without it the CPU side cannot report the
  fault. Later LDAR/STLR and exclusives on misaligned addresses will use it too.
- Seen, not changed: `cargo clippy -p wcontainers-abi --all-targets -- -D warnings` fails on the abi's own
  layout test (`assertions_on_constants`, registers.rs line 79).
- S1, first run: the CPU side kept one word for itself in the thread block's padding (offset 804). Since
  the second run's loop pieces it does not: the CPU side uses no byte of the block outside the abi's
  layout. No change to the shared crate in S1.
