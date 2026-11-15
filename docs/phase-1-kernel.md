# wcontainers, phase one: the kernel

Status: proposed plan, 2026-11-14. Not yet reviewed. Nothing is built yet.

Words used here:
- **guest**: the ARM64 Linux program being run.
- **host**: whatever runs our WebAssembly (wasmtime, a browser, later Cloudflare).
- **CPU side**: whatever runs the guest's instructions. In the finished product that is the translator's
  output. In this phase it is a stand-in (section 4).
- **hook**: a function the kernel calls to reach anything outside itself.
- **backend**: whatever stores the container's files, reached through one hook.
- **parked**: a thread that is waiting for something and is not running.
- **reference**: a default Docker container on real ARM64 Linux.

Every figure below says whether it was measured or is only expected.

## 1. What phase one delivers

A Rust kernel that answers Linux system calls for ARM64 programs, plus the test system that proves it
against real Linux. It is built at the same time as the translator and does not depend on it.

Scope is everything a program can do inside the reference: root inside the container, Docker's default
set of powers, Docker's default list of blocked calls. It is never a subset picked for one program.
What the reference refuses, we refuse the same way.

Done means:
- The three test layers in section 13 meet their pass bars.
- Real programs run under the stand-in CPU and behave as in the reference: a shell with pipelines and
  job control, and the larger runtimes in milestone K6.
- A running container can be saved at a random moment and brought back, and finishes with the same
  result.
- The same kernel, built as WebAssembly, passes the real-program layer under wasmtime.

Out of phase one, but the design must leave room for each:
- Joining the kernel to the real translator. Both sides are built to the boundary in section 4, and
  joining them is the next phase.
- The browser host. The kernel is proven as WebAssembly under wasmtime only.
- Guest threads on separate host threads. One guest thread runs at a time in this phase.
- Any backend beyond the first one, and pulling images from a registry. Tests unpack an image into a
  directory.
- Caching files in the kernel. It is the first speed-up after this phase and has its own part in
  section 8.
- Tracing one process from another (what debuggers and `strace` use). In scope for the product,
  because the reference allows it, but it needs single-stepping from the CPU side.

## 2. Rules already decided

1. Linux is the specification. The reference decides every question of behaviour. Where we differ on
   purpose, the difference is on the written list in section 15 and nowhere else.
2. The kernel reaches the outside only through hooks. It does not know what runs the guest's
   instructions, where files live, or which host it is on.
3. Translated code reaches the kernel through one generic entry: a call number and six raw values in,
   one value out. Inside, there is one typed Rust function per system call behind one dispatch table.
4. The kernel finishes every call it starts and never waits on its own stack. A call that has to wait
   records what it is waiting for as data, parks the thread and returns.
5. All kernel state is plain data in the kernel's own memory, so a container is saved by copying
   memories and nothing else.
6. One process is one guest memory. Page permissions are enforced by a table that translated code
   checks, not by the host.
7. Anything unsupported or approximated has a name, a counter and a warning. A call number we do not
   know returns "not implemented" and is counted.
8. Existing crates are used for everything that exists. Linux's constants and structure layouts come
   from a crate generated from Linux's headers, never typed in by hand.
9. Written in Rust. Comments are brief and say what the thing is for.

## 3. Parts (suggested crate names)

- `wcontainers-kernel`: the kernel. A library that uses no operating system services, so the same
  source builds as WebAssembly and as a native library for tests.
- `wcontainers-kernel-test`: the stand-in CPU runner, the native reference runner, the comparison, and
  the driver for outside test suites.

- `wcontainers-abi`: what the kernel and the translator must agree on: the saved register block
  layout, the system call entry and the hook definitions. It holds the boundary in section 4 and is
  shared with the translator plan.

Reuse, checked 2026-11-14:
- `linux-raw-sys` 0.12.1 (Apache-2.0 or MIT): call numbers, error numbers, flags and structure layouts
  for ARM64, generated from Linux's headers.
- `object` (Apache-2.0 or MIT): reading program files when starting a program.
- `fuse-abi` 0.2.0 (MIT or Apache-2.0): the layouts of FUSE messages.
- `proptest` 1.12.0 (MIT or Apache-2.0): random cases, shrinking, saved failing seeds.
- `unicorn-engine` 2.1.5: an existing ARM64 CPU emulator, as the stand-in CPU. Its licence is GPL-2.0.
  It is used by the test crate only and is never part of anything shipped. **Needs a ruling.**
