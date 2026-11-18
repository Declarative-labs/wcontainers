# Native back end status

Plan: docs/native.md. Branch: phase-2-native. Brief: pyro-parked/wcontainers-scratch/orchestrator/brief-native.md.

## Where things stand

- N0 (the plan) is written (run 1) and revised four times (runs 2 to 5, 2026-11-18) to settle the findings
  of four independent checks. No code in the repository.
- Next: the owner's decisions of plan section 10, and N1 (the floor on Hypervisor.framework). N1 and N2 need
  nothing from other tracks (only decision 2's names; the suggested ones are used until the owner rules).
  N3, the first real programs, needs decision 1 and its changes R1 and R3 (runtime track; the test crate's
  half by the kernel track) and A1. N5 needs decisions 1, 10 and 12 (R2, R3, R4 only under option A). N7
  needs R5.
- The magic-number sweep the brief asks for: this track owns no crate yet (only docs/native.md and this
  file), so there was nothing to sweep. The plan names `wcontainers_abi::PAGE_SIZE` as the one page size and
  says every number it names is one named constant in code.

## What run 5 changed in the plan, finding by finding (fourth check)

- Commit point (blocking): section 4 restated. Before the commit point a sequence may write withheld
  registers and relocate guest values (stand-in moves), never change one; the position map records where
  each relocated value is, and the floor puts them back on both sides. The closed set of roll-forward
  fix-ups now has: a constant into a destination built in place (`adr`, `adrp`, x30 of `blr x30`), slot to
  guest address, the exclusive monitor (a load exclusive's record written whole, the value read again; a
  store exclusive's status and forgotten monitor), and the float status second call. Stand-ins (3.3): t
  where one withheld register is named, else a guest register saved first. Load exclusive (3.4): address in
  t, one load the commit point, record after. Store exclusive: `casp` through four stand-ins, status
  written last as the meaning does.
- Saving (blocking): new R5 (runtime track, kernel track's test crate). The test crate's `fork` save cannot
  carry a virtual machine (the check's forkvm.out and kvmfork.out), so `stopped` only decides and the back
  end saves: the WebAssembly back end by the same fork moved into it, the native one by its image brought
  back in the same machine. Plan 5.8, 6.2, 7.1, N7 and decision 1 updated.
- Partial loads and stores (blocking): new Split route (3.4). Every ARM64 load or store of more than 16
  bytes takes a page test (3 or 4 instructions, no flags) and, when it crosses, a probe of both pages before
  the copied instruction. Measured below: on the M5 loads over 16 bytes also change registers before
  faulting, which the check had not seen.
- Host-model results (blocking): the x87 transcendentals are routed inside (`astro-float`, as the meanings).
  The x87 pointer fields of every x87 save are written as the meaning writes them (`fnstenv`, `fstenv`,
  `fnsave`, `fsave` added to the Float status route). `rcpps` and kin, undefined flags and loads and stores
  outside Split are copied only on a host model whose new behaviour probe (5.8) matches the meanings. A host
  that differs runs the WebAssembly back end. New decision 14 asks refuse (recommended), route always, or
  route per host.
- Speed bar (blocking): the system call half is now the back end's own share of `getppid` (per-call time
  less the kernel's own share), with program, statistic and the same bootstrap rule (8, decision 6).
- Unpredictable encodings (minor): routed by register fields with t in place of the colliding register
  (3.4); the differential test draws register fields with every collision.
- Virtual CPUs (minor): an idle container parks its stack and frees its virtual CPU. The pool grows to the
  containers ready at once, up to the platform's limit, and past it "waited for a virtual CPU" is counted
  (5.1, 6.1).
- N11 bar (3) now waits for K3. The RIP-relative cost is stated and measured statically (3.4). The
  slow-path list gained the stand-in, the Split test, the x87 transcendentals and the virtual CPU wait. The
  count forms' range is now +5.1% to +17.2% (lines 115-118). The `flags_word_function` citation is fixed.
- Found in this run, not by the check: `clrex` is not decoded as nothing. The forms decoder takes it first
  as `ClearExclusive`, which forgets the monitor (arm64/decode.rs:13, decode_forms.rs:63,
  memory.rs:357-360). The plan had it in the Nothing route; it is now the Exclusive route's.

## What run 4 changed (short)

Float status route (FPSR, FPCR, MXCSR and the x86 saves wrapped by two calls into the inside); Nothing route
for the hint space, `prfm` and barriers; the virtual machine's own cost recomputed (+8.5% cycles with 4 KiB
pages, +2.8% with 2 MiB blocks) and 2 MiB entries with counted splits; option C needs EL2 on
Hypervisor.framework; instruction-cache maintenance after every code write; the stack-frame exemption
named (decision 13); TPIDRRO_EL0 holds the ARM64 stop word; retranslation only for replay.

## What run 3 changed (short)

x86-64 TF and AC (`popf` and `iretq` routed, single steps reported only at guest boundaries or a `rep`);
read-only thread page split from the register page, and only our own host addresses jumped to; give-back
by unmap, fresh mmap and map; milestones reordered so nothing before N3 needs a decision; store exclusive
with nothing remembered makes `stadd` of zero; one rule for global entries and shared identifiers; decision
10's options B and C; the 2^45 ceiling under the brief's ruling (decision 9); the finder carries the
meanings; the speed bar in cycles with a bootstrap rule; the every-offset test by hardware step; two
withheld x86-64 registers; `PAGE_SIZE` as the one constant.

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

Not checked, and named in the plan as such: `hv_gic_set_spi` (N1), a guest performance counter's overflow
interrupt (decision 10 option C; N1 with EL2 enabled and PMUVer set, N8), 2 MiB entries and the contiguous
bit in the floor (N1, N4), the instruction-cache maintenance sequence curing the late-write case (N1),
`KVM_IRQFD`, the DR0 breakpoint after a `rep`, TF single step under KVM and its #DB cost, UMIP and PCID under
the laptop's KVM, MADV_DONTNEED give-back (N8), the KVM
ARM64 doorbell (no KVM ARM64 machine at hand; N10), the store exclusive under interrupts (N6), the handoff
cost of a step to a pool thread (N5), `KVM_CAP_MAX_VCPUS` on the laptop (N8), the behaviour probe's time
(N2, N9), the Split route's page test cost (N4).

## How to run

Nothing to run in the repository yet. The experiments: `python3 -I regs.py <ELF files>` (ARM64) and
`python3 -I regs86.py <ELF files>` (x86-64, on the laptop); in experiments/n0r2 and n0r3, `clang -O1 -o X X.c
-framework Hypervisor && codesign -s - --entitlements hvf.entitlements -f X && ./X` for branch, step and
idregs, and in n0r4 for tpidrro (`./tpidrro`, then `./tpidrro el2`); `python3 -I vmcost.py <summary json>`
in n0r4; `clang -O1 -march=armv8.1-a -o stadd stadd.c && ./stadd` and `clang -O1 -o mirror mirror.c &&
./mirror` without the framework; the KVM runs as in kvm-laptop.out from ~/native-plan/vm on the laptop.

## Departures from the plan

- The brief's addendum asks that our pages the guest can reach either fault as on Linux or be named and
  counted. The plan names them (plan 7.2) but does not count accesses to them: counting would need a check
  on every guest access. Decision 4 puts this to the owner.

## For the orchestrator

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
