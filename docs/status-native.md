# Native back end status

Plan: docs/native.md. Branch: phase-2-native. Brief: pyro-parked/wcontainers-scratch/orchestrator/brief-native.md.

## Where things stand

- N0 (the plan) is done. At the start of N1 (2026-11-18) the plan was corrected for the last check's open
  findings and the brief's newest rulings (commit 64e30e56; how, below).
- N1 (the floor on Hypervisor.framework) is done: every case of its bar passes (20 of 20), measurements
  below. Commits a1f1e82b (A1), 91fb63ff (the inside), 0c8ef62d (the monitor and the test), and the two
  after them.
- Next: N2 (the native ARM64 output, instruction by instruction), which needs no other track's work; its
  changes in other tracks' crates are this track's by the orchestrator's leave (plan 6.2, G1). It also
  measures E's cost with the add's placement free. N3 needs decision 1 (R1, R3). N6 needs K2 (below).
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

## N1: what was built

- crates/wcontainers-inside (`no_std`; names are suggestions): `layout` (the guest-physical and virtual
  layout, every number one constant derived from `PAGE_SIZE` where it can be), `link` (the start record,
  the link page, the requests), `boot` (the boot map and boot registers the monitor applies), and, built
  only for `aarch64-unknown-none`, `floor`: entry and vectors (`floor/entry.rs`), frames and translation
  tables with page and block entries, split break before make, TLB invalidation (`floor/tables.rs`),
  address spaces with per-process TTBR1 tops sharing the global entries, a table root and identifier per
  view (per thread), the GIC through `arm-gic`, the virtual timer, code made visible (CTR_EL0's lines),
  the stop word in TPIDRRO_EL0, the doorbell. `src/bin/floor-cases` is N1's test program (never shipped):
  hand-written EL0 snippets, one report per case.
- crates/wcontainers-hypervisor: the `Hypervisor`, `Machine`, `VirtualCpu` and `Stopper` traits (plan 5.9),
  their Hypervisor.framework implementation over `applevisor` (`src/hvf.rs`, the only file that names the
  platform), guest memory as one reservation with give-back by unmap, fresh mmap and map, the ELF loader
  and boot. Its build script builds the inside for the bare-metal target with a nested cargo into its own
  output directory and embeds the programs (`programs::FLOOR_CASES`).
- tests/floor.rs (harness off): boots the cases, serves the link, makes 300 stops from another thread, and
  checks every report; then the counter trial in a machine with EL2, recorded only.

## How to run

From the checkout, in the Nix shell:
- the bar: `CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER=$PWD/crates/wcontainers-hypervisor/sign-and-run nix
  develop -c cargo test -j 4 -p wcontainers-hypervisor --test floor` (the runner signs the binary ad hoc
  with crates/wcontainers-hypervisor/hypervisor.entitlements and runs it; about half a second);
- lint: `nix develop -c cargo clippy -j 4 -p wcontainers-inside -p wcontainers-hypervisor --all-targets --
  -D warnings` and `nix develop -c cargo clippy -j 4 -p wcontainers-inside --target aarch64-unknown-none
  --bins --lib -- -D warnings`.

## N1 measurements (M5 Max, macOS 27.2, the floor test; load 55 to 92 over the runs)

- Bar cases, each run: 20 of 20 pass (five runs after the last fix).
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
Apache-2.0; fastrand 2.5.0 Apache-2.0 OR MIT (test only).

## Literals

- The two new crates were written under the rule: every number is derived (from `PAGE_SIZE`, field offsets,
  the crates' register fields) or one named constant. Marked: the register save and restore in
  floor/entry.rs writes register n at x + n words, the numbers being the registers' own (a comment there).
- The automatic check (phase-1-literals) has not landed (origin/phase-1-literals is still at 68013b31), so
  no baseline was made.

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

- N1, the build: the Nix shell links against the macOS 14.4 SDK, which does not declare Hypervisor.framework's
  GIC calls and its EL2 settings (macOS 15). The monitor's build script tells the linker to leave those nine
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
  on every guest access. Decision 4 puts this to the owner.

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
