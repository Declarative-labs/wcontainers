# wcontainers: the runtime (joining the kernel to the translator)

Status: proposed plan, 2026-11-17. Nothing is built yet.

Words used here:
- **kernel**: crates/wcontainers-kernel, driven from outside through `wcontainers-abi`.
- **translator**: crates/wcontainers-translator and its guest CPU crates.
- **CPU side**: what runs the guest's instructions. Here: the translator's output run by wasmtime.
- **runtime**: the outside that drives both. It is the kernel's `Cpu` side written over the translator, the
  kernel's `Host`, and the loop between them.
- **hook**: a host function a translated module imports (contract.rs in the translator).
- **turn**: one stretch of a guest thread running, from being started or resumed until it comes back.
- **reference**: a default Docker container on real ARM64 Linux, on this Mac.

Every figure below says whether it was measured or is only expected. Measurements are in section 16 with
the machine and its load.

## 1. What the runtime delivers

One runtime that runs an unmodified Linux container image from start to finish: the kernel decides, and
the translator's WebAssembly runs the instructions, in place of the stand-in CPU (Unicorn) the kernel's
tests use today.

The centre stays this simple: when the kernel starts or resumes a thread, the runtime makes sure the code at
that thread's program counter is translated with the container's settings, and runs it. Everything else in
this plan is the edges of that.

Done means the brief's pass bar:
- The kernel's real-program layer (layer 2, and layer 3, the Linux Test Project) passes with the runtime
  behind the boundary, with the same results as with the stand-in and as the reference. This includes the
  runtimes of K6b as they land.
- The kernel's layer 1 random sequences pass with the runtime's CPU side behind the boundary.
- The translator's benchmark programs, run as ordinary programs in a container, are within 5% of their
  figures in the translator's own runner (section 13), with system call and fork cost reported.
- Every measurement records the machine and its load.

## 2. Rules

All rules of phase one hold (phase-1-kernel.md section 2, phase-1-translator.md section 2). In particular:
one primitive over parallel paths; no setting or abstraction without a real use today; never fake; every
slow path has a name, a counter and a warning; run speed is goal number one; established crates where the
job is "same input, same output"; scope is whole architectures and whole images, never one program.

Added for the runtime:
- Unicorn (GPL-2.0) stays in the kernel test crate. Nothing shipped depends on it.
- The translator's dials (look-ahead: none, bounded N, full; speed or size) are settings of a container and
  part of every cache key. No other setting without a measured need.
- The kernel stays `no_std` and driven from outside. The runtime is the outside.
- wasmtime is named in one module of the runtime only. Nothing else in it knows which engine runs the
  modules, so a browser host later replaces that module and nothing else.

## 3. What it is built on, and how it stays generic over the guest CPU

**The base.** The runtime is built on a merge of `origin/main` at 36d20013 (the kernel through K6a, the
translator through M5 and S1) with `origin/phase-2-x86` at 84e9a937 (X1: the shared guest CPU interface
`wcontainers-guest-cpu`, ARM64 moved onto it with byte-identical output, which passed its independent
check at that commit). Why that commit:
- The runtime must be generic over the guest CPU, and only X1 has the interface.
- 84e9a937 already holds every commit of `origin/phase-1-translator` as of 2026-11-17 (checked: the
  translator's head 8b5e4207 is an ancestor), so nothing of the translator is lost.
- X1 does not touch the kernel, the kernel test crate or the abi, and main does not touch the translator
  crates after their common base. The merge is clean (checked with `git merge-tree` on 2026-11-17: no
  conflicts; the merged tree is byte for byte main's kernel crates and X1's translator crates).
- The head of phase-2-x86 is not used: what follows X1 there is x86 work in progress.
The merge is J1's first step. Later the runtime takes the kernel and translator only through main, as the
orchestrator merges them there.

**Generic at compile time.** The runtime's CPU side is one type `CpuSide<G: Guest>` (the interface's
translation half, which includes `Personality`). Everything it needs of a guest comes from `G` and is
monomorphised: the thread state size (`G::BLOCK_SIZE + G::PRIVATE_BYTES`), where pc and the stop flag sit,
`G::initial_block`, the guest's own hooks (`G::HOOKS`, `G::run_hook`), the memory fault classification
(`G::memory_fault`), the probe (`G::probe_module`, `G::host_from_probe`), and `translate::<G>` and
`thread_module::<G>`. Nothing is reached through `dyn`.

Today the abi's `Cpu` trait is written for ARM64's `RegisterBlock`, so the runtime implements
`Cpu for CpuSide<Arm64>` with a body that only calls the generic methods. When the x86 track's X10 makes the
abi's `Cpu` generic over `Personality::Block` and the kernel generic over its personality (x86 plan section
6), that impl becomes `impl<G: Guest> Cpu<G> for CpuSide<G>` and nothing else in the runtime changes. The
program that ships names the one guest it is built for (`Runtime<Arm64>`).

## 4. The boundary, item by item

Kernel side lines are at 36d20013 (this branch); translator side lines are at 84e9a937 (`git show
84e9a937:<path>`). Paths are shortened: `abi/` is crates/wcontainers-abi/src, `contract.rs`, `code.rs` and
`thread.rs` are in crates/wcontainers-translator/src, `runner.rs` is
crates/wcontainers-translator-test/src/runner.rs, `gcpu/` is crates/wcontainers-guest-cpu/src, `arm64/` is
crates/wcontainers-arm64/src. "Fits as is": the runtime uses it unchanged. "Fits with a named change": it
works once the runtime does the change named, and no translator or kernel code changes unless the row says
so. "Missing": nothing on the translator side does it; the runtime builds it.

