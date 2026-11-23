# x86-64 native output: status

Plan: docs/x86-native-output.md. Brief: pyro-parked/wcontainers-scratch/orchestrator/relaunch-agents/
x86-native-output.js. Scratch: pyro-parked/wcontainers-scratch/x86-native-output (probes/, results/, check-xn0/,
check-xn0b/ to check-xn0g/, the independent checks' probes and outputs, and run4/ to run8/, those runs'
probes).

## Where things stand

- XN0 (the plan, checked before anything is built): done. Run 1 (2026-11-23) wrote the plan; the first independent
  check found seven blocking and eleven minor problems, which run 2 fixed. Run 3 made the second check (scratch
  check-xn0b/): three blocking and ten minor, fixed. The third check (scratch check-xn0c/) found three blocking and
  nine minor, fixed by run 4 (scratch run4/). The fourth (scratch check-xn0d/) found two blocking and six minor,
  fixed by run 5 (scratch run5/). The fifth (scratch check-xn0e/) found three blocking and six minor; run 6 fixed
  all nine (list below), measured in scratch run6/. The sixth (scratch check-xn0f/) found two blocking and nine
  minor; run 7 fixed all eleven (list below), measured in scratch run7/. The seventh (scratch check-xn0g/) found two
  blocking and seven minor (one of them no defect, a measurement to record); run 8 fixed all nine (list below),
  measured in scratch run8/. The run-8 fixes have not had a check of their own. Nothing is built; no code of this
  track exists yet.
- XN1 (the output with no memory check): runs 1 to 3 (2026-11-23). Not complete. Done: the eighth check's five
  findings answered in the plan; the shared `Route::Area`; the core's `string_passes`; the route of every decoded
  form per way with its static routing test; the 0x67 segment-operand finding's code with its test (run 1). Run 2:
  the trait takes the way and the target (shared, ARM64 one of each); `Native for X86_64` for the machine and P,
  every route carried (below); the guest's FIP stored after each copied x87 instruction; the flag-free check
  after a copied `fldcw` with the new call kind `X87Control`; a rep string under the count run by the inside;
  `DirectExit::condition`; the position map's operand with segment and address width; `Condition` and its
  plain `holds` in the core; the pre-executor bar's encoding round trips and call-position map checks. Not yet:
  the replay check (`stop_check`), the x87 unmasked mode and the inside's side of `X87Control`, `answer` and the
  x87 unit's move into the core, the pool, `retarget`/`Patch` and the `int3` link protocol, stepping pieces, the
  differential test of `holds` against the meaning's WebAssembly.
  No x86 executor can run a translation yet (P's executor, crates/executor/implementation/host-process, was
  committed by its track during this run without a translation runner; no x86 floor), so the bar's layers,
  programs and speed wait on one (plan 8).
  Run 3: layer 1 runs natively after all, without an executor: the test crate's new
  src/guest/x86_64/native.rs puts each case's piece (way P) in a reference blob that sets up the case's memory,
  the thread area and the first level, loads the case's state and enters the piece; the guest's state at the stop
  is read through the position map and compared with the CPU. Each case runs uncounted, and counted with the trap
  flag stopping it after every host instruction (state, data and count checked at each stop: the every-offset test
  for layer 1's pieces). It found five defects of the output, fixed (below). 2008 forms, 1,000 cases a form, seed
  5150: 0 failing (results below). Layers 2 and 3, the X9 programs and speed still need an executor (they run
  many pieces, links, lookups and system calls).
- XN2: not started. The eighth check's way I findings (0x67 strings, the exact form's 0x67 segment operand and
  `67 d7`, `maskmovq` in the window) are fixed in the plan only; their code and tests are XN2's. `carry` refuses
  the in-process way by an assertion until then.

## Next, in order (XN1)

1. The replay check: `stop_check` in the trait, `InsideCall::Stop`, and `stop: Option<u64>` in
   `translate_native_for`; ARM64 has no replay check yet, so the trait method needs ARM64's form or a ruling
   (coordinate with the ARM64 back end). `Step` and P's stepping pieces with it.
