# Native back end status

Plan: docs/native.md. Branch: phase-2-native. Brief: pyro-parked/wcontainers-scratch/orchestrator/brief-native.md.

## Where things stand

- N0 (the plan) is done. At the start of N1 (2026-11-18) the plan was corrected for the last check's open
  findings and the brief's newest rulings (commit 64e30e56; how, below).
- N1 (the floor on Hypervisor.framework) is done: every item of its bar reports once and passes (21 cases
  since N2 run 1 added `SharedInsideViews`, and the doorbell's round trip as a measurement, 22 named items,
  0 failed), measurements below. Commits
  a1f1e82b (A1), 91fb63ff (the inside), 0c8ef62d (the monitor and the test), the two after them, run 2's
  ce3028f5 and run 3's 77eb6af6 (the independent checks' findings settled, below), each with the plan
  corrected after it.
- The literal check is merged (its second version of the rules, phase-1-literals at 7119e3f3, in 21321c7e)
  and this branch's baseline started at e4b998bd. The check fails, on main's code only: see "Literals".
- main is merged (787f8ceb, N2 run 1): the owner's names are in the plan's text and its runtime citations.
- N2 (the native ARM64 output) is in progress, run 1 done: see "N2" below for what is built, what is
  measured and exactly what comes next. N3 needs decision 1 (R1, R3). N6 needs K2 (below).
- The owner's: decisions 1, 2 (names chosen so far are used), 7, 12, 14 (routing is built until then),
  and 10's final choice, which N1's trial narrows: option C does not work on Hypervisor.framework (below),
  so it is A or B.

## What changed in the plan at the start of N1 (the last check's findings)

- The count is E only (plan 3.1, 4, 8, 9, 10): defined per instruction and its own address; the add's place
  is free within a straight-line run and adds may merge, corrected by the position map; N2 measures E's
  cost so placed and the speed bar uses it. New test: the count at every stop is identical across
  look-aheads and across the two outputs (T2, N4, N6).
- Replay across outputs needs K2: placement low first and the memory size given at start (plan 5.7, 6.2,
  7.2, N6, N11). N6 and N11 now name K2.
- Minor: CNTPCT_EL0 is the Trap route's; the exclusive route's 32-bit pair store (`cas x` with a stand-in),
  the load pair into one register (read again from t) and untagging (fault address from the guest
  instruction); the x86-64 call, return and jump sequences are left to N9, with what any of them must
  meet; G1 names this track's changes in other tracks' crates for N2, and N4 names T1; decision 14's default
  is routing (every load and store that can cross a page takes the Split test, x86-64's reciprocals and
  undefined-flag forms go to the inside), counted "host-model result routed"; every bar includes the
  literal check once it lands.
- The owner's names: `Executor`, `System`, `LocalSystem` (`GuestMachine`, `TranslatorSettings` and
  `TaskId` are not used in the plan's text yet). Section 10 lists which decisions the orchestrator settled.
- The thread design agreed with the owner (plan 5.3): one virtual CPU per running thread, a table root per
  thread, the kernel entered by one caller at a time, the four settings, and pages held elsewhere through
  `page_request`, `page_give`, `page_take`, `page_copy`.

## N2: the native ARM64 output (run 1, 2026-11-18)

Done (each its own commit, in other tracks' crates as plan 6.2 G1 allows):
- 4cf56a71, wcontainers-guest-cpu: `native.rs`, the `Native` trait (`route`, `hook_class`,
  `control_class`), `Route` (the routes of plan 3.4), `NativeClass` (held by the hardware, bookkeeping, or
  routed to a named route) and `counted(address, flow)`, definition E, with a unit test.
- 02a62e99 and 577c71f6, wcontainers-arm64: `native.rs`, the route of every decoded instruction, and
  tests/native_routes.rs, the static routing test. Every form of Arm's table that the CPU we present has,
  words drawn per form (every value of the non-register bits for forms with at most 16 of them, so every
  system register of a move is seen), each run through its meaning with a recording emitter (hooks with the
  float operation read by `wasmparser`, controls read and written, system call, traps, transfers, memory
  checks, own address). It fails a route that does not fit: a copy that touches what a route keeps, a
  system call not sent to the inside, a trap or transfer not routed as one, an alignment trap with no test
  (except a stack-pointer base, whose check SA0 makes), a memory access on a route that makes none, a
  meaning that does nothing not routed as nothing, and the reverse. Passes: 64 words per form by default,
  1,050,063 decoded words of 1215 forms (seed 2685821657736338717); 1000 per form with seed
  0x9e3779b97f4a7c15 (before the sweep was added), 1,209,733 words.
- a589fa06, wcontainers-arm64: `encode.rs`, the small encoder (movz/movk and a constant's moves, ubfx,
  ubfiz, bfxil, and/eor immediate with the logical-immediate encoding, add/sub immediate, cbz/cbnz, tbnz,
  b/bl, br/blr/ret, ldr/str/ldrb with an unsigned offset, staddb, cas, brk/udf). Every word is its form's
  fixed bits from Arm's table (`forms.rs`) with named fields filled in, so no opcode or bit position is
  written. tests/encode.rs reads each back with the crate's decoder: 10,000 seeded cases (seed
  6364136223846793005), 20,000 logical immediates, all as meant.
- E's cost by the add's placement (below), recorded in plan section 4.
- The existing tests of wcontainers-guest-cpu and wcontainers-arm64 pass unchanged; nothing in the
  translator crate or the WebAssembly output was touched.

