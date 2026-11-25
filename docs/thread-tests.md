# The thread tests: a design for the owner's review

Nothing here is built. The owner asked to go through this design before the suite exists (2026-11-23). Everything
else of the threads plan proceeds without it; T1 and T2 bar only on fixed tests, and the search below is what makes
their correctness trustworthy.

## 1. What is controlled

A run has no input the suite does not choose. The guest's clock, random numbers and task ids come from the
system (the seeded test policy); the guest's instructions run on the simulation executor or a real one; the
order in which guest threads, runners and the kernel's locks move is the scheduler's (section 2). Two runs with
the same choices are the same run, step for step, so a failure is a list of choices that replays it.

## 2. The deterministic scheduler

- Each runner keeps its own host thread, as in production, but only one host thread moves at a time: at every
  **gate** it stops and waits for the scheduler's token. The scheduler, on the test's own thread, picks who moves
  next. This is the shape of CHESS (Musuvathi and Qadeer, 2008) and of the shuttle crate.
- Gates: a guest thread's step (a budget of instructions, fixed per test, run by the executor's `execute`
  future), every trap into the kernel, every take and release of a kernel lock, every wake of a parked runner,
  every `interrupt`. The lock gates exist only in a test build of the kernel (a feature of its implementation;
  no interface changes).
- A choice is "which waiting gate moves next". A schedule is the list of choices; it is printed with every
  failure and replays it.
- Written against `Kernel`, `Runner` and `Executor` alone, so the same suite runs every kernel build and every
  executor that can be gated (the simulation executor first, then the unicorn stand-in and wasmtime).

## 3. The bounded exhaustive search

- For small programs (two or three threads, a few hundred gates), every schedule with at most **two preemptions**
  (a switch away from a thread that could have gone on) is run. Musuvathi and Qadeer (PLDI 2007) report that
  the faults they found needed two or fewer (from memory, not rechecked here). The bound and the count of schedules run are printed per test.
- Schedules that differ only in the order of two gates that touch nothing in common (two guest steps of
  different memories) are run once (a sleep set). The reduction is checked by running both forms of a sample
  and comparing results.
- Programs: the plan's section 12 list (locks, condition variables, atomics, thread-local storage, signals to
  threads, fork from a threaded process, exec and exit with threads running, code written by one thread and run
  by another), each cut to the smallest size that still shows its case.

## 4. Beyond the bound: random schedules with even coverage

His requirement: no kind of case may grow exponentially rarer as the sample grows, so that more samples always
find more. Uniform random choice at each gate fails it: a schedule with d preemptions at chosen places becomes
rarer as programs grow.

- Each schedule is drawn **for a stratum**, and strata take turns. A stratum is a combination of: preemption
  depth d (0 to 6), the kind of gate the preemptions land on (guest step, trap, lock, wake, interrupt), the
  number of runners (1, 2, 4, 8), and one shape (late arrival of a thread, a long run on one runner, a rare
  system call, a signal mid-call, a fork or exec while siblings run).
- Inside a stratum the d preemption points are placed by PCT (Burckhardt et al., 2010): random priorities and d
  random change points, which finds any fault of depth d with probability at least 1/(n k^(d-1)) for n threads
  and k steps, the same for every fault of that depth.
- **The evenness signal, measured and printed**: per stratum, the count of schedules run and of distinct
  schedules (by hash of the choice list), at 1,000, 10,000 and 100,000 samples. Bar: the largest stratum's count
  is at most twice the smallest's at every size, and distinct counts keep growing in every stratum that has room.
  A stratum whose distinct count stops growing is exhausted and says so; it is not oversampled.

## 5. What every schedule checks

- The kernel never panics and never hangs (a step budget per test); every failure is a valid error number.
- No runner is idle while a thread it may run is ready; every ready thread runs within a bound of slices
  (fairness), including a thread pinned to a busy runner and a `Serial` task beside `Parallel` overlappers.
- No wake is lost; every futex, pipe and signal outcome is one Linux allows.
- For `Serial` processes: the result equals some one-at-a-time order (a program that updates shared memory with
  no locking gives one of the serial answers).
- The scheduling rule's count "Serial task handed out beside an overlapper" stays at zero.

## 6. Register isolation, fuzzed

- Each guest thread fills every register group it is offered with values made from (its thread id, the seed,
  the register's index), then at gates other threads, of the same container and of another container, run and
  do the same; every thread reads back exactly its own, and a thread that never used a group reads zeros.
- Groups: ARM64 general, FP/SIMD, SVE (Z, P, FFR at the machine's vector length), SME (ZA, ZT0, streaming mode),
  flags, thread pointer; x86 general, x87, MMX, SSE, AVX, AVX-512 with mask registers, AMX tiles, flags, FS and GS
  base. Every executor form, WebAssembly included.
- Strata: group, kind of gate, and whether the thread in between was of the same container or another (the second
  guards isolation between containers and is weighted equally, never left to chance).
- A test build also reads the registers from outside at the moment a thread is about to run and requires its own
  values or zero: a speculative leak shows only as timing, so the rule itself is asserted.

## 7. Hostile guests

- Random system calls with random arguments from one process, and from two interleaved by the scheduler.
  Pointers are drawn by stratum: null, unmapped, mapped without the permission, straddling a mapping's end, high
  addresses, valid; lengths: zero, small, page edges, huge. Generated with proptest from the kernel's call table.
- Checks: no panic or hang; the right error for a bad pointer or length, with nothing touched outside the guest's
  own permitted memory; a victim run beside the attacker equals the victim run alone (memory, files, signals);
  memory handed to a process reads zero; limits (memory, descriptors, tasks) hold.
- Double fetch: a sibling writes a buffer between any two of the kernel's reads of it (a gate at each read);
  the kernel never acts on two different values.

## 8. Guest-memory views

Many guest threads write the same bytes while the kernel does I/O into them; the result is one the hardware
allows, and nothing outside the range is touched. This waits on the same review as any view that needs `unsafe`
to be made (his ruling of 2026-11-23); no agent adds that `unsafe` or these tests before it.

## 9. What it needs, and where it lives

- No interface change. A test-build feature of the kernel for the lock gates; the simulation executor's
  `execute` future stopping at each budget; the seeded test policy as the system.
- One new crate made with `cargo reviewable new` beside the kernel's conformance suite, owned by the threads
  track. Possible dependencies, each to vet: proptest (already used), shuttle (Apache-2.0) or loom (MIT) only if
  their schedulers can drive our own gates; otherwise none.
- Runs on the rented machines, cut into pieces that each write their counts to scratch.

## 10. Questions for the owner, one line each

1. Two preemptions as the exhaustive bound, raised per test where a program is small enough: yes?
2. The strata of section 4, and "largest at most twice the smallest at every sample size" as the evenness bar: right?
3. Register isolation weighted to switches between containers equally with switches inside one: right?
4. Lock gates as a test-build feature inside the kernel implementation (compiled out otherwise): acceptable?
5. The guest-memory view tests of section 8 reviewed together with the view itself, as you said: still so?
