# wcontainers: the runtime (joining the kernel to the translator)

Status: proposed plan, 2026-11-17. Nothing is built yet.

Words used here:
- **kernel**: crates/wcontainers-kernel, driven from outside through `wcontainers-abi`.
- **translator**: crates/wcontainers-translator and its guest CPU crates.
- **back end**: what runs the guest's instructions behind the kernel's `Cpu` side. The first one is the
  translator's WebAssembly run by wasmtime; the kernel tests' stand-in (Unicorn) is another.
- **outside**: what surrounds the kernel: its `Host` (clocks, randomness, output, files, network) and the
  events that arrive from outside.
- **runtime**: the loop that drives the kernel, a back end and an outside.
- **hook**: a host function a translated module imports (contract.rs in the translator).
- **turn**: one stretch of a guest thread running, from being started or resumed until it comes back.
- **progress**: how far a thread got, counted on the guest's own instructions, the same on every back end
  and at every setting (section 9).
- **reference**: a default Docker container on real ARM64 Linux, on this Mac.

Every figure below says whether it was measured or is only expected. Measurements are in section 18 with
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
- The kernel's layer 1 random sequences pass with the WebAssembly back end behind the boundary. Layer 1
  runs no instructions by design (kernel plan 13), so this is read as: the translator's side of the
  boundary (its memories, permission tables, state memories, blocks of shared pages and fork copies) in
  place of the layer 1 buffer CPU. Translated code is proven by layers 2 and 3.
- The translator's benchmark programs, run as ordinary programs in a container, are within 5% of their
  figures in the translator's own runner, counted in instructions retired (section 15), with times, system
  call cost and fork cost reported.
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
- The loop knows no back end. wasmtime is named only in the WebAssembly back end's crate.

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

**Generic at compile time.** The WebAssembly back end is one type `CpuSide<G: Guest>` (the interface's
translation half, which includes `Personality`). Everything it needs of a guest comes from `G` and is
monomorphised: the thread state size (`G::BLOCK_SIZE + G::PRIVATE_BYTES`), where pc and the stop flag sit,
`G::initial_block`, the guest's own hooks (`G::HOOKS`, `G::run_hook`), the memory fault classification
(`G::memory_fault`), the probe (`G::probe_module`, `G::host_from_probe`), and `translate::<G>` and
`thread_module::<G>`. The loop is generic over the back end and the outside (section 5). No guest or back
end is reached through `dyn` in the runtime's own code. One call per exit is: a hook reaches the loop's
answer (section 6) through `&mut dyn Answer` in the store's data, because the store's data type is fixed
when `memory_new` makes the store, inside the kernel, where the loop's types are not known. Making it
static would make the back end generic over the outside for one indirect call, next to the kernel's own
`dyn` calls of the next paragraph.

The kernel's side is not yet generic: every call from the kernel into the CPU side or the host goes
through `&mut dyn Cpu` and `&mut dyn Host` (kernel lib.rs:52-53, :303), which is the kernel's interface
today. That cost is inside the kernel's measured share of a system call (section 10). The x86 track's X10
makes the abi's `Cpu` generic over `Personality::Block` and the kernel generic over its personality (x86
plan section 6), which removes it. Until then the runtime implements `Cpu for CpuSide<Arm64>` with a body
that only calls the generic methods; after X10 that becomes `impl<G: Guest> Cpu<G> for CpuSide<G>` and
nothing else in the runtime changes. The program that ships names the one guest it is built for.

## 4. The boundary, item by item

Kernel side lines are at 36d20013 (this branch); translator side lines are at 84e9a937 (`git show
84e9a937:<path>`). Paths are shortened: `abi/` is crates/wcontainers-abi/src, `contract.rs`, `code.rs` and
`thread.rs` are in crates/wcontainers-translator/src, `runner.rs` is
crates/wcontainers-translator-test/src/runner.rs, `gcpu/` is crates/wcontainers-guest-cpu/src, `arm64/` is
crates/wcontainers-arm64/src, `driver.rs` is crates/wcontainers-kernel-test/src/driver.rs. "Fits as is": the
runtime uses it unchanged. "Fits with a named change": it works once the runtime does the change named, and
no translator or kernel code changes unless the row says so. "Missing": nothing on the translator side does
it; the runtime builds it.

