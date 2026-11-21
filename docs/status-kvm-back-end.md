# KVM back end status

Plan: docs/plan-kvm-back-end.md. Code: crates/executor/implementation/hardware-virtualization (kvm.rs beside hvf.rs)
and hardware-virtualization-inside.

## Where things stand

- V1 (the KVM back end on kvm-ioctls, ARM64), run 1 (2026-11-21): kvm.rs is built and every executor test that
  runs on the Mac was run on the rented ARM64 machine's KVM, same programs and seeds. All pass except two:
  - kernel: fails at one assertion, a difference of the host processor, not of KVM (below, "For the owner").
  - speed: not run on Linux yet; its measurement is macOS's own (below, "Next").
  The Mac's suite passes with the changes (below, "Results"). Not complete: the two items above remain.

## Results (run 1)

KVM, rented ARM64 machine (Graviton4, Neoverse V2, Linux 7.0.0-1018-aws, 4 KiB pages), at the commits of run 1:
- floor: 21 cases and 1 measurement, 0 failed (with the AdjacentPages change below).
- exits: 4 programs, 0 failed, both modes.
- interrupts: 15309 forced interrupts over 18 functions (34 cases, seed 0x14057b7ef767814f), 3 look-aheads, 0 failed.
- native: 1215 forms, 16 drawn cases each (the default; 32562 words in all, seed 0x2545f4914f6cdd1d), 0 failing, after
  the FPSR change below; before it, MSR FPSR failed (word 0xd51b442a: float b0000083, expected 83).
- probe: passes as a test; its verdict for this host: 9 forms of 16 bytes or fewer differ, "this host would not pass"
  (15 larger ones differ, the Mac 44 larger and 0 smaller). A measurement of the processor, for the owner.
- host_branches: 34 cases over 36 variants, 0 with differing guest paths. kept, carry, the monitor's and the inside's
  lib tests (10 and 3): pass.
- busybox: 56 commands gave the reference's output, exit code and files (seed 0x6b65726e); the reference is that
  machine's Docker.
- kernel: fails at "blocks: memory fault counted alike" (8 natively, 10 on the WebAssembly side). With that one
  assertion turned off in the machine's copy only (a diagnostic, never committed), everything after it passes: blocks
  gives that machine's reference's output and exit code, mappings too, and the stops at every counted event are
  alike across look-aheads with the Mac's hashes (bench 177072 stops, 10937141521840235265; calls 3004,
  18260131204484228776; jumptables 5484, 17339527552300285948).
Mac (Hypervisor.framework, M5 Max, load 17 to 102): floor 21 + 1, 0 failed; exits; interrupts 15309, 0 failed; native
16 cases a form (32562 words, after the FPSR change), 0 failing; probe 0 forms of 16 bytes or fewer differ (44 larger, as before); kernel ran alike to the end; host_branches,
kept, carry, lib tests pass.

## What V1 built