| Item | Kernel side | Translator side today | Verdict |
|---|---|---|---|
| Saved register block (kernel plan 4, line 94) | `RegisterBlock` abi/registers.rs:11; stop flag `STOP_FLAG_OFFSET` :57; `THREAD_BLOCK_SIZE` 816 :60 | `Personality::Block` gcpu/lib.rs:24-38; thread block import contract.rs:59-61; `thread_state_size` contract.rs:122 (block + `PRIVATE_BYTES`, 32 on ARM64, arm64/lib.rs:65) | Fits with a named change: a thread's state is 848 bytes; the 32 private bytes (float tracking, exclusive monitor, arm64/lib.rs:78-84) travel with the thread on fork and save (sections 7, 10) |
| Run a thread (kernel plan 4, line 97); the conversation abi/lib.rs:7-14 | `Next::Run` abi/exit.rs:62-68 | `run` export contract.rs:90-92, bottom loop in `thread_module` thread.rs:49; hooks contract.rs:94-114, bodies runner.rs:834-1076 | Fits with a named change: the run loop calls the kernel from inside the system call and stop hooks and leaves `run` only when the thread cannot carry on at once (section 9) |
| Exit: system call | `Exit::Syscall` abi/exit.rs:26 | `HOOK_SYSCALL` contract.rs:94-97; runner.rs:837-865 | Fits as is |
| Exit: stop flag | `Exit::Stop` abi/exit.rs:28 | `HOOK_STOP` contract.rs:98-100; clears the flag runner.rs:867-892 | Fits as is |
| Exit: memory fault | `Exit::MemoryFault` abi/exit.rs:30 | `HOOK_MEMORY_FAULT` contract.rs:101-103 (read, write); runner.rs:894-922; running code from a page that does not allow it is found at translation, runner.rs:940-953 | Fits as is |
| Exit: fault without registers | `Exit::MemoryFaultWithoutRegisters` abi/exit.rs:33 | the engine's out of bounds trap, runner.rs:628-640 | Fits as is |
| Exit: alignment, illegal instruction, breakpoint | abi/exit.rs:37, :39, :41 | `HOOK_TRAP` contract.rs:112-114, `TrapKind` gcpu/lib.rs:133-154; runner.rs:1047-1075 | Fits as is. x86's kinds (runner.rs:54-65) have no `Exit` yet; ARM64 never raises them; X10 adds them |
| Resume: return a value | `Resume::Return` abi/exit.rs:49 | the system call hook's result, runner.rs:858-862 | Fits as is |
| Resume: carry on | `Resume::Continue` abi/exit.rs:51 | the stop hook returns 0, runner.rs:890 | Fits as is |
| Resume: from new registers | `Resume::FromRegisters` abi/exit.rs:55 | `Machine::set_registers` runner.rs:553, then `run` runner.rs:616; a fresh thread instance runner.rs:317 | Fits with a named change: registers the kernel sets while the thread is inside a hook are written to the block after the thread has left `run` |
| Turn length | `slice_nanoseconds` abi/exit.rs:65-67 | a raiser another host thread can call, runner.rs:542-551 | Fits with a named change: the run loop arms a timer that raises the flag (section 8) |
| Idle and finished | `Next::Idle` abi/exit.rs:71, `Next::Finished` :73 | none | Missing: the run loop's own (the kernel test driver does it today, driver.rs:526-559) |
| Guest memory (kernel plan 4, line 101) | `Cpu::read`, `Cpu::write` abi/cpu.rs:45-46 | `Machine::read`, `write` over one wasmtime memory, runner.rs:386-396 | Fits with a named change: memories are mappings the runtime supplies to wasmtime, so the kernel reaches any process's memory by pointer, also from inside another process's hook (section 7) |
| Zeroing | `Cpu::zero` abi/cpu.rs:48 | none | Missing |
| Address space | `Cpu::address_space` abi/cpu.rs:35; `AddressSpace` abi/memory.rs:23 | 64 bit guest memory contract.rs:39-43; size given to `Machine::new` runner.rs:249 | Fits with a named change: the end is 0xFF00_0000, set by the array of slots (section 7) |
| New memory (kernel plan 4, line 103) | `Cpu::memory_new` abi/cpu.rs:38; used at exec, loader.rs:285 | one memory per runner, runner.rs:294-296 | Fits with a named change: one wasmtime store, guest memory and state memory per process |
| Duplicate a memory (fork) | `Cpu::memory_duplicate` abi/cpu.rs:41; used by clone.rs:153 | none | Missing (section 7) |
| Drop a memory | `Cpu::memory_drop` abi/cpu.rs:43; task.rs:706, container.rs:733 | none | Missing: dropping the process's store frees its instances |
| New thread | `Cpu::thread_new` abi/cpu.rs:92; clone.rs:225, container.rs:649 and :722 | one thread instance, runner.rs:1322-1352 | Fits with a named change: one thread instance and code instances per thread, each thread's block in its process's state memory |
| Drop a thread | `Cpu::thread_drop` abi/cpu.rs:93; task.rs:592 and :675 | none | Missing: the block and instances are kept for the process's next thread |
| Read and set registers | `Cpu::registers`, `Cpu::set_registers` abi/cpu.rs:95-97 | the block in the state memory, runner.rs:572 and :553, `snapshot` :689 | Fits as is for reading; setting as the "from new registers" row says |
| "Code is here" (kernel plan 4, line 107) | `Cpu::code_changed` abi/cpu.rs:64; sent at exec loader.rs:871, for the vDSO loader.rs:723, for executable mappings memory.rs:718, when code stops being runnable memory.rs:471, :495, :516, shared.rs:408 | `ic ivau` drops the pieces of a 64 byte line, runner.rs:1032-1041 and :1113-1119; missing pieces are translated from the bytes now in memory, `load_code` runner.rs:924-1008 | Fits with a named change: the notice drops the range's pieces and keeps its starts; translation happens at first use (section 6) |
| Page permissions (kernel plan 4, line 112) | `Cpu::set_permissions` abi/cpu.rs:52; `Permissions` with `EXECUTE` abi/memory.rs:14 | one byte per page with the next page's bits, contract.rs:62-67 and :126-145; the runner keeps the full permissions, `EXECUTE` included, and rewrites the byte before the range, runner.rs:647-663 | Fits as is: the runner's method moves into the runtime, per process |
| Shared pages (kernel plan 4, line 115) | `shared_new`, `shared_map`, `shared_unmap`, `shared_read`, `shared_write`, `shared_drop` abi/cpu.rs:66-89 | none | Missing: blocks are shared memory objects mapped into the process memories (section 7; measured to work in wasmtime) |
| Stop flag (kernel plan 4, line 118) | abi/registers.rs:54-57 | the stop check and `HOOK_STOP` contract.rs:98-100; `raise_stop_flag` runner.rs:533 and :825 | Fits as is |
| Clocks | `Host::monotonic_nanoseconds`, `realtime_nanoseconds` abi/host.rs:95-97 | ARM64's `clock` hook (counter reads) arm64/guest.rs:435-440, answered from the runner's own start, runner.rs:1025-1030 | Fits with a named change: the clock hook answers from the kernel's monotonic clock, which the kernel exposes (one public method), so a counter read and `clock_gettime` agree, also across a restore |
| Randomness | `Host::random` abi/host.rs:99 | none | Missing: the runtime's host, from the operating system |
| Warnings and counters | `Host::warn` abi/host.rs:101; `Kernel::counters` kernel lib.rs:473 | `HOOK_WARN` contract.rs:110-111; `slow_path` runner.rs:796-815; `Reason` gcpu | Fits with a named change: the translator's reasons, the runtime's own and the kernel's go through one `Host::warn` and one list of counters |
| Output | `Host::output` abi/host.rs:109 | the stand-in's `write` and `writev`, runner.rs:1396-1445 | Replaced: the kernel answers those calls; the runtime's host passes the bytes to the user |
| File backend | `Host::backend_send` abi/host.rs:105, `Event::BackendReply` :61 | none | Missing: the runtime's host (J5) |
| Network | `connect`, `send`, `receive`, `finish`, `close`, `look_up`, `listen`, `accept` abi/host.rs:115-141; their `Event`s :67-84 | none | Missing: the runtime's host (J5) |
| Events from outside | `Event` abi/host.rs:57; `Kernel::event` kernel lib.rs:330 | none | Missing: the run loop raises the running thread's flag when one arrives and hands it to the kernel once the thread is back |

