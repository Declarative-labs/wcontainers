# Native back end status

Plan: docs/native.md. Branch: phase-2-native. Brief: pyro-parked/wcontainers-scratch/orchestrator/brief-native.md.

## Where things stand

- N0 (the plan) is written: docs/native.md, run 1, 2026-11-18. No code in the repository.
- Next: the owner's decisions of plan section 10, then N1 (the floor on Hypervisor.framework). N1 needs
  nothing from other tracks. N2 needs the runtime track's R1 (the loop `no_std`) and A1 (toolchain targets),
  or the owner's word that this track makes them.

## What N0 checked, and how

Read: the brief to its end; docs/runtime.md sections 1 to 13; the code cited in the plan (every cited file
and line opened at 68013b31); distributed-everything design.md section 6 and the requirement names of
orchestrator/de-requirements-map.json; the cost rounds' headlines (costs-round-2.json,
progress-count-cost.json) and the round 3 results (costs-round-3/virtual-machine/results/mac-tests.jsonl,
mac-summary-main.json, costs-round-3/native-arm64/results/summary-main-r0.txt).

Experiments (files under pyro-parked/wcontainers-scratch/native/experiments):
- Kernel crate on bare metal. A copy of this branch on the Linux laptop (~/native-plan/wc), Rust 1.99.0 in
  ~/native-plan/rustup: `cargo build --locked -j 3 --release -p wcontainers-kernel --target
  aarch64-unknown-none` and the same for `x86_64-unknown-none` both succeed with no change
  (kvm-laptop.out). The plan's claim that the kernel goes inside unchanged rests on this.
- System registers at EL0 on this Mac (M5 Max, macOS 27.2, load 76 to 87): CNTFRQ_EL0 1 GHz, DCZID_EL0 4,
  CTR_EL0 traps (SIGILL); FPCR bits 0x3fbf07 outside the presented 0x07c00000 stick when written (fp.c,
  sysregs.out). `sysctl hw.optional.arm` lists 55 features, SME and AFP among them; the translator offers 4.
  Shows that counter, identity and FPCR accesses must be routed, never copied.
- Register naming in real ARM64 Linux code (regs.py, regs.out): static count over busybox, musl's loader,
  rg, xz, libpcre2, postgres, libcrypto and a Go program (3.7 million instructions): x18 1.3 per 1000, x15
  2.9, x14 3.9, x13 5.1; x26 to x28 13.7 to 22.4; the Go program names x27 58 per 1000. Static only;
  dynamic counts are N2's.
- KVM stop and in-guest timer on the Linux laptop (i7-8750H, Linux 7.3.4, load 3.4 to 3.8), with the round
  3 prototype vmrun.c built unchanged (kvm-laptop.out): 420 stops by a signal to the virtual CPU's thread,
  pc always inside the loop, count never back, median 11.8 µs from the decision to the registers read
  (9.9 µs with the signal masked outside `KVM_RUN`), 90th percentile about 4 ms; the local APIC timer at a
  100 µs period costs 7.8 µs a tick with 3.1 exits per tick.
- Licences, `cargo info` on 2026-11-18: kvm-ioctls 0.25.1 Apache-2.0 OR MIT; kvm-bindings 0.14.2
  Apache-2.0; applevisor 1.0.0 MIT OR Apache-2.0; hv 0.1.2 Apache-2.0; xhypervisor 0.3.0 MIT;
  aarch64-paging 0.12.3 MIT OR Apache-2.0; x86_64 0.16.0-rc.0 MIT/Apache-2.0; aarch64-cpu 12.0.0
  MIT/Apache-2.0; talc 5.1.1 MIT; buddy_system_allocator 0.13.0 MIT; iced-x86 1.22.0 MIT; dynasmrt 5.1.0
  MPL-2.0 (rejected); vm-memory 0.18.0 Apache-2.0 OR BSD-3-Clause (not needed); gimli 0.34.0 MIT OR
  Apache-2.0. None is a dependency yet.

Not checked, and named in the plan as such: execute-only EL0 pages and `hv_gic_set_spi` on
Hypervisor.framework (N1's first tests), `KVM_IRQFD` and the KVM ARM64 doorbell (no KVM ARM64 machine at
hand; N9), the store-exclusive behaviour under interrupts (N5).

## Self-check before handing over

Read the plan against the brief ruling by ruling and opened every cited line. Corrected on the way: the
entry mirror's reach (slots are 16 bytes with an absolute jump, so the guest's addresses must be below 2^45,
an owner's decision), a count limit with no position (cannot be honoured with no check at counted events:
R4, an owner's decision), EL2 on Hypervisor.framework (offered on newer chips, not used), the floor's
SCTLR, XCR0 and CR4 settings that keep native behaviour equal to what the WebAssembly output presents, and
the other project's borrowed views of kernel state (closed to a host program when the kernel is inside;
named for the owner).

## How to run

Nothing to run in the repository yet. The experiments: `python3 -I regs.py <ELF files>` on the Mac; the KVM
runs as in kvm-laptop.out from ~/native-plan/vm on the laptop.

## Departures from the plan

None yet.

## For the orchestrator

- The plan asks changes of other tracks (plan section 6.2): R1, R2, R3 of the runtime track; T1, T2, T3 of
  the translator track; X1 of the x86 track; K2 of the kernel track only if the owner agrees to an ARM64
  address space of 2^47.
- The laptop keeps ~/native-plan (1.3 GB: a Rust toolchain and the copied sources) for later milestones; its
  build output was deleted.
