# Kernel status

Plan: docs/phase-1-kernel.md. Branch: phase-1-kernel.

## Where things stand

K0 (skeleton) is done, including the fixes from the independent check of K0 (second run). K1 is next.

How to test everything: `nix develop -c cargo test -p wcontainers-kernel -p wcontainers-kernel-test`.
Known failures are `#[ignore]`d with their reason; see them with `-- --include-ignored`.
Needs Docker running; the reference uses the image alpine:3.22 (pulled on first use). Containers are
named `wc-kernel-reference-<pid>-<n>` and removed after each run.

## What exists

- crates/wcontainers-kernel (no_std with alloc). `Kernel::start` (first process from program bytes),
  `Kernel::next` -> `Next`, `Kernel::exit(Exit)`, `Kernel::event(Event)`, `Kernel::counters()`,
  `system_calls()` (every call with built or not), `system_call_name(n)`.
- Dispatch table (src/syscall/mod.rs): built at compile time from `syscall/list.rs`, which is generated
  from linux-raw-sys's ARM64 list (325 calls, numbers 0 to 469). The number is the low 32 bits of x8, as
  in Linux. Every call has an entry; unbuilt ones return ENOSYS, bump the counter "system call not
  implemented" (detail = number) and warn. Numbers Linux does not have do the same under "system call
  number unknown". Built: write, exit, exit_group.
- write: Linux's `access_ok` (48 bit user address limit, src/user.rs) before anything else, then the
  count is capped at MAX_RW_COUNT. fd 0 is the null device (as in `docker run` without -i), fds 1 and 2
  go to the host's output streams with the semantics of a write to an empty pipe: whole pages, and the
  first page that cannot be read in full ends the write (EFAULT if nothing was written).
- Loader (src/loader.rs): static ELF only, ET_EXEC or ET_DYN (placed at 2/3 of the address space). Maps
  segments as Linux's `elf_load` does: whole file pages (so the rest of a segment's last page shows the
  file's following bytes), the tail cleared only for a writable segment with zeroed data after its file
  part, and zeroed pages after that. Sets page permissions, sends "code is here", maps an 8 MiB
  read/write stack at the top, builds argc/argv/envp and an auxiliary vector. AT_SYSINFO_EHDR and
  AT_MINSIGSTKSZ are missing; each program start counts and warns "auxiliary vector entry missing"
  (detail = key). A program with PT_INTERP is refused with ENOEXEC, counted and warned.
- Faults, illegal instructions and breakpoints kill the process with SIGSEGV, SIGILL and SIGTRAP.
- Linux definitions (src/linux/): generated copies of linux-raw-sys 0.12.1 aarch64 general, errno and
  auxvec. Regenerate with `nix develop -c cargo run -p wcontainers-kernel-test --bin generate-linux`.
- crates/wcontainers-kernel-test: the stand-in CPU (src/standin.rs, unicorn-engine 2.1.5), the driver
  loop and test host (src/driver.rs), the reference runner (src/reference.rs, Docker), the guest builder
  (src/guest.rs, zig cc for aarch64-linux-musl), the generator (src/generate.rs), guest programs in
  guest/ (bare.h holds system calls and number printing for the ones with no C library).
- `StandIn::run` returns `Result<Exit, Unsupported>`: what the emulator cannot run as the reference's CPU
  does (an alignment fault QEMU raises, an unknown stop) is reported as `Unsupported`, never as a made-up
  exit, and `driver::run` gives it back as `Failure::StandIn`.
- `driver::Run::warnings_besides_start()` checks the two auxiliary vector warnings came once each and
  returns the rest; tests assert on that. Remove it when the vector is complete.

## Tests (last run 2026-11-15, all pass except the ignored known failures)

- hello: guest/hello.c writes to stdout and stderr and exits 3; same stdout, stderr and exit code as the
  reference, no warnings besides the start ones.
- write: guest/write.c, 18 cases against the reference: buffers running off the end of the program into
  unmapped memory, one byte past, a second page that faults, count 2^40, count -1, a range ending exactly
  at and one past the 48 bit limit, zero counts at 0, at the limit and past it, unmapped buffer, fd 0
  (unmapped buffer, huge count, -1), a closed fd, high bits in the fd and in x8.
- loader: guest/segments.c sums the bytes after each segment's file part to the page end; same as the
  reference (read-only and code tails hold file bytes, the writable tail is zero).
- faults: guest/fault.c dies by read of an unmapped address, write to read-only, executing a non-exec
  page, brk, udf; exit codes 139, 139, 139, 133, 132 and output match the reference.
- dispatch: every Linux call has an entry; an unbuilt call gives -38, one warning and one counter;
  numbers 1000 and -1 give -38 as in the reference (Docker's filter passes numbers above its highest
  call to the kernel) with an "unknown" warning; x8 = 2^32 + 64 is write, as in the reference.
- standin: each exit reason comes back with exact pc and registers; changing permissions of one page in
  a range keeps the pages around it; an unaligned exclusive load comes back as `Unsupported`.