- The Linux Test Project, as the main outside test suite. gVisor's system call tests are a second
  candidate; whether they build for this setup is unchecked.
- A ready-made FUSE server as the first file backend (section 8).

## 4. The boundary with the CPU side

This is everything the kernel assumes about what runs instructions. The translator plan describes the
same boundary from the other side.

- **Saved register block.** One per guest thread, fixed layout: x0 to x30, stack pointer, program
  counter, flags, v0 to v31, the two floating point control registers, the thread pointer, and the
  stop flag.
- **Run a thread.** The kernel asks the CPU side to run one thread from its saved registers. The thread
  comes back for one of five reasons: a system call, the stop flag, a memory fault (with the address,
  and whether it was a read or a write), an instruction the CPU does not have, or a breakpoint. Its
  registers are exact every time.
- **Guest memory.** The kernel reads and writes a process's memory through two hooks: copy bytes out
  of process P, copy bytes into process P. The kernel holds only a handle for each memory.
- **Making memories and threads.** A memory is one process's whole address space. The kernel asks for
  a new one when a program starts, a copy of one on fork, and for one to be dropped when a process
  ends. It asks for a thread to be started on an existing memory. Ranges inside a memory are the
  kernel's own bookkeeping and need no hook.
- **"Code is here."** The kernel tells the CPU side when a range of a memory now holds code: after
  loading a program or a library, and when a program marks memory as executable. It is a notice and
  nothing comes back. The CPU side may translate at once, later, or on first use. The one duty is
  that anything already translated for that range is stale and must be dropped. The same notice is
  sent when a range stops being runnable.
- **Page permissions.** The kernel tells the CPU side what each range of a memory allows: reading,
  writing, both or neither. An access that is not allowed comes back as a memory fault. How it is
  enforced is the CPU side's business (the translator plan uses a table of one byte per page).
- **Shared pages.** The kernel asks for a block of shared pages, and asks for it to be mapped into a
  memory at an address. Every process that maps the same block sees the others' writes. The host
  answers this the best way it can (section 7), and the kernel never knows which way.
- **The stop flag.** The kernel asks the host to raise a thread's flag now, or after a given time.

**The stand-in.** In this phase an existing CPU emulator sits behind this boundary: it runs ARM64
instructions on a block of memory, enforces page permissions, and calls back on a system call or a
fault. That is the same shape as the translator's output. Whether it can map the same pages into two
processes is to be checked; if not, its hooks use the copying method in section 7. It is slower than
the translator will be and that does not matter here. With it the kernel runs real programs from the first
milestone, with no translator.

## 5. Processes, threads and taking turns

- **What exists.** Processes, threads, parents and children, process groups, sessions, exit and wait,
  children handed to the first process when their parent dies, resource limits, and process numbers
  as the guest sees them.
- **The kernel's main loop.** Pick a thread that can run, run it until it comes back, deal with why it
  came back, repeat. One guest thread runs at a time.
- **Taking turns.** When more than one thread can run, the kernel asks the host to raise the running
  thread's stop flag after a time slice. With one runnable thread no timer is set and nothing is
  paid.
- **Who runs next.** The scheduler is Linux's Completely Fair Scheduler (CFS), without the parts that
  spread threads across CPU cores. The thread that has had the least CPU time so far runs next, and
  a higher priority counts its time more slowly. A thread that was asleep has had little time, so it
  runs almost at once when it wakes. When the running thread wakes another, the kernel is already in
  control and may switch on the spot. The time each thread waits between waking and running is
  recorded. Linux's newer scheduler (EEVDF) is considered only if real workloads show that wait to
  be a problem. Later, when guest threads run on separate host threads, the host's own scheduler
  does most of this, and ours decides only when a program has more threads than the host gives us.
- **Waiting.** A call that cannot finish yet parks its thread. The thread's saved registers already
  hold the call number and its arguments, so the record of the wait is: "parked on this thing".
  When the thing happens the kernel runs the same call function again. A thread is in at most one
  call at a time, so the record is a fixed slot in the thread's entry and nothing is allocated. A
  call that is partly done (half of a large write) keeps its progress in the same slot. The one
  interface that queues many calls at once, io_uring, is blocked in the reference (checked in Docker's
  default profile), so it is refused here too.
- **Going cold.** When no thread can run, the kernel returns to the host with "wake me at this time, or
  when one of these things happens". It never spins.

