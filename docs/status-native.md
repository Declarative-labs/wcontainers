# Native back end status

Plan: docs/native.md. Branch: phase-2-native. Brief: pyro-parked/wcontainers-scratch/orchestrator/brief-native.md.

## Where things stand

- N0 (the plan) is written (run 1) and revised (run 2, 2026-11-18) to settle all 17 findings of the
  independent check. No code in the repository.
- Next: the owner's decisions of plan section 10, then N1 (the floor on Hypervisor.framework). N1 needs
  nothing from other tracks. N2 needs the runtime track's R1 (the loop `no_std`) and A1 (toolchain
  targets), or the owner's word that this track makes them. N4 needs decisions 1 and 10 and the changes they
  bring (R2, R3, and R4 only under option A).
- The magic-number sweep the brief asks for: this track owns no crate yet (only docs/native.md and this
  file), so there was nothing to sweep. The plan says every number it names is one named constant in code.

## What run 2 changed in the plan, finding by finding

- ARM64 jump through a register: new sequence `ubfx; cbnz; ubfiz; bfxil; eor; br` ignores the tag byte (bit
  55 set goes to `bad`, reported where `untag` puts it) and keeps the target's low two bits, so a misaligned
  target raises a PC alignment fault the floor reports as `TrapKind::Alignment`. Floor sets TCR_EL1.TBI0 = 1.
- x86-64 GS base: the guest's GS base is kept in the thread block; `rdgsbase`, `wrgsbase`, `mov gs`, `pop gs`,
  segment register writes and `sgdt`, `sidt`, `sldt`, `str`, `smsw`, `lar`, `lsl`, `verr`, `verw` are routed
  to the inside with the x86 plan's answers; CR4.FSGSBASE set, CR4.UMIP where offered (backstop only).
- Routing proof: a static test now classes every hook and control (hardware, WebAssembly bookkeeping, kept,
  answered by the inside), so the `Float` hook is copied correctly; plus a differential test running every
  form natively and on WebAssembly from the same seeded state, which catches fixed-answer forms.
- Call stub and roll-back: every host sequence has one commit point; before it roll back, after it roll
  forward with a closed set of fix-ups (slot address to guest address, store-exclusive status). The call's
  x30 is never lost; the store exclusive commits once.
- Count-limited turns: decision 10 now offers A (R4) and B (checks at counted events only for containers
  whose policy gives count limits); recommended B. N4 waits for the ruling; R3's contradiction removed; N3
  and N10 have checkable bars.
- Runtime interface: section 6.2 lists the interface changes (BackEnd without Execution, the container
  taking a policy and services, `services()` for `outside()`, the stop handle from the back end, Idle still
  returned to the host); decision 1.
- One virtual machine per host process holding all its containers (Hypervisor.framework allows one per
  process), a pool of virtual CPUs, per-container heaps at fixed inside addresses so saves move between
  processes; decision 12.
- Figures corrected: res3 is +0.3% geometric mean (+2.7% sort); the `lea` count is cited as round 2's form E,
  not "cheapest"; the 320 stops are in costs-round-2.json; round 3's count forms are +3.8% to +18.4% wall;
  decision 9's reason is the kernel's top-down search.
- Base words removed: a loaded translation has its guest addresses written at load, so execute-only code
  reads no data; section 7.2 names what of ours the guest's level can reach, per platform, and why replay
  does not rest on it.
- LSE2: atomic, exclusive and acquire/release forms get an alignment test (route "Aligned"); the probe
  refuses native runs on a host lacking the offer, LSE or the presented DCZID block size.
- Mirror per process: per-process TTBR1 top tables, non-global mirror entries, mirror frames shared at fork
  and copied before a write; leaving a translation is per process.
- Replay check compares against the running thread's own stop word, so shared code stops only the target;
  section 4 states the property every count definition must keep (no cycle without a counted event) and a
  test for it.
- Decision 4 no longer offers protection keys.
- Turn ends on KVM x86-64 leave the machine (3.1 exits a tick measured); named as a cost.
- Slow paths named with counters (plan section 3.7); x86 `rep` is finished by the hardware (DR0 breakpoint
  after it), not in software.
- Other project: K3 (extract and merge a process closure) named; N10 carries it and moving between back ends.
- `x86_64` crate: 0.15.5, the newest stable.

## What N0 checked, and how

