# Native back end status

Plan: docs/native.md. Branch: phase-2-native. Brief: pyro-parked/wcontainers-scratch/orchestrator/brief-native.md.

## Where things stand

- N0 (the plan) is written (run 1) and revised twice (runs 2 and 3, 2026-11-18) to settle the findings of two
  independent checks. No code in the repository.
- Next: the owner's decisions of plan section 10, and N1 (the floor on Hypervisor.framework). N1 and N2 need
  nothing from other tracks (only decision 2's names; the suggested ones are used until the owner rules).
  N3, the first real programs, needs decision 1 and its changes R1 and R3 (runtime track; the test crate's
  half by the kernel track) and A1. N5 needs decisions 1, 10 and 12 (R2, R3, R4 only under option A).
- The magic-number sweep the brief asks for: this track owns no crate yet (only docs/native.md and this
  file), so there was nothing to sweep. The plan names `wcontainers_abi::PAGE_SIZE` as the one page size and
  says every number it names is one named constant in code.

## What run 3 changed in the plan, finding by finding

- x86-64 TF and AC: `popf` and `iretq` routed to the inside (their meanings take TF, DF, NT, ID and leave AC
  clear, counted as the x86 plan counts); the guest's TF is the hardware's TF in ring 3, set only by the
  inside; the floor reports a single-step trap only at a guest boundary or at a `rep` (the reference's own
  per-element step) and resumes elsewhere, counted "single step through host instructions"; CR0.AM clear,
  CR0.WP set; the static test records `raise_stop` and classes `Rflags`; KVM's #DB and #AC intercepts
  named as expected, measured at N8.
- Guest-writable thread page: split into a thread page read-only at the guest's level (stop word, limit
  word, x86 lookup cache, kept GS base; the inside writes through its own map) and a register page (withheld
  registers' guest values, exclusive monitor). New rule in 3.2: only host addresses the translator or the
  inside wrote are ever jumped to; none is read from guest-writable memory. 7.2 says what a guest can and
  cannot do there; N1 and N8 test that a store to the thread page faults.
- Memory give-back: now by unmap, fresh mmap and map again (14.7 to 16.6 ms per GiB, footprint 1030.7 to 6.2
  MiB); `MADV_FREE_REUSABLE` rejected (freed nothing); KVM's MADV_DONTNEED named as unmeasured (N8).
- Milestones reordered so nothing before N3 needs an owner's decision: N2 is now the native ARM64 output
  proven instruction by instruction at EL0 (static and differential tests, no kernel); N3 runs the first
  real programs and needs decision 1; its bar is every command of ktest busybox `text` and `files` with the
  root in memory (turns with no limit), the backend half waiting for decision 10. Old N3 to N10 are now N4
  to N11.
- Store exclusive with nothing remembered: makes `stadd` of the zero register at the address (write
  permission, no change) and writes status 1, as the WebAssembly output checks write permission first.
- Heaps and identifiers: one rule for global entries (only the inside, its map of guest-physical memory, and
  the code area); heaps, mirrors, thread and register pages are non-global per process; switching container
  needs no invalidation; 16-bit ASIDs / 12-bit PCIDs across the machine with a counted rollover; per-page
  invalidation by identifier after table changes.
- Decision 10: B's costs stated (two translation modes, suites testing the check-carrying code, so the suites
  also run with the shipped policy); option C added (counter overflow interrupt short of the limit, then
  single step; no check), to be tried at N1 and N8; recommended C if it works on both, else B.
- K2 replaced: the native ARM64 ceiling 2^45 under the container's 2^48 width, the brief's ruled route; it
  gives the same addresses as other back ends once the kernel places low first (already asked by runtime.md
  section 8); hints as well as fixed addresses above 2^45 named. Decision 9 now offers (a) this, (b) width
  2^45 everywhere, (c) 2^48 with a table lookup; recommended (a).
- Finder: the plan now says the native build carries the meanings and wasm-encoder (effects come from
  `G::emit`), the native finder passes the `G::Host` value describing the hardware, and the count is decided
  from `flow` alone, with T2's test running `counted` under every host value. Finder citation corrected to
  full.rs:228-257 and :299-604.
- Speed bar: in cycles (proc_pid_rusage ri_cycles, as round 3), geometric mean of median ratios within the
  count's measured cycles cost (+5.1% Hn to +15.2% En) plus 10 points, decided by a seeded bootstrap 95%
  interval, up to 60 pairs, else reported undecided.