The kernel's own methods the run loop calls are `Kernel::start` (kernel container.rs:383), `next`
(lib.rs:291), `exit` (:303), `event` (:330), `kill` (:383) and `restore` (:408). The translator plan's
own items meet the kernel here: "load new code" (translator plan 7, contract.rs:104-109) and "go to
address" stay inside the runtime and never reach the kernel; "the stack is lost" (translator plan 7) is
what `Resume::FromRegisters` and every leaving of `run` cause; the cache key (translator plan 4) is section
6 here.

## 5. The centre: one run loop

The loop is the kernel test driver's loop (driver.rs:437-561) with the stand-in replaced:

1. Ask the kernel what to do (`next`). For `Run`, look up the thread's process, and call its thread
   instance's `run`. For `Idle`, wait for an event or the time. For `Finished`, end.
2. Inside `run`, the hooks do the kernel's work without leaving:
   - **System call hook**: hand the kernel `Exit::Syscall`, deliver any events that arrived, and ask `next`.
     If the answer is "run this same thread, `Return(v)`", return `v` from the hook: the call is done with no
     stack switch. Anything else is kept as the loop's next answer, the flag is raised, and the hook
     returns, so the thread leaves `run` with its registers saved.
   - **Stop hook**: the same with `Exit::Stop`; "same thread, `Continue`" returns 0 and the thread carries
     on.
   - **Memory fault, trap**: hand the kernel the exit; the thread always leaves (contract.rs:101-114).
   - **Load code**: translate (section 6). The kernel is not involved.
3. When `run` returns, apply registers the kernel set meanwhile, then go to 1 with the kept answer.

That is all of the centre. The kernel never waits on its own stack and never sees WebAssembly; the
translator never sees Linux.

## 6. Translation: where, when, what is cached