## 6. Signals

- **What exists.** Handlers, masks, pending and queued signals, the alternate signal stack, the extra
  information passed to handlers, interrupted calls and automatic restart, and stopping and continuing
  processes. The frame put on the guest's stack matches Linux's layout exactly, because unwinders and
  runtimes read it.
- **The return path.** ARM64 Linux gives every process a small page of code it maps itself (the vDSO),
  and programs return from handlers through it. We supply a tiny one.
- **When a signal is delivered.** On the way out of any system call, when a thread comes back for the
  stop flag, and when it comes back for a fault. If the target thread is running, the kernel raises its
  stop flag.
- **Faults.** A memory fault becomes the signal Linux would send, with the address. With no handler
  the process is killed with the same exit status as on Linux. The same goes for an instruction the
  CPU does not have, and for a breakpoint.

## 7. Memory

- **Layout.** The kernel decides where everything goes in each process: program, heap, mappings,
  stacks. It keeps a list of ranges with the permissions the program asked for, and reports them as
  Linux does. Nothing is placed in the bottom 64 KiB, or above the host's cap.
- **Fresh memory is zero**, as Linux guarantees and programs rely on. Never-used space is already
  zero, so new mappings go there while any is left. A range that is reused is cleared first.
- **Starting a program.** Read the file with `object`, place its parts, place the program's own
  loader if it names one, build the starting stack (arguments, environment, the values Linux passes
  about the machine), tell the CPU side where the code is, and start. Scripts that name their
  interpreter on the first line work the same way as on Linux.
- **Protection.** What a program asks for with Linux's protection calls is enforced, through the
  page permissions in section 4. That covers the forbidden page under each stack, so running off the
  end of a stack is a fault as on Linux, and programs that protect pages on purpose and catch the
  fault.
- **Fork.** The child gets a copy of the parent's memory through the "duplicate a memory" hook. The
  lighter ways of starting a child, where the child borrows the parent's memory until it starts a
  program, copy nothing.
- **Memory shared between processes** uses a block of shared pages (section 4). Linux's shared memory
  is a shared mapping of a file that lives in memory, so this one mechanism serves both.
- **Files mapped into memory.** A private mapping is a copy of the file's bytes, which behaves as on
  Linux. A shared mapping is a block of shared pages: every process in the container that maps or
  reads the file sees the same bytes, and changed pages are written back to the file when the program
  asks, unmaps it or exits.

How a host answers the two hooks that need a real page table:

- **A host that supplies the memory itself (wasmtime).** The operating system underneath maps the
  same real pages into both processes, and can make fork copy a page only when it is first written.
  This is Linux's own method and costs nothing per access. wasmtime documents host-supplied memory;
  using it this way is expected to work and is untested.
- **A host that cannot (browsers).** Each process holds its own copy of a shared block. Shared pages
  start read-only in the permission table, so the first write to each is noticed, and the changed
  pages are copied across before another process that shares them runs. This is exact while one guest
  thread runs at a time. It is the method computers with no common memory have long used to share
  pages ("distributed shared memory"). Fork is a plain copy of the memory in use. Measured earlier in
  Node: copying 128 MB took 8.8 ms into new memory.
- **Later, with guest threads on separate host threads,** processes that share a block on such a host
  take turns, and everything else runs in parallel.

## 8. Files

- **Inside the kernel.** Everything Linux's own file layer does: walking paths, mounts, open files and
  everything about file descriptors, permissions, links, locks, change notifications, readiness for
  the waiting calls. Also the file systems that exist only in memory on Linux: the in-memory one
  mounted at places like `/tmp`, the process information tree, devices, pipes, sockets, and the
  timer, event and signal descriptors.
- **Outside the kernel.** The container's own files live in a backend. The kernel turns file
  operations into FUSE messages and sends each through one hook: request bytes out, reply bytes back.
  This is the protocol Linux itself uses for file systems outside the kernel, and the one Linux
  virtual machines use for folders shared with their host. Anything that speaks it can be a backend.
- **A reply may take time.** In a browser a backend answers later, not at once. A file call that is
  waiting for a reply parks like any other waiting call.
- **The first backend.** A ready-made FUSE server that passes operations through to a directory. In
  tests the directory is in memory, inside the Linux test container. Candidates: the `passthrough`
  example of `fuser` 0.18.0 (MIT), or the server in `fuse-backend-rs` 0.14.0 (Apache-2.0 and
  BSD-3-Clause). Checked: `fuser` can serve over a plain file descriptor with nothing mounted
  (`Session::from_fd`). No in-memory FUSE server crate with real use was found on crates.io.