Not done yet, in order (the next run starts here):
1. The native emitter in wcontainers-translator behind a cargo feature (`native`), generic over
   `G: Guest + Native`, at look-ahead none: each route's host sequence (plan 3.4); `access` as the one place
   a guest access is emitted (the page test and probes for Split, the alignment test); the stand-in rule for
   the three withheld registers (x26 to x28 until N4 chooses); the count's add at the start of the event's
   straight-line run (the placement chosen below); calls into the inside as `brk #code`, the position map
   saying which instruction and route; and the position map itself: per run of host code the guest
   instruction, the side of the commit point, the count register's lead, the relocated values, the fix-up.
2. A runner on the floor: a program in the inside package (as floor-cases is) that maps guest pages, loads
   seeded registers and memory, runs translated code at EL0, answers its calls into the inside (Kernel,
   Float status and Trap routes as the meanings do, plan 4's fix-ups), and reports registers, flags,
   FPSR/FPCR, memory and the stop. A fault's address comes from the guest instruction, untagged as `untag`
   does (the third check's tagged-store finding).
3. The differential test against the WebAssembly output under wasmtime, at least 1000 seeded cases per
   form, with the cases the bar lists (plan 9, N2).
4. A forced interrupt at every host offset (EL1 software step, plan 4), the behaviour probe and its time.

## E's cost by placement (N2 run 1)

Method: the eight bench.c workloads and calls.c of the cost rounds, compiled by clang -O2 with x26 to x28
withheld (round 3's flags), the count's adds put in by `place.py` (scratch native/n2-count/src; one rule per
placement), each run at EL0 inside Hypervisor.framework with 4 KiB stage 1 pages by round 3's runner
(hvfrun, copied unchanged); every placement against the same build with no add, 15 alternating pairs per
workload, warm-up discarded, cycles and instructions of the whole process (`proc_pid_rusage`), outputs
checked equal. M5 Max, macOS 27.2, 2026-11-18 16:44 to 16:57, load 37 to 73. One command: `build.sh`, then
`run.sh 15`, in scratch native/n2-count; `python3 -I summarize.py results/raw.jsonl 1 2000` (bootstrap seed
1, 2000 resamples); raw pairs in results/raw.jsonl, the summary in results/summary.txt.
- Under E every event is a transfer and ends its straight-line run, so a run never holds more than one event
  and there is nothing to merge; the freedom is where in the run the add sits.
- Right before the event (round 3's En): geometric mean of median cycle ratios +13.65% [+12.23 .. +14.97]
  (round 3 measured +13 to +14%). fib +6.0, sieve +3.5, hash +0.3, sort +5.9, interpret +31.7, matrix
  +11.5, walk +1.6, search +60.1; calls +17.7.
- Before the compare a b.cond reads (else right before the event): +10.95% [+9.58 .. +12.96]. fib +2.0,
  sieve +0.6, hash -0.1, sort +4.5, interpret +24.0, matrix +6.6, walk +4.2, search +55.3; calls +17.3.
- At the start of the run (after its last label or branch): +10.51% [+9.61 .. +12.31]. fib +2.2, sieve
  -0.6, hash -0.0, sort +5.8, interpret +23.0, matrix +9.3, walk +1.8, search +51.6; calls +17.8. Chosen.
- Instructions rose 14 to 27% on the eight (37% on calls), alike for every placement.
- What is left: search's inner loop (9 instructions) and interpret's dispatch carry two counted events per
  pass (two backward conditional branches; a jump through a register and the back edge), so two adds on the
  count register's chain of dependent adds bound the loop. That is expected from the code, not measured
  apart.

## N1: what was built

- crates/wcontainers-inside (`no_std`; names are suggestions): `layout` (the guest-physical and virtual
  layout, every number one constant derived from `PAGE_SIZE` where it can be), `link` (the start record,
  the link page, and requests as typed messages: one `#[repr(C)]` struct per request with its answer
  type, written over the page's message words through `zerocopy`, so both sides read one definition; the
  `requests!` macro makes a set, and the floor's own set is print, fatal and give back),
  `build_environment` (the cargo variable names and directives both build scripts use), `boot` (the boot
  map and boot registers the monitor applies; the granule chosen from `PAGE_SIZE` in one place, SA0 as the
  monitor passes it from `Arm64`'s `Guest::SP_ALIGNMENT_CHECKED`), and, built
  only for `aarch64-unknown-none`, `floor`: entry and vectors (`floor/entry.rs`), frames and translation
  tables with page and block entries, split break before make (counted "large entry split" and warned the
  first time), TLB invalidation (`floor/tables.rs`: `tlbi vale1is` for a page's change, `tlbi vae1is` for
  a split),
  address spaces with per-process TTBR1 tops sharing the global entries, a table root and identifier per
  view (per thread), the GIC through `arm-gic`, the virtual timer, code made visible (CTR_EL0's lines),
  the stop word in TPIDRRO_EL0, the doorbell. `src/bin/floor-cases` is N1's test program: hand-written EL0
  snippets, one report per case, each naming its `Case`; `protocol.rs` there is the cases' own request set
  (after the floor's) and the case list, which the host test includes by path. Nothing of the monitor
  builds or carries it.
- crates/wcontainers-hypervisor: the `Hypervisor`, `VirtualMachine` (the host's hardware virtual machine;
  `Machine` until run 3, renamed away from the runtime's `GuestMachine`), `VirtualCpu` and `Stopper` traits
  (plan 5.9),
  their Hypervisor.framework implementation over `applevisor` (`src/hvf.rs`, the only file that names the
  platform), guest memory as one reservation, give-back as `Machine::give_back` with one body per platform
  (Hypervisor.framework's: unmap, fresh mmap, map), the ELF loader and boot. Its build script only tells
  the linker about the GIC calls (Departures).
- tests/floor.rs (harness off): builds the cases program for the bare-metal target with a nested cargo
  into cargo's test scratch folder (target/tmp/wcontainers-inside), boots it, serves the link, makes the
  stops from another thread (300 by default, seeded), and checks that every `Case` reported exactly once
  and passed.
- The counter trial (decision 10, option C) ran in run 1 and its result is below; its code (a machine with
  EL2, the store doorbell, the PMU registers) had no use once C was out and was removed in run 2. The trial
  as run is at commit fe3a4bff: check that commit out into a scratch worktree and run the bar there.

## How to run

From the checkout, in the Nix shell:
- N2 so far: `nix develop -c cargo test -j 4 -p wcontainers-arm64 --test native_routes --test encode --
  --nocapture` (knobs `WCONTAINERS_ROUTES_SEED`, `WCONTAINERS_ROUTES_WORDS`, `WCONTAINERS_ENCODE_SEED`,
  `WCONTAINERS_ENCODE_CASES`, decimal) and `nix develop -c cargo test -j 4 -p wcontainers-guest-cpu`;
- N1's bar: `nix develop -c cargo test -j 4 -p wcontainers-hypervisor --test floor` (about half a second
  once the cases program is built, which the test does first; the test signs a copy of itself ad hoc with crates/wcontainers-hypervisor/hypervisor.entitlements and runs
  that, since Hypervisor.framework refuses an unsigned binary); the stop test's knobs are environment
  variables: `WCONTAINERS_FLOOR_STOPS=1000 WCONTAINERS_FLOOR_SEED=0x0123456789abcdef` before the command
  (seed in decimal or after `0x`; the run prints both); the whole suite of both crates is
  `nix develop -c cargo test -j 4 -p wcontainers-inside -p wcontainers-hypervisor`;
- lint: `nix develop -c cargo clippy -j 4 -p wcontainers-inside -p wcontainers-hypervisor --all-targets --
  -D warnings` and `nix develop -c cargo clippy -j 4 -p wcontainers-inside --target aarch64-unknown-none
  --bins --lib -- -D warnings`;
- the literal check: `nix develop -c cargo run -q -p wcontainers-literals -- check` (exit 0 is clean), and
  `nix develop -c cargo test -j 4 -p wcontainers-literals -- --test-threads 3` before a push.

## N1 measurements (M5 Max, macOS 27.2, the floor test; load 55 to 142 over the runs)

- Run 3 (after the third check's findings, 2026-11-18 16:10 to 16:17, load 76 to 104): four runs, each 20
  cases and 1 measurement of the 21 named items, 0 failed; the third from a clean build (`cargo clean -p`
  of both crates and the cases' scratch build folder removed, then the whole suite: 53 s, 14.8 s of it the
  test's nested build of the cases). Doorbell 885, 886, 889 and 903 ns. Stops, 300 with seed 0x9e3779b97f4a7c15
  (runs 1 to 3): medians 2.29, 2.21 and 2.33 µs, 90th 6.2, 3.7 and 3.0 µs, 99th 14.0 ms, 0.19 ms and 8.1 µs,
  maxima 31.8, 1.46 and 0.18 ms; run 4, 1000 stops with seed 0x0123456789abcdef: median 2.21 µs, 90th 2.9
  µs, 99th 4.8 µs, max 2.8 ms. Every stop in the spin loop, its count never back. Raised interrupt medians
  3.67, 3.38, 3.42 and 2.29 µs;
  timer median 41 ns in each; give-back 67,141,680 bytes of 67,108,864 each time. The two adjacent 4 KiB
  pages in one 16 KiB host page: the store to the read-only one faults into the inside as a level 3
  permission fault (EC 0x24, FSC 0xf) at its own address, the writable one takes the store. Logs: scratch
  native/n1-run3-suite.log and n1-run3-clean.log.

- Run 2 (after the check's findings were settled, 2026-11-18 15:33 to 15:42, load 110 to 142): four runs,
  each 20 cases with 0 failed (the cases now include an inside-half page made read-only after use, and the
  block case checks the split counter); the last from a clean build of both crates (`cargo clean -p` of
  both, then the whole suite, 22 s). Doorbell 926, 886, 884 and 919 ns. Stops (300, seed 0x9e3779b97f4a7c15,
  unless said): medians 2.38, 2.38 and 2.13 µs, 99th 0.33, 2.6 and 0.34 ms, maxima 1.9, 7.5 and 0.89 ms;
  1000 stops with seed 0x0123456789abcdef: median 2.21 µs, 99th 0.26 ms, max 3.5 ms. Every stop in the loop,
  the count never back. Raised interrupt median 2.75 µs, timer median 41 ns, give-back 67,141,680 bytes
  of 67,108,864 (the clean run in full: scratch native/n1-run2-clean.log).
- Run 1, bar cases, each run: 20 of 20 pass (one of them the doorbell, a measurement that always passed;
  run 2 reports it as a measurement) (six runs after the last fix, the last from a clean build of both
  crates: `cargo clean -p` of both, then the whole suite, 27 s, load 41 to 63; doorbell 892 ns, stop median
  2.25 µs there).
- Doorbell round trip (EL1 `hvc` to the monitor and back, 1000 timed by the guest's counter): 897, 912 and
  906 ns (three runs, load 72 to 86).
- Virtual timer interrupt, deadline to the inside's handler (24 per run): median 41 ns in each of three
  runs (the counter's resolution is 41.7 ns); maxima 291 ns, 13.6 µs, 2.19 ms (load 72 to 86).
- An interrupt raised by `hv_gic_set_spi` from another host thread to the inside's handler, by the host's
  clock against the guest's counter less the virtual timer offset (24 per run): medians 2.42, 4.13 and
  3.25 µs; maxima 3.9, 46.5 and 14.4 µs.
- Stopping the virtual CPU from another host thread with `hv_vcpus_exit` (300 stops per run, seeded waits
  of 200 to 2000 µs, seed 0x9e3779b97f4a7c15), from the decision to `hv_vcpu_run` returning on the virtual
  CPU's thread: medians 2.33, 2.21 and 2.46 µs; 90th percentiles 3.8, 3.5 and 3.7 µs; 99th 0.28, 0.02 and
  0.32 ms; maxima 2.4, 0.75 and 1.9 ms (load 55 to 86; the tail is the host's scheduling under load). In
  every run the pc was inside the spin loop at every stop and its count never went back.
- Give-back: 64 MiB touched then given back lowered the footprint from 73.9 MB to 6.8 MB (67,141,680 bytes,
  all of it); the range read zeros after.
- Late code: 24 of 24 rounds ran the code the monitor wrote next to code that had run, after the maintenance.
- Option C (decision 10), a machine with EL2, PMUVer set to 1 by the host (ID_AA64DFR0_EL1 0x10305106
  reads back): the virtual CPU enters at EL1; PMCR_EL0 reads 0x41, N (event counters) 0; PMEVCNTR0_EL0
  written 0xfff0bdc0 reads 0; the cycle counter counts and passes its 64-bit overflow with no overflow flag
  and no interrupt (PMINTENSET_EL1 0x80000000 as written); only the 1 s timeout timer came. In at least
  three runs. So C does not work here.

## The independent check of N1, and how each finding was settled (run 2)

- Blocking, the stop test had no knob and STOPS was defined twice: the inside no longer knows the count; the
  host's test owns it (one constant, the default) with `WCONTAINERS_FLOOR_STOPS` and
  `WCONTAINERS_FLOOR_SEED` as knobs; run with 1000 stops and the check's seed (above).
- The status's stale fact about phase-1-literals: the branch is merged now and the baseline started.
- Give-back decided above the interface: it is `Machine::give_back`, one body per platform; `unmap` left
  the trait.
- The option C trial's setting and second doorbell path: removed (the `el2` flag, `el2_supported`, the
  registers only the trial set, `Start.el2`, `Start.counter_interrupt`, the store doorbell and
  `layout::DOORBELL`); the plan says KVM ARM64's store doorbell and its page come with N10.
- The TLBI operand's address mask: 44 bits now (`TLBI_ADDRESS_BITS`), leaving the level hint 0. A case for
  it was added (a page of the inside's half made read-only after use refuses at once); it passed before
  the fix too on this M5, as the check found.
- The split's counter and 512 invalidations: counted "large entry split" and warned at the first, and one
  invalidation at the block's address replaces the 512 (one cached entry covers the whole block); the
  block case checks the count is 1.
- Link arguments by position: typed messages (above).
- Transitive licences: listed below.
- The doorbell case always passed: it is reported as a measurement, not counted among the cases.
- Decision 10's evidence: the plan cites the check's hpmn.c result (MDCR_EL2.HPMN).

## The third independent check of N1, and how each finding was settled (run 3)

- Blocking, facts owned by other crates defined in the inside: no copy is defined any more. The page shift
  (wcontainers-abi's `PAGE_SHIFT` to come) is written `PAGE_SIZE.trailing_zeros()` where used; the general
  register count is the length of abi's `RegisterBlock::x` (`RegisterBlock::ZERO.x.len()`); an
  instruction's bytes (wcontainers-arm64's `INSTRUCTION_BYTES` to come) is written `size_of::<u32>()` in
  the three places of the cases that use it; nanoseconds per second (abi's `NANOSECONDS_PER_SECOND` to
  come) is not needed: the cases divide a `core::time::Duration` by the counter's frequency. When the
  owners' constants land, these places take them (Literals, below).
- The granule and SA0: the granule comes from `PAGE_SIZE` in one place (`boot::GRANULE`), which knows the
  4 KiB granule only (aarch64-paging's) and stops the build for any other size; SA0 is set as the monitor
  passes `Arm64::SP_ALIGNMENT_CHECKED` (Departures).
- No use today: `max_cpus` and `Start.cpus` removed.
- Test requests in the shipped link, and the monitor building the test program: the link's own set is the
  floor's (print, fatal, give back); the cases' requests moved to `src/bin/floor-cases/protocol.rs` as a
  second set after the floor's; the monitor's build script builds nothing and `programs` is gone; the test
  builds the cases program itself.
- `tlbi vae1is` for every page change: page changes use `tlbi vale1is`; `vae1is` stays for the split, where
  a block entry becomes a table (the plan says both now).
- Stale departure on decision 4: corrected (Departures).
- `Machine` beside the runtime's `GuestMachine`: renamed `VirtualMachine`; decision 2 now lists the
  interface's names for the owner.
- The bar's count not checked: every report and measurement names its `Case`; the test asserts that the
  sorted list of reported cases equals `Case::ALL`, each exactly once.

## The third independent check of N1's remaining findings, settled at the start of N2 (run 1)

- main not merged: merged (787f8ceb). The literal check then fails on main's code; see "Literals".
- The views of one process share the inside's half, but `protect` invalidated for one identifier: every
  change now invalidates with `tlbi vaae1is` (every identifier, every level) through `aarch64-paging`'s
  `El1And0::invalidate_va`, one primitive for a page change and a split; `protect` takes no identifier. A
  case of the bar, `SharedInsideViews`, takes the check's scenario (two views of one process, both store,
  the page made read-only, the other view's store must be refused; 24 rounds) and passes. Cost: other
  processes' entries at that address are dropped too (plan 5.2 says so; N4 measures it if copy-on-write
  shows it).
- The baseline line `tlbi vale1is, {0}` added through an adoption window: gone with the code (the crate's
  call replaces both assembly strings), and so is `tlbi vae1is, {0}`.
- Unused build-dependencies of the monitor (wcontainers-arm64, wcontainers-guest-cpu): removed.
- Plan 5.5 and decision 10's stale "untried": corrected with N1's results.
- boot.rs's comment claimed Linux's settings: it now says what the value is for (the presented guest level,
  `wfi`/`wfe` and CTR_EL0 trapping on purpose).
- Literal points: translation levels are `TOP_LEVEL` (new, layout.rs) and `TOP_LEVEL + 1`; `LINK_BYTES` is the
  page less `size_of::<LinkHead>()`, the link's head now a struct of its own.
- A faulting tagged store's FAR keeps the tag: N2's fault reports take the address from the guest
  instruction and untag it as the WebAssembly output's `untag` does (plan 3.4), compared by the differential
  test.
- Frames used up panicked: `Frames::take` and `take_block` return `Err(Full)`, counted "guest-physical space
  full" (`floor::space_full()`) and warned at the first; every table call that takes a frame passes it on.

## Asked of the kernel track (K2, needed before N6)

1. The memory size the guest is told (MemTotal and the rest of /proc/meminfo, `sysinfo`, tmpfs limits)
   comes from the container's start, as runtime.md section 8 asks ("the runtime gives it in Start"), not
   from `cx.cpu.address_space().end` (kernel/container.rs:420; read at fs/proc.rs:1374-1380 and halved at
   container.rs:803).
2. Placement low first, in tiers, never depending on the ceiling (runtime.md section 8, lines 416-425), in
   place of top-down from 128 MiB below `address_space().end` (kernel/memory.rs:254-260 and :407-430, the end
   from loader.rs:503 and task.rs:726).
With both, a process gets the same addresses and is told the same memory size under the WebAssembly back
end (ceiling just under 4 GiB) and the native one (2^45), which replay across the outputs needs.

## Licences of what N1 added (crates.io, 2026-11-18)

applevisor 1.0.0 and applevisor-sys 1.0.0 MIT OR Apache-2.0; aarch64-paging 0.12.3 MIT OR Apache-2.0;
aarch64-cpu 12.0.0 and tock-registers 0.12.0 MIT/Apache-2.0; arm-gic 0.11.0, arm-sysregs 0.5.2 and
safe-mmio 0.4.0 MIT OR Apache-2.0; spin 0.10.1 MIT; object 0.37.3 Apache-2.0 OR MIT; libc MIT OR
Apache-2.0; fastrand 2.5.0 Apache-2.0 OR MIT (test only). Brought in by those (new in Cargo.lock): num_enum
and num_enum_derive 0.7.6 BSD-3-Clause OR MIT OR Apache-2.0; pastey 0.2.3, rustversion 1.0.23 and
arm-targets 0.4.3 MIT OR Apache-2.0; arm-sysregs-aarch32, -common, -el0 and -el1 0.5.2 MIT OR Apache-2.0.
N2 run 1: wasmparser 0.263.0 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT) and fastrand 2.5.0
(Apache-2.0 OR MIT), dev-dependencies of wcontainers-arm64 for its routing and encoder tests, both already in
Cargo.lock; the floor now calls aarch64-paging's `El1And0::invalidate_va` (already a dependency).
Run 2 added zerocopy to the inside, already the workspace's (0.8.63, BSD-2-Clause OR Apache-2.0 OR MIT).
Run 3: the monitor depends on wcontainers-arm64 and wcontainers-guest-cpu (for `Guest::SP_ALIGNMENT_CHECKED`),
which bring the workspace's translator dependencies into it, all already in Cargo.lock: rustc_apfloat 0.2.3
Apache-2.0 WITH LLVM-exception, wasm-encoder 0.263.0 Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT,
yaxpeax-arch 0.3.3 and yaxpeax-arm 0.5.0 0BSD. zerocopy is a test-only dependency of the monitor (the cases'
messages).

## Literals

- The check is merged and the baseline's start for this branch is e4b998bd. N2 run 1 merged main
  (787f8ceb) and then the literals branch's second version of the rules (21321c7e), and ran `init` in that
  merge, as adoption step 4 allows for new rules. Under version 2 a new version may add lines only for
  literals already in the tree at the baseline's previous change; main's literals were not (main has no
  baseline, and its renames make every literal of a renamed item new to the check). So the 54 lines `init`
  added for main's code were taken out again (7175b04e), and the check fails with 53 `+` findings, every one
  in another track's files: wcontainers-kernel-test/src/driver.rs 8, wcontainers-runtime container.rs 2,
  image.rs 9, spec.rs 7, system.rs 3, wcontainers-wasmtime bin/wcon.rs 12, lib.rs 7, starts.rs 1,
  tests/surface.rs 4. None is in a crate this track owns. For the orchestrator: these pass once main carries
  the check with the runtime track's own baseline, or at the next version of the rules (they are in this
  tree now); this track does not name another track's literals and does not add their lines.
- The changes that could add lines, as the check lists them: 0e0f1102 the start, e4b998bd the init after
  the baseline arrived by a merge, 21321c7e new rules, 68214b2e the start (literals branch), 89f88281 and
  7119e3f3 new rules (literals branch).
- The native crates' lines are all text, none a number: assembly the crates offer no call for (`hvc #0`
  in the doorbell, `dc cvau` and `ic ivau`, the entry and vectors in floor/entry.rs, and the floor cases'
  EL0 snippets with their labels and `extern "C"`). The two `tlbi` strings left in N2 run 1.
- Facts another crate owns, written where used until the owner's constant is on a branch merged here
  (orchestrator/magic-literals/README.md): `PAGE_SIZE.trailing_zeros()` in layout.rs and tables.rs
  (wcontainers-abi's `PAGE_SHIFT`), `size_of::<u32>()` for an instruction's bytes in floor-cases/cases.rs
  (wcontainers-arm64's `INSTRUCTION_BYTES`); the register count is derived from abi's `RegisterBlock`.
- Replaced in run 2: every barrier now calls `aarch64_cpu::asm::barrier`; the build scripts' cargo
  variables, options and file names are named constants, the variables shared in `build_environment`; the
  cases' names went into their format strings; `[1, 2, 3]` is `1..INSTRUCTION_BYTES`; the `1` given to
  `hv_vcpus_exit` is the length of the identifier list; the stop count is defined once, in the test.
- Marked: none. The register save and restore offsets in floor/entry.rs are inside its assembly text.
- The magic-literals fix lists (orchestrator/magic-literals) name nothing in the native crates.

## What N0 checked, and how

Read: the brief to its end; docs/runtime.md sections 1 to 13; the code cited in the plan (every cited file
and line opened at 68013b31; runs 2 and 3 opened their new citations); the x86 plan at origin/phase-2-x86
96c49dbb (sections 2 run 4 and run 10, 3.2, 5.1, 5.6 and the baseline list); distributed-everything
design.md section 6 and the requirement names of orchestrator/de-requirements-map.json; costs-round-2.json,
progress-count-cost.json and the round 3 results (mac-tests.jsonl, mac-summary-main.json, summary-main-r0.txt).

Experiments (under pyro-parked/wcontainers-scratch/native/experiments):
- Kernel crate on bare metal (run 1). On the Linux laptop (~/native-plan/wc), Rust 1.99.0: `cargo build
  --locked -j 3 --release -p wcontainers-kernel --target aarch64-unknown-none` and the same for
  `x86_64-unknown-none` both succeed with no change (kvm-laptop.out).
- System registers at EL0 on this Mac (run 1; M5 Max, macOS 27.2, load 76 to 87): CNTFRQ_EL0 1 GHz,
  DCZID_EL0 4, CTR_EL0 traps (SIGILL); FPCR bits 0x3fbf07 outside the presented 0x07c00000 stick when
  written (fp.c, sysregs.out). `sysctl hw.optional.arm` lists 55 features; the translator offers 4.
- Register naming in real ARM64 Linux code (run 1; regs.py, regs.out): static count over eight files (3.7
  million instructions): x18 1.3 per 1000, x15 2.9, x14 3.9, x13 5.1; x26 to x28 13.7 to 22.4.
- KVM stop and in-guest timer on the Linux laptop (run 1; i7-8750H, Linux 7.3.4, load 3.4 to 3.8), round 3's
  vmrun.c unchanged (kvm-laptop.out): 420 stops by signal, pc always inside the loop, count never back,
  median 11.8 µs (9.9 µs with the signal masked outside `KVM_RUN`), 90th percentile about 4 ms; the local
  APIC timer at 100 µs costs 7.8 µs a tick with 3.1 exits per tick.
- Branch targets and tags under Hypervisor.framework (run 2; n0r2/branch.c, branch.out; load 38 to 47): a
  `br` at EL0 to a target 1 or 2 bytes off gives EC 0x22 with FAR and ELR the target; TBI0 = 1 makes tagged
  branches and loads work, TBI0 = 0 makes both fault. `hv_vm_get_max_vcpu_count` 64;
  `hv_vm_config_get_max_ipa_size` 40 bits, default 36.
- The jump sequence's arithmetic (run 2; n0r2/mirror.c, mirror.out): as the plan says.
- The laptop (run 2): 39 physical address bits, no UMIP (/proc/cpuinfo). Run 3: `pcid` and `invpcid` present.
- EL1 software step of EL0 under Hypervisor.framework (run 3; n0r3/step.c, step.out; M5 Max, load 43): with
  MDSCR_EL1.SS = 1 set from the host and SPSR_EL1.SS = 1 in the return frame, each EL0 instruction gives EC
  0x32 with ELR at the next one; a `br` steps to its target; `svc` gives EC 0x15 instead; a `br` to target+2
  steps there, then EC 0x22. `hv_vcpu_get_trap_debug_exceptions` and `..._debug_reg_accesses` both false by
  default; `msr oslar_el1` at EL1 still exits to the host (EC 0x18), and stepping works without it.
- `stadd` of zero as a write probe (run 3; n0r3/stadd.c, a copy of check-n0b/casro.c with cases added;
  stadd.out, three runs; EL0 on macOS, load 43): `staddb`/`staddh`/`stadd` of `wzr` and `stadd xzr` give
  SIGBUS on a read-only page; on a writable page `stadd xzr` and `staddb wzr` (odd address) leave the bytes
  unchanged (5, 6); on an unmapped page SIGSEGV. The checker's `cas`/`casal`/`casp`/`stxr` cases repeat
  SIGBUS.
- Hypervisor.framework virtual CPU identity (run 3; n0r3/idregs.c, idregs.out): ID_AA64MMFR0_EL1
  0x10000f100022: PARange 2 (40 bits), ASIDBits 2 (16-bit identifiers), TGran16 1 and TGran4 0 (both
  granules).
- Register naming in real x86-64 Linux code (run 3; regs86.py, regs86.out; run on the laptop under
  ~/native-plan/regs, load 2.3): 8 files, 5.3 million instructions: r11 15.4 per 1000, r10 18.7, r9 21.5, r8
  30.7, r13 40.1, r15 42.0, r14 50.1, r12 51.1, the legacy eight 55.7 to 210.9.
- TPIDRRO_EL0 and the PMU with EL2 (run 4; n0r4/tpidrro.c, copied from check-n0c/pmu_fpsr.c; tpidrro.out;
  M5 Max, load 54 to 76). TPIDRRO_EL0, for three values: an EL1 `msr` does not exit to the host, an EL0
  `mrs` reads the value, and an EL0 `msr` is undefined (ESR_EL1 0x2000000) and leaves it unchanged. With
  `hv_vm_config_set_el2_enabled` true (two runs), the virtual CPU's ID_AA64DFR0_EL1 is 0x10305006 (PMUVer
  0), and `hv_vcpu_set_sys_reg` sets PMUVer to 1 to 9, which reads back. The emulated PMU itself was not run.
- Round 3's virtual machine cost over the eight bench.c workloads (run 4; n0r4/vmcost.py over
  mac-summary-main.json and mac-summary-16k.json; vmcost.out). Geometric mean of median ratios: 4 KiB pages
  cycles 1.0846 and instructions 1.0092; 2 MiB blocks 1.0279 and 1.0068; 16 KiB granule 1.0294 and 1.0055.
  This agrees with the third check.
- Page-crossing stores and loads on this M5 (run 5; n0r5/partial2.c and partload.c, partial2.out and
  partload.out; EL0 under macOS with its 16 KiB pages; three runs of every byte offset; load 56 to 64).
  Stores where the second page refuses: `stp q`, `stnp q`, `st1` and `st4` of four registers write the
  registers wholly in the first page (48 of 93 and 144 of 189 cases). `str` of w, x and q, `stp` of w, x
  and d, `st1` of one register and of two 8-byte ones write nothing. When the first page refuses, no store
  writes in the second. Loads onto an unreadable page: `ldp q` (48 of 93), `ld1` (144 of 189) and `ld4`
  (96 of 189) of four registers change destination registers before faulting. `ldr x`, `ldp x` and `ldr q`
  change none.
- The same on the Linux laptop (run 5; n0r5/partial86.c, partial86.out; i7-8750H, user mode, core 4, load
  4.9 to 6.1): `mov` of 8 bytes, `movdqu`, `vmovdqu` of 32 bytes, `fxsave64` and `xsave64` crossing onto a
  read-only page wrote nothing, and `movdqu` and `vmovdqu` loads onto an unreadable page left xmm0 as it
  was, at every offset tried (16-byte steps for `fxsave64`, 64 for `xsave64`).
- RIP-relative operands (run 5; n0r5/rip86.py, rip86.out; on the laptop over the same 8 files as regs86,
  5,332,103 instructions): `lea` 23.6 per 1000, other uses 19.3 per 1000 (0.7 in libpcre2 to 61.9 in xz).
- `astro-float` 0.9.6 (run 5, crates.io): MIT; features `std` (in the default set), `random`, `serde`, so it
  is expected to build `no_std` (not built).
- The independent checks' experiments the plan cites (check-n0/, check-n0b/, check-n0c/: pmu_fpsr.out cases
  P, F and H; icache_late.out; check-n0d/: forkvm.out, kvmfork.out, partial.out, unpred.out):
  execute-only EL0 pages work and refuse loads and stores (xo.out cases 1 to 5); misaligned LSE2 accesses
  complete at EL0 in the machine
  (case 6); a second `hv_vm_create` in one process returns HV_BUSY (twovm.out); `cas` and kin fault on a
  read-only page whatever the compare (casro.out).
- Licences, crates.io on 2026-11-18: kvm-ioctls 0.25.1 Apache-2.0 OR MIT; kvm-bindings 0.14.2 Apache-2.0;
  applevisor 1.0.0 MIT OR Apache-2.0; hv 0.1.2 Apache-2.0; xhypervisor 0.3.0 MIT; aarch64-paging 0.12.3 MIT
  OR Apache-2.0; x86_64 0.15.5 MIT/Apache-2.0 (newest stable; 0.16.0-rc.0 is a pre-release, not used);
  aarch64-cpu 12.0.0 MIT/Apache-2.0; talc 5.1.1 MIT; buddy_system_allocator 0.13.0 MIT; iced-x86 1.22.0 MIT;
  dynasmrt 5.1.0 MPL-2.0 (rejected); vm-memory 0.18.0 Apache-2.0 OR BSD-3-Clause (not needed); gimli 0.34.0
  MIT OR Apache-2.0. None is a dependency yet.

Not checked at N0, and named in the plan as such (N1 has since checked `hv_gic_set_spi`, the instruction-cache
maintenance, block entries and their split, and the PMU's overflow, above): `hv_gic_set_spi` (N1), a guest performance counter's overflow
interrupt (decision 10 option C; N1 with EL2 enabled and PMUVer set, N8), 2 MiB entries and the contiguous
bit in the floor (N1, N4), the instruction-cache maintenance sequence curing the late-write case (N1),
`KVM_IRQFD`, the DR0 breakpoint after a `rep`, TF single step under KVM and its #DB cost, UMIP and PCID under
the laptop's KVM, MADV_DONTNEED give-back (N8), the KVM
ARM64 doorbell (no KVM ARM64 machine at hand; N10), the store exclusive under interrupts (N6), the handoff
cost of a step to a pool thread (N5), `KVM_CAP_MAX_VCPUS` on the laptop (N8), the behaviour probe's time
(N2, N9), the Split route's page test cost (N4).

## How to run N0's experiments

The experiments: `python3 -I regs.py <ELF files>` (ARM64) and
`python3 -I regs86.py <ELF files>` (x86-64, on the laptop); in experiments/n0r2 and n0r3, `clang -O1 -o X X.c
-framework Hypervisor && codesign -s - --entitlements hvf.entitlements -f X && ./X` for branch, step and
idregs, and in n0r4 for tpidrro (`./tpidrro`, then `./tpidrro el2`); `python3 -I vmcost.py <summary json>`
in n0r4; `clang -O1 -march=armv8.1-a -o stadd stadd.c && ./stadd` and `clang -O1 -o mirror mirror.c &&
./mirror` without the framework; the KVM runs as in kvm-laptop.out from ~/native-plan/vm on the laptop.

## Departures from the plan

- N2, `Route::Collision`: the plan's register-field collisions (plan 3.4, "Encodings Arm leaves CONSTRAINED
  UNPREDICTABLE") are a route of their own, so the routing test and the emitter see them by name.
- N2, `counted` lives in wcontainers-guest-cpu's native module beside `Native` until T2 moves the count
  into the shared finder; it depends only on `Flow` and the address.
- N2, the encoder is in wcontainers-arm64 (`encode.rs`), not the translator: the translator knows no one
  CPU; the emitter will call it through the guest crate.
- N2, E's cost was measured with round 3's runner (hvfrun) at EL0 inside Hypervisor.framework with 4 KiB
  pages, not on our floor: both put the same programs at EL0 in a virtual machine with 4 KiB stage 1
  pages, and the ratio of each pair is what the speed bar uses. The floor runner comes with the emitter.
- N2, a Kernel-routed form may touch what other routes keep (`svc` forgets the exclusive monitor): the
  inside answers it wholly as the meaning does, so the routing test allows it.

- N1, the build: the Nix shell links against the macOS 14.4 SDK, which does not declare Hypervisor.framework's
  GIC calls (macOS 15). The monitor's build script tells the linker to leave those seven
  calls to be found when the program loads (`-Wl,-U,_name`, list `LATER_CALLS` in
  crates/wcontainers-hypervisor/build.rs); this reaches only that package's own binaries and tests. A
  binary in another package that links the monitor needs the same, until the flake's SDK is 15 or later
  (the flake is not this track's; for the orchestrator).
- N1, boot: the monitor writes the boot map of the inside's half and the boot registers (computed by the
  inside's `boot` module, so the facts stay in one place), and the inside starts at its link address with
  its MMU on, instead of turning it on itself: no early position-independent code is needed.
- N1, the interrupt controller: the floor sets up only the redistributors of virtual CPUs made so far
  (those of virtual CPUs not yet made do not exist and trap to the host).
- N1, two containers: no kernel or container exists yet, so each of the two processes has its own heap page
  in the heap region; a container's heap shared by its processes comes with N3 and N5.
- N1, block entries: the floor maps and splits them (break before make) and the test exercises both;
  choosing them from the kernel's record comes with the native `Cpu` side.

- The brief's addendum asks that our pages the guest can reach either fault as on Linux or be named and
  counted. The plan names them (plan 7.2) but does not count accesses to them: counting would need a check
  on every guest access. The orchestrator settled decision 4: accepted as a named difference, with the two
  free measures the plan lists (plan section 10).
- N1, the cases program: the test builds it with a nested cargo at test time instead of a build script of
  the monitor, so nothing shipped builds or carries it; the first test run after a change to the inside
  pays that build (14.8 s cold at load 80, 0.1 s when up to date).
- N1, SA0: the plan has the floor set it as `Guest::SP_ALIGNMENT_CHECKED` says; that trait is in the CPU
  crates' translating (`std`) half, so the monitor reads it and passes it to the inside's boot registers.

## For the orchestrator

- Decision 10: option C does not work on Hypervisor.framework on this M5 (no event counters; the cycle
  counter's overflow raises nothing). By the plan's recommendation the choice is now A or B, the owner's.
- The flake's macOS SDK (14.4) lacks Hypervisor.framework's GIC and EL2 calls; see Departures.

- The plan asks changes of other tracks (plan section 6.2): R1, R2, R3, R5 of the runtime track (R3 and R5
  also the kernel track's test crate); R4 only under option A of decision 10; T1, T2, T3 of the translator
  track; X1 with the x86 track's meanings; K2 is now the low-first placement runtime.md section 8 already asks
  of the
  kernel track; K3 of the kernel track for the other project.
- Fourteen owner's decisions are in plan section 10; 1, 10 and 12 change interfaces or the ruling's letter;
  14 decides whether a host whose behaviour probe differs (another x86-64 vendor, say) is refused natively;
  13 asks whether the translator track closes its stack-frame exemption (tr/contract.rs:20-22), a place
  where the WebAssembly output differs from the reference and from the native output.
- The laptop keeps ~/native-plan (a Rust toolchain, the copied sources, and regs/ with the x86 count script)
  for later milestones; its build output was deleted.