**One primitive, one place.** All translation happens in the load code hook, when the bottom loop reaches
an address with no piece: at a thread's first instruction, after a jump to code not yet translated, after a
stack loss, and after code changed. The "code is here" notice translates nothing. It drops what was
translated from the range (clears the range's entries in the process's array of slots) and keeps the
range's starts. Why not at the notice: a range announced is often not run (a library mapped and never
called), and code a program writes is announced again at every permission change, so translating at the
notice would translate what never runs and translate changing code again and again. At first use each
range is translated once, when it is needed.

**What is translated.** At first use of an address in a range announced by a notice, the whole announced
range is translated at the container's look-ahead, with the notice's starts, the address reached, and the
translator's own finding of functions. Every later address in that range with no piece (an unknown jump
target, a resume after a stack loss) is translated at bounded look-ahead from that address, as the
translator plan says. At look-ahead none, every piece is one instruction, as the reference.

**Code a program writes (Node, Java, any JIT).** A program must invalidate the instruction cache line
(`ic ivau`) before running code it wrote, because the `CTR_EL0` we present says invalidation is needed
(arm64/decode_forms.rs:200-202); the guest's code changed hook drops the pieces of that line
(runner.rs:1113-1119), and permission changes bring a notice. A range whose bytes change after it was
translated is from then on translated only at bounded look-ahead, from each address reached. Slow path:
"code changed after it ran", counted with its range and warned. No setting.

**The cache.** A translation is cached as wasmtime's compiled module and the translation's piece list. Key:
- BLAKE3 hash of the code bytes (the range as it is in memory),
- the starts given (blob offsets), sorted,
- the look-ahead and what it is optimised for (the container's settings),
- the guest (`G::ID`), the host probe's answer and the guest's offer,
- the translator's build identity: a hash of the translator's and the guest crate's sources made at build
  time, so any change to them changes the key without manual version numbers,
- wasmtime's own compatibility check, which `Module::deserialize` makes.
Output never holds an absolute address (translator plan 4), so one entry serves every process that maps
the same bytes at any address. Two levels: compiled modules kept in memory for the runtime's life, shared
by every process and container; and a directory on disk, if the user gives one. Measured need for the disk
level: busybox compiles in 1.1 to 2.1 s and comes back from disk in 2.7 to 3.3 ms; libpython in 6.6 s
against 12.6 ms. Hashing the code each time a range is first run costs 0.16 ms for musl, 0.30 ms for
busybox and 1.3 ms for libpython (measured).

**Starts learned at run time.** Addresses translated at run time inside a cached range could be recorded
with its entry, so the next process that maps the same bytes translates them ahead. This is built only if
J6 measures that run-time translation is a real cost on the K6b runtimes (one piece costs 1.1 to 6.7 ms in
wasmtime, measured at the translator's M4).

## 7. Memories

**One store per process.** wasmtime frees instances and memories only when their store is dropped. A shell
that forks thousands of times, each child with its own memory and instances, would grow without bound in one
store. So each process has its own store, holding its guest memory and its threads' instances, dropped
with the process (exec makes a new one). The engine and compiled modules are shared by all.

**Host-supplied guest memory.** Each guest memory is a mapping the runtime makes and hands to wasmtime
(`Config::with_host_memory`), with `memory_may_move(false)`. Because the runtime owns the mapping:
- the kernel reads and writes any process's memory by pointer, also while another process is inside a hook;
- a block of shared pages is mapped into it at the same place in every process that maps it;
- a host whose operating system can copy pages on first write can make fork do that later, behind
  `memory_duplicate`, with nothing else changing.
Measured on this Mac in wasmtime 50: a 1 MiB block of shared pages mapped into two host-supplied guest
memories at different addresses; 1000 of 1000 stores through one memory were seen by loads through the
other; mapping the block took 0.6 µs. This settles the kernel plan's open question of section 17 ("sharing
pages through host-supplied memory in wasmtime") for macOS; Linux is checked at J2. Mappings use `rustix`
(an anonymous shared memory object for each block: `memfd_create` on Linux, `shm_open` on macOS).

**State memory per process.** The translator keeps threads' blocks, the page permission table and the array
of slots in one shared 32 bit memory (contract.rs:44-67), at most 4 GiB. The array has one 32 bit entry per
4 bytes of guest address it covers (contract.rs:7-11), so covering a process's whole address space takes as
many bytes as the address space. Code sits at both ends of a process (the program low, libraries and the
vDSO below the top), so the array must cover all of it. Decision: one state memory per process, laid out as
16 MiB for the permission table and thread blocks, then the array covering from 64 KiB to the end of the
address space; and the address space ends at 0xFF00_0000 (4 GiB less 16 MiB), so both fit in 4 GiB. Untouched
pages cost nothing. 16 MiB leaves room for about 18,000 threads in one process; past that `thread_new` answers
"out of memory" and the guest's `clone` fails as Linux's does at its limit. A larger address space needs the
array in its own 64 bit memory, which is the translator's contract to change; it is asked of the translator
track and not needed for the pass bar.

**Fork.** `memory_duplicate` makes the child's store, guest memory and state memory, then:
- copies the parent's private pages that hold anything: the pages resident in the parent (`mincore`) within
  the ranges that allow any access, skipping ranges that show a block;
- maps the same blocks at the same places;
- copies the permission table and the array of slots, resident pages only;
- leaves instances to the child's first thread (`thread_new`), which instantiates the process's code
  modules at the same slot bases.
Measured: a new 4 GiB memory 6 to 16 µs; copying into it 4 µs for busybox sh's 48 KB of anonymous memory, 168
µs for python's 3.3 MB, 388 µs for node's 7.1 MB, 1.6 ms for java's 24.5 MB (sizes from the reference's
`smaps_rollup`); scanning residency costs 2.8 ms for a whole 4 GiB, so only the mapped ranges are scanned.
The kernel's own share of fork, exit and wait is 2.4 to 2.7 µs. Instantiating code modules for the child's
thread costs 0.43 to 0.60 ms for busybox and 0.27 to 0.42 ms for musl; wasmtime's pooling allocator made no
difference. Copying a page only when it is first written (the kernel plan's expectation for wasmtime,
section 7 there) is not built in the first form: the copy is small next to instantiation, and it can come
later behind `memory_duplicate` on a host whose operating system offers it. So a shell's fork is expected to
cost about 0.75 ms, almost all of it instantiation (an
expectation from parts measured apart). unkernel's fork was measured at 20 µs. Making instantiation cheap is
J7's first task, after measuring where its time goes (the runner builds a new linker and its host functions
for every instance, which the runtime need not).