2. `answer` and the core moves (the x87 unit, transcendental.rs, root.rs, the lazy_static line), then
   x87_answer.rs and the unmasked mode; the differential test of `Condition::holds` against the meaning's
   WebAssembly (through the translator test crate's runner) and P's table.
3. `retarget` returning a `Patch` (plan 3.2, 9) in place of `link`, for the loader's `int3` protocol.
4. The pool for a call through a register, with `HostCode::pool` (today the return address is built in rax saved
   to the area: the plan's measured 1.146 form, not the pool's 1.120).
5. The executor-dependent bar once an x86 executor runs a translation (P's, or the floor), and the plan's
   measurements (dynamic register count, mirror memory, links' batches) with it: layers 2 and 3, the X9 programs,
   speed, the count across look-aheads, replay, links, register groups, hostile guests. Native layer 1 (run 3) is
   done and stays the first check of any change to the output; the next step short of an executor would be layer
   2's programs in the same blob (pieces translated as the run reaches them, exits linked, a `syscall` stub), which
   is most of P's stub, the native-without-virtual-machine track's W1.
6. Layer 1 does not see the x87 pointers a save writes (FIP, FDP, FOP: zero on both sides, no x87 instruction runs
   before the case), so the inside's FloatAfter for `fxsave`, `fnsave`, `fnstenv` (the pointers from the register
   area into the image) has no check yet; build it with `answer` and test it with an x87 instruction before.

## What XN1 run 3 changed

- crates/wcontainers-translator-test/src/guest/x86_64/native.rs (new, Linux x86-64 only) and the test
  `every_form_matches_the_cpu_natively_x86_64` in tests/layer1.rs (`WC_LAYER1_CASES`, `WC_LAYER1_SEED`,
  `WC_LAYER1_JOBS`, `WC_LAYER1_FORMS` as layer 1's). A case's instruction is translated at look-ahead none for P
  (`translate_native_for`, `Way::Process`, the reference's `HostMatches`: 5-level, partial writes and undefined
  results as the meaning's, since this CPU is the reference) and run as a reference blob at 2^43: its prologue maps
  the data and code areas, the area (register area with the guest's r10, r11, r15 and GS base; the fixed part's
  entries pointing at unmapped addresses, one a kind; the first level's 2^15 lower-region words pointing into an
  unmapped second level at 2^44), installs a handler on its own stack, loads the state (`xrstor`, `popfq`, moves)
  and jumps to the piece. The handler writes the frame, the data area and the register area. How a run ends: an
  exit (the Exit call, also X87Control and FloatAfter; FloatBefore's entry is `jmp t`, a host process holding the
  float status in hardware), a fault in the piece, a lookup miss at the second level (by its host instruction),
  a call the inside answers or a first-level miss (the state must be the case's start, then the meaning's outcome
  stands for the inside's). The native outcome is reconciled as the reference's is (both are a CPU seen through a
  frame) and compared with the reference. Two classes are counted as known failures, not passes: the reference's
  5-level paging (a page fault where the 4-level meaning gives #GP, x86-machine.md) and a meaning-answered case where
  the meaning differs from the reference exactly as the WebAssembly layer 1 does (checked: `smsw [r15d]` fails
  there the same way). Cases with TF or AC set are not run (P keeps both off the hardware; stepping pieces not
  built), counted.
- Each case runs twice: without the count, and with it and TF set after the prologue, the handler returning at
  every trap in the piece: at each stop the state read through the map must be the start (before the commit point)
  or the final one (after), the data likewise, the count (r10 less the lead) 0 or the events done. A control (the
  expectation flipped in the machine's copy) fails 3 of 4 forms at host offset 0.
- Defects found by it and fixed in carry.rs and native.rs, with a named test (`what_layer_1_found_on_the_cpu`):
  a `gs:` operand based on rsp or rbp rebuilt as `[rsp/rbp + t]` took the stack segment's #SS where the guest
  takes #GP (139 of the first failures): t is now the base; a stand-in for a withheld base could be rbp
  (`cmpxchg8b [r15]`), and the high-byte form's address register too: never rbp; a `pop m` based on rsp on `gs:`
  formed its address before the pop: the pop's size is added; float status instructions (`ldmxcsr`, `fxsave`,
  `fnstenv`, ...) on `gs:` used the hardware's GS, our area: rebuilt as Area now; a bit test with a register offset
  on `gs:` under 0x67 lost the 32-bit wrap of the offset's part (the carried 0x67 finding's other case in the
  unchecked form): answered by the inside.

## XN1 run 3 results (rented x86 machine, Xeon 8488C, 5-level paging)

- Native layer 1, 2008 forms (every form layer 1 has), 1,000 cases a form, seed 5150, 32 at once, uncounted and
  counted (before the stepping check), load 0.3 before: 0 failing, 511.5 s; 2,594 known failures (91 form lines with
  the 5-level class, 11 with the meaning class: sldt, smsw, str (the host kernel's UMIP emulation), wrfsbase and
  wrgsbase; 11 transcendental forms as WebAssembly's). Ended (uncounted run): exit 1,416,015, fault 437,876,
  answered 88,693, trap 62,329, first-level miss 2,495, second-level miss 592.
- With the stepping check (commit a4551138), 2008 forms, 1,000 cases a form, seed 5150, 32 at once, load 25 to 34
  during it (this run's own jobs and other users): 0 failing, 511.3 s, the same 2,594 known failures and endings;
  no case had TF or AC set, so none was left unrun. Also seed 7, 8 cases a form: 0 failing. The x86 crate's and
  guest-cpu's tests and the translator's native lib tests in the same job: all pass.
- clippy with warnings denied on wcontainers-x86 and wcontainers-translator-test (all targets, `--no-deps`):
  clean. x86 crate's native tests: routes 2, output 18, carry 2: pass.

## What XN1 run 2 changed

- Shared (commit 0b3267fc): `Native` gains `type Way` and `type Target`; `route`, `carry`, `call`, `exit` take
  the way (and `route`, `carry` the target). `translate_native_for` takes both; `translate_native` stays for a
  guest whose way and target are `Default` (ARM64's are `()`), so the hardware-virtualization executor's calls
  are unchanged; its tests' `Arm64::route` calls were updated. ARM64's clippy, tests and the translator's native
  tests passed on the ARM64 machine; hardware-virtualization's lib and tests build.
- Shared (commit 179ca957): `Fixup::X87Pointer(u64)` (moved with the code like `Fixup::Address`; the ARM64
  inside's stop.rs takes it as nothing, since ARM64 never writes it) and `NativeReason::RepeatedStringCounted`.
- x86/native.rs: `Target { host, offer }` and `impl Native for X86_64`. x86/carry.rs: every route for the machine
  and P. Copy as the guest's bytes; an instruction naming r10, r11 or r15 re-encoded on stand-ins chosen by iced's
  register use (implicit ones included), saved to and loaded from the register area by `seg:` moves (counted
  `WithheldRegisterNamed`); Rebase (`mov t, imm64` an address place, `lea` likewise, `mov t32, t32` under 0x67);
  Area (the run-1 forms, a RIP-relative operand on the area through u, a `moffs` move through u, `rdgsbase` from
  the area); an instruction naming ah, ch, dh or bh whose operand is rebuilt takes its address in a register of
  the first eight (REX would forbid the byte register); Kernel, Trap, FloatStatus through `call` (machine `int
  32 + code`, P `lea t, [rip]; jmp qword ptr gs:[entries + 8 * code]`); Transfer: direct `jmp`/`call` by rel32
  exits (`call` pushes the guest return address from `mov t, imm64`), `jcc` rel32 linkable in place when the count
  is not ahead, else through a settling exit, `loop`/`jrcxz` by their rel8 over a short jump to an exit,
  returns and jumps and calls through a register or memory by the two-level mirror of plan 3.3; targets at or above
  2^47 answered by the inside. The count is `lea r10, [r10 + k]` with a 32-bit displacement, so a link's `jmp
  rel32` fits over a settling add (the rest filled with `nop`). area.rs: the register area (GS base, withheld
  values, stand-in values, FIP), the fixed part (entries), the first level at two pages.
- After a copied x87 instruction that is not a control one (fpu.rs's own `control`), `mov t, imm64; mov
  gs:[fip], t` with `Fixup::X87Pointer` over it.
- After a copied `fldcw` (commits 6855550d, 161ece86): `fnstcw` into the register area, `movzx`, `not`, `pext`
  of the six masks, rcx borrowed into u for `jrcxz`, and a call of the new kind `InsideCall::X87Control` when one
  is clear; no arithmetic flag written, nothing relocated at the call. The ARM64 inside's stop.rs and the
  hardware-virtualization test take the kind as unexpected. The SDM leaves C0 to C3 undefined after `fnstcw`:
  a host that changed them would change the guest's condition codes after a `fldcw` (not measured; the meaning
  leaves them).
- `DirectExit::condition` (commit 97b3f852): set by x86's linkable `jcc` exits from iced's condition code,
  `None` elsewhere and on ARM64; `branch_exit` takes it.
- x86/condition.rs (core, commit 5053d6bf): `Condition` and `Test` moved from flags.rs, with `ALL`, `from_code`
  and `holds`, unit-tested on all 16 conditions and 32 flag combinations against the SDM's table.
- The position map's `Operand` gains `segment` (`Fs`, `Gs`) and `address_bits` (commit 47b3dfc2; ARM64 flat, 64);
  x86's `stopping` gives `Access` for every memory operand but `lea`'s.
- A gather whose vector index is on the area's segment under 0x67 goes to the inside (the 32-bit `lea` cannot
  take a vector index); found by reading the carrying code, now a named test.
- The sweep also checks that nothing is relocated at any call into the inside and nothing relocated or pending
  at its return point.
- Tests (crates/wcontainers-x86/tests/native_output.rs): a seeded sweep of instruction mixes with branches, calls
  and returns, translated in both ways with and without the count, decoded back by iced: every piece decodes
  whole, every span, exit and address place is at an instruction start of its kind, no host `call`/`ret`, no
  RIP-relative access but P's `lea t, [rip]`, every copied instruction's bytes unchanged, every stand-in form the
  same code naming no withheld register; and twelve named shapes (return, call through a register, direct call,
  a loop linked in place, stand-ins, `gs:` rebuilt and `fs:` copied, `moffs` on `gs:`, RIP-relative, `syscall`
  per way, a trap's map, the count, FIP, a rep string). native_carry.rs now reads the translated piece.

## XN1 run 2 results (these tests run no host code, so they hold on either machine)

- Rented x86 machine (Xeon 8488C, load 0.08 before, 1.36 after), at commit ab6fc245 plus other tracks' edits:
  clippy with warnings denied on wcontainers-x86, wcontainers-guest-cpu, wcontainers-translator (native, all
  targets, `--no-deps`): clean; `cargo test -p wcontainers-x86 -p wcontainers-guest-cpu`: all pass (x86 lib 9,
  native_output 17, native_carry 2, native_routes 2, the others as before); the translator's native lib tests 7.
  The x86 machine did not answer SSH for part of the run, so the work in between was built and tested on the
  ARM64 machine.
- Rented ARM64 machine (Graviton4, load 10 to 36 from other tracks): the same clippy plus wcontainers-arm64 clean;
  `cargo check -p wcontainers-x86 --no-default-features`, hardware-virtualization's lib and tests and its inside:
  build; `cargo test -p wcontainers-guest-cpu -p wcontainers-arm64` and the translator's native tests: pass, after
  each shared change.
- native_output sweep: seed 0x5eed 20,000 cases (80,000 pieces), seed 0x2b17 500,000 cases (2,000,000 pieces,
  5,705,008 spans, before the fldcw and call-map checks), seed 0x51 4,000,000 cases (16,000,000 pieces, 45,570,238
  spans, release, all checks): 0 broken. native_routes seed 0x51, 2,000,000 cases: 0 broken. native_carry seed 11,
  1,000,000 cases: 0 differences.

## Departures from the plan (XN1 run 3)

- Layer 1 runs natively before any executor exists (plan 8 waits for one), through a reference blob the test crate
  builds: the same translation, way and map an executor would use, the inside's answers taken from the meaning
  where the state at the call is the case's start. Its every-offset check uses the hardware's TF, which P itself
  never sets; that is the test's stop, not a way of running.
- Two classes of disagreement are counted as known failures by name (the reference's 5-level paging; the meaning's
  own difference from the reference on inside-answered forms), and cases with TF or AC set are not run natively.

## Departures from the plan (XN1 run 2)

- The trait's second type is `Target` (the host's matches and the process's offer), not `HostMatches`, since the
  route needs both.
- A rep string under the count goes to the inside (counted `RepeatedStringCounted`) instead of the inline
  after-code of plan 3.6: correct by construction, slow only with exact replay asked for, and no count stack. The
  after-code remains the plan's form if the inside's cost matters.
- FIP is stored after each copied x87 instruction, not once a run (the trait's `carry` sees no instructions
  after the current one yet).
- A call through a register builds its guest return address in rax saved to the area (the plan's measured
  stand-in form), until the pool exists.
- `DirectExit::retarget` holds, for x86, the byte offset of the branch's rel32 within it, until the shared field
  changes as plan 9 says.

## What XN1 run 1 changed

- The eighth check's findings, in the plan: (1, blocking, way I) a string with the guest's 0x67 goes to the inside
  in both checked forms, since its rdi side cannot be rebased (plan 4, routing test, way I tests with str67.c's
  case); code at XN2. (2) a segment operand with the guest's 0x67: the offset part is wrapped by `lea u32, [base32 +
  index32*scale + disp]` before the segment base is added (`[t + u]`), in the machine and P (plan 3.1) and in the
  exact form (plan 4), `67 d7` by a 32-bit `lea`; the wrapping form needed nothing (the sum modulo 2^32 is the
  same). Code for the machine and P done (`carry::area_operand`), the exact form's at XN2. (3) `maskmovq`
  named beside `maskmovdqu`: Kernel on the area's segment in the machine and P, re-encoded in I, in the routing
  test's list (plan 3.1, 4, 7). (4) the rep string count is a second implementation of the meaning's rule: named,
  with the full rule (each pass's own length, the cap, one element a pass for compares, `lods`, 0x67, DF and TF),
  its closed form (the check's case 91 passes), `string_passes` in the core, the after-code tested against the
  WebAssembly output, and no cost estimate (plan 3.6, 7, 9). (5) the unmasked x87 mode needs a plain-Rust twin of
  fpu.rs's instruction-level part: named as new code, x87_answer.rs (plan 3.1, 9).
- `Route::Area` added to gcpu/native.rs (ARM64's carrying code takes it as unreachable), as plan 9 now says.
- x86/strings.rs (core): `StringKind` and `BULK_STRING_BYTES` moved there from meaning.rs, and `string_passes`.
- x86/native.rs: `Way` (machine, process, in process with its check), `HostMatches` (paging depth, partial writes,
  undefined results), `route` for every decoded form.
- x86/area.rs (core): the register area's first word, the guest's GS base; x86/carry.rs: the withheld registers
  and `area_operand`, built on iced's encoder (feature `encoder` added).
- Tests: tests/native_routes.rs (a seeded sweep of instruction-shaped bytes, every decoded form in all four ways on
  two hosts against the plan's rules, and 54 named forms); tests/native_carry.rs (the check's gs67 case, and a
  seeded sweep of operand shapes, both address sizes, comparing the address the host code reaches, run through
  iced's `virtual_address`, with the meaning's).

## XN1 results (rented x86 machine, Xeon 8488C, load 0.1 to 0.7, commit of this run plus other tracks' edits)

- clippy with warnings denied on wcontainers-x86, wcontainers-guest-cpu, wcontainers-arm64 (all targets, `--no-deps`,
  since the kernel interface crate, not ours, has two type-complexity findings): clean. `cargo check -p
  wcontainers-x86 --no-default-features` (the core alone, which the kernel takes): builds.
- `cargo test -p wcontainers-x86 -p wcontainers-guest-cpu`: 37 tests, 0 failed.
- native_routes: seed 0x5eed 200,000 cases and seed 0x2b17 2,000,000 cases of instruction-shaped bytes, each
  decoded form in 4 ways on 2 hosts (15,999,984 routes in the second: Copy 4,003,401, Nothing 107,840, Rebase
  64,464, Transfer 750,824, Kernel 820,723, FloatStatus 10,576, Trap 9,740,928, Area 501,228): 0 broken; 54
  named forms right.
- native_carry: the gs67 case reaches 0x300000010 (and without 0x67 0x400000010); operand shapes, seed 0x5eed
  100,000 cases and seed 11 1,000,000 cases: 0 differences from the meaning's address.
- While this run worked, crates/executor/implementation/host-process (another track's, uncommitted) named a
  missing test file, which stops every cargo command in the tree; the runs above put an empty file there in the
  rented machine's copy only.

## How to run XN1's tests

`orchestrator/cloud/x86-run <checkout> --name x86-native-output -- 'cargo test -q -p wcontainers-x86 --test
native_routes --test native_carry -- --nocapture'`; the sweeps take `X86_ROUTE_SEED`, `X86_ROUTE_CASES`,
`X86_CARRY_SEED` and `X86_CARRY_CASES` (decimal, or hex with 0x for the route seed); `cargo test -p
wcontainers-x86 --lib strings` for the pass rule.

Native layer 1 (one command, on the rented x86 machine, which is its own reference; about 9 minutes without the
stepping run at 32 jobs): `x86-run <checkout> --name x86-native-output --jobs 32 -- 'WC_LAYER1_CASES=1000
WC_LAYER1_SEED=5150 WC_LAYER1_JOBS=32 cargo test -q -p wcontainers-translator-test --test layer1
every_form_matches_the_cpu_natively_x86_64 -- --nocapture'`; `WC_LAYER1_FORMS=<names>` for some forms. The
summary lines start "layer 1 natively".

## Departures from the plan (XN1)

- `route` takes the process's `Offer` beside the way and the host's matches (plan 9 gives `route(way, i, host)`),
  since a form of a feature the process was not told of takes the Trap route and the offer says which.
- `route` is a free function of x86/native.rs until the trait of plan 9 is changed; the trait's other methods come
  with it.

## What run 8 changed (seventh check), finding by finding

- Speculation in way I (blocking): the exact form's access is now the wrapping form's, cut to the window by its
  addressing mode (address-size prefix, `gs:[t32]`), after either check; the bit tests are re-encoded the same way
  (the hardware wraps t32 plus the offset's part, which is the checked A); strings cut rsi and rdi with `mov esi,
  esi` and `mov edi, edi` after the probe and before the rebase; `maskmovdqu` and `xlat` take the address-size
  prefix in both forms; `pushf` reads the flags on the count stack and pushes them through `access`, so no rsp
  rebase remains. A new Speculation item says what this covers and why, the routing test fails on an uncut access,
  section 7 has the bounds-check-bypass test for every access shape and XN2's bar includes it. Measured
  (run8/specx3.c, two seeds, 100,000 rounds each): the uncut live-flags forms read the secret 99,997 to 99,999
  times, the uncut dead-flags forms 4 to 7, every cut form 0 (plan 4, 7, 8).
- The no_std core (blocking): `Condition`, the x87 unit and the transcendental answers are translate-only today and
  transcendental.rs uses std; section 9 now lists their move into the core as shared-code changes (a new
  condition.rs, the x87 unit, transcendental.rs and root.rs moved, the `Mnemonic` helpers left behind `translate`,
  the `thread_local!` cache passed in as `&mut Consts`). Measured (run8/nostd): rustc_apfloat 0.2.3 and
  astro-float-num 0.3.7 build for `x86_64-unknown-none` only with lazy_static's `spin_no_std` feature, so the core
  gains that dependency line (plan 1, 3.1, 3.2, 3.5, 9).
- SA_RESTORER (minor): P's handlers are installed with it and a never-reached restorer; why (raw `rt_sigaction`, the
  check's bt16.c) and that this track's probes went through glibc (plan 3.7).
- The unmasked x87 mode across saves (minor): saves in that mode take the control and status words from the block,
  loads put them back there and keep the hardware's masked, the thread or child stays in the mode; new shapes for
  a switch, a returning handler and `clone` (plan 3.1, 7).
- X87Control's two positions (minor): before an MMX instruction it resumes at that instruction, after a `fldcw` at
  the next; the map records per position and XN1's map test checks both (plan 3.7, 7).
- The guest's address-size prefix on a bit test (minor): A is formed with a 32-bit `lea`, wrapping as the meaning
  does; the check's case (`67 0f a3 08`, eax 0xfffffff0, ecx 0x100, reaching 0x10) is a way I test and was run in
  run8/btcut.c (plan 4, 7).
- c from the frame's copy in P (minor): named as P's difference, changing only that process's counts (plan 3.7, 5).
- The stale coordination item (minor): removed; the P/I plan's 3.10 already takes the mirror with no slot table.
- The 16-bit bit-test sequences (no defect): the check's measurement recorded in plan 4.

## What run 7 changed (sixth check), finding by finding

- Bit tests in way I's exact form (blocking): `bt`, `bts`, `btr`, `btc` on memory with a register offset reach A = t
  + (w/8) * (offset SAR log2 w); the check and the probe are now made on A, computed flag-free in u (`mov u32, 6;
  sarx; lea` for 64-bit, with `movsxd` for 32-bit, `movsx` and a count register for 16-bit), the instruction
  re-encoded on `gs:[t]`. The wrapping form needed nothing. The guards claim and the routing test now name every
  form whose reach is not its operand's address. Measured (run7/btseq.c): 4,000,000 seeded cases, the sequence gave
  the meaning's A every time with no flag changed, and `lock bts` on `gs:[t]` set exactly the bit at B + A. New way
  I tests with offsets up to 2^60 (plan 4, 7).
- Saved return points across a dropped piece (blocking): every drop converts, before the piece's map goes and with
  the memory's other threads stopped, each saved t in the piece (the machine's `iretq` frame, P's request record,
  I's thread record) into its guest pc and count, and the thread leaves through the lookup; space is reused only
  after that. Per way how the stop covers a thread in the inside (floor lock, embedder's records, I's drop gate).
  New shape tests: a thread in a `futex` wait while its page is rewritten, unmapped, unprotected, replaced and the
  space reused (plan 3.2, 3.6, 3.7, 7).
- The vector image's place (minor): rdx (the context the host kernel passes) plus a fixed delta, not a place below
  the stack's top. Measured (run7/altdelta.c): 448 bytes for rsp on the program's stack and for 128 rsp values
  inside the alternate stack, where the distance from the top ranged over -68,544 to -68,672. A guest rsp near the
  alternate stack's bottom killed the process by SIGSEGV: named for P (plan 3.7, 5).
- The run-6 handler probe (minor): it never loaded the copy's r10 and r11, so that part of its hostile test could not
  fail. Measured again (run7/presume7.c) restoring all sixteen registers and leaving through a read-only word to a
  lookup entry reading the pc from the register area: same results, 924 to 931 ns (plan 3.7).
- The `fldcw` check in P (minor): the register-area word is sibling-writable in P, so the race is not closed there;
  named as P's difference, inside the guest's own process (plan 3.1, 5).
- The `jcc` condition (minor): `Condition::holds` in plain Rust for the floor and I, the flags table and a 16-entry
  `jcc` table in P's assembly, a differential test of the three against the meaning (plan 3.2, 7, 9).
- MMX in the unmasked x87 mode (minor): the register stack stays in the hardware (control word masked there), x87
  instructions go to the x87 unit, MMX instructions stay copied after an X87Control call that raises a pending #MF
  (plan 3.1, 9).
- XSAVE routing (minor): keyed on the offer's XSAVE and AVX bits apart; XSAVE without AVX is offered with the
  576-byte image (plan 3.1).
- The inside-call kinds (minor): `X87Control`, `Step` and `Stop` given as code, `ALL` 11 (plan 9).
- The handler's choice on an `int3` hit (minor): it also reads the copy's rip and `si_code`, which siblings can
  write; why that is harmless is stated (plan 3.2).
- The vDSO (minor): no x86-64 vDSO exists (the kernel's is ARM64's one-function image); asked of the kernel track,
  with the P/I plan's "clock_gettime is a system call" as the case without one (plan 3.7).
- The header (minor): run 7, six checks.

## What run 6 changed (fifth check), finding by finding

- A link's `int3` hit on a `jcc` exit (blocking): the handler now does what the branch would have done: a `jmp`
  exit resumes at its unlinked path, a `jcc` exit evaluates its condition on the interrupted rflags (the meaning's
  condition function) and resumes at the unlinked path or after the `jcc`, as Linux's `int3_emulate_jcc`;
  `DirectExit` gains `condition: Option<u8>` (plan 3.2, 9). Measured (run6/jcc16.out): all 16 conditions, seeded
  flags, 20,000 links each, 16 and 4 workers: 0 wrong arrivals; the check's form (always the unlinked path) had
  50,983 and 142,005.
- The flags table in every way (blocking): it is P's alone. In the machine and I the carrying code's `popfq` reads
  the word its `pushfq` wrote, in memory no other thread reaches, so the hardware TF (the guest's there) survives a
  rep string count; a new test steps a thread through `rep movsb` and ten one-byte instructions in each way (plan
  3.6, 7). I's routine's last `popfq` puts the guest's TF and AC back.
- t after a stop in P's stub (blocking): every way out of the stub is `jmp qword ptr gs:[resume]`, a word of the
  thread area's read-only part that only the embedder writes, after checking in its position map that t is the
  return point of a call of the kind asked, in a live piece of this process; a t from the handler's copy is checked
  the same way, so a sibling can choose only another call return point of its own process, where the guest's state
  is its registers and register area alone (no fix-up pending, nothing relocated at any call; the FIP store comes
  before a call; XN1's map test checks it). Other words the stub uses after a stop come from the read-only part
  again. A new hostile-guest test edits the copy's t in each step of the stub (plan 2, 3.6, 3.7, 7).
- UMIP (minor): the host's kernel emulates all five of `sgdt`, `sidt`, `sldt`, `str`, `smsw` for a 64-bit process
  (the check's umip.c); the routing was already Kernel (plan 3.1).
- Linux's function names (minor): `smp_text_poke_batch_add`, `smp_text_poke_batch_finish`,
  `smp_text_poke_int3_handler` (plan 3.2).
- The XSAVE family (minor): Kernel only with the AVX setting (`xsave`, `xrstor`); without it they are outside the
  offer (XSAVE is in the setting's group), and `xsavec`, `xsaveopt` are outside it always: Trap (plan 3.1).
- The check after `fldcw` (minor): `fnstcw` into a register-area word, then the test on that word, so a sibling's
  store to the operand cannot hide an unmasked exception (plan 3.1).
- The handler measured was not the one specified (minor): measured again, all in assembly with no call or push on
  the alternate stack, the image read at a fixed place below the top (replaced in run 7 by rdx plus a delta), every
  deciding word read-only, the resume through a read-only table; it did not load the copy's r10 and r11 (run 7's
  probe does) (plan 3.7).
- rflags in the count stack word (minor): the word sits at a fixed offset of the register page, so the map names
  it with the existing `Held::Page`, and rflags gets the x86 crate's map number 16; the passes are added by one
  `lea` and `RepPasses` covers the span up to it (plan 3.6, 9).

## What run 5 changed (fourth check), finding by finding

- P's `popfq` from a word siblings can write (blocking): every `popfq` of the carrying code and the stub now reads
  a word of the flags table, a read-only page of 512 words indexed by `pext` of the value read once into a
  register (CF PF AF ZF SF DF OF NT ID; TF and AC clear in every word). Measured (run5/flags.out): with a sibling
  storing 0x40302 into the count-stack slot, the old form set TF and AC 6 times each in 20,000,000 rounds, the
  table form 0 in each of 3 runs; it costs about 0.9 ns a round more (9.12 to 9.18 against 8.25 to 8.26) (plan
  3.6).
- Stops nested in P's resume (blocking): the handler now follows the P/I plan's 3.6: installed with `SA_NODEFER`
  and `sa_mask` the stop signal, the stub thread blocking all but the stop and the synchronous faults, rip
  classified by the position map or the stub's own ranges (the tail takes the exit already written, from the
  copy), rsp moved off the alternate stack before the unblock. Measured: 200,000 stops flooded while another
  sibling ORs TF and AC into the copy, 3 runs: 53,145 to 62,085 taken in the tail, the deepest frame 3 KiB, the
  guest loop's flags, r12..r14 and ymm7 kept; a round trip 948 to 953 ns against 1,073 to 1,080 with
  `rt_sigreturn` (plan 3.7).
- The mask from the frame (minor): the handler unblocks the stop signal alone, its set a word of the stub's
  read-only page; the P/I plan's 10-second kill stays for a stub that blocks it all the same (plan 3.7).
- SIGTRAP and SIGBUS in P (minor): a SIGTRAP is a link's `int3` only with `SI_KERNEL` at an exit of the batch; in P
  TF and AC are never set, so `TRAP_TRACE` or `BUS_ADRALN` is a fault of ours, the thread ended, counted
  "unexpected host trap" (plan 3.2).
- FIP, FDP, FOP (minor): the hardware's are never read. FIP is stored in the register area after a run's last
  non-control x87 instruction, with a fix-up `X87Pointer` before it; a copied `fldcw` is followed by a flag-free
  check (or a call where the flags are live), and a thread with an unmasked x87 exception runs its x87 and MMX
  instructions through the meaning's x87 unit in the inside, which sets FDP and FOP (plan 3.1, 7, 9).
- The rep string twin (minor): it adds nothing to c, does not store rcx again, and counts the whole instruction's
  passes from the original's word; a fault inside counts the passes completed before the faulting element (plan
  3.7).
- P's link syncs (minor): registration at the stub's start named; the membarrier is a command to a stub thread in a
  kernel entry, or to one the embedder stops first. Measured across processes (run5/plink.out): 14.92 to 79.79 µs
  a link with a stub thread waiting (0 to 64 running threads), 15.81 to 231.34 µs when one must be stopped first
  (1 to 64) (plan 3.2).
- Run 4's trap counts (minor): the count was not atomic, so run 4's figures were lower bounds, and the check is the
  workers' `ud2` with exit 0, not the printed line. Re-run with an atomic count (run5/patch2.c): 0, 115,878,
  321,673 and 1,191,515 traps (plan 3.2).

## What run 4 changed (third check), finding by finding

- Calls read the code area (blocking): the guest return address is pushed from a pool, a read-only,
  non-executable range at one fixed distance from the code area, so P's execute-only code area holds. Measured
  (run4/calls-resume.out): fib(30) through `call r`, pool outside execute-only code 1.120
  against plain call/ret, pool in the code page 1.119, stand-in register 1.146, `push imm32` plus high-half store
  3.271 (plan 2, 3.2, 3.3).
- Unsynchronized cross-modifying code (blocking): links are now made by Linux's `int3` protocol in batches, three
  syncs a batch (membarrier SYNC_CORE in P and I, interrupts in the machine), new code reached only after the first
  sync; an `int3` hit resumes at the exit's unlinked path. Measured (run4/patch.out): 0 to 64 threads running the
  exit while it was linked 20,000 times, every turn on a target (checked by each worker's `ud2`, exit 0), every
  trap resumed; one membarrier 0.65 to 5.88 µs, one link 1.90 to 69.05 µs (plan 3.2).
- The machine's area in the lower half (blocking): put back where docs/native.md has it, the upper canonical half,
  user-accessible pages, guest ceiling 2^47; its readable pages named in plan section 5 as native.md 7.2's
  difference (decision 4) (plan 2, 5, 6). So it is no departure.
- Minor: `exit` takes the way and `retarget` (returning a `Patch`) replaces `link`; `Held::InScratch` dropped for
  the existing `Relocated` (plan 9). `syscall` per way, and the floor writes the guest's r11 slot with rflags (plan
  3.1, 3.6). XRSTOR's mask is the offered groups, measured with the full handler (943 to 950 ns against 1,080 to
  1,088; a PKRU edit in the frame left PKRU unchanged with mask 7 and killed the process with the frame's mask)
  (plan 3.7). t may be read from the copy as a guest value only (3.7). TF and AC cleared by I's routine before
  Rust code and by P's stub when it puts rflags back (3.6; P's form replaced by the flags table in run 5). The rep
  string pass count until ruled: inline, out of line, `pushfq` on the area's count stack, a `RepPasses` fix-up; cost expected, measured at XN1 (3.6, 9).
  Register-group tests per way and a new P thread's vector registers loaded by XRSTOR (3.6, 7, 8). Shadow stacks: P
  refuses a stub thread with one, I resumes by `rt_sigreturn` (3.1, 3.7). The exactness figure re-measured with
  exactly 2^15 first-level words (21 targets fault on the first level, 15 on the second, t kept at all) (3.3).

## What run 3 changed (second check), finding by finding

- P resumed through `rt_sigreturn` (blocking): a sibling thread can rewrite the frame's rip on the alternate stack;
  measured 198,432 of 200,000 signals returned to the sibling's address. Now P's handler resumes by its own jump
  (mask by `rt_sigprocmask`, vector state by `XRSTOR` from its copy, since a handler starts with the vector state
  reset); measured working and cheaper, 925 against 1,089 ns a round trip (plan 3.7).
- Guest TF in P (blocking): the silent resume mid-sequence would take t and u from that editable frame, and a
  chosen u sends the second-level `jmp` anywhere. P now keeps the hardware TF clear and runs a stepping thread on
  stepping pieces (the request's `step`; plan 3.6, 9).
- The machine's per-processor areas (blocking): mapped at distinct addresses in a process's tables, a thread of
  process A could write the register area of a thread of process B on another processor. Now one virtual address
  mapped per processor through its own top-level entry (plan 2).
- Minor: I's host signal handler starts with FS on the thread area (measured), so it sets the host's FS first
  (plan 3.7); `wrfsbase` is not copied in P on a 5-level host, which accepted 2^47, 2^50 and 0xff00000000000000
  (measured, plan 3.1); the reserved-NOP space (`rdsspq` and the rest) is Nothing, never Copy; strings and `xlat`
  on the area's segment go to the inside; `jrcxz` is rel8, so the replay check's layout is given, and the check
  sits before its run's count add; way I's `call r/m` takes stand-ins, `pushf` rebases rsp, `push m`/`pop m` go
  through t; the stub moves t (r11) and c (r10) before a host `syscall`; host requirements named (BMI2, FSGSBASE,
  XSAVEC); a sibling's store into another thread's register area named for P (plan 5); the vDSO clock's cost and
  I's first-level remap with busy runners given to XN1 and XN2 as measurements; sigreturn's other cs values.

## What run 2 changed (first check), finding by finding

- Top-bits bound wrapped (blocking): replaced. The first level is indexed by bits 32..63 (2^32 words, 32 GiB reserved
  per thread area, 256 KiB mapped), so every target at or above 2^47 faults on the first-level read with t intact;
  no bound, no add (plan 3.3). Measured exact over 37 targets and cheaper than run 1's form.
- Kernel-half targets (blocking): the inside now answers by the meaning's 4-level rule: not canonical is general
  protection at the transfer with rsp not popped; a canonical upper-half target, vsyscall included, is a page fault
  at the target with rip there and the pop or push done (plan 3.3, 3.7). The first-level read is placed before the
  commit (and before a call's push) so nothing is to undo.
- FS/GS bases (blocking): the thread area is on GS in the machine and P and on FS in I; the guest's FS is the
  hardware's in the machine and P, so `fs:`, `rdfsbase`, `wrfsbase` are copied there. The area segment's operands
  and base instructions are routed (`rd*base` inline from the register area, `wr*base` to the inside, selector reads
  to the inside); the static routing test is per way with this plan's classes and fails if any is copied (plan 2,
  3.1, 7). This also removes run 1's extra load on every guest `fs:` operand in the machine and P (the dynamic count
  the check asked for is no longer leaned on).
- Strings in I (blocking): the start (rdi and rsi) is bounded by the flag-free probe before rdi is rebased; a run
  from inside the window faults on one guard page; `maskmovdqu` is re-encoded on `gs:` (plan 4).
- Host return address on a guest-writable stack in P (blocking): the call into the inside in P and I is `lea t,
  [rip]; jmp seg:[entry + 8*kind]`, nothing pushed; the routine keeps t in a register (P) or its own memory (I)
  (plan 3.6).
- Replay (blocking): stop word in the area's read-only part, `stop` in the translation request, a flag-free check
  with rcx borrowed into t and `jrcxz`, rep strings run pass by pass by the inside in a replay piece (plan 3.7, 9).
- Partial writes (blocking): `enter` with a level, `fstp m80` and `fbstp` do leave partial writes (measured again);
  they are routed by the probe's match like undefined flags; the masked stores measured whole (plan 3.6).
- Minor: XSAVE sizes corrected (832 and 576 bytes against 11,008); the register-count reasoning restated with the
  files that disagree; the partial-write results now in results/partial.out; Rust in the inside in I saves the
  offered groups with XSAVEC; the P/I plan reconciled (below); the area's fields are disp32 offsets from the segment
  base, no address place; the stack cost in I declared unmeasured and given to XN2 with the alternative; self-modifying
  code with other threads, locking order, the kernel's writes, shared mappings and unmap or execute loss; sigreturn
  with cs 0x23; pass bars with figures; the miss routine in empty second-level entries and the memory bound.

## Measured

All on the rented x86 machine (Xeon Platinum 8488C, Sapphire Rapids, 96 processors, Linux 7.0.0-1018-aws, 5-level
paging, AT_HWCAP2 0x2 = user FSGSBASE, vsyscall execute-only, mmap_min_addr 65536), probes copied there with
`x86-run --ssh`, timings pinned to processor 5.
- Run 1, static shapes (probes/shapes86.py, results/shapes86.out; 13 files, 19.2 million instructions; load 1.3 to
  2.8): RIP-relative `lea` 24.5 and other 22.0 per 1000; `fs:` 1.22 (glibc 12.7); `gs:` 0.00; rep strings 0.22;
  r8..r15 named per 1000: r10 13.9, r15 15.0, r11 17.1, r9 19.1, r14 21.3, r13 21.8, r8 28.6, r12 30.8, with files
  that disagree (libm r15 2.9, libz r15 50.7, the Go daemon reversing r12..r15); memory operands where the flags may
  be live 20.9%.
- Run 1, return lookups (probes/returns.S and returns.c, fib(30), 3 runs of 21 alternating rounds, `rdtsc`; load 1.4
  to 7.2; results/returns.out, returns2.out): against plain call/ret, two-level mirror (16-bit index) 1.120 to 1.133,
  cache with `cmp` 1.17 to 1.39, flag-free cache 1.52 to 1.62.
- Run 2, exact lookups (probes/mirror3.S and mirror3.c, results/mirror3.out; load 9.4 at the start, 2.4 a minute
  later): over 37 targets, the 32-bit index, the `pext` index and the saved-and-bounded form reach no wrong piece and
  keep the target at every fault; the 16-bit index reaches a wrong piece for 4 (bits 61..63). fib(30), 3 runs of 21
  rounds: 32-bit index 1.065 to 1.070, 16-bit 1.112 to 1.117, `pext` 1.119 to 1.123, saved-and-bounded 1.265 to
  1.272.
- Run 2, partial writes (probes/partial86.c, partial3.c and check-xn0/partial2.c, results/partial.out; load 2.4):
  none for `mov` 8 bytes (21 cases), `movdqu` (45), `vmovdqu` 32 bytes (93), `fxsave64` (93), `xsave64` (24),
  `fnsave` (107), `vmaskmovps` xmm and ymm (15, 31), `vpmaskmovq` ymm (31), `maskmovdqu` (15); loads changed no
  register (45, 93). Partial: `enter 0,3` (6, 14, 22 of 8, 16, 24 bytes), `fstp m80` (8 bytes before the fault, 2 of 9
  offsets), `fbstp` (1 byte, 2 of 9).
- Run 3, signals (check-xn0b/psig.c, psig.out; load 0.17, `gcc -O2 -mavx2 -mfsgsbase -fno-stack-protector`,
  main thread on processor 5, sibling on 6): a sibling storing one address into the frame's rip slot in a loop
  diverted 198,432 of 200,000 `rt_sigreturn`s; ymm7 (all ones before) read zero at handler entry and all ones after
  `rt_sigreturn`; resuming by jump (`rt_sigprocmask`, `XRSTOR` with the frame's xfeatures) kept ymm7 and handled all
  200,000; a `tgkill` to itself and back, 3 rounds of 200,000: 1,088 to 1,089 ns with `rt_sigreturn`, 923 to 927 ns
  by jump; the FS base at handler entry is the one set before the signal.
- Run 3, FS bases (check-xn0b/wrfs.c, wrfs.out; load 0.05): `wrfsbase` accepts 2^46, 2^47, 2^50,
  0xff00000000000000, 0xffff800000000000, 0xfff0000000000000 and faults on 2^56 and 2^57 (5-level canonical).
- Run 4 (scratch run4/; load 0.04 to 0.12; `gcc -O2`, one pinned processor where timed):
  - calls.c and calls.S, calls-resume.out: the 32-bit-index lookup with 2^15 first-level words mapped, 37
    targets, none wrong, t kept at every fault, 21 on the first level (all at or above 2^47), 15 on the second.
    fib(30), every guest call `call r`, 21 rounds, median against plain call/ret: pool in the code page
    (read-execute) 1.119, pool in a read-only page outside execute-only code 1.120, stand-in 1.146, `push imm32` and
    `mov dword [rsp+4]` 3.271.
  - resume.c, calls-resume.out: P's full resume by jump (frame copied, mask 7, flags with TF and AC cleared, every
    general register and rsp from the copy, jump through a table word): ymm7 and r12..r14 kept; 943 to 950 ns
    against 1,080 to 1,088 with `rt_sigreturn`, 3 rounds of 200,000; a PKRU edit to 0x55555555 in the frame left
    PKRU 0x55555554 with mask 7, and killed the child by SIGSEGV with the frame's own mask.
  - patch.c, patch.out: the `int3` link protocol with membarrier SYNC_CORE, 20,000 links, threads on their own
    processors running the exit: 0, 4, 16, 64 threads: membarrier 0.65, 1.48, 2.15, 5.88 µs; one link 1.90, 6.03,
    16.48, 69.05 µs; at least 110,263, 309,928 and 1,150,833 traps resumed (the count was not atomic: lower
    bounds); every turn on one of the targets (checked by each worker's `ud2` at its end and exit 0; the printed
    line is unconditional).
- Run 5 (scratch run5/; load 0.3 to 0.5 before each, `gcc -O2 -pthread`, timed parts on one pinned processor):
  - flags.c, flags.out, 3 runs: the count stack with a sibling storing 0x40302 into its slot, 20,000,000 rounds:
    `andq` and `popfq` of that word set TF 6, 6, 6 and AC 6, 6, 6 times; the flags table 0 and 0 each run; without
    the sibling 8.25 to 8.26 ns a round against 9.12 to 9.18. P's resume (tail off the alternate stack, unblock from
    a read-only set, XRSTOR mask 7, the table, registers, jump), 200,000 stops flooded by one sibling while another
    ORs TF and AC into the copy's rflags, 128 KiB alternate stack: 2 to 4 taken in guest code, 53,145 to 62,085 in
    the tail, deepest frame 3 KiB, the guest loop's flags (CF PF ZF OF DF NT ID set, TF AC clear) and r12..r14 and
    ymm7 kept. A round trip `tgkill` to itself, 3 rounds of 200,000 a run: 948 to 953 ns, `rt_sigreturn` 1,073 to
    1,080.
  - patch2.c, plink.out (run 4's patch.c with an atomic trap count; the probe's own 64 threads raised the load to
    10 to 15): membarrier 0.74, 1.50, 2.11, 5.88 µs; one link 1.97, 5.91, 16.69, 69.99 µs; traps 0, 115,878,
    321,673, 1,191,515; exit 0.
  - plink.c, plink.out: P's links across processes (embedder writes a shared memfd, the stub's threads run it
    read-execute, three futex requests a link, 20,000 links): with a stub thread waiting, 0, 4, 16, 64 running
    threads, 14.92, 19.37, 38.83, 79.79 µs a link (traps 0 to 1,422,111); with none waiting, each request stopping
    a running thread whose handler makes the call, 1, 4, 16, 64 threads, 15.81, 29.84, 83.96, 231.34 µs (traps
    67,487 to 4,394,533); every stub exited 0 with every turn checked.
- Run 6 (scratch run6/; `gcc -O2 -pthread`, timed parts on one pinned processor):
  - jcc16.c, jcc16.out (load 0.13 before, 1.76 after the probe's own 16 workers): a `jcc` rel32 exit of each of
    the 16 conditions relinked 20,000 times by the `int3` protocol, workers loading seeded arithmetic flags each
    turn, the handler evaluating the condition from the frame's rflags: 16 workers (seed 0x5eed) 320,571 to 321,559
    traps a condition, 4 workers (seed 0x2b17) 91,297 to 93,199; 0 wrong arrivals in all 32; exit 0.
  - presume.c, presume.out (load 0.03 before, 0.11 after): P's handler as the plan specifies it (assembly, no call
    or push, image at its fixed place, measured 2,880 bytes below the alternate stack's top, 832 bytes, XRSTOR
    mask 7, the flags table, deciding words read-only, resume through a read-only table): 200,000 stops flooded
    while a sibling ORs TF and AC into the copy's rflags and stores handler addresses into its r10 and r11 (which
    this probe never loaded, so that edit could not matter), 3 runs: 3 to 7 taken in guest code, 55,133 to 63,419
    in the tail, deepest frame 3 KiB, flags, r12..r14 and ymm7 kept, exit 0; round trip 930 to 942 ns against 1,078
    to 1,097 with `rt_sigreturn` (3 rounds of 200,000 in each of 3 runs).
- Run 7 (scratch run7/; load 2.0 to 3.2 from other users of the machine, Linux 7.0.0-1018-aws):
  - btseq.c, btseq-altdelta.out (`gcc -O2 -mno-red-zone -mbmi2 -mfsgsbase`, processor 5): way I's exact bit-test
    address, 2,000,000 cases with seed 0x5eed and 2,000,000 with 0x2b17 (half 64-bit, half 32-bit; offsets small,
    negative, bit 62 set, any): 0 differences from the meaning's A, flags changed 0 times, `lock bts` on `gs:[t]`
    landed at exactly B + A's bit in every case; exit 0.
  - altdelta.c, same file: the vector image at the context (rdx) + 448 bytes for rsp on the program's stack and for
    rsp at top - 64 KiB - k, k = 0..127 (the same delta each time; from the top -68,544 to -68,672); rsp 512 bytes
    above the alternate stack's bottom: the process killed by signal 11.
  - presume7.c, presume7.out (`gcc -O2 -pthread -fno-stack-protector -mfsgsbase`, main thread on processor 5):
    the handler restoring all sixteen general registers (r10 and r11 included) and leaving by `jmp` through a
    read-only word to a flag-free lookup entry that reads the copy's pc from the register area at `gs:8`; 200,000
    stops flooded while a sibling ORs TF and AC into the copy's rflags and stores handler addresses into its r10 and
    r11, 3 runs: 8 to 10 taken in guest code, 54,952 to 57,568 in the tail, deepest frame 3 KiB, the guest loop's
    flags, r12..r14 and ymm7 kept, exit 0; round trip 924 to 931 ns against 1,069 to 1,079 with `rt_sigreturn` (3
    rounds of 200,000 in each of 3 runs).
- Run 8 (scratch run8/, btcut-specx3.out; load 0.1 to 0.8 before and after each, processor 5):
  - btcut.c (`gcc -O2 -mno-red-zone -mbmi2 -mfsgsbase`): 2,000,000 cases with seed 0x5eed and 2,000,000 with
    0x2b17 (16, 32, 64-bit offsets, A in a 1 MiB buffer at B, t at or above 2^32 in 625,255 and 625,142): `lock
    bts` on `gs:[t32]` with the offset register missed exactly B + A's bit 0 times; the 32-bit `lea` A against the
    meaning's wrapped A 0 wrong, flags changed 0 times; `67 0f a3 08`'s case reached bit 0 of B + 0x10; exit 0.
  - specx3.c (`gcc -O2 -mbmi2 -mfsgsbase`), 100,000 rounds each, seeds 0x1234567 (secret 0xc7) and 0xbeef01 (0xc1),
    rounds where the secret's byte (or bit) was read: `cmp; jae; gs:[t]` 4 and 7, `gs:[t32]` 0 and 0; probe then
    `gs:[t]` 99,999 and 99,999, then `gs:[t32]` 0 and 0 (the window's byte 99,999 and 99,996); `bt r, gs:[t]` 7 and
    5, `bt r, gs:[t32]` 0 and 0; probe then `lods gs:[rsi]` 99,997 and 99,997, with `mov esi, esi` first 0 and 0;
    exit 0.
  - nostd/ (a no_std crate using astro-float-num 0.3.7 without default features and rustc_apfloat 0.2.3, built in
    the checkout's Nix shell, Rust 1.99.0, `cargo build --release --target x86_64-unknown-none`): fails in
    lazy_static (E0463, no std) as the x86 crate takes them today; builds with `lazy_static` and its
    `spin_no_std` feature added (tree: either, itertools 0.10.5, lazy_static 1.5.1, spin 0.9.9, smallvec, bitflags).
- Measured by the seventh check (check-xn0g/): specx.c, specx2.out (the uncut forms leaking, the cut one not; the
  figures are in plan 4); bt16.c, bt16.out (the 16-bit sequences, 10,000,000 cases, 0 wrong; raw `rt_sigaction`
  without `SA_RESTORER` killed by signal 11).
- Measured by the fifth check (check-xn0e/run.out): UMIP emulation of all five of sgdt, sidt, sldt, str, smsw in a
  64-bit process (none raised a signal); a `popfq` of the guest's word with TF set gave 10 steps over ten `nop`s,
  of a flags-table word 0.
- Taken from cloud/x86/x86-machine.md, not measured by this track: XSAVE 11,008 bytes, a signal frame's xstate_bv
  0x207, split-lock detection.

## Departures from docs/native.md (the ARM64-first plan's x86 parts)

- The thread area on GS in the machine and P (as native.md) but on FS in I, where GS holds the window; the guest's
  FS base in the register area in I only.
- A stop inside a rep string resumes at the instruction's twin instead of a DR0 breakpoint, which P and I cannot set
  without perf events.
- Jumps through a register and returns through a two-level mirror with a 32-bit first-level index, not a compared
  cache (measured above).
- The partial-write forms and undefined-flag forms routed by the probe's match on the host, not a Split route.
- The XSAVE family goes to the inside (native.md copied it between two float calls): the host's XCR0 sizes the image.
- Three withheld registers, not two (the mirror needs a second scratch).
- The replay check is `jrcxz` with rcx borrowed into t (native.md left x86's form open, "chosen at N9").
- Guest single step in P by stepping pieces with the hardware TF clear (native.md 3.4 steps in hardware, which P
  cannot resume from safely); the machine and I keep native.md's form.
- `wrfsbase` routed to the inside in P on a 5-level host (copied elsewhere in the machine and P).
- Links by Linux's `int3` protocol in batches of three syncs, not one store (native.md 3.2 relies on ARM64's
  allowed concurrent change of a branch, which x86 does not give); a hit on a `jcc` exit evaluates the exit's
  condition, so `DirectExit` gains `condition`.
- The flags table in P only; the machine and I put rflags back from the word they saved, keeping the guest's TF.
- A call's guest return address from a read-only pool outside the code area, so the code area is never read as
  data (P maps it execute-only).
- A rep string's pass count computed inline after the instruction until the count is ruled (native.md section 4
  does not count rounds).
- x87 arithmetic is copied only while every x87 exception is masked; a copied `fldcw` is checked, and a thread
  with one unmasked runs its x87 instructions in the inside and its MMX instructions after a pending-exception
  call (native.md 3.4 copies x87 arithmetic always, which leaves FDP and FOP to the host). FIP is kept by a store a
  run in the register area, not taken from the hardware.
- Saved return points converted when their piece is dropped (native.md does not say what a thread waiting in the
  inside resumes at after its piece goes).

## Coordination with other tracks (read theirs at the start of every run)

- native-without-virtual-machine (docs/native-without-virtual-machine.md, the P/I plan, written the same day as
  this one). Asked of it, to reconcile before XN1 or its W1:
  - Way I's guarantee against speculation (its 3.2, 9 and W2's test, which its 3.10 takes for x86 "as it stands"):
    holds for x86 since run 8, every access in both forms being cut to 32 bits (plan 4, Speculation; measured).
    Before run 8 the exact form's access was on 64-bit `gs:[t]` and leaked (the seventh check's specx.c).
  - `SA_RESTORER` on x86-64: its 3.6 now says it; plan 3.7 says the same for P's stub.
  - I on x86: the thread area is on FS (its 3.8's `wrfsbase` swap at entry and exit stays, but the value during guest
    code is our area, not the guest's FS base) and GS is the process's window base; one area per runner, the
    process's first level mapped in at a process switch. The x86 window (2^32 plus 2^35 and two pages below, one
    above) fits its 2^36 slots; the x86 guest ceiling in I is 2^32, not 2^r.
  - Calls into the inside in P and I: `lea t, [rip]; jmp seg:[entry + 8*kind]`, nothing pushed. In I the routine
    keeps t in the inside's memory and jumps back to it. In P (changed in run 6, replacing "the stub keeps t in a
    register" of its 3.7) the stub hands t to the embedder in the request and leaves only by `jmp qword ptr
    gs:[resume]`, a word of the thread area's read-only part the embedder writes after its position map shows t
    is a call return point of the kind asked in a live piece of the process (else the lookup's entry, or the thread
    ended "call from an unknown host address"); a t from a stop's copy is checked the same way, and every other word
    the stub uses after a stop (its pages, the answer word, the futex address) is read again from the read-only
    part. A pure answer the stub makes itself may jump back to t only when no stop landed in it.
  - Signals: resumption at a guest boundary through the lookup, as its 3.3; the rep-string twin is entered that way.
    P's handler never returns by `rt_sigreturn` (its 3.7 says so; the plan's 3.7 has the measurement and the
    jump-out it measured); t from a frame copy is read only as a guest value and u never, so P has no hardware
    single step (stepping pieces instead). In I the handler sets the host's FS base before any Rust code and the
    area's again before guest code; its 3.8 swaps FS at entry and exit only.
  - The stub's routines: t is r11 and c is r10, which a host `syscall` overwrites or takes as an argument.
  - Links in P: the embedder writes the code by the `int3` protocol (plan 3.2); the stub's first thread registers
    `MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_SYNC_CORE` before any guest code; each of a batch's three syncs is a
    command (its 4.2) to a stub thread in a kernel entry, or to one the embedder stops first, which runs
    `membarrier(PRIVATE_EXPEDITED_SYNC_CORE)` and answers (measured 14.92 to 231.34 µs a link, run5/plink.out);
    the stub's SIGTRAP handler takes an `int3` hit (`SI_KERNEL`, at an exit of the batch) as the exit's branch
    would: a `jmp` exit at its unlinked path, a `jcc` exit by its condition on the copy's rflags, at the unlinked
    path or after the `jcc`; any other SIGTRAP or an alignment SIGBUS is a fault of ours.
  - The code area may stay execute-only (its 3.4): translated code reads no data from it; it needs a pool range,
    read-only and not executable, at one fixed distance from the code area.
  - The stub's handlers follow its 3.6 as written (mask, `SA_NODEFER`, the tail taking the exit already written;
    run 4's x86 text restored the frame's mask and is replaced), are assembly that makes no call or push on the
    alternate stack, unblock the stop signal from a read-only set, and move rsp off the alternate stack before the
    unblock. The stub has a read-only page holding the flags table and that set, and puts rflags back only by
    `popfq` of a table word (plan 3.6), restores vector state with the offered groups as the XRSTOR mask, reads the
    offered groups' part of the vector image at rdx (the context address) plus a fixed delta (learnt from its first
    frame; 448 bytes on the Xeon, run7/altdelta.c; a place below the stack's top is wrong once a guest rsp points
    into the alternate stack), never through `fpregs`, loads a new thread's groups from our kernel's image before its first instruction, and its threads
    have no user shadow stack (refused by name otherwise). I's handler resumes by `rt_sigreturn` with the frame's
    rip set to the lookup's entry; I's routine clears TF and AC before Rust code.
  - Saved return points (plan 3.7, added in run 7): a drop converts every saved t in a dropped piece into its guest
    pc and count before the piece's map goes; in P the embedder validates each t at the request (recording that
    pc and count) and answers a thread whose piece was dropped with the lookup's entry; in I a thread leaves the
    inside only past the memory's drop gate, which the 4.2 stop waits on. Its 3.3 rule ("never at a host pc in the
    middle of a piece") is kept for every resume after a drop or a stop; the jump back to t is only for a live
    piece.
  - P on a 4-level host: about 3,000 guest threads per process (32 GiB of first level each).
  - What each way must give: plan section 2 (read-only part of the area, no host address in guest-writable memory).
- KVM back end (the floor): the thread areas at one virtual address mapped per virtual processor through its own
  top-level entry, so no thread reaches another processor's register area (plan 2); guest page tables 4-level; our
  area in the upper canonical half on user-accessible pages, guest ceiling 2^47 (plan 6); links synced by an
  interrupt to every processor running the process, three times a batch, and vector 3 in translated code resumed
  as the exit's branch would (a `jcc` exit by its condition on the saved rflags, plan 3.2); the carrying code's
  `popfq` reads its own saved word, not P's flags table, so the guest's TF holds (plan 3.6); on a copied `syscall` the floor writes rcx and the guest's r11 slot (plan
  3.6).
- kernel: `rt_sigreturn` with cs 0x23 refused by name ("32-bit code segment in sigreturn, not provided", SIGSEGV);
  the kernel's writes into translated pages drop them (plan 3.7). Asked in run 7: what an x86-64 program gets as
  its vDSO. Today vdso.rs builds only ARM64's one-function image and loader.rs hands it out through
  `AT_SYSINFO_EHDR`; an x86-64 guest needs either no vDSO (glibc's `clock_gettime` then a system call, as the P/I
  plan assumes) or an x86-64 one (plan 3.7).
- x86 track (run 3): on the rented Xeon, the meaning differs from the reference for some cases of `sldt`, `smsw`,
  `str` (which the host kernel emulates under UMIP; e.g. `66 67 41 0f 01 24 e7`: the reference faults at
  0x43ffffff having changed the byte at DATA + 0x2fff, the meaning at 0x44000000 with nothing changed), `wrfsbase`
  and `wrgsbase`; WebAssembly layer 1 fails the same cases (checked for
  smsw), so they are its to answer; native layer 1 counts them by name.
- x86 and translator tracks: the rep string count (plan 3.6 and 9) changes the WebAssembly output's count if ruled;
  shared code changes (plan section 9) go in small commits that keep ARM64's suites passing. The x86 crate's core
  gains `Condition`, the x87 unit, the transcendental answers and their dependencies (plan 9), so the kernel,
  which depends on the core, compiles them too.
- instruction design: the x86-64 `ProcessorArchitecture` and precise instruction sets do not exist yet; the route of
  undefined-result and partial-write forms takes the host's match from them once they do.

## For the owner

- How a rep string instruction counts (plan 3.6): today the WebAssembly output counts passes, which depend on data;
  proposed: its elements, as code in plan section 9.
- Split-lock detection on cloud hosts (plan 3.6, 5): a named difference in P and I, or the inside completes the
  access.
- Decision 14 for x86 (plan 10): partial-write and undefined-flag forms routed by the probe's match per host.
- P on a 4-level host: about 3,000 guest threads per process (plan 3.3, 5).

## Licences

XN1 run 3 made libc 0.2.191 (MIT OR Apache-2.0, already in the lock) a dependency of wcontainers-translator-test on
Linux x86-64 only (the signal frame's layout and system call numbers of native layer 1's blob), and turned on the
translator's `native` feature there. No new crate.

XN1 run 2 turned on iced-x86's `instr_info` feature (for the stand-ins' register use) and, in tests only, `intel`
(listings), both of iced-x86 1.22.0 (MIT, already a dependency), and made wcontainers-translator (this
workspace's) a dev-dependency of the x86 crate. No new crate.

XN1 run 1 turned on iced-x86's `encoder` feature (iced-x86 1.22.0, MIT, already a dependency) and added fastrand
2.5.0 (MIT OR Apache-2.0, already in the lock for wcontainers-arm64's tests) as a dev-dependency of the x86 crate,
for the seeded sweeps. Plan
section 9 moves rustc_apfloat 0.2.3 (Apache-2.0 WITH LLVM-exception) and astro-float-num 0.3.7 (MIT), both already
dependencies, into the x86 core and adds `lazy_static` 1.5 (MIT OR Apache-2.0) for its `spin_no_std` feature, which
brings spin 0.9.9 (MIT); licences read from cargo tree on the rented machine.

## Security

- Unsafe code: none in this track (XN1 run 3's native.rs in the test crate holds none: the blob is machine code in
  a byte buffer the reference runs; XN1 run 2's carry.rs, native.rs, area.rs and native_output.rs hold none;
  XN1 run 1's x86/native.rs, carry.rs, area.rs, strings.rs and two test files hold
  none; grep finds no `unsafe` in them); none planned (the translator and wcontainers-x86 keep the workspace ban;
  emitting code into a byte buffer needs none). No imports to vet beyond fastrand, a test-only dependency.
- Hostile-guest points the plan answers: every host address translated code jumps to is a word of our tables for any
  guest value (the mirror, plan 3.3, measured over 37 targets); no host return address is ever pushed or read back
  from guest-writable memory (plan 3.6); the guest cannot set or read the thread area's segment base (plan 3.1, the
  routing test); prefetches are never copied; strings in I are bounded at their start (plan 4); `xsave` never runs
  with the guest's mask on the host's XCR0; 32-bit code is never entered, sigreturn included; the inside's Rust in I
  cannot clear the guest's ymm upper halves (XSAVEC); P never resumes through a signal frame a sibling can edit
  (measured: 198,432 of 200,000 diverted when it does) and has no hardware single step; P's XRSTOR mask is the
  offered groups, never the frame's (measured: the frame's mask let an edited PKRU through); every `popfq` in P
  reads a word of a read-only table, so no sibling store can set TF or AC (measured: clearing a writable word first
  let 6 of 20,000,000 through, the table 0); P's handler unblocks the stop from a read-only set, never the frame's
  mask, and keeps the alternate stack to one frame under a flood of stops (measured: 3 KiB deep at 200,000, with
  the handler as specified, run6/presume.c); P's stub never jumps to a host address a stop may have exposed to a
  sibling: it leaves through a read-only word the embedder writes only for a call return point its map knows;
  P's handler writes nothing of ours on the alternate stack and reads the vector image at rdx plus a fixed delta,
  never through `fpregs` (measured constant for 129 interrupted rsp values); its exit restores all sixteen
  registers and jumps only through read-only words (measured with a sibling storing handler addresses into the
  copy's r10 and r11, run7/presume7.c); an `int3` hit on a `jcc` exit chooses only between two host addresses of
  the map (measured: 0 wrong arrivals over 16 conditions); way I's exact form checks the address a bit test
  reaches, not its operand's (checking the operand alone let the check's probe set a host byte 1 GiB away; the new
  sequence measured over 4,000,000 seeded cases); every way I access in both forms is cut to the window by its
  addressing mode, so a speculative access past either check reaches nothing outside it (measured: the uncut
  live-flags forms read a byte outside 99,997 to 99,999 times in 100,000, every cut form 0, run8/specx3.c);
  P's handlers carry `SA_RESTORER`, without which the host kernel kills the stub at its first signal;
  no thread resumes at a host address in a piece dropped since its call (plan 3.7, tests in plan 7);
  no code is changed while another processor may run it except by the `int3` protocol
  (measured over 20,000 links with 64 threads running the exit); the code area is never read as data, so it may be
  execute-only; the machine maps each processor's area only for that processor; I's handler restores the host's FS
  before Rust code. Points it hands to
  P: writable pages of the guest's host process, the signal frame and other threads' register areas included, are
  the guest's (so a sibling can change a stopped thread's count c, named in plan 5).
- Tests for these: plan section 7 (shapes, way I, routing).

## How to run the probes

`tar cf - <probes> | orchestrator/cloud/x86-run --ssh 'mkdir -p /work/probes/xno && cd /work/probes/xno && tar xf -
&& gcc -O2 -o mirror3 mirror3.c mirror3.S && taskset -c 5 ./mirror3 30 21'`; the same for returns (`returns.c
returns.S`, `./returns 30 21`) and the partial probes (`gcc -O2 -mavx2`, no arguments); `python3 -I shapes86.py <ELF
files>`. Run 3's: `gcc -O2 -mavx2 -mfsgsbase -fno-stack-protector -pthread -o psig psig.c && ./psig 200000` and
`gcc -O2 -mfsgsbase -o wrfs wrfs.c && ./wrfs`, in /work/probes/xn0b. Run 4's, in /work/probes/xn0r4: `gcc -O2
-mbmi2 -o calls calls.c calls.S && taskset -c 5 ./calls 30 21`, `gcc -O2 -mavx2 -fno-stack-protector -o resume
resume.c && ./resume 200000`, `gcc -O2 -pthread -o patch patch.c && ./patch 20000`. Run 5's, in /work/probes/xn0r5:
`gcc -O2 -pthread -fno-stack-protector -o flags flags.c && ./flags 20000000 200000 200000` (rounds, stops, round
trips), `gcc -O2 -pthread -o plink plink.c && ./plink 20000`, `gcc -O2 -pthread -o patch2 patch2.c && ./patch2
20000`. Run 6's, in /work/probes/xn0r6: `gcc -O2 -pthread -o jcc16 jcc16.c && ./jcc16 20000 16 0x5eed` (links,
workers, seed; exit 1 on any wrong arrival), `gcc -O2 -pthread -fno-stack-protector -o presume presume.c &&
./presume 200000 200000` (stops, round trips). Run 7's, in /work/probes/xn0r7: `gcc -O2 -mno-red-zone -mbmi2
-mfsgsbase -o btseq btseq.c && taskset -c 5 ./btseq 2000000 0x5eed` (cases, seed; exit 1 on any difference), `gcc
-O2 -o altdelta altdelta.c && ./altdelta`, `gcc -O2 -pthread -fno-stack-protector -mfsgsbase -o presume7
presume7.c && ./presume7 200000 200000`. Run 8's, in /work/probes/xn0r8: `gcc -O2 -mno-red-zone -mbmi2 -mfsgsbase -o
btcut btcut.c && taskset -c 5 ./btcut 2000000 0x5eed` (cases, seed; exit 1 on any difference), `gcc -O2 -mbmi2
-mfsgsbase -o specx3 specx3.c && taskset -c 5 ./specx3 100000 0x1234567` (rounds, seed), and in nostd/ `nix develop
<a checkout copy on the machine> -c cargo build --release --target x86_64-unknown-none`.