Read: the brief to its end; docs/runtime.md sections 1 to 13; the code cited in the plan (every cited file
and line opened at 68013b31; run 2 opened the new citations too); the x86 plan at origin/phase-2-x86
96c49dbb, sections 5.5 and the baseline list; distributed-everything design.md section 6 and the
requirement names of orchestrator/de-requirements-map.json; costs-round-2.json, progress-count-cost.json
and the round 3 results (mac-tests.jsonl, mac-summary-main.json, summary-main-r0.txt).

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
- Branch targets and tags under Hypervisor.framework (run 2; n0r2/branch.c, a copy of check-n0/xo.c with
  new cases; branch.out; M5 Max, macOS 27.2, load 38 to 47): a `br` at EL0 to a target 1 or 2 bytes off gives
  EC 0x22 with FAR and ELR the target; with TCR_EL1.TBI0 = 1 a tagged `br` lands untagged (ELR after the
  `svc` 0x10000010) and a tagged load works, and with TBI0 = 0 both fault (EC 0x20 and 0x24, FSC 0x4).
  `hv_vm_get_max_vcpu_count` 64; `hv_vm_config_get_max_ipa_size` 40 bits, default 36.
- The jump sequence's arithmetic (run 2; n0r2/mirror.c, mirror.out, at EL0 on the Mac): aligned targets give
  their slot, misaligned ones keep their low bits, a tagged target gives the untagged slot, bit 55 or bits at
  or above 45 go to `bad`, and the call's x30 decodes to G + 4.
- The laptop (run 2): 39 physical address bits, no UMIP (/proc/cpuinfo).
- The independent check's experiments the plan cites (check-n0/): execute-only EL0 pages work and refuse
  loads and stores (xo.out cases 1 to 5); misaligned LSE2 accesses complete at EL0 in the machine (case 6);
  a second `hv_vm_create` in one process returns HV_BUSY (twovm.out).
- Licences, crates.io on 2026-11-18: kvm-ioctls 0.25.1 Apache-2.0 OR MIT; kvm-bindings 0.14.2 Apache-2.0;
  applevisor 1.0.0 MIT OR Apache-2.0; hv 0.1.2 Apache-2.0; xhypervisor 0.3.0 MIT; aarch64-paging 0.12.3 MIT
  OR Apache-2.0; x86_64 0.15.5 MIT/Apache-2.0 (newest stable; 0.16.0-rc.0 is a pre-release, not used);
  aarch64-cpu 12.0.0 MIT/Apache-2.0; talc 5.1.1 MIT; buddy_system_allocator 0.13.0 MIT; iced-x86 1.22.0 MIT;
  dynasmrt 5.1.0 MPL-2.0 (rejected); vm-memory 0.18.0 Apache-2.0 OR BSD-3-Clause (not needed); gimli 0.34.0
  MIT OR Apache-2.0. None is a dependency yet.

Not checked, and named in the plan as such: `hv_gic_set_spi` (N1), `KVM_IRQFD`, the DR0 breakpoint after a
`rep` and UMIP under the laptop's KVM (N7), the KVM ARM64 doorbell (no KVM ARM64 machine at hand; N9), the
store exclusive under interrupts (N5), the handoff cost of a turn to a pool thread (N4).

## How to run

Nothing to run in the repository yet. The experiments: `python3 -I regs.py <ELF files>`; in
experiments/n0r2, `clang -O1 -o branch branch.c -framework Hypervisor && codesign -s - --entitlements
hvf.entitlements -f branch && ./branch` and `clang -O1 -o mirror mirror.c && ./mirror`; the KVM runs as in
kvm-laptop.out from ~/native-plan/vm on the laptop.

## Departures from the plan

None yet.

## For the orchestrator

- The plan asks changes of other tracks (plan section 6.2): R1, R2, R3 of the runtime track (R3 also the
  kernel track's test crate); R4 only under option A of decision 10; T1, T2, T3 of the translator track; X1
  with the x86 track's meanings; K2 of the kernel track only if the owner agrees to 2^45; K3 of the kernel
  track for the other project.
- Twelve owner's decisions are in plan section 10; 1, 10 and 12 change interfaces or the ruling's letter.
- The laptop keeps ~/native-plan (1.3 GB: a Rust toolchain and the copied sources) for later milestones; its
  build output was deleted.