**Page permissions and the notices.** The runtime keeps, per process, each page's full permissions as the
kernel last set them (execute included) and writes the table bytes from them, as the runner does
(runner.rs:647-663). Reading and writing through `Cpu` ignores permissions (abi/cpu.rs:31-32). A page may be
locked further by the runtime for its own use (x86's code written without an announcement, x86 plan 5.7);
a fault on such a page is settled by the runtime and never reaches the kernel. Running code is checked at
translation: an address on a page that does not allow running gives `Exit::MemoryFault` with
`Access::Execute`. With one guest thread running at a time, every other thread is stopped whenever the
kernel changes a table, as contract.rs:17-27 requires.

## 8. Threads, the stop flag, signals, faults and exact registers

- **Threads.** Each guest thread has its block in its process's state memory and its own thread instance and
  code instances (one instance per thread, translator plan 6). Code made at run time by one thread is
  instantiated in every thread of the process at once, with the same slot, so every thread's table holds the
  same piece in the same slot (contract.rs:10-11). A dropped thread's block and instances are kept and given
  to the process's next new thread, so a program that starts and ends many threads does not grow its store.
- **Turns.** For `Run` with a slice, the runtime arms one timer thread that raises the running thread's
  flag (an atomic store to the state memory, as runner.rs:542-551) at the deadline on the host's monotonic
  clock. With no slice no timer is armed. In seeded tests, the clock is the test host's repeatable clock: the
  flag is raised at every stop check while a slice is set (the translator's existing "stop at every check"
  mode, runner.rs:527-530) and each check is one nanosecond, so where a turn ends repeats exactly for a
  seed, as the stand-in's instruction count does today.
- **Events from outside** (timers of the host, backend replies, network, terminal input) arrive on host
  threads. One arriving while a guest thread runs raises that thread's flag; the run loop hands it to the
  kernel the next time the thread is in a hook or has left.
- **Signals** are the kernel's (kernel plan 6). Delivery comes back as "run this thread from new registers",
  so the thread leaves `run`, the new registers are written, and the handler runs from the bottom loop. A
  return through the vDSO's `rt_sigreturn` is a system call like any other.
- **Faults and exact registers.** Every hook is called with the registers saved (contract.rs:13-15), so the
  block is exact at every return to the kernel: `pc` after the system call instruction, at the stopped
  instruction, at the faulting instruction or trap (abi/exit.rs:21-42). The one exception is an access
  beyond the memory, which the engine stops and which becomes `Exit::MemoryFaultWithoutRegisters` (on the
  written list of differences, kernel plan 15).
- **The private words** of a thread (ARM64: whether float flags are tracked, and the exclusive monitor) are
  the CPU side's. They are copied to a fork child's thread and saved with the thread. Whether the exclusive
  monitor must be cleared at a return from a system call or a signal is checked against the reference at
  J4 (a load-exclusive, a system call, then a store-exclusive).

## 9. System calls

A call that finishes at once is answered inside the system call hook (section 5). A call that waits, or
whose answer switches threads or sets new registers, leaves `run`. Measured through the translator's runner
as it is:
- answered inside the hook (`brk`, which the runner's stand-in answers at once): 10.5 ns a call (median of
  7 alternating pairs against the same loop with a `nop`; min 9.4 ns);
- leaving `run` and coming back: 301 ns a call (median of 5; min 286 ns).
The kernel's own share of a call, measured with the layer 1 CPU side (no instructions): 117 to 120 ns for
`getppid`. So a call answered at once is expected to cost about 130 ns end to end (two measurements added,
not measured together); J2 measures it. unkernel's `getppid` was measured at 56 ns. The kernel's share is the
kernel track's to bring down; the runtime adds about 10 ns.

Leaving is the slow twin of answering in the hook, so it is counted by why, with the call number: "left
run: the call waits", "left run: another thread runs", "left run: new registers"; each warns as every
counter does.

Leaving `run` loses the WebAssembly stack: every function in progress carries on from the bottom loop
(translator plan 7, counted "continued after stack loss"), and its resume address may need a piece made at
run time the first time. This is the translator's design, kept. If J6 measures it to matter for real
programs, the candidate is to keep each guest thread's WebAssembly stack across a switch (wasmtime's
asynchronous calls, which run each call on its own stack); that is decided only by measurement.