### File caching: the first speed-up after phase one

Caching in the kernel is expected to be one of the largest speed wins of the whole project, most of
all for agent work, which reads the same files and looks up the same names again and again. It is
not built in phase one, where every file call goes to the backend. It is the first thing built after.

What it covers: file contents, names and file details, reading ahead, and holding writes back to send
them together.

What phase one does so that it drops in later:
- Every round trip to the backend is counted and timed, per kind of call, so the win is a number.
- Nothing in the kernel assumes a file call reaches the backend. The cache sits between the file layer
  and the hook, and neither side changes.
- The FUSE protocol already carries what a cache needs: replies say how long a name or a file's
  details may be kept, and a backend can send a notice that something changed. Phase one reads those
  fields and handles those notices even though it keeps nothing.

## 9. Terminals, pipes and sockets

- **Terminals.** Pseudo-terminals with Linux's line handling: echo, line editing, window size, and the
  keys that send signals. This is what makes a shell usable and what turns Ctrl-C into a signal.
- **Pipes and local sockets.** All inside the kernel, including passing file descriptors between
  processes.
- **Network inside the container.** Connections between programs in the same container never leave
  the kernel.
- **Network to the outside.** Through the network hooks: connect, send, receive, close, look up a
  name. Programs look names up by sending packets to a name server themselves, so the kernel answers
  those packets on a local address and uses the "look up a name" hook to do it. Accepting connections
  from outside depends on the host and is a hook that may answer "not supported".

## 10. Time, randomness and identity

Clocks and randomness come from hooks. Sleeps and every kind of timer go through one queue inside the
kernel, which is where "wake me at this time" comes from. What the guest is told about the machine
(CPU features, memory size, system name) is one description, shared with the translator, so the two
never disagree.

## 11. Users and powers

Processes start as root inside the container, as in the reference, with the same reduced set of
powers. User and group numbers, permission checks, changing user, and the per-process power sets all
exist, because images switch users and some programs refuse to run as root. A program may install a
filter on its own system calls, as the reference allows; the filter is run by the kernel.

## 12. Saving and bringing back

- **Saving** is copying the kernel's memory, every process's memory and every block of shared pages.
- **Bringing back** starts every thread from its saved registers. Parked threads have their calls run
  again. Timers are set again from the kernel's own queue, and the clock that must never go backwards
  does not.
- **What cannot come back.** Connections to the outside are reported closed. A backend must give the
  same files the same numbers afterwards; that is a requirement on backends.

## 13. Testing

Every failure is replayable from a seed and shrunk automatically. The kernel's own choices (which
thread runs next, when a time slice ends) come from a seed in tests, so a run can be repeated exactly.

**Layer 1: system calls against real Linux.** `proptest` generates short sequences of calls with their
arguments. A small native runner makes the calls in the reference. The same sequence goes into our
kernel, with a plain buffer as guest memory. Compare every return value, every byte written back,
and the state seen through later calls. No instructions are run, so this layer is fast.

**Layer 2: real programs.** Unmodified ARM64 programs run under the stand-in CPU on our kernel, and
in the reference. Compare output, exit status and the files left behind. Shell scripts with pipelines
and job control, the C library's own tests, and programs written to exercise threads, signals,
fork, page protection with fault handlers, and memory shared between processes.

**Layer 3: outside test suites.** The Linux Test Project. Each of its tests says what it needs and
skips itself when that is missing, so there is no separate list for containers.

**Across layers 2 and 3, injected at random:** the stop flag raised, a signal arriving, a thread
switch, and "save here, bring back in a fresh kernel".

**Proposed pass bar:** layer 1 with zero mismatches over one million sequences. Layers 2 and 3 with
the same verdict as the reference for every test (pass, fail or skipped), except tests named on the
written list in section 15, each with the gap it comes from.

**Reported:** every system call as done, partial or missing; the verdict of every outside test; every
counter.

## 14. Milestones

- **K0, skeleton.** Crates, the hooks, the boundary, and a dispatch table built from Linux's own list
  of calls with every call answering "not implemented". The stand-in CPU runner. A hello world that
  only writes and exits. The kernel also builds as WebAssembly from this point on.