| Item | Kernel side | Translator side today | Verdict |
|---|---|---|---|
| Saved register block (kernel plan 4, line 94) | `RegisterBlock` abi/registers.rs:11; stop flag `STOP_FLAG_OFFSET` :57; `THREAD_BLOCK_SIZE` 816 :60 | `Personality::Block` gcpu/lib.rs:24-38; thread block import contract.rs:59-61; `thread_state_size` contract.rs:122 (block + `PRIVATE_BYTES`, 32 on ARM64, arm64/lib.rs:65) | Fits with a named change: the 32 private bytes (float tracking, exclusive monitor, arm64/lib.rs:78-84) are the back end's and never travel: the monitor is cleared at every exit handed to the kernel, and float tracking starts as "tracked" whenever a thread is made from a block (section 9), so a thread's state is its block |
| Run a thread (kernel plan 4, line 97); the conversation abi/lib.rs:7-14 | `Next::Run` abi/exit.rs:62-68 | `run` export contract.rs:90-92, bottom loop in `thread_module` thread.rs:49; hooks contract.rs:94-114, bodies runner.rs:834-1076 | Fits with a named change: the loop of section 5 hands exits to the kernel from inside the system call and stop hooks and leaves `run` only when the thread cannot carry on in place |
| Exit: system call | `Exit::Syscall` abi/exit.rs:26 | `HOOK_SYSCALL` contract.rs:94-97; runner.rs:837-865 | Fits with a named change: when the thread leaves from this hook, the hook returns the call's first argument, so x0 in the block is what it was at the call (section 6) |
| Exit: stop flag | `Exit::Stop` abi/exit.rs:28 | `HOOK_STOP` contract.rs:98-100; clears the flag runner.rs:867-892 | Fits as is |
| Exit: memory fault | `Exit::MemoryFault` abi/exit.rs:30 | `HOOK_MEMORY_FAULT` contract.rs:101-103 (read, write); runner.rs:894-922; running code from a page that does not allow it is found at translation, runner.rs:940-953 | Fits as is |
| Exit: fault without registers | `Exit::MemoryFaultWithoutRegisters` abi/exit.rs:33 | the engine's out of bounds trap, runner.rs:628-640 | Fits as is |
| Exit: alignment, illegal instruction, breakpoint | abi/exit.rs:37, :39, :41 | `HOOK_TRAP` contract.rs:112-114, `TrapKind` gcpu/lib.rs:133-154; runner.rs:1047-1075 | Fits as is. x86's kinds (runner.rs:54-65) have no `Exit` yet; ARM64 never raises them; X10 adds them |
| Resume: return a value | `Resume::Return` abi/exit.rs:49 | the system call hook's result, runner.rs:858-862 | Fits with a named change: in place it is the hook's result; for a thread that has left, x0 is written in the block before `run`, as the stand-in does |
| Resume: carry on | `Resume::Continue` abi/exit.rs:51 | the stop hook returns 0, runner.rs:890 | Fits as is |
| Resume: from new registers | `Resume::FromRegisters` abi/exit.rs:55 | `Machine::set_registers` runner.rs:553, then `run` runner.rs:616; a fresh thread instance runner.rs:317 | Fits with a named change: registers the kernel sets while the thread is inside a hook are held and written to the block after the thread has left `run` (section 6) |
| Turn length | `slice_nanoseconds` abi/exit.rs:65-67 | a raiser another host thread can call, runner.rs:542-551; stop at every check, runner.rs:525-530 | Fits with a named change: a turn is counted in progress or ended by the outside raising the flag (section 9); counting progress exactly needs the translator's count (section 9), until then stop checks stand in |
| Idle and finished | `Next::Idle` abi/exit.rs:71, `Next::Finished` :73 | none | Missing: the loop's (section 5); the kernel test driver does it today, driver.rs:526-559 |
| Guest memory (kernel plan 4, line 101) | `Cpu::read`, `Cpu::write` abi/cpu.rs:45-46 | `Machine::read`, `write` over one wasmtime memory, runner.rs:386-396 | Fits with a named change: memories are mappings the runtime supplies to wasmtime, so the kernel reaches any process's memory by pointer, also from inside a hook (sections 6, 8) |
| Zeroing | `Cpu::zero` abi/cpu.rs:48 | none | Missing |
| Address space and memory size | `Cpu::address_space` abi/cpu.rs:35; `AddressSpace` abi/memory.rs:23; the size the guest is told is the same value, container.rs:398 | 64 bit guest memory contract.rs:39-43; size given to `Machine::new` runner.rs:249; array of slots one i32 per `G::ALIGN` bytes, contract.rs:7-11 | Fits with a named change: both sizes become the container's, held in the kernel (asked of the kernel track); `Cpu::address_space` is then only the back end's cap; 0xFF00_0000 for ARM64 in the first form (section 8) |
| New memory (kernel plan 4, line 103) | `Cpu::memory_new` abi/cpu.rs:38; used at exec, loader.rs:285 | one memory per runner, runner.rs:294-296 | Fits with a named change: one wasmtime store, guest memory and state memory per process |
| Duplicate a memory (fork) | `Cpu::memory_duplicate` abi/cpu.rs:41; used by clone.rs:153 | none | Missing (section 8) |
| Drop a memory | `Cpu::memory_drop` abi/cpu.rs:43; task.rs:706, container.rs:652 and :733 | none | Missing: dropping the process's store frees its instances; for the running process, once it has left `run` (section 6) |
| New thread | `Cpu::thread_new` abi/cpu.rs:92; clone.rs:225 and :309, container.rs:649 and :722 | one thread instance, runner.rs:1322-1352 | Fits with a named change: one thread instance and code instances per thread, made at the thread's first run; each thread's block in its process's state memory |
| Drop a thread | `Cpu::thread_drop` abi/cpu.rs:93; task.rs:592 and :675 | none | Missing: the block and instances are kept for the process's next thread |
| Read and set registers | `Cpu::registers`, `Cpu::set_registers` abi/cpu.rs:95-97 | the block in the state memory, runner.rs:572 and :553, `snapshot` :689 | Fits as is for reading; setting as the "from new registers" row says |
| "Code is here" (kernel plan 4, line 107) | `Cpu::code_changed` abi/cpu.rs:64; sent at exec loader.rs:871, for the vDSO loader.rs:723, for executable mappings memory.rs:718, when code stops being runnable memory.rs:471, :495, :516, shared.rs:408 | `ic ivau` drops the pieces of a 64 byte line, runner.rs:1032-1041 and :1113-1119; missing pieces are translated from the bytes now in memory, `load_code` runner.rs:924-1008 | Fits with a named change: the notice drops the range's pieces and keeps its starts; translation happens at first use (section 7) |
| Page permissions (kernel plan 4, line 112) | `Cpu::set_permissions` abi/cpu.rs:52; `Permissions` with `EXECUTE` abi/memory.rs:14 | one byte per page with the next page's bits, contract.rs:62-67 and :126-145; the runner keeps the full permissions, `EXECUTE` included, and rewrites the byte before the range, runner.rs:647-663 | Fits with a named change: the runner's method moves into the back end, per process, and writes each byte as the kernel's permissions less the back end's write locks (section 8) |
| Shared pages (kernel plan 4, line 115) | `shared_new`, `shared_map`, `shared_unmap`, `shared_read`, `shared_write`, `shared_drop` abi/cpu.rs:66-89 | none | Missing: blocks are shared memory objects mapped into the process memories (section 8; measured to work in wasmtime) |
| Stop flag (kernel plan 4, line 118) | abi/registers.rs:54-57 | the stop check and `HOOK_STOP` contract.rs:98-100; `raise_stop_flag` runner.rs:533 and :825 | Fits as is |
| Machine description (kernel plan 10) | `HWCAP`, `HWCAP2` abi/machine.rs:20-24, reported at loader.rs:619 and :941 | `OFFERED_FEATURES` arm64/forms.rs:6-8, which match them; `CTR_EL0` arm64/decode_forms.rs:200-203 | Fits as is for ARM64 (the guest's offer is `()`); X10 brings x86's |
| Clocks | `Host::monotonic_nanoseconds`, `realtime_nanoseconds` abi/host.rs:95-97 | ARM64's `clock` hook (counter reads) arm64/guest.rs:435-440, answered from the runner's own start, runner.rs:1025-1030 | Fits with a named change: the clock hook first tells the outside the progress since the last exit (`ran`), then answers from the kernel's monotonic clock, which the kernel exposes (one public method) and which reads `Host::monotonic_nanoseconds`; so a counter read is a host answer like any other, moves with the work done in a counted turn, and agrees with `clock_gettime`, also across a restore |
| Randomness | `Host::random` abi/host.rs:99 | none (ARM64's `RNDR` is not offered and is an illegal instruction, arm64/decode_forms.rs:251-252) | Missing: the outside's, from the operating system |
| Warnings and counters | `Host::warn` abi/host.rs:101; `Kernel::counters` kernel lib.rs:473 | `HOOK_WARN` contract.rs:110-111; `slow_path` runner.rs:796-815; `Reason` gcpu | Fits with a named change: the back end counts its own slow paths (the translator's reasons and the runtime's) and warns through the outside, kept apart from the kernel's (section 5) |
| Output | `Host::output` abi/host.rs:109 | none: the runner's stand-in answers `write` and `writev` itself, runner.rs:1396-1445 | Missing: the outside passes the kernel's output to the user |
| File backend | `Host::backend_send` abi/host.rs:105, `Event::BackendReply` :61 | none | Missing: the outside's (J5) |
| Network | `connect`, `send`, `receive`, `finish`, `close`, `look_up`, `listen`, `accept` abi/host.rs:115-141; their `Event`s :67-84 | none | Missing: the outside's (J5) |
| Events from outside | `Event` abi/host.rs:57; `Kernel::event` kernel lib.rs:330; `Kernel::kill` :383 | none | Missing: the outside hands them to the kernel after an exit (section 5) |

The kernel's own methods the loop calls are `Kernel::start` (kernel container.rs:383), `next` (lib.rs:291),
`exit` (:303), `event` (:330), `kill` (:383) and `restore` (:408). The translator plan's own items meet the
kernel here: "load new code" (translator plan 7, contract.rs:104-109) and "go to address" stay inside the
back end and never reach the kernel; "the stack is lost" (translator plan 7) is what
`Resume::FromRegisters` and every leaving of `run` cause; the cache key (translator plan 4) is section 7
here.

## 5. The centre: one loop

There is one loop in the runtime crate: `step`, and `drive` which repeats it. The shipped runtime and every
kernel test run through it.
The kernel test driver's loop (driver.rs:437-561) is replaced by it, and the driver's per-turn work moves into
the test outside (section 15). There is no second loop.

**The two interfaces it is generic over**, both resolved at compile time:
- `Execution`, the back end: the kernel's `Cpu` plus `run(thread, resume, turn, answer) -> Result<(),
  Self::Failure>`. `run` runs the thread and hands every exit to `answer.exit(cpu, exit, progress)`, which
  says either "carry on in place" (with the resume and the next turn) or "leave". The back end carries on in
  place when told to and returns `Ok` only when told to leave. `Failure` is what the back end cannot run at
  all: the stand-in's `Unsupported` (driver.rs:458-464, which the tests report as `Failure::StandIn`); for
  `CpuSide<G>` it is `Infallible`, since everything the guest does becomes an exit and a module wasmtime
  refuses is a translator fault that panics with the module's key. Slow paths go to `answer.warn`.
  Implemented by `CpuSide<G>` (WebAssembly) and, in the kernel test crate, by the stand-in, whose `run` loops
  over Unicorn's turns the same way.
- `Outside`: the kernel's `Host` plus five methods the loop calls: `turn(slice)` gives how the next turn
  ends; `ran(progress)` is told how far the thread got; `arrived(kernel, cpu)` hands the kernel what arrived
  since the last exit (events, signals) and says whether something needs the thread out of `run` first;
  `stopped(kernel, cpu) -> ControlFlow<Self::End>` does that something once no thread is in `run` (a save,
  section 11) and may end the run with its own result; `idle(kernel, cpu, until)` waits for an event or the
  time. Implemented by the shipped outside (J3), by the test outside (J1), and by any user who brings their
  own: the outside is a type the user supplies, never built into the loop.

**The loop** is one step, `step(kernel, cpu, outside) -> Result<Step<O::End>, E::Failure>`, called again and
again:
1. `next` is the answer kept from the last step, or `kernel.next()`.
2. `Run { thread, resume, slice }`: `cpu.run(thread, resume, outside.turn(slice), answer)?`. The answer to
   each exit is: `outside.ran(progress)`; `kernel.exit(cpu, outside, exit)`; `outside.arrived(...)`, and leave
   if it asks; then `n = kernel.next()`. If `n` runs the same thread with `Return(v)` or `Continue`, carry on
   in place with `outside.turn` of its slice. Anything else is kept as the next step's answer and the thread
   leaves. When `run` returns, `outside.stopped(...)`: `Break(end)` gives `Step::Ended(end)`, otherwise
   `Step::Ran`.
3. `Idle { until }`: `Step::Idle { until }`, handed to the caller.
4. `Finished { status }`: `Step::Finished { status }`.
`drive` is that step in a loop, calling `outside.idle` at `Idle`, until `Finished` or `Ended`. A user with
their own event loop (the owner's other project drives many kernels) calls `step` instead, waits in its own
way at `Idle`, and hands what arrived through `kernel.event` before the next step. So the runtime runs to the
end in one call or is stepped from outside, through the same code.

With the stand-in this makes exactly the kernel calls, in the same order, that the driver makes today, and
draws the seed's numbers in the same order, so the stand-in's results do not change (J1 checks this). The
kernel never waits on its own stack and never sees WebAssembly; the translator never sees Linux.

**Carrying on in place in the WebAssembly back end.** In the system call hook, "carry on with `Return(v)`"
returns `v` from the hook: no stack switch. In the stop hook, "carry on with `Continue`" returns 0. "Leave"
raises the thread's flag and returns, so the thread leaves `run` at the next stop check with its registers
saved (contract.rs:94-100). A memory fault or a trap always leaves (contract.rs:101-114); its answer is
kept for the loop. The load code hook is the back end's own (section 7).

**A thread that has left and runs again** gets its resume applied before `run`: `Return(v)` writes `v` to x0 in
the block, as the stand-in does (standin.rs:446-448); `Continue` changes nothing; `FromRegisters` writes the
registers the kernel set, and the thread starts from the bottom loop. So a call that waited and is woken later
returns its value exactly as one answered in place.

## 6. Inside a hook: the kernel calls back into the back end

The kernel runs inside a hook of the running process's store and calls `Cpu` methods on the back end that owns
that store (`&mut dyn Cpu`, kernel lib.rs:303). The rules that make this sound and simple:
- **The running store is out of the map.** `run` takes the process's store out of the back end's table of
  processes for the duration of the call. The hook reaches the back end and the loop's answer (`&mut dyn
  Answer`, section 3) through pointers in the store's data, set for that one call (one `unsafe`, with its
  reason). So the back end is free to be handed to the kernel as `&mut dyn Cpu` while the store is
  borrowed.
- **No `Cpu` method needs the running store.** Guest memories are host-supplied mappings and state memories
  are wasmtime shared memories, which are reached without a store; so `read`, `write`, `zero`,
  `set_permissions`, `code_changed`, the shared block calls, `registers`, and `memory_duplicate` (reading the
  parent) act by pointer. `memory_new` makes a new store.
- **Instances are made at a thread's first run**, when its store is at hand: `thread_new` only takes a block
  in the state memory. So a new thread of the running process (clone.rs:225) and the first thread of an
  exec'd program (container.rs:649) need no store inside the hook. New code made by the load code hook is
  instantiated through the hook's own `Caller`, in every thread of the process.
- **Drops of what is running wait until it has left.** `memory_drop` of the running process (exit,
  task.rs:706; exec, container.rs:733) and `thread_drop` of the running thread mark them; the store is
  dropped once `run` has returned. The kernel never runs a thread of a dropped memory, so the next answer
  always leaves.
- **Registers set inside a hook are held.** `set_registers` on the thread in the hook keeps the registers
  aside, `registers` on it returns them, and they are written to the block after the thread has left (the
  abi promises `FromRegisters` after any change, abi/exit.rs:47-55, so the thread always leaves).
- **x0 at a leave from the system call hook.** The hook returns the call's first argument when the thread
  leaves, so the block holds exactly what the stand-in's does at `Exit::Syscall` (x0 unchanged, pc after the
  call) for a call that waits, is interrupted by a signal, or is restarted.
- **Code changed under a running thread.** A notice that drops pieces of the running process makes the
  thread leave (counted "left run: code changed under it"), so nothing translated from the old bytes runs
  after the call returns. A notice that drops nothing (a new mapping) does not.

## 7. Translation: where, when, what is cached

**One primitive, one place.** All translation happens in the load code hook, when the bottom loop reaches
an address with no piece: at a thread's first instruction, after a jump to code not yet translated, after a
stack loss, and after code changed. The "code is here" notice translates nothing. It drops what was
translated from the range (clears the range's entries in the process's array of slots) and keeps the
range's starts. Why not at the notice: a range announced is often not run (a library mapped and never
called), and code a program writes is announced again at every permission change, so translating at the
notice would translate what never runs and translate changing code again and again. At first use each
range is translated once, when it is needed.

**What is translated.** At first use of an address in a range announced by a notice, the whole announced
range is translated at the container's look-ahead, from its starts, and the translator finds what else it
can (call targets, built addresses). The starts are:
- the notice's own (the program's entry at exec, the vDSO's return path);
- when the range is the code segment of an ELF image in memory, the starts the translator's M5 takes from
  a file (translator plan 5; crates/wcontainers-translator-test/src/binaries.rs:225-330 at 84e9a937), read
  from the image as it is in memory: its entry and `DT_INIT`/`DT_FINI`; the functions of its dynamic symbol
  table; every function in its unwind table index (`PT_GNU_EH_FRAME`); every address into the range that its
  relocations leave in data (`RELA` from each entry's addend; `RELR` from the word at each place); the init,
  preinit and fini arrays (`DT_INIT_ARRAY` and the others of the dynamic table); and, for an image that is
  not moved (`ET_EXEC`), every aligned word of its other `PT_LOAD` segments that points into the code. The
  runtime finds the image by its ELF header on a readable page below the range, and accepts it only if the
  header's executable `PT_LOAD` maps exactly onto the range. A shared library mapped by the program's own
  loader brings no starts in its notice, and this is where its starts come from, with no change to the
  kernel. Reading uses the `object` crate's ELF types.
`RELR` is where the starts of Alpine's programs are: every binary of the translator's list except python
keeps all its relative relocations there (measured, section 18: busybox 1074 `RELR` places, 1046 into code,
and no relative `RELA`; musl 80 and 45). A word in data is read either as it is (not yet relocated: musl's
own words when its entry first runs, since it relocates itself) or less the image's load offset (relocated:
busybox's words when its entry first runs, after musl has relocated them); each reading that falls in the
code is a start. A word the program has changed by then gives a different start or none. That is never
wrong: a start only says where translation begins, an extra start costs translation time, and a missing one
a piece made at run time. The starts are part of the cache key, so the same bytes with the same starts give
the same entry. Two things differ from the translator's runner: the static symbol table (`.symtab`) is not in
memory, and the address first reached is not a start, so that programs that first reach a library at
different places share its entry. The runner's measured counts (translator status, M5) are therefore the
expectation for dynamically linked programs, and J1 and J7 record how many pieces each program needed at run
time. Reading M5's starts from busybox's file took 0.07 ms, musl's 0.08 ms and libpython's 1.37 ms (measured,
section 18), so reading them before each cache lookup is cheap.

An address with no piece (an unknown jump target, a resume after a stack loss) is translated at bounded
look-ahead from it, as the translator plan says. At look-ahead none every piece is one instruction, as the
reference, and the rule is the same: the whole range at first use, which the translator follows from the
starts. Measured (section 18): busybox at none is 135,972 pieces in one module, compiled in 8.45 s with the
process peaking at 3.1 GB; musl 85,954 pieces, 5.09 s; libpython 703,235 pieces, 113.9 s, 16.9 GB. Translating
at none address by address instead would make one module per instruction reached, each a separate compile,
which is slower for any program that runs more than a few thousand instructions. So none is used for the
programs of J1 to J5, where the cache's disk level pays the compile once per build, and not for the large
runtimes of J6 (section 15), as the translator plan says of none ("not meant for large blobs").

**Code a program writes (Node, Java, any JIT).** A program must invalidate the instruction cache line
(`ic ivau`) before running code it wrote, because the `CTR_EL0` we present says invalidation is needed
(arm64/decode_forms.rs:200-203); the guest's code changed hook drops the pieces of that line
(runner.rs:1113-1119), and permission changes bring a notice. A range whose bytes change after it was
translated is from then on translated only at bounded look-ahead, from each address reached. Slow path:
"code changed after it ran", counted with its range and warned. No setting. Each new translation is a new code
module instantiated in every thread's store, and wasmtime frees instances only with their store, so a
long-running JIT grows its process's store and its lookup table. J6 measures that growth on Node and Java;
if it matters, the candidate is to move the process to a fresh store holding only the code in use, as exec
already does, decided by that measurement.

**The cache.** A translation is cached as wasmtime's compiled module and the translation's piece list. Key:
- BLAKE3 hash of the code bytes (the range as it is in memory),
- the starts given (blob offsets), sorted,
- the look-ahead and what it is optimised for (the container's settings),
- the guest (`G::ID`), the host probe's answer and the guest's offer,
- the back end (WebAssembly under wasmtime),
- the translator's build identity: a hash of the translator's and the guest crate's sources made at build
  time, so any change to them changes the key without manual version numbers,
- wasmtime's own compatibility check, which `Module::deserialize` makes.
Output never holds an absolute address (translator plan 4), so one entry serves every process that maps
the same bytes at any address. Two levels: compiled modules kept in memory for the runtime's life, shared
by every process and container; and a directory on disk, if the user gives one, one file per key, which
any runtime of the same back end and build can share. Measured need for the disk level: busybox compiles in
1.12 to 2.08 s and comes back from disk in 2.77 to 3.59 ms; libpython in 6.60 s against 12.56 ms. Hashing the
code each time a range is first run costs 0.16 ms for musl, 0.30 ms for busybox and 1.28 ms for libpython.

**Starts learned at run time.** Addresses translated at run time inside a cached range could be recorded
with its entry, so the next process that maps the same bytes translates them ahead. This is built only if
J1 and J6 measure that run-time translation is a real cost (one piece costs 1.1 to 6.7 ms in wasmtime,
measured at the translator's M4).

## 8. Memories

**One store per process.** wasmtime frees instances and memories only when their store is dropped. A shell
that forks thousands of times, each child with its own memory and instances, would grow without bound in one
store. So each process has its own store, holding its guest memory and its threads' instances, dropped
with the process (exec makes a new one). The engine and compiled modules are shared by all.

**Host-supplied guest memory.** Each guest memory is a mapping the runtime makes and hands to wasmtime
(`Config::with_host_memory`), with `memory_may_move(false)`. Because the runtime owns the mapping:
- the kernel reads and writes any process's memory by pointer, also while a thread is inside a hook;
- a block of shared pages is mapped into it at the same place in every process that maps it;
- fork copies it with the host's own exact copy (below), which on macOS copies each page on its first
  write.
Measured on this Mac in wasmtime 50: a 1 MiB block of shared pages mapped into two host-supplied guest
memories at different addresses; 1000 of 1000 stores through one memory were seen by loads through the
other; mapping the block took 0.6 µs. This settles the kernel plan's open question of section 17 ("sharing
pages through host-supplied memory in wasmtime") for macOS; Linux is checked at J2. Mappings use `rustix`
(an anonymous shared memory object for each block: `memfd_create` on Linux, `shm_open` on macOS); the
macOS copy uses `mach2`.

**The address space and the memory size are the container's.** Both are held in the kernel's state, saved
with it, and the same on every machine and every back end; they are never read from where the container
happens to run. Today the kernel takes both from `Cpu::address_space` (container.rs:398, loader.rs:476,
task.rs:695), so the stand-in's 4 GiB (standin.rs:22) would differ from this back end's. Asked of the
kernel track: `Start` carries the container's address-space end and memory size, the kernel keeps them, and
`Cpu::address_space` becomes only the back end's cap, which the kernel checks at `start` and `restore`
(a container larger than the cap does not start, with the error said, never a smaller space in silence).
The runtime gives one value to every container, 0xFF00_0000 for both, on the stand-in and on every back end,
for the reason below. It is not a user setting.

**State memory per process.** The translator keeps threads' blocks, the page permission table and the array
of slots in one shared 32 bit memory (contract.rs:44-67), at most 4 GiB. The array has one 32 bit entry per
`G::ALIGN` bytes of guest address it covers (contract.rs:7-11). Code sits at both ends of a process (the
program low, libraries and the vDSO below the top), so the array must cover the whole address space: 4 /
`G::ALIGN` bytes of array per byte of address. The back end derives the layout from the container's size and
`G::ALIGN`: 16 MiB for the permission table and thread blocks, then the array from 64 KiB to the end. For
ARM64 (`ALIGN` 4, arm64 guest.rs:151) one byte of array per byte of address, so the end 0xFF00_0000 (4 GiB
less 16 MiB) is what fits beside the 16 MiB in 4 GiB. Untouched pages cost nothing. 16 MiB leaves room for
about 18,000 threads in one process; past that `thread_new` answers `CpuError::OutOfMemory` and the guest's
`clone` fails with the error the kernel gives for that, never a thread that does not run.

x86 (`ALIGN` 1, x86 plan 3.5) needs four bytes of array per byte of address: about 16 GiB for the same
0xFF00_0000, which no 32 bit memory holds. So before x86 runs in the runtime, the translator's contract moves
the array of slots into its own 64 bit memory, sized from the container's size and `G::ALIGN`, for every guest
at once (asked of the translator and x86 tracks; the cost of a lookup in a 64 bit memory is theirs to
measure). The guest memory is already a 64 bit memory (contract.rs:39-43). One contract, one layout, and the
container's size unchanged: an x86 container and an ARM64 container are both told 0xFF00_0000. The back end's
layout code is the only place in the runtime that changes.

**Fork.** `memory_duplicate` makes the child's store, guest memory and state memory, then:
- copies every run of private pages that may hold data, exactly, whatever the host has done with the pages.
  "May hold data" is a bit of the back end's per-page record: set when a page is given any access or written
  through `Cpu::write`, cleared when `Cpu::zero` covers it, so a page made inaccessible after it was written
  is still copied. Blocks of shared pages are skipped. Guest memory holds everything the kernel loaded, code included, not only what Linux counts as
  anonymous. The copy is one operation with one body per host operating system:
  - macOS: `mach_vm_copy` of each range, which copies exactly by construction (the system shares the pages
    and copies each on its first write in either process), so a page that is compressed or paged out is
    copied like any other;
  - Linux: the pages `/proc/self/pagemap` shows present or swapped (bits 63 and 62), copied with `memcpy`; a
    page that is neither has never been written, or was given back, and reads as zero in both. Measured at J2.
  Residency (`mincore`) is not used: it reports a compressed or swapped page as not resident, which would
  give the child zeros;
- maps the same blocks at the same places;
- writes the child's permission table from the back end's record, and rebuilds its array of slots from the
  process's code modules and their piece lists at the same slot bases; neither is copied;
- leaves instances to the child's first run, which instantiates the process's code modules at the same slot
  bases.
Measured on macOS (section 18): a new 4 GiB memory 6.3 to 15.6 µs. `mach_vm_copy` of busybox sh's ranges
(11 MB mapped, 1.7 MB written: both images, heap and stack, more than the reference's 892 kB resident) 19.1 µs,
against 115.6 µs to `memcpy` the written bytes; a python-sized process (24 MB mapped, 8.6 MB written) 198 µs
against 611 µs; java's (104 MB, 44 MB) 1.29 ms against 3.35 ms; node's (88 MB, 47 MB) 0.48 ms against 3.52 ms.
The copy was exact in every case (the child saw the parent's bytes, and neither saw the other's later
writes), and 2000 forks of the same parent in a row stayed at 18 to 20 µs each. Each first write after the
copy is a fault the system settles (the parent rewriting all 1.7 MB took 266 µs); a shell's child writes a
few pages before exec. The array of slots holds one entry per piece: 5,344 for busybox and musl at full
(3,520 and 1,824, translator status M5), so rebuilding it is expected to take microseconds (not measured).
The kernel's own share of fork, exit and wait is 2.42 to 2.65 µs. Instantiating code modules for the child's
thread costs 0.43 to 0.60 ms for busybox and 0.27 to 0.42 ms for musl; wasmtime's pooling allocator made no
difference. So a shell's fork is expected to cost about 0.75 ms, almost all of it instantiation (an
expectation from parts measured apart). unkernel's fork was measured at 20 µs. Making instantiation cheap is
J7's first task, after measuring where its time goes (the runner builds a new linker and its host functions
for every instance, which the runtime need not).

**Page permissions and the notices.** The back end keeps, per process, each page's full permissions as the
kernel last set them (execute included) and writes the table bytes from them, as the runner does
(runner.rs:647-663). Reading and writing through `Cpu` ignores permissions (abi/cpu.rs:31-32). One layer
sits on top, written by the back end alone: each page may carry write locks, each with its reason
(translated code written without an announcement, x86 plan 5.7, which ARM64 does not need; tracking written
pages; copying a page aside before its first write, section 12; shared pages on a host that cannot share,
section 17). The table byte is always the kernel's permissions less write wherever a lock stands, made by one
function from the record; no lock has its own override. A write that faults on a locked page goes to the
back end, which settles every lock on the page by its reason, and then either lets the write go on (the
kernel's permissions allow it) or hands the kernel its `Exit::MemoryFault` (they do not); the kernel never
sees a lock. Running code is checked at translation: an address on a page that does not allow running gives
`Exit::MemoryFault` with `Access::Execute`. With one guest thread running at a time, every other thread is
stopped whenever the kernel changes a table, as contract.rs:17-27 requires.

## 9. Threads, turns, the stop flag, signals, faults and exact registers

- **Threads.** Each guest thread has its block in its process's state memory and its own thread instance and
  code instances (one instance per thread, translator plan 6). Code made at run time by one thread is
  instantiated in every thread of the process at once, with the same slot, so every thread's table holds the
  same piece in the same slot (contract.rs:10-11). A dropped thread's block and instances are kept and given
  to the process's next new thread, so a program that starts and ends many threads does not grow its store.
- **Progress** has one meaning on every back end, at every look-ahead and in both settings: a count of the
  guest's own instructions. Where the translator puts its stop checks changes with look-ahead, with the
  starts and with code made at run time, so a count of stop checks would differ between settings and back
  ends; it is not the unit. The unit is the owner's ruling, not yet made; until then it is a placeholder,
  the proposal of the owner's requirements map: every executed transfer whose target is at or below the
  transferring instruction (taken branches and calls, and indirect branches and returns compared at run
  time), plus one at every change of pc the kernel makes (a signal delivered, `rt_sigreturn`, exec). Between
  two counted events the pc only rises, so (count, pc) names a point. The count is thread state, kept with
  the block (the map proposes a `progress` word and a `limit` word after the register block, in its padding),
  so fork, saves and moving a thread carry it. Who counts:
  - the stand-in, from J1: Unicorn's block hook gives each block's address and size, so a block that starts
    at or below the end of the block before it is a counted transfer (exact for the guest's own transfers;
    the kernel's changes of pc are added with the ruling, since only the kernel tells a signal from a
    restore);
  - the WebAssembly back end, once the translator emits the count (the map's design: `progress += 1; if
    progress >= limit`, the stop check and the counter as one piece of code, raising the flag becoming
    `limit = 0`; asked of the translator track, which owns the emitted code and its measurement). Until then
    it reports stop checks passed in its place: a named gap ("progress counted by stop checks"), counted and
    warned once per container, recorded in the status file. Nothing in J1 to J3's bars depends on where a
    counted turn ends (each compares output with the reference); J4's seeded thread switches do, so J4 is
    built on the translator's count.
- **Turns.** The outside turns the kernel's slice into a `Turn`, one of two kinds:
  - *Counted*: the back end counts progress and stops the thread after the given amount (or never, with no
    limit). Until the translator's count exists, the WebAssembly back end raises the flag at every check, as
    the runner's "stop at every check" does (runner.rs:525-530), and its stop hook counts and carries on in
    place until the count is reached, without calling the kernel; with the count, the limit word does this
    in the emitted code. `progress` handed with each exit is the count since the last one. The test outside
    always uses counted turns, with one unit as one nanosecond on its repeatable clock, as the stand-in's
    instruction does today, so a seed repeats exactly, and the clock moves with the work done whether or not
    the kernel set a slice. A counter read (the clock hook) reports the progress since the last exit before
    it reads the clock (section 4), so a guest that reads the counter in a loop sees time move within one
    turn. Counting at every check costs a hook call per check; it is for tests, never for speed.
  - *Free*: nothing is counted; the turn ends when the outside raises the running thread's flag. The back
    end gives the outside a `Send` handle that raises whichever thread is running (an atomic store to the
    state memory, as runner.rs:542-551). The shipped outside arms one timer thread with it at the slice's
    deadline on the host's monotonic clock; with no slice no timer is armed. Costs nothing between checks.
- **Events from outside** (host timers, backend replies, network, terminal input) arrive on host threads. In
  a free turn one arriving raises the running thread's flag; the outside hands it to the kernel in
  `arrived`, at the next exit. The test outside models arrival as a counted limit drawn from the seed, as the
  driver does today (driver.rs:446-457).
- **Signals** are the kernel's (kernel plan 6). Delivery comes back as "run this thread from new registers",
  so the thread leaves `run`, the new registers are written, and the handler runs from the bottom loop. A
  return through the vDSO's `rt_sigreturn` is a system call like any other.
- **Faults and exact registers.** Every hook is called with the registers saved (contract.rs:13-15), so the
  block is exact at every return to the kernel: `pc` after the system call instruction, at the stopped
  instruction, at the faulting instruction or trap (abi/exit.rs:21-42). The one exception is an access
  beyond the memory, which the engine stops and which becomes `Exit::MemoryFaultWithoutRegisters` (on the
  written list of differences, kernel plan 15).
- **The private words** of a thread (ARM64: whether float flags are tracked, and the exclusive monitor) are
  the back end's and are not guest state. The monitor is cleared at every exit handed to the kernel: a
  store-exclusive may always fail and programs retry it, so clearing is correct at any point, as when Linux
  moves a thread between processors. Float tracking is a speed hint, and "tracked" is always correct, so it
  starts as "tracked" whenever a thread is made from a block (a new thread, a fork child, a restore, a thread
  from another back end). So fork, saves and moving a thread take the block and nothing else.

## 10. System calls

A call that finishes at once is answered inside the system call hook (section 5). A call that waits, or
whose answer switches threads or sets new registers, leaves `run`. Measured through the translator's runner
as it is:
- answered inside the hook (`brk`, which the runner's stand-in answers at once): 10.5 ns a call (median of
  7 alternating pairs against the same loop with a `nop`; min 9.4 ns);
- leaving `run` and coming back: 300.8 ns a call (median of 5; min 285.7 ns).
The kernel's own share of a call, measured with the layer 1 CPU side (no instructions): 116 to 120 ns for
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

## 11. Saving and bringing back

Saving copies the kernel's memory, every process memory, every block of shared pages, and each thread's
block (kernel plan 12); the private words are not saved (section 9). Translated code is never saved: it is a cache.
Bringing back makes a fresh engine, stores and instances; every thread starts from its registers
(`Resume::FromRegisters`, which the kernel's `restore` gives every thread, kernel lib.rs:408-450); code comes
back from the cache's files or is translated again at first use. The kernel's clock carries on through its
own offset, and the clock hook answers from that same clock (section 4).

In the first form the kernel is Rust data spread over the runtime's own heap, not one memory that can be
copied to a file. So the first form supports exactly what the kernel's tests do today: a save made by
copying the whole test process with `fork` (driver.rs:494-524), brought back in the copy on a new back end made
from the saved memories and threads. That save happens in the test outside's `stopped`, when no thread is in
`run`. The copy holds only the forking thread, so:
- **Traps.** On macOS wasmtime handles traps with Mach ports by default, and its documentation says a child
  made by `fork` then cannot run WebAssembly (wasmtime 50, `Config::macos_use_mach_ports`). The back end's
  engine always uses Unix signal handling instead (`macos_use_mach_ports(false)`), which a copy inherits.
  Traps are rare (an access beyond the memory), so this costs nothing on the paths that matter.
- **Compiling.** wasmtime compiles on rayon's pool, whose threads are not in the copy. The back end made by
  `restored()` in a copy makes a new engine that compiles on the calling thread, and takes compiled code
  from the cache's directory (the tests give one under the target directory) or compiles it there.
- **Timers.** Tests use counted turns (section 9), so no timer thread exists to be lost.
This is the same bringing back as a restore in a new process, which the final form does from a file, except
that the copy compiles on one thread.

Saving to a file comes with the final form, where the kernel's memory is a WebAssembly memory (section 13).

## 12. What the owner's other project needs of the runtime

The owner's project (distributed-everything design.md sections 6, 7 and 18) needs the following. None is
built in this track's milestones unless named; each has its place, and nothing here has to be undone for it.

- **More than one way to run the same guest.** The place is `Execution` (section 5): the loop, the outside
  and the kernel know nothing of WebAssembly. A native back end (the translator's second output, confined by
  inline checks, by host processes or by hardware virtualization) is another implementation of the same
  trait, chosen at compile time; a build names the back ends it holds. The cache key names the back end.
- **Guest state is the same on every back end:** the register block (with the progress count, section 9)
  and the guest memory, and nothing else. At every return to the kernel after a thread has left `run`, that
  is all a back end needs to continue it: the private words are cleared or reset (section 9). Moving a thread
  is then `FromRegisters` on the new back end with the same memories.
- **Translated code cached per back end and per settings, as files that can be shared:** section 7's disk
  level, one file per key, with the back end, the container's dials and the build identity in the key.
- **Dirty pages and point-in-time copies.** The place is the back end's write locks (section 8), the one
  layer over the kernel's permissions: a mark puts a "tracking written pages" lock on every writable private
  page; the first write to a page faults to the back end, which records the page as written, copies it aside
  first if a point-in-time copy is open (its own lock reason), lifts both locks, and lets the thread go on;
  the kernel never sees it. Slow path "first write since a mark", counted. Two methods on `Execution` (pages written since a
  mark; open a point-in-time copy) are added when a user exists; not built now.
- **Repeatable runs.** The kernel takes every input through its `Host`, its `Cpu` answers and its events, so
  it is a pure function of what the loop hands it. Every source of non-repeatability enters the runtime at
  one of these places today:
  - the clocks: `Host::monotonic_nanoseconds`, `Host::realtime_nanoseconds` (the outside);
  - randomness: `Host::random` (the outside); `AT_RANDOM`, `getrandom` and `/dev/urandom` all come from it;
  - timer instructions: ARM64's counter reads go to the guest's `clock` hook (arm64/guest.rs:435-440), which the
    back end answers from the kernel's clock, so through `Host::monotonic_nanoseconds`, a logged answer; x86's
    `rdtsc` comes the same way when x86 lands;
  - the address-space size and memory size: the container's, held by the kernel (section 8), never the
    host's;
  - random number instructions: ARM64's `RNDR` is not offered and is an illegal instruction
    (arm64/decode_forms.rs:251-252), so it is repeatable as it is;
  - where turns end and where events land: the stop flag's timing in free turns (the outside's timer and
    event threads), and which exit an event is handed in after (`Outside::arrived`);
  - the host's answers: `backend_send` and the network calls' results, `Event::BackendReply`, the network
    events and terminal input (the outside), and when `idle` returns;
  - the back end's own answers to the kernel: `CpuError` from `memory_new`, `memory_duplicate` and
    `thread_new` when the host runs out of memory.
  So a recorder is one more outside wrapping the real one, logging each of these, and a replayer is an
  outside answering from the log, with every event handed at the exact (count, pc) it was handed at. That
  needs the guest-defined count of section 9 in the shipped back end at full speed, with the limit word
  stopping a thread at an exact count; it is the translator's change to make and measure (asked of the
  translator track). To reach a recorded (count, pc), the back end runs to the count, then steps at
  look-ahead none until pc matches, as the map proposes.

## 13. The first form and the final form, as one path

**First form.** One native program: the runtime embeds wasmtime, the kernel is linked in as a Rust
library, and the guest's code is the translator's WebAssembly. The hooks call the kernel directly. This is
the translator runner's shape with the stand-in kernel replaced.

**Final form.** The same runtime; the kernel is the WebAssembly module the kernel track's K7 builds,
instantiated in its own store. The kernel module imports the `Cpu` and `Host` operations, which the runtime
provides with the same back end and outside; the loop calls the kernel module's exports instead of the Rust
methods. Saving is then copying memories, as the kernel plan intends.

**Why this is one path.** The loop, the back end, the outside, the cache and every test are the same in both.
What changes is only where the kernel's six methods (`start`, `next`, `exit`, `event`, `kill`, `restore`) are
reached: by a direct call, or by a call into the kernel's instance. At J9 the loop takes the kernel as a type
with those six methods, implemented by the native kernel and by the kernel's instance, chosen at compile
time like the back end. Both are the same kernel source built two ways, not two runtimes: the loop, back
ends, outsides, cache and tests are one. The natively linked kernel stays a first-class form: nothing in the
runtime works only when the kernel is its own WebAssembly instance (saving to a file, the one thing that
needs the kernel's memory as one piece, is offered where that form is built). Whether the shipped form on
native machines is the linked kernel is the owner's decision. What K7 must export for this (the kernel's six
methods as exports, `Cpu` and `Host` as imports) is asked of the kernel track.

## 14. Crates and the smallest user surface (proposals)

**Crates.**
- `wcontainers-runtime` (MIT OR Apache-2.0, shipped): `step` and `drive`, the `Execution` and `Outside` traits, the
  shipped outside for an operating system (clocks, randomness, output, files, network, the timer thread),
  and the user surface, generic over the back end. Depends on the kernel and the abi; never on wasmtime,
  Unicorn or any test crate.
- `wcontainers-wasmtime` (MIT OR Apache-2.0, shipped): `CpuSide<G>`, the WebAssembly back end, and its
  cache. Depends on the runtime crate, the translator, the guest interface and the ARM64 guest, wasmtime,
  rustix, blake3, object (0.41, the kernel's version, so one build holds one) and, on macOS, mach2. The split keeps wasmtime out of every build that does not run WebAssembly
  (the kernel tests on the stand-in, a later native back end).
- No new test crate. The kernel test crate's driver calls `drive`; its stand-in implements `Execution`; its
  `TestHost` implements `Outside`, holding the per-turn work the driver loop does today (backend answers,
  notices, signals and saves at the seed's moments, the repeatable clock). A cargo feature `runtime` of that
  crate puts `CpuSide<Arm64>` behind every one of its tests (layers 1 to 3). One loop, one set of tests, the
  back end chosen when the test binary is built. This is the one change outside the runtime's own crates;
  it is made in J1 on this branch as its own commits and named in the status file, because the kernel
  track also works in that crate.

**User surface (proposal; names are the owner's).** A library first; a command line program only if the
owner wants one.

```rust
let runtime = Runtime::<CpuSide<Arm64>>::new(RuntimeSettings {
    cache: Some(path), // where compiled translations are kept between runs; None keeps them in memory
})?;
let mut container = runtime.container(&ContainerSettings {
    image: Image::Tar(path),           // a `docker export` or image layer tar, unpacked into the kernel
    command: vec!["sh".into(), "-c".into(), "echo hi".into()],
    environment: vec!["PATH=/usr/bin:/bin".into()],
    working_directory: "/".into(),
    hostname: "container".into(),
    look_ahead: LookAhead::Full,       // the translator's dials
    optimise: Optimise::Speed,
}, System::new(output))?;              // the outside: the shipped one, or the user's own `Outside`
let status = container.run()?;         // to the end; or `container.step()` from the user's own event loop
```

The container's settings are what `docker run` gives the kernel today (`Start`, kernel container.rs:52-64)
plus the translator's two dials, and nothing else; the address-space and memory sizes the kernel is given are
the runtime's one value (section 8), not settings. The outside is a parameter: `System` (the shipped outside,
J3) takes byte streams for standard output and error, and a user such as the owner's other project passes
its own type implementing `Outside`. Standard input is not in the surface yet: the kernel gives the first
process /dev/null as descriptor 0, the abi has only output streams, and terminal input events are counted
as unexpected, so what the abi needs for input is settled with the kernel track before J3 fixes the
surface. Mounting a host directory through the file backend, published ports and a terminal come with the
milestones that build them (J5), each as one more field. Tar images are read with the `tar` crate (already
used by the kernel test crate).

## 15. Testing

- **Layer 2 and 3 (real programs, LTP).** Every test of the kernel test crate runs with `--features runtime`.
  The bar is the same verdict as the same test with the stand-in, and as the reference, test by test. A test
  the stand-in cannot run (its emulator's alignment limits, driver.rs `Failure::StandIn`) must pass with the
  runtime against the reference.
- **The CPU side's warnings.** The back end's slow paths reach the test outside apart from the kernel's, kept
  in their own list beside `Run::warnings`, so the kernel tests' assertions about the kernel's warnings
  stay exactly as written. Each milestone lists the CPU side's warnings per test in the status file, by
  reason and count.
- **Layer 1.** The same sequences with `CpuSide` in place of the layer 1 buffer CPU: real host-supplied
  memories, permission tables, blocks of shared pages and fork copies, no instructions (the reading of the
  brief's layer 1 bar in section 1). Zero mismatches over
  the kernel's bar of one million sequences (a long run for the orchestrator; seeded, one command, with a
  knob for the count).
- **At every look-ahead.** The kernel test crate's look-ahead comes from the environment, as its seeds do
  (`SCHEDULE_SEED`, lib.rs:150-161): `LOOK_AHEAD` is `full` (also when unset), `none`, or a number N for
  bounded. Layer 2 runs at full, at none (the reference) and bounded with N from 1 to 64, for the programs of
  J1 to J5; the K6b runtimes of J6 run at full and bounded, not at none (section 7: libpython at none
  measured 113.9 s to compile and 16.9 GB). Injections (the stop flag, a signal, a thread switch, save and
  bring back) come from the test outside, at the seed's moments, as the driver gives them today. The tests
  give the cache a directory under the target directory, so a translation at none is compiled once per
  build.
- **The stand-in unchanged.** Without the feature, every test gives the same verdict as before J1, and a
  seeded run's output, counters and turn count are the same before and after the driver moves onto `drive`.
  The stand-in's change of unit (instructions to the placeholder count, section 9) is a later commit,
  checked to change no verdict.
- **Benchmarks.** The translator's benchmark set (programs/bench.c and the others its benchmark test runs),
  built as ordinary static programs, run in a container on the shipped outside (free turns), and in the
  translator's runner. The bar is instructions retired by the process during the guest's run, after
  translation and compilation (macOS `proc_pid_rusage`, `ri_instructions`), median of 3 on each side,
  within 5%. Times are reported beside it as the median ratio of 11 alternating pairs with the load, but are
  not the bar: identical runs vary 10 to 15% in time on this machine under load (translator status), while
  instructions retired by one program varied 0.17% over 8 runs at load 88 to 102 (section 18). The benchmark
  programs are static and not moved, so the runtime reads the same starts as the runner from their data
  words; it lacks only their `.symtab` (section 7), so the pieces each made at run time are reported beside
  the bar. One command.
- **System call and fork cost.** A static program looping on `getppid`, and one looping on fork, exit and
  wait, run on the runtime and in the reference; reported per call with the machine and load, beside
  unkernel's 56 ns and 20 µs.

## 16. Milestones

Each bar is checkable by running the named command on this Mac with Docker.

- **J1, first light: a static hello, then busybox's single programs.**
  - Merge 84e9a937.
  - `wcontainers-runtime`: `step`, `drive`, `Execution` (with `Failure`), `Outside` (with `End`), `Answer`,
    `Turn`, `Step` (section 5).
  - `wcontainers-wasmtime`: host-supplied guest memories and per-process stores and state memories laid out
    from the address-space end and `G::ALIGN` (section 8); `memory_new`, `memory_drop`, `read`, `write`,
    `zero`, `set_permissions` (with the lock layer, no lock reason used yet), `code_changed`, the shared block
    calls, `thread_new`, `thread_drop`, `registers`, `set_registers`, with the rules of section 6; translation
    at first use with the starts of section 7 and the in-memory cache, plus the disk level for the tests'
    directory; `run` with carrying on in place, leaving, and counted turns (stop checks standing in for
    progress, section 9, with its warning); the clock hook through `ran` and the kernel's clock; traps and
    faults to exits; the monitor cleared at every exit; the engine with Unix signal traps.
    `memory_duplicate` answers `CpuError::NotSupported`, which the kernel counts (no program in this bar
    forks). `Failure` is `Infallible`.
  - The kernel test crate, as its own commits, in this order: (1) the stand-in implements `Execution`
    (`Failure` = `Unsupported`), `TestHost` implements `Outside` with the driver's per-turn work, its
    `stopped` makes the save by copying the process and answers `Break` with the copy's report in the parent
    (driver.rs:494-524), and the driver calls `drive`; checked: a seeded hello run's output, counters and turn
    count unchanged. (2) The `runtime` feature and `LOOK_AHEAD` (section 15). (3) The stand-in counts the
    placeholder unit with Unicorn's block hook; checked: every test without the feature keeps its verdict.
  - The kernel's interface asks of section 8 (the container's sizes in `Start`) are the kernel track's; until
    they land, the back end's `address_space` answers 0xFF00_0000 and the stand-in keeps 4 GiB, so the sizes
    differ between the two in J1 (recorded in the status file; no test of J1's bar prints them).
  - What this covers besides hello: exec (`sh -c "exec ..."`, `nice`, `setpriv`, `ionice`), a call that waits
    and `Idle` (`sleep 1`), and every file call parking until the backend answers (`busybox::compare` runs
    each command with the root in memory and on the backend). All of these are the loop's leaving and the
    test outside's work, so none needs fork, outside threads or a timer.
  - Bar, first step: `cargo test -p wcontainers-kernel-test --features runtime --test hello -- --test-threads
    3` passes, and passes again with `LOOK_AHEAD=none` in front. Bar: `cargo test -p wcontainers-kernel-test
    --features runtime --test hello --test busybox -- --test-threads 3` passes (full look-ahead: the same
    output, exit status and files as the reference, with the root in memory and on the backend); the same
    tests without the feature still pass, with a seeded hello run's output, counters and turn count unchanged
    by the move to `drive` (step 1 above). Recorded: the CPU side's warnings per command; the time to
    translate and compile busybox and musl in the runtime; the time of a second container in the same process
    (cache hit); how many pieces each command needed translated at run time; how many starts each range got
    from each source.
- **J2, processes.** Fork with the exact copy of section 8 (macOS `mach_vm_copy`, Linux `pagemap`); signals
  and frames; job control and terminals. Linux checked for host-supplied memory, shared blocks and the fork
  copy, including a page the system has swapped out (Linux, `MADV_PAGEOUT`) seen by the child. Bar: with `--features runtime`, the tests `shell`,
  `script`, `scriptline`, `processes`, `zombie`, `orphans`, `procsignals`, `sigframe`, `faults`, `align`,
  `memory`, `mapflags`, `privatemap`, `sharedmap`, `memfd`, `sysvshm`, `loader`, `loaderrors`, `terminal`,
  `write` and `unknown` all pass as they do with the stand-in. Layer 1 with `CpuSide` passes 10,000 seeded
  sequences, and the one-million run is one command. System call and fork cost measured (section 15).
- **J3, the user surface.** The library surface of section 14, as the owner decided it, with images from a
  tar, the on-disk cache, the shipped outside (free turns with its timer thread, output and input streams).
  Bar: a test runs `sh -c` with a pipeline in `alpine:3.22` (exported with Docker) through the public surface
  and matches the reference; a second run of the same container in a new process starts from the disk cache
  (measured).
- **J4, threads and turns.** Starts once the translator emits the guest-defined count (section 9), so seeded
  thread switches land at the same guest points at every look-ahead. Many threads per process, futexes,
  timers, outside events raising the flag in free turns. Bar: `threads`, `turns`, `timers`, `deadlines`,
  `cputime`, `cpulimit`, `clocks`, `lockwaits`, `rseqabort`, `priorities`, `schedattr` and
  `shell::runs_repeat_for_a_seed` pass with `--features runtime` at full, none and bounded, with the same
  count at every exit for a seed at each look-ahead and on the stand-in; a test runs a threaded program
  through the public surface in free turns and matches the reference.
- **J5, files outside, sockets, network, and the whole suite.** The shipped outside's backend and network
  (taken from what the kernel test host does, written for shipping). Bar: the whole kernel test crate with
  `--features runtime` gives the same verdicts as without, test by test; the Linux Test Project's verdicts
  equal the stand-in run's for every test (the kernel's ltp harness, one command, run by the orchestrator).
- **J6, the runtimes of K6b.** Python, Node, a Go program and Java as the kernel track lands them, with code
  written at run time. Measured: run-time translations, "code changed after it ran", "continued after stack
  loss" and their times; a JIT's store growth (section 7); decide on starts learned at run time. Bar: K6b's
  tests pass with `--features runtime` at full and bounded (not none, section 15).
- **J7, speed.** Find where instantiation time goes and bring fork down; the benchmark bar of section 15;
  system call and fork cost reported against unkernel's. Bar: the benchmark set within 5% of the translator
  runner in instructions retired, times reported; fork and system call figures in the status file.
- **J8, saving and bringing back with translated code present.** The kernel's save injections across
  layer 2 with `--features runtime` (section 11). Bar: `saving` and the shell tests with saves injected pass
  at full and bounded look-ahead.
- **J9, the final form.** After K7 is on main: the kernel as WebAssembly under the runtime, saving to a
  file. Bar: J5's whole-suite result again with the kernel as WebAssembly, and a container saved to a file
  mid-run and brought back in a new process finishing with the reference's output. The linked kernel's
  results stay as they were; both forms run the same suite from then on.

The brief's pass bar is met at J5 (layers 1 to 3), J6 (K6b) and J7 (benchmarks).

## 17. What stays out, and why nothing here rules it out

- **The browser host.** The loop and the outside's traits know no engine; only the WebAssembly back end's
  crate names wasmtime. In a browser the guest memories cannot be host-supplied, so shared pages use the
  kernel plan's copying method (section 7 there) behind the same `shared_*` calls, with the "shared pages"
  write lock of section 8, and fork copies every mapped range in full, another body of the same one copy
  operation (a browser's memory has no residency to ask about, and every page of it is already backed). The
  stop flag is raised from the page's own thread through the shared state memory, as the
  translator measured in Chrome. Nothing in the loop depends on keeping the WebAssembly stack across a
  switch.
- **Guest threads on separate host threads.** Threads already have their own blocks and instances, and the
  state memory is shared. What is single-threaded is that the kernel runs inside a hook of the one running
  thread; with threads in parallel the kernel is entered under one lock, and permission changes first stop
  the process's other threads (contract.rs:17-27). The guest memory then has to be a shared memory, which a
  host-supplied mapping already allows.
- **x86-64 guests.** The back end is generic over `G: Guest` (section 3). X10 makes the abi and the kernel
  generic over the personality and adds x86's exits; the back end's impl of `Cpu` then becomes generic. The
  page locks x86 needs for code written without an announcement are one reason of the back end's write
  locks (section 8). x86's array of slots (four bytes per byte of address) needs the array in its own 64 bit
  memory, which the translator's contract takes on for every guest before x86 runs here (section 8); the
  container's address-space size stays the same, so x86 and ARM64 containers are told the same sizes. The
  progress count includes each element of an x86 `rep` instruction (the map's proposal), so the count does
  not depend on the translator's chunk size.
- **Graphics.** Not part of the runtime.

## 18. Measurements behind the decisions

All on this Mac (Apple M5, 18 cores, macOS), wasmtime 50.0.1, release builds, 2026-11-17, while other
tracks were running. Load is `sysctl vm.loadavg` (1, 5 and 15 minutes) at the time. Scratch code and logs are
under pyro-parked/wcontainers-scratch/runtime (outside the repository); the log for each figure is named.

- Resident memory in the reference (ARM64 Linux in Docker, `/proc/<pid>/smaps_rollup`; load 29;
  forksize/sizes.txt): busybox sh in `alpine:3.22` after a few commands: resident 892 kB, anonymous 48 kB;
  python 3.13 (`python:3.13-alpine`) after starting: 8592 kB, 3364 kB; node 22 (`node:22-alpine`): 48204 kB,
  7268 kB; java 21 (`eclipse-temurin:21-jdk-alpine`, a class reading the file): 44656 kB, 25064 kB.
- A new 4 GiB 64 bit wasmtime memory, and copying N bytes into it (median of 15; load 39.8, 33.6, 28.5;
  measure-translator.log): 48 KB 6.4 µs + 4.0 µs; 3364 KB 6.3 µs + 167.8 µs; 7268 KB 9.0 µs + 387.6 µs; 25064
  KB 11.6 µs + 1587.7 µs; 48204 KB 15.6 µs + 3229.6 µs.
- Copying a process's guest memory on fork, macOS (scratch forkcopy/forkcopy.c, built with `cc -O2`; median of
  15; load 27.7, 35.3, 44.9; forkcopy/forkcopy.log): each case is a fresh 4 GiB anonymous reservation with
  ranges laid out as the process's images, heap and stack, every 64th byte written, copied into another.
  busybox sh (4 ranges, 11,200 KB mapped, 1,784 KB written): `mach_vm_copy` of the ranges 19.1 µs (min 16.1),
  `memcpy` of the written bytes 115.6 µs; python-sized (24,640 KB, 8,808 KB): 197.9 µs against 610.6 µs;
  java-sized (106,560 KB, 44,936 KB): 1292.4 µs against 3354.6 µs; node-sized (90,112 KB, 48,396 KB): 483.0 µs
  against 3521.2 µs. Exact in all 60 rounds (the child read the parent's bytes; later writes on either side
  were not seen by the other). The parent rewriting all its written pages after the copy: 266.3 µs for
  busybox sh's. 2000 copies of one parent in a row, the parent writing between them: 20.3 µs each for the
  first ten, 18.0 µs for the last ten, all exact.
- Relocations of the translator's M5 binaries (scratch check-j0-run2/relr.py, reading each file's dynamic
  table; relr.log): busybox no relative `RELA`, 1074 `RELR` places (1046 into code); musl none and 80 (45);
  python 12,063 relative `RELA` (10,353 into code) and no `RELR`; git none and 4253 (3858); rg none and 14,146
  (13,198).
- Whole binaries at look-ahead none from M5's starts, and reading those starts from the file (`Binary::read`,
  median of 7), speed setting, wasmtime compiling on all cores (test g of the scratch runtime_measure.rs;
  load 27.1 to 26.2 over the 1 minute figure; measure-none.log; peak memory is the process's largest resident
  size so far): busybox starts read in 0.07 ms, translated in 0.12 s to 135,972 pieces in one module
  (19,505,404 bytes), compiled in 8.45 s, peak 3,120 MB; musl 0.08 ms, 0.08 s, 85,954 pieces (12,668,018
  bytes), 5.09 s, peak still 3,120 MB then 3,485 MB; libpython 1.37 ms, 0.71 s, 703,235 pieces (106,375,194
  bytes), 113.92 s, peak 16,918 MB.
- System call through the translator's runner, at full look-ahead (same log and load): answered in the hook
  10.5 ns (median of 7 alternating pairs, 1,000,000 calls each; min 9.4 ns); leaving `run` and back 300.8 ns
  (median of 5, 100,000 calls each; min 285.7 ns).
- The kernel's own share with the layer 1 buffer CPU (static hello as the started program; load 64.8, 45.7,
  34.1; measure-kernel.log): `getppid` 116.3 to 120.0 ns (5 runs of 1,000,000); fork, the child's exit and the
  parent's wait 2.42 to 2.65 µs (5 runs of 20,000).
- Real binaries from the translator's M5 list, full look-ahead, speed, default allocator: 8 runs
  (measure-binaries.log, measure-instantiate-plain-1 to 6.log, and the first test of measure-hostmemory.log;
  load 41.5 to 60.6 over the 1 minute figure). In each run instantiation is the median of 3 and bringing back
  from a serialised file the median of 4. Busybox: translated in 0.15 to 0.35 s to 32,472,392 bytes of
  WebAssembly, compiled by wasmtime in 1.12 to 2.08 s, instantiated for a thread in 0.43 to 0.60 ms, brought
  back from a serialised file (51,218,600 bytes) in 2.77 to 3.59 ms. Musl: 0.06 to 0.16 s, 14,546,689 bytes,
  0.51 to 0.99 s, 0.27 to 0.42 ms, 1.19 to 1.67 ms (22,644,376 bytes). Libpython (one run, measure-binaries.log,
  load 57.1): 0.46 s, 137,603,117 bytes, 6.60 s, 1.19 ms (min 1.10), 12.56 ms (210,339,800 bytes).
- wasmtime's pooling allocator: 2 runs interleaved with 4 default runs (measure-instantiate-pooling-1 and 2
  against plain-3 to 6; load 54.2 to 63.7): instantiation busybox 0.60 and 0.55 ms against 0.44 to 0.60 ms;
  musl 0.36 and 0.35 ms against 0.33 to 0.42 ms: no difference.
- BLAKE3 of the code (median of 21; load 40.3; measure-hash.log): busybox 704,368 bytes 0.300 ms; musl 370,304
  bytes 0.159 ms; libpython 2,959,628 bytes 1.275 ms.
- Host-supplied memory in wasmtime (macOS; load 59.2; measure-hostmemory.log): a 1 MiB shared memory object
  mapped into two guest memories at different places; 1000 of 1000 stores through one seen by loads through
  the other; mapping it 0.6 µs (median of 50).
- Instructions retired against time (macOS `/usr/bin/time -l`, `gzip -c` of the same 20 MB file, 8 runs; load
  88 to 102; retired-gzip.log): 2,716,771,743 to 2,721,493,991 instructions (0.17% apart); 0.25 to 0.29 s.
- From the brief, not measured here: unkernel's `getppid` 56 ns and fork 20 µs; the benchmark set's times
  native in the translator's runner (fib 5.8, sort 3.4, interpret 3.6, sieve 1.4, matrix 2.6, search 1.9).

## 19. Not yet measured

- A system call and a fork end to end through the runtime (J2).
- Where instantiation time goes, and fork with it brought down (J7).
- Host-supplied memory, shared blocks and the `pagemap` fork copy on Linux (J2).
- Rebuilding a fork child's array of slots from the piece lists (J2).
- A lookup in a 64 bit array of slots, before x86 runs here (the translator and x86 tracks).
- What emitting the guest-defined progress count costs (the translator track; the map measured +1.2 to
  15.5% instructions retired for a cruder counter at today's check sites).
- How many pieces real programs need translated at run time with this plan's starts (J1, J6).
- What a stack loss costs real programs after a call that waits (J6).
- A JIT's store growth (J6).
- What counting every stop check costs the tests (J1).
- Creating a per-process 4 GiB state memory (J1).
- The final form's extra cost per system call (J9).

## 20. Decisions for the owner

- **Names**: suggested `wcontainers-runtime` and `wcontainers-wasmtime` for the crates, `Runtime`,
  `RuntimeSettings`, `ContainerSettings` and `Image` for the surface; all yours to choose.
- **The user surface**: the library surface of section 14 (a tar image, the command and its environment, the
  translator's two dials, a cache directory, byte streams for input and output), and whether a command line
  program is wanted.
- **The address space and memory size** every container is told: 0xFF00_0000 (4 GiB less 16 MiB) for both,
  instead of the 4 GiB the kernel plan's written list of differences names, on every back end including the
  stand-in, held by the kernel as the container's; 4 GiB becomes possible once the array of slots has its own
  memory.
- **The progress unit** (section 9): the plan carries the map's proposal as a placeholder; the ruling, and
  the speed cost of emitting it, are yours.
- **How the benchmark margin is checked**: 5% in instructions retired, with times reported beside it, since
  times under this machine's load vary more than the margin.
- **The shipped form on native machines**: the linked kernel or the kernel as WebAssembly (section 13); the
  plan keeps both first-class.
- **Interfaces of other tracks** this plan asks for: the kernel exposes its monotonic clock (one method),
  takes the container's address-space and memory sizes in `Start`, and settles standard input in the abi
  before J3; the kernel test crate's driver loop is replaced by the runtime's `step` and `drive`, its stand-in
  implements `Execution` and counts the placeholder unit, its test host implements `Outside`, with a
  `runtime` feature and `LOOK_AHEAD` (made by this track in J1); K7's module exports the kernel's six methods
  and imports `Cpu` and `Host`; the translator emits the guest-defined progress count with a limit word
  (before J4), and moves the array of slots into its own 64 bit memory for every guest (before x86 runs
  here).