## 10. Saving and bringing back

Saving copies the kernel's memory, every process memory, every block of shared pages, and each thread's
whole state (block and private words) (kernel plan 12). Translated code is never saved: it is a cache.
Bringing back makes fresh stores and instances; every thread starts from its registers (`Resume::FromRegisters`,
which the kernel's `restore` gives every thread, kernel lib.rs:408-450); code is translated again at first use,
which for unchanged code is a cache hit. The kernel's clock carries on through its own offset, and the clock
hook answers from that same clock (section 4).

In the first form the kernel is Rust data in the runtime's own process, which cannot be copied to a file.
So the first form supports exactly what the kernel's tests do today: a save made by copying the whole test
process (driver.rs:494-524), brought back on a new CPU side made from the saved memories and threads. Saving
to a file comes with the final form, where the kernel's memory is a WebAssembly memory (section 11).

## 11. The first form and the final form, as one path

**First form.** One native program: the runtime embeds wasmtime, the kernel is linked in as a Rust
library, and the guest's code is the translator's WebAssembly. The hooks are host functions that call the
kernel directly. This is the translator runner's shape with the stand-in kernel replaced.

**Final form.** The same runtime; the kernel is the WebAssembly module the kernel track's K7 builds,
instantiated in its own store. The kernel module imports the `Cpu` and `Host` operations, which the runtime
provides with the same `CpuSide` and host code; the hooks call the kernel module's exports instead of the
Rust methods. Saving is then copying memories, as the kernel plan intends.

**Why this is one path.** The run loop, `CpuSide`, the host, the cache and every test are the same in both.
What changes is only where the kernel's six methods (`start`, `next`, `exit`, `event`, `kill`, `restore`) are
reached: by a direct call, or by a call into the kernel's instance. When the final form passes the same bar
(J9), the linked-in kernel leaves the shipped runtime; the kernel crate keeps its native build for its own
tests, as today. There is never a time when two runtimes are kept working side by side. What K7 must export
for this (the kernel's six methods as exports, `Cpu` and `Host` as imports) is asked of the kernel track.

## 12. Crates and the smallest user surface (proposals)

**Crates.**
- `wcontainers-runtime` (MIT OR Apache-2.0, shipped): the run loop, `CpuSide<G>`, the cache, the host for
  wasmtime, and the user surface. Depends on the kernel, the translator, the guest interface, wasmtime,
  rustix and blake3. Never on Unicorn or any test crate.
- No new test crate. The kernel test crate's driver is made generic over its CPU side at compile time, and a
  cargo feature `runtime` of that crate puts the runtime behind every one of its tests (layers 1 to 3). One
  driver, one set of tests, the CPU side chosen when the test binary is built. This is the one change
  outside the runtime's own crate; it is made in J1 on this branch as its own commit and named in the status
  file, because the kernel track also works in that crate.

**User surface (proposal; names are the owner's).** A library first; a command line program only if the
owner wants one.

```rust
let runtime = Runtime::<Arm64>::new(RuntimeSettings {
    cache: Some(path), // where compiled translations are kept between runs; None keeps them in memory
})?;
let status = runtime.run(&ContainerSettings {
    image: Image::Tar(path),           // a `docker export` or image layer tar, unpacked into the kernel
    command: vec!["sh".into(), "-c".into(), "echo hi".into()],
    environment: vec!["PATH=/usr/bin:/bin".into()],
    working_directory: "/".into(),
    hostname: "container".into(),
    look_ahead: LookAhead::Full,       // the translator's dials
    optimise: Optimise::Speed,
}, &mut io)?;                          // io: standard input, output and error as byte streams
```

The container's settings are what `docker run` gives the kernel today (`Start`, kernel container.rs:52-64)
plus the translator's two dials, and nothing else. Mounting a host directory through the file backend,
published ports and a terminal come with the milestones that build them (J5), each as one more field.

## 13. Testing

- **Layer 2 and 3 (real programs, LTP).** Every test of the kernel test crate runs with `--features runtime`.
  The bar is the same verdict as the same test with the stand-in, and as the reference, test by test. A test
  the stand-in cannot run (its emulator's alignment limits, driver.rs `Failure::StandIn`) must pass with the
  runtime against the reference.
- **Layer 1.** The same sequences with the runtime's `CpuSide` in place of the layer 1 buffer CPU: real
  host-supplied memories, permission tables, blocks of shared pages and fork copies, no instructions. Zero
  mismatches over the kernel's bar of one million sequences (a long run for the orchestrator; seeded, one
  command, with a knob for the count).
- **At every look-ahead.** Layer 2 runs at full and at none (the reference), and bounded with N from the
  seed. Injections (the stop flag, a signal, a thread switch, save and bring back) come from the kernel
  driver's seeds, as today.
- **Benchmarks.** The translator's benchmark set (programs/bench.c and the others its benchmark test runs),
  built as ordinary static programs, run in a container; their times are taken in the same session as the
  translator runner's, as alternating pairs, and the median ratio is within 5%. One command.
- **System call and fork cost.** A static program looping on `getppid`, and one looping on fork, exit and
  wait, run on the runtime and in the reference; reported per call with the machine and load, beside
  unkernel's 56 ns and 20 µs.

## 14. Milestones

Each bar is checkable by running the named command on this Mac with Docker.

- **J1, first light: a static hello, then busybox's single programs.** Merge 84e9a937. Create
  `wcontainers-runtime` with: host-supplied memories and per-process stores and state memories; `memory_new`,
  `memory_drop`, `read`, `write`, `zero`, `set_permissions`, `code_changed`, the shared block calls,
  `thread_new`, `thread_drop`, `registers`, `set_registers`; translation at first use with the in-memory
  cache; the run loop with system calls and stops answered in the hooks; traps and faults to exits. Fork
  (`memory_duplicate`) answers `CpuError::NotSupported`, which the kernel counts (no program in this bar
  forks). Make the kernel test driver generic and add its `runtime` feature. Bar:
  `cargo test -p wcontainers-kernel-test --features runtime --test hello --test busybox -- --test-threads 3`
  passes, at full look-ahead, and `--test hello` also at none: the same output, exit status and files as the
  reference, and no warning the stand-in run does not give besides "translated at run time". The same tests
  without the feature still pass. Record the time to translate and compile busybox and musl in the runtime
  and the time of a second container in the same process (cache hit).
- **J2, processes.** Fork with the copy of section 7; exec; leaving `run` when a call waits or the kernel
  switches; signals and frames; job control and terminals. Linux checked for host-supplied memory and shared
  blocks. Bar: with `--features runtime`, the tests `shell`, `script`, `scriptline`, `processes`, `zombie`,
  `orphans`, `procsignals`, `sigframe`, `faults`, `align`, `memory`, `mapflags`, `privatemap`, `sharedmap`,
  `memfd`, `sysvshm`, `loader`, `loaderrors`, `terminal`, `write`, `unknown` and `saving` all pass as they
  do with the stand-in. Layer 1 with the runtime's `CpuSide` passes 10,000 seeded sequences, and the
  one-million run is one command. System call and fork cost measured (section 13).
- **J3, the user surface.** The library surface of section 12, as the owner decided it, with images from a
  tar, the on-disk cache, and the host's output and input streams. Bar: a test runs `sh -c` with a pipeline
  in `alpine:3.22` (exported with Docker) through the public surface and matches the reference; a second
  run of the same container in a new process starts from the disk cache (measured).
- **J4, threads and turns.** Many threads per process, futexes, timers, turns ended by the timer thread,
  outside events raising the flag, seeded turn ends. The exclusive monitor question of section 8 settled
  against the reference. Bar: `threads`, `turns`, `timers`, `deadlines`, `cputime`, `cpulimit`, `clocks`,
  `lockwaits`, `rseqabort`, `priorities`, `schedattr` and `shell::runs_repeat_for_a_seed` pass with
  `--features runtime`.
- **J5, files outside, sockets, network, and the whole suite.** The host's backend and network hooks
  (taken from what the kernel test host does, written for shipping). Bar: the whole kernel test crate with
  `--features runtime` gives the same verdicts as without, test by test; the Linux Test Project's verdicts
  equal the stand-in run's for every test (the kernel's ltp harness, one command, run by the orchestrator).
- **J6, the runtimes of K6b.** Python, Node, a Go program and Java as the kernel track lands them, with code
  written at run time. Measured: run-time translations, "code changed after it ran", "continued after stack
  loss" and their times; decide on starts learned at run time (section 6). Bar: K6b's tests pass with
  `--features runtime`.
- **J7, speed.** Find where instantiation time goes and bring fork down; the benchmark bar of section 13;
  system call and fork cost reported against unkernel's. Bar: the benchmark set within 5% of the translator
  runner (median of alternating pairs, load recorded); fork and system call figures in the status file.
- **J8, saving and bringing back with translated code present.** The kernel's save injections across
  layer 2 with `--features runtime`. Bar: `saving` and the shell tests with saves injected pass at full and
  bounded look-ahead.
- **J9, the final form.** After K7 is on main: the kernel as WebAssembly under the runtime, saving to a
  file. Bar: J5's whole-suite result again with the kernel as WebAssembly, and a container saved to a file
  mid-run and brought back in a new process finishing with the reference's output. Then the linked-in
  kernel leaves the shipped runtime.

The brief's pass bar is met at J5 (layers 1 to 3), J6 (K6b) and J7 (benchmarks).

## 15. What stays out, and why nothing here rules it out

- **The browser host.** Only one module of the runtime names wasmtime (section 2). In a browser the guest
  memories cannot be host-supplied, so shared pages use the kernel plan's copying method (section 7) behind
  the same `shared_*` calls and fork is a plain copy, as here. The stop flag is raised from the page's own
  thread through the shared state memory, as the translator measured in Chrome. Nothing in the run loop
  depends on keeping the WebAssembly stack across a switch.
- **Guest threads on separate host threads.** Threads already have their own blocks and instances, and the
  state memory is shared. What is single-threaded is that the kernel runs inside a hook of the one running
  thread; with threads in parallel the kernel is entered under one lock, and permission changes first stop
  the process's other threads (contract.rs:17-27). The guest memory then has to be a shared memory, which a
  host-supplied mapping already allows.
- **x86-64 guests.** The runtime is generic over `G: Guest` (section 3). X10 makes the abi and the kernel
  generic over the personality and adds x86's exits; the runtime's impl of `Cpu` then becomes generic. The
  page locks x86 needs for code written without an announcement are the CPU side's, which section 7 keeps
  apart from the kernel's permissions.
- **Graphics.** Not part of the runtime.

## 16. Measurements behind the decisions

All on this Mac (Apple M5, 18 cores, macOS), wasmtime 50.0.1, release builds, 2026-11-17, while other
tracks were running; load averages as `sysctl vm.loadavg` gave them (1, 5 and 15 minutes). Scratch code and
logs are under pyro-parked/wcontainers-scratch/runtime (outside the repository).

- Resident memory in the reference (ARM64 Linux in Docker, `/proc/<pid>/smaps_rollup`; load 29): busybox sh
  in `alpine:3.22` after a few commands: resident 892 kB, anonymous 48 kB; python 3.13 (`python:3.13-alpine`)
  after starting: 8592 kB, 3364 kB; node 22 (`node:22-alpine`): 48204 kB, 7268 kB; java 21
  (`eclipse-temurin:21-jdk-alpine`, a class reading the file): 44656 kB, 25064 kB.
- A new 4 GiB 64 bit wasmtime memory, and copying N bytes into it (median of 15; load 40 to 28): 48 KB
  6.4 µs + 4.0 µs; 3364 KB 6.3 µs + 168 µs; 7268 KB 9.0 µs + 388 µs; 25064 KB 11.6 µs + 1.59 ms; 48204 KB
  15.6 µs + 3.23 ms. `mincore` over a whole 4 GiB memory (262,144 pages of 16 KiB): 2.77 ms.
- System call through the translator's runner, at full look-ahead: answered in the hook 10.5 ns (median of 7
  alternating pairs, 1,000,000 calls each; min 9.4 ns; load 40 to 28); leaving `run` and back 301 ns (median
  of 5, 100,000 calls each; min 286 ns; same load).
- The kernel's own share with the layer 1 buffer CPU (static hello as the started program; load 65 to 34):
  `getppid` 116 to 120 ns (5 runs of 1,000,000); fork, the child's exit and the parent's wait 2.42 to 2.65 µs
  (5 runs of 20,000).
- Real binaries from the translator's M5 list, full look-ahead, speed (load 41 to 63): busybox translated in
  0.15 to 0.35 s to 32,472,392 bytes of WebAssembly, compiled by wasmtime in 1.12 to 2.08 s, instantiated for
  a thread in 0.43 to 0.60 ms (medians of 3 across 6 runs), brought back from a serialised file in 2.65 to
  3.26 ms (51,218,600 bytes); musl 0.06 to 0.16 s, 14,546,689 bytes, 0.51 to 0.95 s, 0.27 to 0.42 ms, 1.19
  to 1.56 ms (22,644,376 bytes); libpython 0.46 s, 137,603,117 bytes, 6.60 s, 1.10 to 1.19 ms, 12.6 ms
  (210,339,800 bytes). With wasmtime's pooling allocator, alternated with the default in two rounds (load 55
  to 63): busybox 0.44 to 0.60 ms against 0.43 to 0.56 ms, musl 0.28 to 0.36 against 0.26 to 0.34: no
  difference.