- Every-offset test: made by the hardware's single step from the inside's level (MDSCR_EL1.SS; TF on x86),
  taking a real stop at the first visit of each host instruction; N1's bar includes the step case.
- `Container::run` hands the virtual CPU a repetition of `Stepper::step` that leaves only on Idle, Finished,
  Ended or a host stop; `step` stays one handoff per step, named "step handed to a virtual CPU".
- x86-64 withheld registers: two (count, scratch), the pages through GS; stand-in rewriting with four moves;
  static count over 5.3 million instructions; N9 measures against count-only.
- `PAGE_SIZE` named in the rules; the 16 KiB mode on ARM64 by granule (both offered), on x86-64 by four
  entries.

## What run 2 changed (short)

ARM64 jump through a register (tag ignored, misaligned target faults in hardware, TBI0); x86 GS base and
descriptor reads routed; routing proof as a static and a differential test; one commit point per host
sequence with roll-back and roll-forward; runtime interface changes as decision 1; one virtual machine per
host process (decision 12); load-time addresses; per-process mirror; LSE2 alignment tests; the probe refusing
hosts short of the offer; the replay check against the thread's own stop word; the count property and its
test; slow paths named; x86 `rep` finished by hardware; K3 for the other project.

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
- The independent checks' experiments the plan cites (check-n0/, check-n0b/): execute-only EL0 pages work and
  refuse loads and stores (xo.out cases 1 to 5); misaligned LSE2 accesses complete at EL0 in the machine
  (case 6); a second `hv_vm_create` in one process returns HV_BUSY (twovm.out); `cas` and kin fault on a
  read-only page whatever the compare (casro.out).
- Licences, crates.io on 2026-11-18: kvm-ioctls 0.25.1 Apache-2.0 OR MIT; kvm-bindings 0.14.2 Apache-2.0;
  applevisor 1.0.0 MIT OR Apache-2.0; hv 0.1.2 Apache-2.0; xhypervisor 0.3.0 MIT; aarch64-paging 0.12.3 MIT
  OR Apache-2.0; x86_64 0.15.5 MIT/Apache-2.0 (newest stable; 0.16.0-rc.0 is a pre-release, not used);
  aarch64-cpu 12.0.0 MIT/Apache-2.0; talc 5.1.1 MIT; buddy_system_allocator 0.13.0 MIT; iced-x86 1.22.0 MIT;
  dynasmrt 5.1.0 MPL-2.0 (rejected); vm-memory 0.18.0 Apache-2.0 OR BSD-3-Clause (not needed); gimli 0.34.0
  MIT OR Apache-2.0. None is a dependency yet.

Not checked, and named in the plan as such: `hv_gic_set_spi` (N1), a guest performance counter's overflow
interrupt (decision 10 option C; N1, N8), `KVM_IRQFD`, the DR0 breakpoint after a `rep`, TF single step
under KVM and its #DB cost, UMIP and PCID under the laptop's KVM, MADV_DONTNEED give-back (N8), the KVM
ARM64 doorbell (no KVM ARM64 machine at hand; N10), the store exclusive under interrupts (N6), the handoff
cost of a step to a pool thread (N5).

## How to run

Nothing to run in the repository yet. The experiments: `python3 -I regs.py <ELF files>` (ARM64) and
`python3 -I regs86.py <ELF files>` (x86-64, on the laptop); in experiments/n0r2 and n0r3, `clang -O1 -o X X.c
-framework Hypervisor && codesign -s - --entitlements hvf.entitlements -f X && ./X` for branch, step and
idregs; `clang -O1 -march=armv8.1-a -o stadd stadd.c && ./stadd` and `clang -O1 -o mirror mirror.c &&
./mirror` without the framework; the KVM runs as in kvm-laptop.out from ~/native-plan/vm on the laptop.

## Departures from the plan

None yet.

## For the orchestrator

- The plan asks changes of other tracks (plan section 6.2): R1, R2, R3 of the runtime track (R3 also the
  kernel track's test crate); R4 only under option A of decision 10; T1, T2, T3 of the translator track; X1
  with the x86 track's meanings; K2 is now the low-first placement runtime.md section 8 already asks of the
  kernel track; K3 of the kernel track for the other project.
- Twelve owner's decisions are in plan section 10; 1, 10 and 12 change interfaces or the ruling's letter.
- The laptop keeps ~/native-plan (a Rust toolchain, the copied sources, and regs/ with the x86 count script)
  for later milestones; its build output was deleted.
