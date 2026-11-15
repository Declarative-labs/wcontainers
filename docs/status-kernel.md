# Kernel status

Plan: docs/phase-1-kernel.md. Branch: phase-1-kernel.

## Where things stand

K0 (skeleton) is done. K1 is next.

How to test everything: `nix develop -c cargo test -p wcontainers-kernel -p wcontainers-kernel-test`.
Needs Docker running; the reference uses the image alpine:3.22 (pulled on first use). Containers are
named `wc-kernel-reference-<pid>-<n>` and removed after each run.

## What exists

- crates/wcontainers-kernel (no_std with alloc). `Kernel::start` (first process from program bytes),
  `Kernel::next` -> `Next`, `Kernel::exit(Exit)`, `Kernel::event(Event)`, `Kernel::counters()`,
  `system_calls()` (every call with built or not), `system_call_name(n)`.
- Dispatch table (src/syscall/mod.rs): built at compile time from `syscall/list.rs`, which is generated
  from linux-raw-sys's ARM64 list (325 calls, numbers 0 to 469). Every call has an entry; unbuilt ones
  return ENOSYS, bump the counter "system call not implemented" (detail = number) and warn. Numbers Linux
  does not have do the same under "system call number unknown". Built: write (fds 1 and 2 go to the host's
  output streams), exit, exit_group.
- Loader (src/loader.rs): static ELF only, ET_EXEC or ET_DYN (placed at 2/3 of the address space). Maps
  segments as Linux does (from the page start of the file part, rest of last page cleared), sets page
  permissions, sends "code is here", maps an 8 MiB read/write stack at the top, builds argc/argv/envp
  and an auxiliary vector (no vDSO, no AT_MINSIGSTKSZ, no rseq entries yet). A program with PT_INTERP is
  refused with ENOEXEC, counted and warned.
- Faults, illegal instructions and breakpoints kill the process with SIGSEGV, SIGILL and SIGTRAP.
- Linux definitions (src/linux/): generated copies of linux-raw-sys 0.12.1 aarch64 general, errno and
  auxvec. Regenerate with `nix develop -c cargo run -p wcontainers-kernel-test --bin generate-linux`.
- crates/wcontainers-kernel-test: the stand-in CPU (src/standin.rs, unicorn-engine 2.1.5), the driver
  loop and test host (src/driver.rs), the reference runner (src/reference.rs, Docker), the guest builder
  (src/guest.rs, zig cc for aarch64-linux-musl), the generator (src/generate.rs), guest programs in
  guest/.

## Tests (all pass, last run 2026-11-15)

- hello: guest/hello.c writes to stdout and stderr and exits 3; same stdout, stderr and exit code as the
  reference, no warnings.
- faults: guest/fault.c dies by read of an unmapped address, write to read-only, executing a non-exec
  page, brk, udf; exit codes 139, 139, 139, 133, 132 and output match the reference.
- dispatch: every Linux call has an entry; an unbuilt call gives -38, one warning and one counter; call
  number 1000 gives the same output as the reference (-38) and an "unknown" warning.
- standin: each exit reason comes back with exact pc (syscall: after svc; brk, fault, udf: at the
  instruction; stop: where it stopped), vector, flag and thread-pointer registers survive; changing the
  permissions of one page in the middle of a range keeps the pages around it.
- boundary: the kernel builds for wasm32-unknown-unknown; `cargo tree` of wcontainers-kernel and
  wcontainers-abi (wasm32 and all targets) has no unicorn, while the test crate's does; generated files are
  current.
- linux_layouts (generated): every copied structure has linux-raw-sys's size and alignment (aarch64 hosts).

No random tests exist yet.

## Measurements

- Building unicorn-engine-sys from scratch with the rest of the test crate: 54 s (load about 20 to 35).
- Whole test suite after a build: each test binary finished in under 2 s; faults (6 Docker runs) 1.46 s,
  boundary 0.40 s with a warm wasm build (load 23.86 at the start of that run). Noisy; not a benchmark.

## Departures from the plan

- linux-raw-sys picks its bindings by the target it is compiled for, so on wasm32 it has no ARM64
  definitions and its C long would be 32 bits. The kernel therefore carries generated copies of the
  ARM64 modules, with every guest pointer, function pointer and usize field as u64. The copies come only
  from linux-raw-sys (nothing typed by hand); a test checks they are current and another checks every
  structure's size and alignment against linux-raw-sys itself on an ARM64 host.
- K0 has a small static-only program loader and `Kernel::start` takes the program's bytes, because a
  hello world must be started and there is no file system yet. K1 extends the loader (shared libraries,
  brk, vDSO, full auxiliary vector) and should make `start` take a path once files exist.
- The stand-in uses unicorn's "max" ARM64 model, the only one with single instruction atomics (which the
  guest is told it has). It also has features the guest is not told about (SVE, crypto), and runs at EL1,
  so an instruction outside the advertised set does not fault in the stand-in as it would on the
  translator.
- Stand-in: memory_duplicate and the shared page calls answer NotSupported until K2. The stop flag is a
  unicorn time limit on each run (the slice); raising it "now" from outside comes with K3.
- A death by signal reports the plain signal in the wait status, without the core dump bit. Docker's exit
  code cannot show that bit; to be checked when wait exists (K2).
- The stack is a fixed 8 MiB mapping with no guard page and no growth (K1).
- Each counted event warns the host every time; the warning carries the count, so a host can thin them.

## Changes to the shared crate (wcontainers-abi)

- host.rs: `Host::terminal_output(bytes)` became `Host::output(stream, bytes)` with a new
  `Stream { Output, Error }` enum, exported from lib.rs. The reference's stdout and stderr are separate
  pipes and must be compared separately; with a terminal, everything comes on `Stream::Output`.
- host.rs: `CounterReading` gained `detail: u64`, matching `Warning::detail`, so there is one counter per
  reason and value (for example per system call number).

## Known issues

- Building the test crate prints a linker warning: unicorn-engine-sys's build script passes a lib64
  search directory that does not exist. Harmless.

## Next (K1, in order)

1. File descriptor table proper (now only fds 1 and 2 as host streams), and the calls musl's start-up
   and stdio make: set_tid_address, ioctl(TIOCGWINSZ), writev, brk, mmap/munmap/mprotect with page
   protection and faults, read.
2. Loader: PT_INTERP and shared libraries, brk start, vDSO page (signal return path comes in K2),
   AT_MINSIGSTKSZ and the rest of Linux's auxiliary vector, stack guard and growth.
3. The in-memory file systems and time calls.
4. Layer 1 (proptest sequences against a native runner in the reference), seeded, with a case-count
   environment variable.
5. Busybox single programs from an Alpine arm64 image, compared with the reference.