- BLAKE3 of the code (median of 21; load 40): busybox 704,368 bytes 0.30 ms; musl 370,304 bytes 0.16 ms;
  libpython 2,959,628 bytes 1.28 ms.
- Host-supplied memory in wasmtime (macOS; load 59): a 1 MiB shared memory object mapped into two guest
  memories at different places; 1000 of 1000 stores through one seen by loads through the other; mapping it
  0.6 µs (median of 50); ending the sharing by mapping fresh pages and copying the bytes back 114 µs.
- From the brief, not measured here: unkernel's `getppid` 56 ns and fork 20 µs; the benchmark set's times
  native in the translator's runner (fib 5.8, sort 3.4, interpret 3.6, sieve 1.4, matrix 2.6, search 1.9).

## 17. Not yet measured

- A system call and a fork end to end through the runtime (J2).
- Where instantiation time goes, and fork with it brought down (J7).
- Host-supplied memory and shared blocks on Linux (J2).
- What a stack loss costs real programs after a call that waits (J6).
- How many pieces real runtimes need translated at run time, first run and later (J6).
- Creating a per-process 4 GiB state memory (J1).
- The final form's extra cost per system call (J9).

## 18. Decisions for the owner

- **Names**: suggested `wcontainers-runtime` for the crate, `Runtime`, `RuntimeSettings`,
  `ContainerSettings` and `Image` for the surface; all yours to choose.
- **The user surface**: the library surface of section 12 (a tar image in memory, the command and its
  environment, the translator's two dials, a cache directory, byte streams for input and output), and
  whether a command line program is wanted.
- **The address space** in the first form ends at 0xFF00_0000 (4 GiB less 16 MiB) instead of the 4 GiB the
  kernel plan's written list of differences names, until the translator's array of slots has its own memory.
- **Interfaces of other tracks** this plan asks for: the kernel exposes its monotonic clock (one method); the
  kernel test crate's driver becomes generic over its CPU side with a `runtime` feature (made by this track
  in J1); K7's module exports the kernel's six methods and imports `Cpu` and `Host`; later, the translator
  moves the array of slots into its own memory for a larger address space.
