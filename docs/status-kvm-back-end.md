# KVM back end status

Plan: docs/plan-kvm-back-end.md. Code: crates/executor/implementation/hardware-virtualization (kvm.rs beside hvf.rs)
and hardware-virtualization-inside.

## Where things stand

- V1 (the KVM back end on kvm-ioctls, ARM64), run 1 (2026-11-21): kvm.rs is written and runs the floor test on the rented
  ARM64 machine's KVM. Partly done: see "Next".

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

- Floor test, KVM, rented ARM64 machine (Graviton4, Linux 7.0.0-1018-aws, load 10 to 11 of 96 cores), first run:
  doorbell round trip 1592 ns (1000 round trips); 300 stops from another thread, decision to exit: min 2861 ns, median
  3080, 90th 3387, 99th 4270, max 48515; timer interrupt after its deadline median 711 ns, max 3502; raised interrupt to
  the inside median 3545 ns, max 8821. Hypervisor.framework on the Mac with the new doorbell (load 16 to 29): round trip
  905 ns, stops median 1042 ns.

## Departures from the plan

- The floor's AdjacentPages case asked that two adjacent guest pages lie in one host page, which a 4 KiB host page
  never allows (the ARM64 machine has 4 KiB pages, the Mac 16 KiB). It now asks that they share a host page exactly
  when the host page is larger than a guest page; the permission checks are unchanged. On the Mac it asks the same
  as before.

## Next

- Run the other tests on KVM: exits, interrupts, probe, native (the bar's case count), kernel, busybox, host_branches,
  kept, carry, the lib tests; and the whole suite on the Mac again.
- The speed test is macOS's own (cycles from `proc_pid_rusage`, the plain run's breakpoints by macOS's signal context):
  it needs a Linux side (perf events) before it can run on KVM.

## How to run

- Mac (Hypervisor.framework), from the checkout: `CARGO_TARGET_DIR=$PWD/target/kvm-back-end nix develop -c cargo test
  -j 4 -p wcontainers-executor-hardware-virtualization --test floor` (and the other tests by name; knobs as in
  docs/status-native.md "How to run").
- KVM: `arm-run <checkout> --name kvm-back-end -- '<the same cargo command with -j $ARM_JOBS>'`; prefix the command with
  the loop that removes empty crate folders other tracks leave (see the logs in cloud/logs/kvm-back-end).
