# x86-64 native output: status

Plan: docs/x86-native-output.md. Brief: pyro-parked/wcontainers-scratch/orchestrator/relaunch-agents/
x86-native-output.js. Scratch: pyro-parked/wcontainers-scratch/x86-native-output (probes/, results/).

## Where things stand

- XN0 (the plan, checked before anything is built): run 1 (2026-11-23) wrote the plan and re-measured what it
  leans on (below). Waiting for its independent check. Nothing is built; no code of this track exists yet.
- XN1 and XN2: not started.

## Measured in XN0 run 1

All on the rented x86 machine (Xeon Platinum 8488C, Sapphire Rapids, 96 processors, Linux 7.0.0-1018-aws,
5-level paging, AT_HWCAP2 0x2 = user FSGSBASE, vsyscall execute-only, mmap_min_addr 65536), probes copied
there with `x86-run --ssh`, timings pinned to processor 5. Load 1.3 to 7.2.
- Partial writes behind a fault (probes/partial86.c, the native track's laptop probe unchanged): `mov` 8 bytes
  21 cases, `movdqu` 45, `vmovdqu` 32 bytes 93, `fxsave64` 93, `xsave64` 24: none wrote a byte in the first
  page; `movdqu` and `vmovdqu` loads 45 and 93 cases: no register changed. Same as the i7-8750H.
- Static shapes (probes/shapes86.py, results/shapes86.out; 13 files, 19.2 million instructions): RIP-relative
  `lea` 24.5 and other 22.0 per 1000; `fs:` 1.22 (glibc 12.7); `gs:` 0.00; rep strings 0.22; r8..r15 named per
  1000: r10 13.9, r15 15.0, r11 17.1, r9 19.1, r14 21.3, r13 21.8, r8 28.6, r12 30.8 (the Go daemon is 10.6 of the
  19.2 million and reverses r12..r15); memory operands where the flags may be live 20.9%.
- Return lookups (probes/returns.S and returns.c, fib(30), 3 runs of 21 alternating rounds, `rdtsc`;
  results/returns.out at load 1.4 to 2.8, returns2.out at load 7.2): against plain call/ret, one-level mirror
  1.05 to 1.13, two-level mirror 1.120 to 1.133, cache with `cmp` (flags clobbered) 1.17 to 1.39, flag-free
  cache 1.52 to 1.62. The two-level figure has no top-bits bound (the plan adds it at XN1, unmeasured).

## Departures from docs/native.md (the ARM64-first plan's x86 parts)

- The thread area on FS and the guest's FS and GS bases kept in the register area (native.md: guest FS in the
  hardware, thread page on GS), so that one layout serves the machine, P and I, where I needs GS.
- A stop inside a rep string resumes at an out-of-line twin of the instruction instead of a DR0 breakpoint,
  which P and I cannot set without perf events.
- Jumps through a register and returns through a two-level mirror, not a compared cache (measured above).
- No Split route or page test for x86: the architecture restores the state before a faulting instruction
  (Intel SDM volume 3, 6.5), measured again above.
- The XSAVE family goes to the inside (native.md copied it between two float calls): the host's XCR0 sizes the
  image (11,008 bytes here against the guest's 1,088).
- Three withheld registers, not two (the mirror needs a second scratch).

## Coordination with other tracks (read theirs at the start of every run)

- native-without-virtual-machine (docs/status-native-without-virtual-machine.md, when it exists): XN1's tests
  need an executor that runs a translation; the first one, P's W1 or the KVM x86 floor, is used. What the native
  output needs of each way is plan section 2 and 3.6: the thread area at an address place reached through FS,
  its read-only part really read-only to guest code (in P every writable page of the guest's host process is
  writable by the guest and its sibling threads, the inside's stack included), signals on an alternate stack,
  the call into the inside saving what it touches, `membarrier` for links, and the host's vdso and vvar.
- x86 and translator tracks: the rep string count (plan 3.6 and 9) changes the WebAssembly output's count if
  ruled; shared code changes (plan section 9) go in small commits that keep ARM64's suites passing.
- instruction design: the x86-64 `ProcessorArchitecture` and precise instruction sets do not exist yet; the
  route of undefined-result forms takes the host's match from them once they do.

## For the owner

- How a rep string instruction counts (plan 3.6): today the WebAssembly output counts passes, which depend on
  data; proposed: its elements, as code in plan section 9.
- Split-lock detection on cloud hosts (plan 3.6, 5): a named difference in P and I, or the inside completes the
  access.
- Decision 14 for x86 (plan 10).
- The fixed thread-area layout costing `fs:` operands one load (plan 2).

## Licences

No dependency added. XN1 adds iced-x86's `encoder` feature (iced-x86 1.22.0, MIT, already a dependency).

## Security

- Unsafe code: none in this track; none planned (the translator and wcontainers-x86 keep the workspace ban;
  emitting code into a byte buffer needs none).
- Hostile-guest points the plan answers: every host address translated code jumps to is read from our tables
  for any guest value (the mirror, plan 3.3); prefetches are never copied; `xsave` never runs with the guest's
  mask on the host's XCR0; 32-bit code is never entered. Points it hands to P: writable pages of the guest's
  host process are the guest's.

## How to run the probes

`tar cf - <probes> | orchestrator/cloud/x86-run --ssh 'mkdir -p /work/probes/xno && cd /work/probes/xno && tar xf -
&& gcc -O2 -o returns returns.c returns.S && taskset -c 5 ./returns 30 21'`; `python3 -I shapes86.py <ELF files>`.
