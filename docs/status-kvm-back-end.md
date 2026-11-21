# KVM back end status

Plan: docs/plan-kvm-back-end.md. Code: crates/executor/implementation/hardware-virtualization (kvm.rs beside hvf.rs)
and hardware-virtualization-inside.

## Where things stand

- V1 (the KVM back end on kvm-ioctls, ARM64), runs 1 and 2 (2026-11-21): kvm.rs is built and every executor test
  that runs on the Mac now runs on the rented ARM64 machine's KVM too, same programs and seeds. All pass there
  except two, both from the host processor, not from KVM (below, "For the owner"):
  - kernel: one assertion, the blocks program's count of memory faults (`dc cvau`).
  - speed: the compute part of the bar fails on the Neoverse V2 (+24.12% against +21.2%); the system-call part
    passes; the untranslated run (the virtual machine without translation) costs about 1%.
  The Mac's suite passes with the changes (below, "Results"). Not complete: both wait on the owner's ruling.

## Results (run 2)

- Run 2 gave the speed test its Linux side (perf events through perf-event2 for the whole process's cycles and
  instructions, Linux's `ucontext_t` for the plain run's breakpoints, `__clear_cache`, the share on performance cores
  reported absent) and builds the register count wherever the speed test builds.
- Speed bar on KVM, the test's own command (`cargo test -p wcontainers-executor-hardware-virtualization --test
  speed`, 15 pairs, look-ahead 256, seed 1, 2000 resamples; Graviton4, load 1.4 at the start to 4.1 at the end of 96
  cores, two other tracks' jobs running; log scratch kvm-back-end/speed-bar.log, pairs speed-raw.jsonl):
  compute part +24.12% [+23.75 .. +24.47], FAILS against +21.2%. System-call part: a call 497.8 ns, the kernel's
  share 423.5 ns, the back end's 74.3 ns [72.7 .. 79.5], PASSES against 100 ns. Each program, native against plain,
  cycles (instructions), untranslated cycles: fib +41.6 (+69.6) +1.2, sieve +5.7 (+31.1) +0.1, hash +2.8 (+25.0)
  +0.2, sort +40.4 (+166.6) +2.3, interpret +36.8 (+85.3) +0.6, matrix +40.9 (+26.4) +7.8, walk +2.0 (+117.0) +0.1,
  search +32.6 (+21.9) +0.8, calls +23.1 (+162.7) +1.3. The Mac's last bar (docs/status-native.md, N4 run 25):
  +20.09%, with fib +28.0, sieve +18.5, sort +26.6, interpret +0.2, walk +17.9 (so the cores differ by workload).
- Short speed run on the Mac after the change (fib and calls, 1 pair, load 67): runs as before, the share on
  performance cores printed as before.
- The whole suite again at run 2's code. KVM (Graviton4, load 7 to 24, scratch kvm-back-end/suite-run2.log):
  registers, floor (21 + 1, 0 failed), exits, interrupts (15309, 0 failed, 176 s), native (32562 words, 0 failing),
  probe, host_branches, kept, carry, busybox (56 of 56), lib tests (10 and 3) pass; kernel fails at the one known
  assertion. Mac (load 21 to 40, mac-suite-run2.log): registers, floor, exits, interrupts, native, probe (0 forms of 16
  bytes or fewer differ), host_branches, kept, carry, kernel (ran alike to the end), lib tests (12 and 3) pass.
  Clippy with warnings denied on both crates, on both hosts; the literal check has nothing in this track's files.
- A mistake of run 2, set right: commit 90e1d39e took the library-swaps track's uncommitted edit of translate.rs
  (its item L3, tempfile) with it, and 5296c9c4, meant to take it back, committed two other tracks' staged crate
  renames from the shared index; 93340ae9 takes those back out. translate.rs is again as at cc307a0d, the other
  tracks' edits and staged renames are as they left them in the checkout. Commits here go by path only.

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
- perf-event2 0.7.4 (MIT OR Apache-2.0), a dev-dependency on Linux only, for the speed test's counts; it brings
  perf-event-open-sys2 5.0.6, perf-event-data 0.1.8 and c-enum 0.2.3 (each MIT OR Apache-2.0) and memoffset (MIT).

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

- The speed test on Linux opens its counters at the child's `main`, so its counts leave out the dynamic loader's
  start, which macOS's `proc_pid_rusage` counts; every side is the same binary, so each side leaves out the same
  work. The counters count every thread made after them, at every level (the kernel's work for the process and its
  guest included), and a count that did not run all its enabled time fails the run rather than being scaled.
- The speed test's children drop the loader's variables of their system: `DYLD_` on macOS, `LD_` on Linux.

## For the owner

- `dc cvau` at EL0 on a page that allows nothing, or is not mapped, runs on the Neoverse V2 (its kernel's reference
  agrees: the blocks program gives the reference's output natively), and faults on the M5, as the WebAssembly output
  does. So the kernel test's count of memory faults differs there (8 against 10). Which one the defined machine does
  is yours: routing `dc cvau` (decision 14's way) would make the native output the WebAssembly output's on every host.
  The test is left failing on that machine.
- The speed bar's compute part fails on the Neoverse V2, +24.12% [+23.75 .. +24.47] against +21.2% (the Mac's last
  bar +20.09%). It is the translated code on that core, not KVM: the untranslated run in the same machine costs about
  1% (matrix 7.8%), and the workloads that cost more there are others than on the M5 (interpret +36.8% against
  +0.2%, fib +41.6% against +28.0%; walk +2.0% against +17.9%). Whether the bar holds for every ARM64 host or for the
  M5 only is yours; the test is left failing on that machine. Speed is for later, by your priorities of today.
- The speed test needs `kernel.perf_event_paranoid` at most 1 to count the kernel's work (Ubuntu's default is 4,
  which refuses every count); I set it to 1 on the rented ARM64 machine with sudo for run 2. It does not survive a
  new machine: it belongs in cloud/setup-machine.sh, which is the orchestrator's.
- The behaviour probe says the Neoverse V2 would not pass (9 forms of 16 bytes or fewer differ).
- To send upstream to kvm-ioctls: a wrapper for KVM_ARM_SET_COUNTER_OFFSET, and KVM_GET_DEVICE_ATTR on a vCPU.

## Next

- Both remaining failures on the Neoverse V2 (the kernel test's blocks count, the speed bar's compute part) wait on
  the owner (above). Nothing of V1 is left to build otherwise.
- Later, for speed: the interrupts test takes 176 s on KVM against 23 s on the Mac (not looked into).

## How to run

- Mac (Hypervisor.framework), from the checkout: `CARGO_TARGET_DIR=$PWD/target/kvm-back-end nix develop -c cargo test
  -j 4 -p wcontainers-executor-hardware-virtualization --test floor` (and the other tests by name; knobs as in
  docs/status-native.md "How to run").
- KVM: `arm-run <checkout> --name kvm-back-end -- '<the same cargo command with -j $ARM_JOBS>'`; prefix the command with
  the loop that removes empty crate folders other tracks leave (see the logs in cloud/logs/kvm-back-end).
- The speed bar on KVM: first `arm-run --ssh 'sudo sysctl -w kernel.perf_event_paranoid=1'`, then `arm-run ... --
  'cargo test -j $ARM_JOBS -p wcontainers-executor-hardware-virtualization --test speed'` (knobs as on the Mac;
  `WCONTAINERS_SPEED_RAW` for the pairs file, under target/ there, fetched with `arm-run --fetch`).
