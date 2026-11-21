# The KVM back end: plan

The native executor's monitor (crates/executor/implementation/hardware-virtualization) has four traits of its own,
`Hypervisor`, `VirtualMachine`, `VirtualCpu` and `Stopper`, and one implementation of them, on Hypervisor.framework
(hvf.rs). This track adds the second, on Linux KVM, for ARM64 hosts first, on the rust-vmm crates docs/native.md
section 3.6 names: kvm-ioctls 0.25.1 (Apache-2.0 OR MIT), kvm-bindings 0.14.2 (Apache-2.0) and vmm-sys-util 0.15.0
(BSD-3-Clause). The audit (orchestrator/library-audit/RESULT.txt, "buildOnALibrary" item 1) is the source.

## V1: the KVM back end on kvm-ioctls (ARM64)

kvm.rs beside hvf.rs, built on Linux ARM64 only; `Host` is it there. Each trait method, and what it rests on:
- `Hypervisor::machine`: `Kvm::new`, `create_vm`; the in-kernel GICv3 (`create_device`, its distributor and
  redistributor addresses at the layout's `GIC_DISTRIBUTOR` and `GIC_REDISTRIBUTORS`, as Hypervisor.framework's);
  the doorbell's hypercall forwarded to the monitor (`set_device_attr`, KVM's SMCCC filter); the counter offset.
- `VirtualMachine::map`: `set_user_memory_region`, one slot per call. `give_back`: `madvise(MADV_DONTNEED)` on the
  range, after which private anonymous memory reads as zeros and KVM drops its own mappings through its MMU
  notifier. `raise`: `set_irq_line` with an SPI. `interrupts`: the vCPU's virtual timer interrupt as KVM reports
  it, and the first SPI.
- `VirtualCpu::run`: one `VcpuFd::run` (one KVM_RUN), which borrows the shared run page; no copy, no allocation.
  `get`/`set`/`vector`/`set_vector`: `get_one_reg`/`set_one_reg` on the core registers and the system registers.
- What the crates do not give, and is ours:
  1. Setting the counter offset: KVM_ARM_SET_COUNTER_OFFSET has no wrapper; one raw ioctl through vmm-sys-util's
     ioctl helpers. To send upstream to kvm-ioctls.
  2. Reading it back: KVM has no register for it, so the machine sets it once, at its making, and keeps it.
  3. Reading the vCPU's timer interrupt: `VcpuFd` has no KVM_GET_DEVICE_ATTR; one raw ioctl, also for upstream.
  4. Stopping a vCPU from another thread: the stopper writes the run page's `immediate_exit` atomically through
     its own mapping of the vCPU's run page, then sends a signal to the vCPU's thread. KVM_RUN reads the flag
     when it starts and the signal ends a run in progress, so a stop is never lost; the run that comes back
     with EINTR clears the flag and gives `Exit::Stopped`.

Two shapes KVM needs, settled for both back ends so nothing above the trait names a platform:
- KVM refuses a vCPU once the GIC is set up, and sets it up at the first run, while Hypervisor.framework makes one
  at any time: `Hypervisor::machine` takes how many virtual CPUs the machine has, and KVM makes them all then.
- KVM answers an `hvc` itself unless its x0 is a function the SMCCC filter forwards; Hypervisor.framework answers
  `hvc #0` itself when x0 reads as a fast call. The doorbell becomes one form for both: `hvc #1` with x0 holding
  the doorbell's function number (the inside's link module defines both), so x0 is the doorbell's, never what
  the inside happened to hold. The native cases' vectors save the guest's x0 in TPIDR_EL1 first.

Tests: the executor's tests run on both hosts (a build-script cfg says which hosts have a hypervisor). Their
macOS-only parts get a Linux side: the counter and its frequency (the host's own counter registers), the process
footprint (/proc/self/statm), no code signing.

Pass bar: every executor test that passes on the macOS back end passes on KVM on the rented ARM64 machine with the
same guest programs and seeds, and the macOS back end's behaviour does not change (its tests pass on the Mac).

Order: (1) the doorbell's form and the machine's CPU count, on the Mac, its tests passing; (2) kvm.rs, the build
cfg and the tests' Linux sides, the floor test first on the ARM64 machine; (3) the other tests there; (4) the speed
test, whose measurement is macOS's (`proc_pid_rusage` cycles) and needs a Linux one (perf events).