- **K1, one process.** Starting programs, including ones that load shared libraries. Memory calls, with page protection and faults. The
  in-memory file systems. Time. Single programs from busybox run. Layer 1 running.
- **K2, many processes.** Fork and its lighter forms, wait, pipes, signals, terminals, job control,
  memory shared between processes. A
  shell runs pipelines. The save and bring back test passes.
- **K3, threads.** Threads, the waiting primitive that locks are built on, taking turns with the stop
  flag, timers. Seeded schedules.
- **K4, files outside.** The FUSE side and the first backend. A container's file system unpacked from
  an image.
- **K5, sockets and network.** Local sockets, the calls that wait on many things at once, network
  inside the container, the hooks to the outside, name lookup.
- **K6, the bar.** The Linux Test Project. Larger runtimes of different kinds: Python, Node, a Go
  program, a Java runtime. The written list of gaps is final.
- **K7, as WebAssembly.** The kernel built as WebAssembly runs layer 2 under wasmtime, with the hooks
  and the stand-in CPU supplied by the host.

## 15. Where we differ from Linux on purpose

This is the whole list. Tests that depend on one of these are the only ones excused. Each has a
counter and a warning where the event can be seen at all.

Behaviour that differs and stays:

- **The address space is small.** 4 GiB on a host with 32 bit memory and 16 GiB in Chrome, against
  256 TiB on ARM64 Linux. A program that insists on reserving more is refused.
- **An address above the address space** kills the process, as Linux would, but its own handler
  cannot run.
- **A permission change to a page that holds a running function's own stack frame** is noticed at the
  next function entry, not at once.
- **A shared file mapping does not see changes made to the file from outside the container** while it
  is mapped.
- **Signals and thread switches land only at stop checks:** loops, function entries and system
  calls, never between two arbitrary instructions. A sampling profiler sees only those points.
- **Floating point status flags are tracked only from the moment a program first clears or reads
  them.** Flags raised before that are lost. From then on floating point runs in software.
- **The network is not Linux's network stack.** Connections are handed to the host. No raw packets,
  so no `ping`. Network interfaces and routes are made up. Accepting connections from outside depends
  on the host, and in a browser every outside connection goes through a relay.
- **`/proc` and `/sys` describe a made-up machine** and hold a chosen subset of Linux's files.
- **Clock and timer precision are the host's.** Browsers make both coarser on purpose.
- **32 bit ARM programs do not run,** as on an ARM64 CPU without that mode.
- **Memory is never given back to the host.** A container's real memory use only grows.

Missing in phase one only:

- **Tracing one process from another** (debuggers, `strace`).
- **More than one CPU.** Guest threads take turns, and the guest is told it has one CPU.

Same behaviour, slower:

- **Fork on hosts without a real page table** copies the whole memory in use.
- **Programs that switch stacks themselves** (Go, Java virtual threads, fibers). Unmeasured.
- **Code a program generates while running** (Node, Java, and other runtimes that compile as they go)
  is translated and compiled a second time by the host. Unmeasured.
- **File access,** until the cache in section 8 exists.
- **Floating point in software,** for programs that change the rounding mode or use status flags.

Later, with guest threads on separate host threads:

- **Processes that share memory take turns** on hosts without a real page table.
- **The older two-instruction atomics** can miss a value that was changed and changed back in
  between, which a real CPU notices.

## 16. Hooks the kernel uses

Naive or "not supported" bodies are fine; the interface is not optional.

Run a thread. Copy out of a memory. Copy into a memory. New memory. Duplicate a memory. Drop a memory.
Set page permissions. Shared pages: make a block, map it into a memory, drop it. Start a thread. Code
is here. Raise a stop flag, now or after a time. Wait until a time or an event.
Clock. Randomness. Network: connect, send, receive, close, look up a name, accept from outside. File
backend: one message out, one reply back. Warn. Read all counters.

## 17. Not yet measured or checked

- What copying between the kernel and guest memory costs per call.
- What a FUSE round trip costs per file call.
- Fork at real program sizes.
- Sharing pages through host-supplied memory in wasmtime, which is untested.
- What the copying method costs for programs that share memory heavily.
- Whether the stand-in CPU can map the same pages into two processes.
- Whether the stand-in CPU emulator covers every instruction the translator will offer.
- How the ready-made FUSE servers behave when driven without a real mount.
- How many Linux Test Project tests pass in the reference, which sets the size of the bar.
- Whether gVisor's tests can be used.