- boundary: the kernel builds for wasm32-unknown-unknown; `cargo tree` of wcontainers-kernel and
  wcontainers-abi (wasm32 and all targets) has no unicorn, while the test crate's does; generated files are
  current.
- linux_layouts (generated): every copied structure has linux-raw-sys's size and alignment (aarch64 hosts).

Known failures (ignored, with the reason in the attribute):
- dispatch numbers_in_gaps_match_reference: 244 and 400 give EPERM (-1) in the reference (Docker's
  seccomp profile), ENOSYS here.
- align unaligned_exclusive_matches_reference: ldxr from an address 1 mod 8 runs on the reference's
  hardware (prints 0, exit 0); unicorn raises an alignment fault, so our run gives no outcome.
- align misaligned_stack_pointer_matches_reference: a load through sp when sp is 4 mod 16 is SIGBUS in
  the reference (exit 135); unicorn never checks sp alignment, so ours exits 0.

No random tests exist yet.

## Measurements

- Building unicorn-engine-sys from scratch with the rest of the test crate: 54 s (load about 20 to 35).
- Whole test suite after a build, second run: each test binary under 6 s; faults (6 Docker runs) 5.25 s,
  write 1.55 s, boundary 0.42 s with a warm wasm build (load 49.67 before, 54.96 after). Noisy; not a
  benchmark.

## Departures from the plan

- linux-raw-sys picks its bindings by the target it is compiled for, so on wasm32 it has no ARM64
  definitions and its C long would be 32 bits. The kernel therefore carries generated copies of the
  ARM64 modules, with every guest pointer, function pointer and usize field as u64. The copies come only
  from linux-raw-sys (nothing typed by hand); a test checks they are current and another checks every
  structure's size and alignment against linux-raw-sys itself on an ARM64 host. The one value not from
  linux-raw-sys is the 48 bit user address limit (`TASK_SIZE_MAX`), which is kernel configuration, not a
  header; the reference's kernel (7.0.14-linuxkit) has CONFIG_ARM64_VA_BITS=48.
- Docker's seccomp profile against rule 7: the plan says an unknown number returns "not implemented",
  and also that what the reference refuses we refuse the same way. In the reference, numbers inside the
  gaps of Linux's list, and calls the profile blocks, give EPERM; numbers above the profile's highest
  call give ENOSYS. K0 gives ENOSYS for all of them (counted, warned) and the gap case is a known-failure
  test. The profile should be modelled with the powers work (section 11), then gaps give EPERM.
- The reference's standard output and error are pipes that Docker drains at its own pace. For a write
  that faults part way and is not a whole number of pages, Linux's pipe write gives a different count
  depending on whether the pipe still holds earlier bytes (it first fills the last partial buffer), so
  the reference itself varies (seen: 4096 one run, 200 the next). The kernel always behaves as for an
  empty pipe, which is what the reference does when it is deterministic; the write test uses only cases
  where the answer does not depend on the reader.
- K0 has a small static-only program loader and `Kernel::start` takes the program's bytes, because a
  hello world must be started and there is no file system yet. K1 extends the loader (shared libraries,
  brk, vDSO, full auxiliary vector) and should make `start` take a path once files exist.
- The stand-in uses unicorn's "max" ARM64 model, the only one with single instruction atomics (which the
  guest is told it has). It also has features the guest is not told about (SVE, crypto), and runs at EL1,
  so an instruction outside the advertised set does not fault in the stand-in as it would on the
  translator. Two more differences from the reference's hardware, each a known-failure test: unicorn
  faults an exclusive access to an unaligned address (the reference's CPU runs it), and it never checks
  stack pointer alignment (the reference gives SIGBUS).
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
- None in the second run.

## Questions for the orchestrator (boundary)

- A load or store through a misaligned stack pointer is SIGBUS (BUS_ADRALN) on ARM64 Linux. `Exit` has no
  alignment fault, and the translator plan says only that unaligned access matches. Whether the
  translator checks sp alignment, and how it would report it, is open.

## Known issues

- Building the test crate prints a linker warning: unicorn-engine-sys's build script passes a lib64
  search directory that does not exist. Harmless.

## Next (K1, in order)

1. File descriptor table proper (now fd 0 as the null device and fds 1 and 2 as host streams), and the
   calls musl's start-up and stdio make: set_tid_address, ioctl(TIOCGWINSZ), writev, brk,
   mmap/munmap/mprotect with page protection and faults, read. Every call that takes a user range checks
   it with `user::access_ok` first, as Linux does.
2. Loader: PT_INTERP and shared libraries, brk start, vDSO page (signal return path comes in K2),
   AT_MINSIGSTKSZ and the rest of Linux's auxiliary vector (then drop `warnings_besides_start`), stack
   guard and growth.
3. Docker's default seccomp profile and powers (section 11), so blocked calls and gap numbers give EPERM.
4. The in-memory file systems and time calls.
5. Layer 1 (proptest sequences against a native runner in the reference), seeded, with a case-count
   environment variable.
6. Busybox single programs from an Alpine arm64 image, compared with the reference.