- kvm.rs: `Hypervisor`, `VirtualMachine`, `VirtualCpu` and `Stopper` on kvm-ioctls; `Host` is it on Linux ARM64.
  - The machine: KVM_CREATE_VM, the counter offset set to the host's counter (so the guest's counter starts near 0, as
    on Hypervisor.framework), the SMCCC filter forwarding the doorbell's one function to the monitor, every virtual CPU
    made and initialised (preferred target), then the in-kernel GICv3 at the layout's addresses and set up. The timer's
    interrupt number is read from the first virtual CPU; the first raised is the first SPI (arm-gic's `IntId::spi(0)`).
  - map: one memory slot per call. give_back: `madvise(MADV_DONTNEED)`. raise: KVM_IRQ_LINE with an SPI.
  - run: one `VcpuFd::run`; the hypercall exit with the doorbell's function is `Exit::Doorbell`; EINTR with
    `immediate_exit` set is `Exit::Stopped` (the flag is cleared by an atomic swap); EINTR without it (another
    signal) runs again; every other exit is an error naming it.
  - Registers: KVM_GET/SET_ONE_REG, core registers by their offsets in `kvm_regs` (FPSR and FPCR are 32 bits there),
    system registers by their encodings (named constants in kvm.rs).
  - Stopping: the stopper owns its own shared mapping of the CPU's run page, writes `immediate_exit` = 1 atomically
    (release), then `tgkill`s the CPU's thread with SIGRTMIN, whose handler does nothing and is installed once with
    SA_RESTART (so the signal never makes another call of that thread fail with EINTR). A dropped CPU zeroes its
    thread, and a stop after that is refused.
- Ours, not in the crates (for sending upstream to kvm-ioctls): KVM_ARM_SET_COUNTER_OFFSET (no wrapper; one raw ioctl
  through vmm-sys-util's `ioctl_iow_nr!` and `ioctl_with_ref`), and KVM_GET_DEVICE_ATTR on a virtual CPU (`VcpuFd` has
  set and has, not get). KVM has no register that reads the counter offset back, so the machine keeps the one it set.
- Shared changes for both back ends (nothing above the trait names a platform):
  - `Hypervisor::machine(cpus)`: KVM refuses a virtual CPU once its GIC is set up, so it makes them all at the start;
    Hypervisor.framework ignores the count.
  - The doorbell is smccc's `hvc32` (an `hvc #0`) with the inside's `link::DOORBELL_FUNCTION` in w0, a yielding OEM
    call: KVM forwards only the functions its filter names and answers every other `hvc` itself, so x0 must be ours.
    It was an `hvc #0` with whatever x0 held. The native cases' vectors keep the guest's x0 in TPIDR_EL1
    (`Register::InsideThread`) before the doorbell; the runner reads it back from there.
  - The tests run where the build script sets `host_hypervisor` (macOS, and Linux on ARM64). Linux sides: the counter
    and its frequency from CNTVCT_EL0 and CNTFRQ_EL0, the footprint from /proc/self/statm, no code signing.

## Dependencies added (licences confirmed in each crate's Cargo.toml)

- kvm-ioctls 0.25.1: Apache-2.0 OR MIT. kvm-bindings 0.14.2: Apache-2.0. vmm-sys-util 0.15.0: BSD-3-Clause (Linux ARM64
  only, for the monitor).
- arm-gic 0.11.0 (MIT OR Apache-2.0) for the monitor on Linux ARM64 (already the inside's).
- smccc 0.2.3 (MIT OR Apache-2.0) for the inside's doorbell.

## Measurements

- Floor test, KVM (load 7 to 11 of 96 cores): doorbell round trip 1592 and 1539 ns (two runs, 1000 round trips each);
  300 stops from another thread, decision to exit: median 3080 and 3147 ns, 99th 4270 and 5131, max 48515 and 19216;
  timer interrupt after its deadline median 711 and 707 ns; raised interrupt to the inside median 3545 and 3378 ns.
  Hypervisor.framework on the Mac with the new doorbell (load 16 to 29): round trip 905 ns, stops median 1042 ns.
- interrupts took 176 s on KVM (load 11) against 23 s on the Mac (load 20 to 29): not looked into; every register the
  runner reads or writes is one KVM_GET/SET_ONE_REG call there. Speed is for later.
- native 16 cases a form 34 s, busybox 45 s, kernel 49 s on KVM.

## Departures from the plan

- FPSR: the inside's thread status kept FPSR whole; on the Neoverse V2 a write keeps the bits above QC, which the
  architecture reads as zero, so the native output gave b0000083 where the WebAssembly output gives 83. The inside now
  keeps only FPSR's own bits (`FPSR_BITS`) when it takes the hardware's status (stop.rs); on the M5 that is the same.

- The floor's AdjacentPages case asked that two adjacent guest pages lie in one host page, which a 4 KiB host page
  never allows (the ARM64 machine has 4 KiB pages, the Mac 16 KiB). It now asks that they share a host page exactly
  when the host page is larger than a guest page; the permission checks are unchanged. On the Mac it asks the same
  as before.

## For the owner

- `dc cvau` at EL0 on a page that allows nothing, or is not mapped, runs on the Neoverse V2 (its kernel's reference
  agrees: the blocks program gives the reference's output natively), and faults on the M5, as the WebAssembly output
  does. So the kernel test's count of memory faults differs there (8 against 10). Which one the defined machine does
  is yours: routing `dc cvau` (decision 14's way) would make the native output the WebAssembly output's on every host.
  The test is left failing on that machine.
- The behaviour probe says the Neoverse V2 would not pass (9 forms of 16 bytes or fewer differ).
- To send upstream to kvm-ioctls: a wrapper for KVM_ARM_SET_COUNTER_OFFSET, and KVM_GET_DEVICE_ATTR on a vCPU.

## Next

- The speed test (tests/speed.rs, still `cfg(target_os = "macos")`) needs a Linux side before it runs on KVM: the
  whole process's cycles and instructions (macOS `proc_pid_rusage`; on Linux perf events opened at the child's start,
  counting every thread and the guest), the plain run's breakpoint handler (macOS's `ucontext_t`; Linux's
  `ucontext_t` has `uc_mcontext.regs` and `pc`), the code made visible after it is written (`sys_icache_invalidate`;
  on Linux `__clear_cache`), and the share on performance cores, which has no meaning on Graviton (report it absent,
  never a made-up figure). Then its bar's command on a quiet machine.
- The kernel test's blocks count on the Neoverse V2 waits on the owner (above).

## How to run

- Mac (Hypervisor.framework), from the checkout: `CARGO_TARGET_DIR=$PWD/target/kvm-back-end nix develop -c cargo test
  -j 4 -p wcontainers-executor-hardware-virtualization --test floor` (and the other tests by name; knobs as in
  docs/status-native.md "How to run").
- KVM: `arm-run <checkout> --name kvm-back-end -- '<the same cargo command with -j $ARM_JOBS>'`; prefix the command with
  the loop that removes empty crate folders other tracks leave (see the logs in cloud/logs/kvm-back-end).
