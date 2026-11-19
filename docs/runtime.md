# wcontainers: the runtime (joining the kernel to the translator)

Status: plan, 2026-11-17; J1 to J4a built (2026-11-18); J4b waits for the progress unit.

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
- 84e9a937 holds `origin/phase-1-translator` up to 8b5e4207 (an ancestor). Three translator commits came
  after it (de8f4a23, 2f091273, 11cc5adb, 2026-11-17 21:21 to 21:23). Two of them are status records. One,
  de8f4a23, changes the test runner's stop hook to read only pc and counts slow paths in fx maps. No shipped
  translator crate changes, so the runtime does not need these commits and gets them through main. The
  back end writes its own stop hook (section 9), which reads only what its turn needs, as de8f4a23 does.
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
| Saved register block (kernel plan 4, line 94) | `RegisterBlock` abi/registers.rs:11; stop flag `STOP_FLAG_OFFSET` :57; `THREAD_BLOCK_SIZE` 816 :60 | `Personality::Block` gcpu/lib.rs:24-38; thread block import contract.rs:59-61; `thread_state_size` contract.rs:122 (block + `PRIVATE_BYTES`, 32 on ARM64, arm64/lib.rs:65) | Fits with a named change: the 32 private bytes (float tracking, exclusive monitor, arm64/lib.rs:78-84) are the back end's and never travel: they are zeroed whenever a thread starts from its block (section 9), so a thread's state is its block |
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
| Address space and memory size | `Cpu::address_space` abi/cpu.rs:35; `AddressSpace` abi/memory.rs:23 ("the host's cap"); the kernel lays out every process inside it (loader.rs:476, task.rs:695), and the memory size the guest is told is the same value, container.rs:398 | 64 bit guest memory contract.rs:39-43; size given to `Machine::new` runner.rs:249; a 32 bit state memory of at most 4 GiB holding the permission table (one byte per page, `PERMISSION_PAGES`) and the array of slots (one i32 per `G::ALIGN` bytes from `TABLE_BASE` over `SLOTS_COVER`), contract.rs:7-11, :44-67 | Fits with a named change: the container's address-space size (large, the guest's Linux size) and its memory size become the kernel's, and `Cpu::address_space` becomes only the back end's address ceiling, computed from the output's memory widths and `G::ALIGN` (0xFF00_0000 for ARM64 with today's contract); placement low first is the kernel's (asked of the kernel track; section 8) |
| New memory (kernel plan 4, line 103) | `Cpu::memory_new` abi/cpu.rs:38; used at exec, loader.rs:285 | one memory per runner, runner.rs:294-296 | Fits with a named change: one wasmtime store, guest memory and state memory per process |
| Duplicate a memory (fork) | `Cpu::memory_duplicate` abi/cpu.rs:41; used by clone.rs:153 | none | Missing (section 8) |
| Drop a memory | `Cpu::memory_drop` abi/cpu.rs:43; task.rs:706, container.rs:652 and :733 | none | Missing: dropping the process's store frees its instances; for the running process, once it has left `run` (section 6) |
| New thread | `Cpu::thread_new` abi/cpu.rs:92; clone.rs:225 and :309, container.rs:649 and :722 | one thread instance, runner.rs:1322-1352 | Fits with a named change: one thread instance and code instances per thread, made at the thread's first run; each thread's block in its process's state memory |
| Drop a thread | `Cpu::thread_drop` abi/cpu.rs:93; task.rs:592 and :675 | none | Missing: the block and instances are kept for the process's next thread |
| Read and set registers | `Cpu::registers`, `Cpu::set_registers` abi/cpu.rs:95-97 | the block in the state memory, runner.rs:572 and :553, `snapshot` :689 | Fits as is for reading; setting as the "from new registers" row says |
| "Code is here" (kernel plan 4, line 107) | `Cpu::code_changed` abi/cpu.rs:64; sent at exec loader.rs:871, for the vDSO loader.rs:723, for executable mappings memory.rs:718, when code stops being runnable memory.rs:471, :495, :516, shared.rs:408 | `ic ivau` drops the pieces of a 64 byte line, runner.rs:1032-1041 and :1113-1119; missing pieces are translated from the bytes now in memory, `load_code` runner.rs:924-1008 | Fits with a named change: the notice drops the range's pieces and keeps its starts; translation happens at first use (section 7) |
| Page permissions (kernel plan 4, line 112) | `Cpu::set_permissions` abi/cpu.rs:52; `Permissions` with `EXECUTE` abi/memory.rs:14 | one byte per page with the next page's bits, contract.rs:62-67 and :126-145; the runner keeps the full permissions, `EXECUTE` included, and rewrites the byte before the range, runner.rs:647-663 | Fits with a named change: the runner's method moves into the back end, per process, and writes each byte as the kernel's permissions less the back end's write locks (section 8) |
| Shared pages (kernel plan 4, line 115) | `shared_new`, `shared_map`, `shared_unmap`, `shared_read`, `shared_write`, `shared_drop` abi/cpu.rs:66-89; every file mapping is shown through a block, the ELF segments loaded at exec included (loader.rs:614, :814-834; shared.rs:1-40) | none | Missing: the kernel plan's copying method with the "shared pages" write lock, since this Mac's pages are 16 KiB and the guest's 4 KiB (section 8) |
| Stop flag (kernel plan 4, line 118) | abi/registers.rs:54-57 | the stop check and `HOOK_STOP` contract.rs:98-100; `raise_stop_flag` runner.rs:533 and :825 | Fits as is |
| Machine description (kernel plan 10) | `HWCAP`, `HWCAP2` abi/machine.rs:20-24, reported at loader.rs:619 and :941 | `OFFERED_FEATURES` arm64/forms.rs:6-8, which match them; `CTR_EL0` arm64/decode_forms.rs:200-203 | Fits as is for ARM64 (the guest's offer is `()`); X10 brings x86's |
| Clocks | `Host::monotonic_nanoseconds`, `realtime_nanoseconds` abi/host.rs:95-97 | ARM64's `clock` hook (counter reads) arm64/guest.rs:434-441, answered from the runner's own start, runner.rs:1025-1030 | Fits with a named change: the clock hook first tells the outside the progress since the last exit (`ran`), then answers from the kernel's monotonic clock (`Context::monotonic`, lib.rs:74, crate-private: `Host::monotonic_nanoseconds` plus an offset that is 0 until a `restore`, lib.rs:230 and :410), which the kernel is asked to make public as one method; until then the hook answers `Host::monotonic_nanoseconds` itself, equal to the kernel's clock in every run with no restore; on a back end brought back from a save each counter read is counted and warned as "counter read without the kernel's offset", and when the new host's clock reads earlier than the saved kernel's the guest sees the counter go back (a known failure, tests/counterclock.rs, ignored with the feature, until the kernel's method lands); with the method a counter read is a host answer like any other, moves with the work done in a counted turn, and agrees with `clock_gettime`, also across a restore |
| Randomness | `Host::random` abi/host.rs:99 | none (ARM64's `RNDR` is not offered and is an illegal instruction, arm64/decode_forms.rs:253) | Missing: the outside's, from the operating system |
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
kernel test that runs guest instructions run through it.
The kernel test driver's loop (driver.rs:437-561) is replaced by it, and the driver's per-turn work moves into
the test outside (section 15). There is no second loop. Two kinds of test call below it, by what they test:
layer 1 (layer1.rs) is itself the CPU, whose sequences are the exits, so it calls `kernel.next()` and
`kernel.exit` directly over its buffer CPU (and over `CpuSide`'s memories from J2); tests/standin.rs tests the
stand-in's own exits with no kernel.

**The two interfaces it is generic over**, both resolved at compile time:
- `Execution`, the back end: the kernel's `Cpu` plus `run(thread, resume, turn, answer) -> Result<(),
  Self::Failure>`. `run` runs the thread and hands every exit to `answer.exit(cpu, exit, progress)`, which
  says either "carry on in place" (with the resume and the next turn) or "leave". The back end carries on in
  place when told to and returns `Ok` only when told to leave. `Failure` is what the back end cannot run at
  all: the stand-in's `Unsupported` (driver.rs:458-464, which the tests report as `Failure::StandIn`); for
  `CpuSide<G>` it is `Infallible`, since everything the guest does becomes an exit and a module wasmtime
  refuses is a translator fault that panics with the module's key. Slow paths go to `answer.warn`.
  Implemented by `CpuSide<G>` (WebAssembly) and, in the kernel test crate, by the stand-in, whose `run` loops
  over Unicorn's turns the same way. One more method, `restored(&self) -> Self`, gives a new back end of
  the same kind holding the same memories, blocks of shared pages and thread blocks, with nothing
  translated or cached and nothing of the host's threads assumed (it is called in a copy of the process
  made at a save, section 11). It is on `Execution` because the outside that makes a save is generic over
  the back end and must not know which one it has.
- `System`: the kernel's `Host` plus six methods the loop calls: `turn(slice)` gives how the next turn
  ends; `ran(progress)` is told how far the thread got; `arrived(kernel, cpu)` hands the kernel what arrived
  since the last exit (events, signals) and says whether something needs the thread out of `run` first;
  `stopped(kernel, cpu: &mut E) -> ControlFlow<Self::End>` does that something once no thread is in `run`
  (a save, section 11, which replaces the back end with `cpu.restored()` in the copy) and may end the run
  with its own result; `idle(kernel, cpu, until)` waits for an event or the time; `cpu_warning(warning)`
  is told each slow path of the back end, kept apart from the kernel's `Host::warn`. These take `&self`, and
  `System` requires `Sync` (the owner's ruling of 2026-11-19), so that the kernel may later call them for
  different processes at once: a system keeps what they change in atomics or behind its own lock. `Host`'s
  calls, the kernel's interface, still take `&mut self` until the kernel track changes them; a system with a
  lock answers them through `Mutex::get_mut`, with no locking. Implemented by the
  shipped outside (J3), by the test outside (J1), and by any user who brings their own: the outside is a
  type the user supplies, never built into the loop.

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
saved (contract.rs:94-100), counted by why (section 10). A memory fault or a trap always leaves
(contract.rs:101-114); its answer is kept for the loop. The load code hook is the back end's own (section 7).

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
  kernel. Reading uses the `object` crate's ELF types for the headers, dynamic table, symbols and
  relocations, and `gimli` for the unwind table (`EhFrameHdr` and `EhFrame`), as M5 does
  (binaries.rs:342-357 at 84e9a937).
Both relocation forms occur in the translator's list, so both are read (measured, section 18): busybox,
musl, git and rg keep all their relative relocations in `RELR` (busybox 1074 places, 1046 into code); python,
java's libjvm (118,737, 112,799 into code) and node (96,357, 91,968) keep them all in `RELA`; bun has
neither. A `RELA` start is the entry's addend, which is in the relocation table and does not depend on where
the image sits. A `RELR` start is read from the word at the place, which is either not yet relocated (musl's
own words when its entry first runs, since it relocates itself) or relocated (busybox's words when its
entry first runs, after musl has relocated them). The image is read one way for all its words: relocated
when more of its `RELR` words, less the load offset, fall inside the image's own span than do as they are,
otherwise as they are (an image at load offset 0 reads the same both ways). So every word gives one start
or none, and a library placed at different addresses gets the same starts unless its words are split
nearly evenly. J1's check counted busybox and musl: each, laid out at three load addresses, relocated and
not, gave the same starts every time (busybox 558, musl 1537); git and rg are counted when they first run in
a container. A word the program has changed by then
gives a different start or none. That is never wrong: a start only says where translation begins, an
extra start costs translation time, and a missing one a piece made at run time. The starts are part of the cache key, so the same bytes with the same starts give
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
- the output's memory widths: the width of each memory the code addresses (today's contract: the guest
  memory 64 bit, the state memory 32 bit). The width is a property of the output, and a wider form gives
  a higher address ceiling (section 8). The ceiling itself is not in the key: the code reads the array's
  cover and the permission pages from imported globals (contract.rs `SLOTS_COVER`, `PERMISSION_PAGES`), so
  one entry serves every ceiling of the same form,
- the translator's build identity: a hash made at build time of the files of every crate the emitted
  code depends on (every file of the crate's package directory, cargo's own bound of what a package is
  built from, so files its sources include, such as the ARM64 decoder's table of Arm's forms, are in it;
  only the directories never built into a dependent, `tests`, `benches`, `examples` and `target`, are left
  out): the translator, the guest interface (`wcontainers-guest-cpu`: `Personality`, `Guest`),
  every guest crate of the workspace (each crate that depends on the guest interface and not on the
  translator, found by the build script, so x86-64's joins when it lands), and `wcontainers-abi` (the
  register block, `STOP_FLAG_OFFSET`, `THREAD_BLOCK_SIZE`), and the locked entry (name, version, source,
  checksum) of every other crate they are built from, followed along normal and build dependencies through
  the workspace as `cargo metadata --no-deps` gives it (the cargo_metadata crate) and the lock file (the
  cargo-lock crate), so nothing is fetched (wasm-encoder, the guest's decoder and float crates, the
  translator's petgraph and what those depend on), so any change to them changes the key without
  manual version numbers (built with the disk level, J2: the in-memory level lives inside one build). The
  build script reruns when the lock file, a crate's manifest or a hashed file changes, and for nothing
  else under the crates directory (a hashed crate's tests among them),
- wasmtime's own compatibility check, which `Module::deserialize` makes. A file that exists but cannot be
  read whole or is refused is a counted, warned slow path ("translation in the cache's directory refused"):
  the translation is made again and written in its place.
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
- fork copies it with the host's own exact copy (below), which on macOS copies each page on its first
  write.
Measured on this Mac in wasmtime 50: a 1 MiB block of shared pages mapped into two host-supplied guest
memories at different addresses; 1000 of 1000 stores through one memory were seen by loads through the
other; mapping the block took 0.6 µs. That measurement mapped at 256 MiB boundaries and hid what decides
the method: this Mac's pages are 16 KiB (`sysctl hw.pagesize`: 16384) and the guest's 4 KiB, and the kernel
maps and unmaps blocks at 4 KiB boundaries (a private file mapping ends its sharing one written page at a
time). A host mapping cannot show a block at 4 KiB granularity on a 16 KiB host. So blocks of shared pages
use the kernel plan's copying method (its section 7), the one that also serves browsers, on every host:
- a block's bytes are the back end's own; each memory that maps it holds a copy at the mapped place;
- every page that shows a block carries the "shared pages" write lock (the lock layer below), so the
  guest's first write to it faults to the back end, which marks the page written in that memory, lifts the
  lock and carries the thread on from its block (counted "first write to a shared page");
- one memory may show the same page of a block at more than one place (a file mapped twice, a ring buffer
  mapped twice side by side). While the guest writes through one of them, the others carry the "view" lock,
  which takes reading as well as writing, so the guest reaching any of them faults to the back end, which
  makes the copies first (counted "shared page written through another page"). Code read for translation
  makes the copies first too;
- when the neighbouring page shows the same page of the block, one write may cross from one copy into the
  other, which no single copy can hold. Such a write is made one instruction at a time (counted "write next
  to another view made one instruction at a time"): both pages are unlocked, the lookup sends the
  instruction's address to its own piece in the exact slot and the address after it to `load_code`, and
  there the copies are made, each page giving the bytes it changed;
- `ic ivau` (the guest's code changed hook) on a page that shows a block drops the pieces of that line
  wherever the block is shown, in every memory, as the instruction reaches every CPU;
- before the kernel sees anything (at every exit handed to it, and when the thread leaves `run`) the
  written pages are copied into the block and into every other memory that maps them, and locked again.
  With one guest thread running at a time and the view lock this is exact: no other memory runs, no other
  place of the same memory is reached before the copies are made, and the kernel reads nothing until then;
- `Cpu::write` into a page that shows a block, and `shared_write`, write the block and every memory that
  maps it; `shared_unmap` and `Cpu::zero` end the sharing there, the memory keeping its copy or the zeros.
A real shared mapping (rustix, `memfd_create` or `shm_open`) is a later speed path only on hosts whose
pages are 4 KiB, decided by measuring a program that writes shared memory heavily; it is not built.

**Address space, memory size and ceilings.** Three sizes, kept apart:
- *The container's address-space size* is large, as on real Linux: the guest's own Linux user size (ARM64
  2^48, as the reference has it, its stack just below 0x1_0000_0000_0000, ceiling/reference-maps.log; x86-64
  2^47). It is the kernel's, held in its state, saved with it, the same on every machine and every back end,
  and never read from where the container runs. The memory size the guest is told (today
  `machine.memory_bytes`, container.rs:398) is the container's too, held by the kernel; the runtime gives it
  in `Start`, from the container's `GuestMachine` (section 14; proposed default 4 GiB).
- *An address ceiling* belongs to a way of running a process: the back end and the form of its output. A
  process can run there only while its highest address in use is under that ceiling. Nothing in the runtime
  assumes one ceiling: the WebAssembly back end computes its own from the output's memory widths and
  `G::ALIGN` (below) and answers it as `Cpu::address_space`, which then means only that (the abi already calls
  it "the host's cap", abi/memory.rs:21-24).
- *Placement* is the kernel's: low first, by one rule that is the same everywhere and never depends on the
  ceiling, so a run places the same mappings at the same addresses on every back end, and a process can stop
  on one and continue on another. Asked of the kernel track (the owner's direction, details not final): the
  kernel keeps each process's highest address in use; a mapping goes above the current tier only when the
  program asks for that address (a hint or a fixed address) or the tier below is full; the stack and the vDSO
  are placed by the same rule, so a small program's highest address is small; a mapping, `brk` or stack
  growth that would cross the process's ceiling fails as running out of memory does (`ENOMEM`, or `SIGSEGV`
  where Linux sends it for a stack that cannot grow), counted "address above the ceiling" and warned.
  Proposed tiers: up to 4 GiB, up to 16 GiB, then the container's size, matching the ceilings the brief names
  (a 32 bit memory, a browser's 64 bit memory, the host's limit).
- *Crossing a ceiling* is later a move to a way of running with a higher one: stop, translate again for the
  wider form, continue from the same block and memory (section 12). Until that exists, the failure above is
  the answer.
Until the kernel track lands this, the kernel lays every process out inside `Cpu::address_space` as today
(loader.rs:476, task.rs:695) and tells the guest that value as its memory size. So the stand-in (4 GiB,
standin.rs:22) and the WebAssembly back end (its ceiling) lay processes out differently and tell different
sizes. This is recorded in the status file. No test of J1 to J3's bars compares addresses or the memory size
with the reference, whose addresses are 48 bit.

**State memory per process and the back end's ceiling.** Today's contract keeps threads' blocks, the page
permission table (one byte per page up to `PERMISSION_PAGES`) and the array of slots (one i32 per `G::ALIGN`
bytes of address over `SLOTS_COVER` from `TABLE_BASE`) in one shared 32 bit memory of at most 4 GiB
(contract.rs:7-11, :44-67). The back end lays it out from `G::ALIGN`: 15 MiB of thread blocks, then the
table, then the array covering addresses from 0 to the ceiling C. C is the largest multiple of 4 MiB for which
15 MiB + C / 4096 + 4C / `G::ALIGN` fits in 4 GiB. For ARM64 (`ALIGN` 4, arm64 guest.rs:151) that is
0xFF00_0000 (4080 MiB: 1020 KiB of table, 4080 MiB of array); for x86 (`ALIGN` 1, x86 plan 3.5) 0x3FC0_0000
(1020 MiB). Untouched pages cost nothing. The array covers from 0 because placement is low first, so code is
low. 15 MiB of blocks holds about 18,500 threads in one process (848 bytes each with the private words); past
that `thread_new` answers `CpuError::OutOfMemory` and the guest's `clone` fails with the error the kernel
gives for that.

That ceiling comes from the 32 bit state memory, not from the host. The translator plan's section 6 says a
process gets the largest address space the host offers, 64 bit memory where the host has it. The guest memory
already is 64 bit (contract.rs:39-43); the table and the array are not. Asked of the translator and x86
tracks: a form of the output whose table and array are in 64 bit memory where the host has 64 bit memories,
with its widths in the cache key (section 7); the cost of a lookup there is theirs to measure. The back end's
layout code is then the only place in the runtime that changes: the same formula without the 4 GiB bound. x86
needs that form for any ceiling above 1020 MiB, ARM64 above 4080 MiB.

In that form the host's limit is the bound, and every process of the runtime shares it. Measured on this Mac,
one host process can reserve 2^47 bytes (128 TiB) of address in all (ceiling/reserve.log). A process at
ceiling C reserves C of guest memory and 4C / `G::ALIGN` of array: 2C for ARM64, 5C for x86. So a ceiling near
the host's limit leaves room for one process, and at 64 GiB about a thousand ARM64 processes fit. The final
shape is therefore a ceiling per process: each process starts at the lowest ceiling above its highest address
in use, and moves up when it crosses it, by the same move as above. That is built with the move, after the
wider form exists. Until then the back end has one ceiling for all its processes, and `Cpu::address_space`
stays one value, since a ceiling per process has no use before the move.

**Fork.** `memory_duplicate` first makes the copies of the parent's written shared pages (as before any
exit), then makes the child's store, guest memory and state memory, and:
- copies every run of pages that may hold data, exactly, whatever the host has done with the pages, each
  run widened to whole host pages (16 KiB on this Mac; the pages added are the parent's too, so the copy
  stays exact). "May hold data" is a bit of the back end's per-page record: set when a page is given any
  access, written through `Cpu::write` or shown a block, cleared when `Cpu::zero` covers the whole page, so a
  page made inaccessible after it was written is still copied. Pages that show a block of shared pages are
  copied with the rest: once the written pages' copies are made they hold the block's bytes, and the host's
  copy of them costs less than copying the block in (it shares the pages). The copy is one operation with one
  body per host operating system (memory.rs `copy_pages`):
  - macOS: `mach_vm_copy` of each range, which copies exactly by construction (the system shares the pages
    and copies each on its first write in either process), so a page that is compressed or paged out is
    copied like any other;
  - Linux: the pages `/proc/self/pagemap` shows present or swapped (bits 63 and 62), copied with `memcpy`; a
    page that is neither has never been written, or was given back, and reads as zero in both. The page map
    is opened for each copy, since an open one stays bound to the process that opened it, and a save's
    copy of the host process is another;
  - any other host: `memcpy` of the whole range.
  Residency (`mincore`) is not used: it reports a compressed or swapped page as not resident, which would
  give the child zeros;
- takes the parent's per-page record (permissions, write locks, may hold data) and blocks shown, so the
  child shows the same blocks at the same places. The record is kept in chunks of 4096 pages that copies
  share until one side changes one (a chunk that holds nothing is not there), so taking it costs the
  chunks a process uses, not its whole address space;
- copies the parent's permission table, for the pages of its record's chunks and the page before each
  (the rest of the table is zero), and the entries of its array of slots for every range of code, by
  the same page copy: the child's record and translations are the parent's, so their bytes are too, and
  copying shares pages where rewriting them would touch every one;
- takes the parent's code modules and piece lists at the same slot bases, and leaves instances to the
  child's first run, which instantiates them.
`restored()` is the same copy into a new back end, without the code: every range is to be translated again
at first use, the table is written from the record, and each thread's block is copied.
Measured on macOS (section 18): a new 4 GiB memory 6.3 to 15.6 µs. `mach_vm_copy` of busybox sh's ranges
(11 MB mapped, 1.7 MB written: both images, heap and stack, more than the reference's 892 kB resident) 19.1 µs,
against 115.6 µs to `memcpy` the written bytes; a python-sized process (24 MB mapped, 8.6 MB written) 198 µs
against 611 µs; java's (104 MB, 44 MB) 1.29 ms against 3.35 ms; node's (88 MB, 47 MB) 0.48 ms against 3.52 ms.
The copy was exact in every case (the child saw the parent's bytes, and neither saw the other's later
writes). Over 2000 forks of the same parent in a row, the mean of the first ten was 20.3 µs and of the last
ten 18.0 µs, all exact. The other side of the cost: each first write after the copy is a fault the system
settles. The parent then rewriting every page it had written took 266 µs for busybox sh, 1.58 ms for the
python-sized process, 8.92 ms for java's and 9.58 ms for node's, more than `memcpy` of the whole written set
(115.6 µs, 0.61, 3.35 and 3.52 ms). So `mach_vm_copy` is cheaper only when few pages are written after the
fork, as for a shell's child, which writes a few pages before exec, and a parent waiting for it. A process
that forks and then rewrites most of its memory would pay more than a full copy. The reason for
`mach_vm_copy` is exactness first (a compressed or paged-out page is copied like any other, with no
residency question), and its speed in the common fork-then-exec case second. J2 and J7 measure both sides
on real programs, and a program that loses by it is a measured case for copying eagerly, not a setting. The array's entries for busybox's and musl's code
(0xd2000 and 0xa2000 bytes of address, as many bytes of entries) are copied by the same page copy.
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
pages; copying a page aside before its first write, section 12; shared pages, above), and one lock that
takes reading too (the view lock, above). The table byte is always the kernel's permissions less write
wherever a lock stands, and less read under the view lock, made by one function from the record; no lock
has its own override. A write that faults on a locked page goes to the
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
  to the process's next new thread, so a program that starts and ends many threads does not grow its store:
  the instances import the block's address, so they are kept by block, with the thread's exact pieces and
  how many of the process's translations they hold; at the new thread's first run the translations added
  since are instantiated in them. A process brought back from a save makes its instances again.
- **Progress** has one meaning on every back end, at every look-ahead and in both settings: a count of the
  guest's own instructions. Where the translator puts its stop checks changes with look-ahead, with the
  starts and with code made at run time, so a count of stop checks would differ between settings and back
  ends; it is not the unit. The unit is the owner's ruling, not yet made; until then it is a placeholder,
  the proposal of the owner's requirements map: every executed transfer whose target is at or below the
  transferring instruction (taken branches and calls, and indirect branches and returns compared at run
  time), plus one at every change of pc the kernel makes to a running thread (a signal delivered,
  `rt_sigreturn`). Between two counted events the pc only rises, so (count, pc) names a point. The count is
  thread state, kept with the block (the map proposes a `progress` word and a `limit` word after the
  register block, in its padding), so saves and moving a thread carry it. Exec is not counted and the
  count does not survive it: the kernel drops the thread and makes a new one for the new program
  (container.rs:718-728), whose block, count included, starts at zero (abi cpu.rs:91), so points of the new
  program are counted from its first instruction, the same on every back end. A fork child's thread is new
  too and starts from zero. Who counts, once the owner rules on the unit (with J4b; none of the three is
  built yet, and the orchestrator confirmed that the first two wait for the ruling too, since either alone
  changes the stand-in's results):
  - the kernel's changes of pc, on every back end, by the loop: one at each
    `Resume::FromRegisters` it applies to a thread that has run before, except the first run of each thread
    after `Kernel::restore` (a restore puts back a point, it does not move it). The loop is the one place
    that sees every resume, so every back end gets the same count for it. That a delivered signal and
    `rt_sigreturn` come as `FromRegisters` is the abi's promise (abi/exit.rs:47-55); a new thread (exec, a
    fork child, `clone`) has not run before, so its first `FromRegisters` is not counted;
  - the guest's own transfers on the stand-in: Unicorn's block hook gives each block's address and
    size (standin.rs:138-140). The last instruction of a block is at start + size - 4. A block is a counted
    transfer when its start is at or below the address of the last instruction of the block before it in the
    same turn. A block that Unicorn split for its own reasons (its size limit, a page boundary, a hook) starts
    exactly at the end of the one before, above that instruction, so it is not counted; neither is a branch
    not taken. The first block of a turn compares with the last instruction the thread ran in its previous
    turn, kept with the thread: pc - 4 when that turn ended inside its last block, else that block's last
    instruction. After the kernel set new registers nothing is compared (the loop counts that change);
  - the WebAssembly back end, once the translator emits the count (the map's design: `progress += 1; if
    progress >= limit`, the stop check and the counter as one piece of code, raising the flag becoming
    `limit = 0`; asked of the translator track, which owns the emitted code and its measurement). Until then
    it reports stop checks passed in its place: a named gap ("progress counted by stop checks"), counted and
    warned once per container, recorded in the status file. Nothing in J1 to J4a's bars depends on where a
    counted turn ends (each compares output with the reference, and J4a's repeats compare runs at one
    look-ahead only); J4b's comparison of counts across settings and back ends does, so J4b is built on the
    translator's count.
- **Turns.** The outside turns the kernel's slice into a `Turn`, one of two kinds:
  - *Counted*: the back end counts progress and stops the thread after the given amount (or never, with no
    limit). Until the translator's count exists, the WebAssembly back end raises the flag at every check, as
    the runner's "stop at every check" does (runner.rs:525-530), and its stop hook counts and carries on in
    place until the count is reached, without calling the kernel; with the count, the limit word does this
    in the emitted code. `progress` handed with each exit is the count since the last one. The test outside
    always uses counted turns, with one unit as one nanosecond on its repeatable clock, as the stand-in's
    instruction does today, so a seed repeats exactly, and the clock moves with the work done whether or not
    the kernel set a slice. A counter read (the clock hook) reports the progress since the last exit or
    read before it reads the clock (section 4), so a guest that reads the counter in a loop sees time move
    within one turn; the turn's own count is not reset by it, so the turn still ends at its limit (tested by
    crates/wcontainers-wasmtime/tests/turns.rs). Counting at every check costs a hook call per check; it is for tests, never for speed.
  - *Free* (`Turn::Free`): nothing is counted; the turn ends when the outside raises the running thread's
    flag. The back end gives the outside a `StopFlag` (runtime crate; `Send`, cloned) that raises whichever
    thread is running (an atomic store to its flag word in the state memory, as runner.rs:542-551), or the
    next to run when none runs; the back end enters it at `run` and leaves it after. The stop hook then hands
    the kernel `Exit::Stop` with no progress. The local system arms one timer thread with it at the
    slice's deadline on the host's monotonic clock; with no slice no timer is armed. Costs nothing between
    checks. The kernel charges a thread for a free turn the time on the outside's monotonic clock from
    handing it out to its return (kernel task.rs `account`), so the CPU side's own work in the turn
    (instantiating a thread's instances when its block kept none, translating at run time) and the host's
    preemption of the host thread show in the guest's CPU clocks, timers and `RLIMIT_CPU`. Charging only
    the guest's own run needs the kernel to take the charge from the outside; asked of the kernel track
    (status file).
  - The two kinds exist only because counting at every check costs a hook call per check. Once the
    translator emits the count with its limit word, they are one mechanism: every turn counts at full speed,
    a free turn is a counted turn with no limit, and raising the flag is storing a limit of 0. `Turn` then
    becomes one limit (or none), and the stop-check counting goes, with its named gap.
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
  the back end's and are not guest state. The back end zeroes them whenever a thread starts from its block
  (`Resume::FromRegisters`: a new thread, a fork child, a signal handler, a restore, a thread from another
  back end), which is the state the contract gives a new thread (contract.rs:120-121), and keeps them while
  the thread carries on. Zeroing is generic over the guest: the back end never names ARM64's words. For
  ARM64 it clears the exclusive monitor (a store-exclusive may always fail and programs retry it, so
  clearing is correct at any point, as when Linux moves a thread between processors) and sets float
  tracking to "not yet written", which is the state of every new thread. So fork, saves and moving a thread
  take the block and nothing else.

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

Leaving is the slow twin of answering in the hook, so it is counted by why, with the call number as the
detail (`u64::MAX` after a stop): "left run: the call waits" (nothing can run), "left run: another thread
runs", "left run: new registers" (a signal delivered, `rt_sigreturn`), "left run: the container finished"
and "left run: the system asked" (`System::arrived`); each warns as every counter does. The loop says why
(`Carry::Leave(Leaving)`). A fault or a trap has already left when it is answered, so leaving after it costs
nothing more and is not counted again.

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
own offset. The clock hook is to answer from that same clock once the kernel makes it public; until then it
answers the host's clock, which after a restore lacks the offset, so each counter read on a back end brought
back is counted and warned ("counter read without the kernel's offset") and the counter can go back (section 4).

In the first form the kernel is Rust data spread over the runtime's own heap, not one memory that can be
copied to a file. So the first form supports exactly what the kernel's tests do today: a save made by
copying the whole test process with `fork` (driver.rs:494-524), brought back in the copy on a new back end made
from the saved memories and threads. That save happens in the test outside's `stopped`, when no thread is in
`run`. The copy holds only the forking thread, so:
- **Traps.** On macOS wasmtime handles traps with Mach ports by default, and its documentation says a child
  made by `fork` then cannot run WebAssembly (wasmtime 50, `Config::macos_use_mach_ports`). The back end's
  engine always uses Unix signal handling instead (`macos_use_mach_ports(false)`), which a copy inherits.
  Traps are rare (an access beyond the memory), so this costs nothing on the paths that matter.
- **Translating and compiling.** Both run on rayon: the translator plans and emits on its pool
  (crates/wcontainers-translator/Cargo.toml, rayon 1.12), and wasmtime's parallel compiling does too. The
  global pool's threads are not in the copy, so any work first reached after the restore would wait for
  them forever. So the back end owns its own rayon pool, made when the back end is made, and runs every
  translation and compile inside that pool's `install`, which sends rayon work started inside it to that
  pool. One path, in every process: the back end made by `restored()` in a copy makes its own pool there,
  with threads that exist. Compiled code also comes from the cache's directory (the tests give one under
  the target directory). That wasmtime's compiling follows `install` is expected, not checked. The first
  test in a bar that saves with the feature on is `procsignals` (three saves, J2), and J2 adds a test that
  translates and compiles new code in a copy and finishes. If wasmtime does not follow `install`, the
  back end's engine compiles on the calling thread (`Config::parallel_compilation(false)`) everywhere, not
  only in copies, since the translator's own planning stays parallel.
- **Timers.** Tests use counted turns (section 9), so no timer thread exists to be lost.
- **Other threads' locks.** A lock another thread holds at the fork (the system's unwinder, wasmtime's
  registry of code) never comes free in the copy. The kernel tests run several tests in one process, so a
  run that saves holds a process-wide lock alone while it has a back end, and every other run shares it
  (the driver's `SAVING`). The back end itself drops a compiler copied by fork without ending its pool,
  whose threads are not in the copy.
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
  the kernel never sees it. Slow path "first write since a mark", counted.
- **Moving a process to a higher address ceiling** (section 8): the same as moving it between back ends,
  from its blocks and memory alone, with its code translated again for the wider form (a different cache
  key, section 7). Not built; until it is, crossing the ceiling fails as running out of memory does. Two methods on `Execution` (pages written since a
  mark; open a point-in-time copy) are added when a user exists; not built now.
- **Repeatable runs.** The kernel takes every input through its `Host`, its `Cpu` answers and its events, so
  it is a pure function of what the loop hands it. Every source of non-repeatability enters the runtime at
  one of these places today:
  - the clocks: `Host::monotonic_nanoseconds`, `Host::realtime_nanoseconds` (the outside);
  - randomness: `Host::random` (the outside); `AT_RANDOM`, `getrandom` and `/dev/urandom` all come from it;
  - timer instructions: ARM64's counter reads go to the guest's `clock` hook (arm64/guest.rs:434-441), which the
    back end answers from the kernel's clock, so through `Host::monotonic_nanoseconds`, a logged answer; x86's
    `rdtsc` comes the same way when x86 lands;
  - the address-space size and memory size: the container's, held by the kernel (section 8), never the
    host's. The ceiling of where a process runs is the host's: placement never depends on it, but a request
    that crosses it fails, so a crossing is a host answer, logged like a `CpuError`;
  - random number instructions: ARM64's `RNDR` is not offered and is an illegal instruction
    (arm64/decode_forms.rs:253), so it is repeatable as it is;
  - where turns end and where events land: the stop flag's timing in free turns (the outside's timer and
    event threads), and which exit an event is handed in after (`System::arrived`);
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
- `wcontainers-runtime` (MIT OR Apache-2.0, shipped): `step` and `drive`, the `Execution` and `System` traits, the
  shipped outside for an operating system (clocks, randomness, output, files, network, the timer thread),
  and the user surface, generic over the back end. Depends on the kernel and the abi; never on wasmtime,
  Unicorn or any test crate.
- `wcontainers-wasmtime` (MIT OR Apache-2.0, shipped): `CpuSide<G>`, the WebAssembly back end, and its
  cache. Depends on the runtime crate, the translator, the guest interface and the ARM64 guest, wasmtime,
  rustix, blake3, object (0.41, the kernel's version; wasmtime 50 brings its own object 0.40 and gimli
  0.33, so a build holds two versions of each), gimli (0.34, the translator test crate's version) for the
  unwind tables, rayon (1.12, the translator's) for the back end's own pool
  (section 11), on macOS mach2, and, at build time only, blake3, cargo_metadata and cargo-lock (the build identity). The split keeps wasmtime out of every build that does not run WebAssembly
  (the kernel tests on the stand-in, a later native back end).
- No new test crate. The kernel test crate's driver calls `drive`; its stand-in implements `Execution`; its
  `TestHost`, behind a lock as `TestSystem`, implements `System`, holding the per-turn work the driver loop does today (backend answers,
  notices, signals and saves at the seed's moments, the repeatable clock). A cargo feature `runtime` of that
  crate puts `CpuSide<Arm64>` behind every one of its tests (layers 1 to 3). One loop, one set of tests, the
  back end chosen when the test binary is built. This is the one change outside the runtime's own crates;
  it is made in J1 and J2 on this branch as its own commits and named in the status file, because the kernel
  track also works in that crate.

**User surface (the owner's rulings of 2026-11-17), as built in J3.** The library is the product; the
command line program is a thin demonstration that only calls the library (J3, last). Names approved by
the owner: the crates `wcontainers-runtime` and `wcontainers-wasmtime`, the types `Runtime`,
`RuntimeConfig` and `Image`. `ContainerSettings` is withdrawn: it hand-rolled what the Open Container
Initiative (OCI) standardises and mixed what to run with how it runs. The surface takes what to run, the
root's files, how we run and what machine the container sees as separate arguments, and an outside:

```rust
let runtime = Runtime::<CpuSide<Arm64>>::new(&RuntimeConfig { cache: Some(path) });  // None: memory only
let image = Image::oci(&layout, CpuSide::<Arm64>::ARCHITECTURE)?;  // or Image::tar(&file)
let spec: oci_spec::runtime::Spec = image.spec()?;   // the image's conversion; or spec_from_json(config.json)
let mut container = runtime.container(
    &spec,
    &image,
    &TranslatorConfig { look_ahead: LookAhead::Full, optimise: Optimise::Speed }, // how the translator works
    &GuestMachine { offer: () },                                  // what machine the guest is told it runs on
    LocalSystem::new(Box::new(stdout()), Box::new(stderr())),     // the local system, or the user's
)?;                       // Error::Refused names the first field it cannot honour
let ending = container.run()?;   // to the end; or container.step() and container.idle() from the user's loop
```

- **What to run** is `oci_spec::runtime::Spec` (oci-spec 0.10.0, Apache-2.0). No type of ours repeats any of
  it. `base_spec()` is the configuration of everything the kernel gives a container (its mounts, its
  read-only path, its namespaces, Docker's capabilities, root, `PATH`), with no program and no `root`: a
  plain tar's configuration (`Image::spec` of a tar) and an image's conversion start from it.
  `spec_from_json` reads a `config.json` with oci-spec's types through `serde_ignored` (MIT OR Apache-2.0),
  and gives back with the configuration the path of every property the specification, as oci-spec reads
  it, does not define, and of every annotation (the runtime acts on none): the standard's extensibility rule
  ("Runtimes MAY log unknown properties but MUST otherwise ignore them"), by the owner's ruling of
  2026-11-18. Those are ignored, and the caller warns of each by path with a count (`wcon` does). A field the
  specification defines that this runtime cannot honour is refused by name ("Runtimes MUST generate an error
  when invalid or unsupported values are encountered").
- **An OCI image** (`Image::oci`, a directory in the OCI image layout, which `docker save` writes as a tar):
  the index's manifest for linux and the back end's architecture (through nested indexes; a manifest whose
  configuration is not an image's, such as an attestation, is passed over), every blob checked against its
  sha256 digest and size, its layers (plain, gzip, or Docker's gzip media type; others refused by media
  type) applied in order with whiteouts and opaque directories, by the `tar` and `flate2` crates. Entries with
  extended attributes are refused (the kernel's `add_file` takes none). `Image::spec` is the image
  specification's conversion: Entrypoint and Cmd to `process.args`, Env to `process.env`, WorkingDir to
  `process.cwd` (`/` when empty), User to `process.user` (names from the root's `/etc/passwd` and
  `/etc/group`; for a user named without a group, its group and then the groups listing it as additional
  groups, as Docker gives them; none for a number or a named group, as conversion.md says), `os`,
  `architecture`, `variant`, `os.version`, `os.features`, `author`, `created`, StopSignal and ExposedPorts
  to their `org.opencontainers.image.` annotations, Labels to `annotations` over any of those, Volumes
  refused by name (`config.Volumes`). Env is kept whole, with the base `PATH` added when it has none.
  With no User (implementation-defined), and for a plain tar, the user is root with its group and the
  groups listing it, as Docker gives them. A plain tar (`Image::tar`) is the simple case of a root.
- **HOME**: as runc, a process whose `process.env` has no `HOME` gets its user's home directory from the
  root's `/etc/passwd` (`/` when none).
- **`process.args[0]`** is found as runc finds it: with a slash from the working directory, else in the
  directories of `PATH` in `process.env`, the first regular file anyone may run, following the root's links.
  A name with no slash that is not found there makes no container (`Error::NotFound`, runc's "executable
  file not found in $PATH"; the command line program exits 127, as Docker does); it is never looked up in
  the working directory. A path with a slash goes to the kernel, which fails the start as exec does
  (status 127).
- **The files Docker writes in every container**: `/etc/hostname` (the host name), `/etc/hosts` (as
  Docker writes it, with the kernel's made-up address for the host name) and `/etc/resolv.conf` (the
  kernel's name server), mode 0644, owned by root, in place of the image's own, from either form of image.
  Docker writes them over its mounts; an OCI image's layers have no `/etc/resolv.conf`. `/.dockerenv` is
  not written: it marks the Docker engine, which this is not.
- **How we run it** is the back end's own type (`Executor::TranslatorConfig`; for the WebAssembly back end
  `TranslatorConfig { look_ahead, optimise }`): it changes speed and never behaviour, and is part of every cache key.
- **What machine the container appears to run on** is `GuestMachine<Offer>`, separate because the guest sees it:
  today the CPU feature claim (`offer`, the guest's `Guest::Offer`, `()` for ARM64; x86-64's AVX claim with
  that guest). The address-space size and the memory size the guest is told join it when the kernel takes
  them at start (section 8; asked of the kernel track): until then the kernel lays out inside the back end's
  `address_space` and the guest is told that size, so `GuestMachine` holds no field that would do nothing. The
  architecture comes from the image and must be the back end's.
- **The back end the runtime makes** is a trait of the runtime crate, `Executor: Execution` (name suggested):
  what a runtime's containers share (the compiler and its cache), `TranslatorConfig`, `Offer`, its architecture as the
  image configuration names it, and its stop flag. The kernel tests' stand-in is no `Executor`; the test
  driver makes it itself.
- **Every field of the configuration is honoured or refused by name.** A property the specification does
  not define is ignored with a warning (`spec_from_json`, above), and `accept` refuses any field of
  oci-spec's it does not know. Honoured:
  `ociVersion` 1.x, `process.args`, `process.env`, `process.cwd` (absolute),
  `process.user.additionalGids`, `process.terminal` and `process.consoleSize` (below), `hostname` (absent or empty: `localhost`; runc sets no host name when it is
  empty), `annotations` (data, no behaviour), and
  these when they are what the kernel gives: `process.user.uid` and `gid` 0, `process.user.umask` 0022,
  `process.capabilities` Docker's 14 in bounding, effective and permitted and none in inheritable and ambient
  (a set left out is empty, as the specification says, so leaving out `capabilities` or one of those three
  is refused),
  `process.rlimits` at the kernel's values, `process.noNewPrivileges` false,
  `mounts` exactly the kernel's six, once each and in the order the kernel makes them (`/proc`, `/dev`,
  `/dev/pts`, `/sys`, `/dev/mqueue`, `/dev/shm`, each with Docker's type, source and options; the standard
  makes mounts in the listed order, so another order or a repeat is refused at the first `mounts[n]` that
  differs), then at most one bind mount of a host directory (J5: type `bind` or option `bind`/`rbind`, options
  only those and `rw`, `private`, `rprivate`, an absolute source, an absolute destination other than `/`),
  which the system serves as the kernel's file backend mounted there (`System::serve_directory`; a system
  that cannot refuses the mount by name; `bind_mount(source, destination)` makes one), `linux.readonlyPaths` exactly `/proc/sys` (the one the kernel
  makes), `linux.namespaces` exactly pid, network, ipc, uts, mount and
  cgroup with no `path`; a configuration without `mounts`, `linux` or `linux.readonlyPaths` is refused as
  one leaving those out is, since the kernel makes them whatever it says. The surface tests check that the
  guest sees each of these values (umask, every limit, the capability sets, the mounts and the read-only
  path), so a change in the kernel fails a test rather than leaving a refusal stale. Refused, each with what
  supporting it would take:
  - `root.path`: the root's files are the `Image` argument. The local system's file backend (J5) serves a
    host directory with the rights of the user the runtime runs as, so a root from a directory would show
    every file owned by that user and could make no set-id or device files, unlike the image's own root.
    Honouring it needs an `Image` read from a directory and owners mapped (user namespaces, refused below);
    and `root.path` with the kernel's one file backend leaves no bind mount;
  - the user, umask, capabilities, limits and no_new_privs at other values, and `linux.sysctl`: the kernel
    taking them at start (today `Start` takes the program, arguments, environment, working directory, host
    name, groups and the backend, kernel container.rs);
  - `mounts` other than the kernel's six and one bind mount after them, or lacking one of the six: other file
    systems need the kernel; a second bind mount the kernel taking more than one file backend; a read-only
    bind mount (`ro`) or one with other options read-only and propagated mounts in the kernel; a bind mount
    at `/` is `root.path`'s; leaving one of the six out, another order or a repeat the kernel taking its
    mounts at start; id-mapped mounts (`mounts[n].uidMappings`, `gidMappings`): user namespaces;
  - `hooks` (they run host programs): the local system running them at the lifecycle points the runtime
    specification names;
  - `linux.resources` and `linux.cgroupsPath` (cgroups): the kernel accounting CPU, memory, pids and I/O per
    group and enforcing limits;
  - `linux.namespaces` with a `path`: several containers in one kernel; user and time namespaces, and
    `linux.uidMappings`, `linux.gidMappings` (and oci-spec's top-level `uidMappings`, `gidMappings`): user
    and time namespaces in the kernel; a list lacking one of the six: the container would share the host's;
  - `linux.devices`: device nodes in the kernel for each, with the outside behind them;
  - `linux.seccomp`: the kernel installing the filter on the first process at start (it runs seccomp filters
    a program installs);
  - `root.readonly`, `linux.maskedPaths`, `linux.readonlyPaths` other than `/proc/sys`, `linux.rootfsPropagation`,
    `linux.mountLabel`: read-only, masking and labelled mounts in the kernel;
  - `process.apparmorProfile`, `process.selinuxLabel`: a security module in the kernel;
  - `process.oomScoreAdj`, `process.ioPriority`, `process.scheduler`, `process.execCPUAffinity`,
    `linux.personality`, `linux.intelRdt`, `linux.memoryPolicy`, `linux.timeOffsets`, `linux.netDevices`,
    `domainname`: each the kernel taking the value at start;
  - `process.commandLine`, `process.user.username`, and the `solaris`, `windows`, `vm` and `zos` sections:
    another kind of host.
- **The system** is a parameter. `LocalSystem`, the shipped one, writes standard output and error to the byte
  streams it is given, answers the clocks from the host's, randomness from the system's (`getrandom`), keeps
  the kernel's and the back end's warnings by reason, gives free turns ended by one timer thread at the
  kernel's slice (the turn stores its deadline in an atomic and wakes the thread only for an earlier one; a
  turn's deadline is dropped when the turn ends), and waits in real time in `idle` for the timer or an
  arrival. Since J5 it has:
  - a file backend (src/files.rs): a host directory served in Linux's FUSE messages (fuse-abi's types, which
    the kernel uses), through the host's file calls (rustix), each name opened relative to its directory's
    descriptor without following links. Requests are served at once inside `backend_send` and the replies
    handed over at the next exit or idle, so a file call costs no thread hop. Written for this purpose since
    no crate serves Linux's FUSE messages from a macOS host (fuse-backend-rs's passthrough, which the kernel
    tests use, is Linux only, and its macOS form speaks macFUSE's messages). Host errors are given Linux's
    numbers by name (src/errno.rs); open flags the host lacks, requests not served, device nodes, a
    directory not opened and a host error with no Linux number are counted and warned. Owners are the
    host's: files the container makes belong to the user the runtime runs as, and a change of owner the
    host refuses fails as it does there (EPERM);
  - the host's network (src/net.rs): connections are host sockets connected without waiting (socket2), a
    thread of its own waits for their changes (polling) and hands them over as arrivals that raise the stop
    flag and wake `idle`, names are looked up by the host's resolver on a thread each, and a container's TCP
    port is reached from the host only where `LocalSystem::publish(port, address)` puts it (as `docker run
    --publish`); a listen on a port not published succeeds and nothing outside reaches it.
  - standard input, output and error on the kernel's stream ends (docs/streams.md): `System::standard` names
    the ends of descriptors 0 to 2 when the container is made, a terminal of `process.consoleSize`'s size
    when `process.terminal` is true (0 by 0 without a size, as `docker run --tty` gives a client with no
    terminal; a size past 65,535 is refused by name), else pipes. The local system gives each output end all
    the credit it will ever take (it writes everything as it comes) and lets go of an end the kernel closed.
    With `LocalSystem::input(reader)` (as `docker run --interactive`) descriptor 0 is a pipe, or the terminal
    takes input; without it, /dev/null. The input is read on a thread of its own, never more than the
    credit the kernel gave, each piece handed over as an arrival that raises the stop flag and wakes `idle`;
    its end of file lets go of a pipe's end (the guest reads end of file) and leaves a terminal's end open;
    a read error is counted and warned. A terminal's resizing has no surface yet.
  A system learns the back end's stop flag through `System::stop_flag` before the
  first turn; one that counts every turn, as the kernel tests' does, needs none.
- **The command line program** (`wcon`, the owner's name; crates/wcontainers-wasmtime/src/bin/wcon.rs):
  `--oci DIRECTORY` or `--tar FILE`, `--config config.json`, `--cache DIRECTORY`, `--look-ahead
  full|none|N`, `--size`, `--volume SOURCE:DESTINATION` (adds `bind_mount`'s mount), `--publish
  ADDRESS:PORT:CONTAINER_PORT` (repeatable; `LocalSystem::publish`), `--interactive` (its own standard input
  as the container's; `LocalSystem::input`), `--tty` (`process.terminal`), and `-- ARGUMENTS` for `process.args`; it prints the warnings by reason and exits
  with the container's status, 125 when the library refuses. Its arguments are read with pico-args.

## 15. Testing

- **Layer 2 and 3 (real programs, LTP).** Every test of the kernel test crate runs with `--features runtime`.
  The bar is the same verdict as the same test with the stand-in, and as the reference, test by test. A test
  the stand-in cannot run (its emulator's alignment limits, driver.rs `Failure::StandIn`) must pass with the
  runtime against the reference. One does not yet: align's unaligned exclusive load, which the translator
  faults as Unicorn does; with the feature it is ignored as the translator's known failure, and its other
  test, which only the stand-in cannot run, runs.
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
  (`SCHEDULE_SEED`, lib.rs:150-161): `LOOK_AHEAD` is `full` (also when unset), `none` or `bounded`. At
  `bounded` the container's N is drawn from 1 to the translator's default bound (`BOUND`, 256) from the
  run's seed, as the translator plan's "bounded with random N" (phase-1-translator.md line 456), and printed, so the run repeats. One N per container,
  not per range: the look-ahead is a setting of the container, and drawing it per range would be a test's
  behaviour inside the shipped back end. The back end's surface tests (crates/wcontainers-wasmtime/tests/surface.rs)
  read `LOOK_AHEAD` the same way; having no run seed, their bounded N is drawn from `LOOK_AHEAD_SEED` alone. Which look-aheads each bar runs is named in the
  milestone (section 16): J1 full, with hello also at none and bounded; J2 to J5 full, none and bounded;
  J5's Linux Test Project full only; J6 full and bounded, not none (section 7: libpython at none measured 113.9 s to compile and 16.9 GB); J7
  full; J8 full and bounded. Bounded runs with the default seed unless the bar names more. Injections (the stop flag, a signal, a thread switch, save and
  bring back) come from the test outside, at the seed's moments, as the driver gives them today. The tests
  give the cache a directory under the target directory, so a translation at none is compiled once per
  build.
- **The stand-in unchanged.** Without the feature, every test gives the same verdict as before J1, and a
  seeded run's output, counters and turn count are the same before and after the driver moves onto `drive`.
  The check is a `diff` of every test's verdict line before and after (J1). The stand-in's change of
  unit (instructions to the placeholder count, section 9) is a later commit (J2), checked the same way.
- **Benchmarks.** The translator's benchmark set (programs/bench.c and the others its benchmark test runs),
  built as ordinary static programs, run in a container on the local system (free turns), and in the
  translator's runner. The bar is instructions retired by the process during the guest's run, after
  translation and compilation (macOS `proc_pid_rusage`, `ri_instructions`), median of 3 on each side,
  within 5%. Times are reported beside it as the median ratio of 11 alternating pairs with the load, but are
  not the bar: identical runs vary 10 to 15% in time on this machine under load (translator status), while
  instructions retired by one program varied 0.17% over 8 runs, with the load at 88.45 when they ended
  (section 18). The benchmark
  programs are static and not moved, so the runtime reads the same starts as the runner from their data
  words; it lacks only their `.symtab` (section 7), so the pieces each made at run time are reported beside
  the bar. One command.
- **System call and fork cost.** A static program looping on `getppid`, and one looping on fork, exit and
  wait, run on the runtime and in the reference; reported per call with the machine and load, beside
  unkernel's 56 ns and 20 µs.

## 16. Milestones

Each bar is checkable by running the named command on this Mac with Docker.

- **J1, first light: a static hello, then busybox's single programs.** Only what those two need.
  - Merge 84e9a937 (section 3).
  - `wcontainers-runtime`: `step`, `drive`, `Execution` (with `Failure` and `restored`), `System` (with
    `End`), `Answer`, `Turn`, `Step` (section 5). The loop's count of the kernel's changes of pc comes in
    J2 with the stand-in's own count: in J1 the stand-in's unit stays its instructions, so that its results
    are unchanged, and progress has two meanings (instructions on the stand-in, stop checks on the
    WebAssembly back end), accepted by the orchestrator until the owner rules on the count. No test
    compares progress between the two back ends or between look-aheads until the translator emits the
    guest-defined count.
  - `wcontainers-wasmtime`, the parts hello and busybox reach:
    - host-supplied guest memories, and per-process stores and state memories laid out from `G::ALIGN`, with
      the ceiling of section 8, which `address_space` answers;
    - `memory_new`, `memory_drop`, `read`, `write` and `zero`;
    - `set_permissions` writing each table byte through the lock layer's one function, with the one lock
      reason J1 needs, "shared pages" (section 8);
    - `code_changed`, `thread_new`, `thread_drop`, `registers` and `set_registers`, with the rules of
      section 6;
    - translation at first use with the starts of section 7 and the in-memory cache, in the back end's own
      rayon pool (section 11);
    - `run` with carrying on in place, leaving (counted by why, section 10), and counted turns (stop checks
      standing in for progress, with its warning, section 9);
    - the clock hook through `ran`, telling the progress since the last exit or clock read while the turn's
      count goes on to its limit. It answers `Host::monotonic_nanoseconds` until the kernel's clock method
      lands (section 4); no J1 test restores;
    - traps and faults as exits, the private words zeroed at every `FromRegisters` (section 9), and the
      engine with Unix signal traps;
    - the six shared block calls (section 8). The kernel shows every file mapping through a block of
      shared pages, the ELF segments it loads at exec included (kernel loader.rs:814-834 and :614,
      shared.rs:1-40 and :322-325), so the static hello already needs `shared_new`, `shared_write` and
      `shared_map`: with `shared_new` alone answering `NotSupported` the stand-in fails hello (check 4 of
      J0, check-j0-run4/noshared.log).
    `memory_duplicate` answers `CpuError::NotSupported`, which the kernel turns into `ENOSYS` and counts
    (kernel lib.rs:62-70); no command in this bar forks (tests/busybox.rs). `restored()` (section 5) is
    not built for this back end until J2, where it is the fork copy's body; no J1 test saves. `Failure` is
    `Infallible`.
  - The kernel test crate, as its own commits, in this order:
    1. The stand-in implements `Execution` (`Failure` = `Unsupported`), and `TestHost` implements `System`
       with the driver's per-turn work. Its `stopped` makes the save by copying the process and answers
       `Break` with the copy's report in the parent (driver.rs:494-524). The driver calls `drive`.
       Checked against the merge commit, before this commit lands:
       - a seeded hello run's output, counters and turn count;
       - `turns` and `shell::runs_repeat_for_a_seed`, whose turn counts depend on the order of the kernel
         calls and of the seed's draws, with their output and counters;
       - the verdict of every test of the crate without the feature: `cargo test -p wcontainers-kernel-test
         --no-fail-fast -- --test-threads 3`, its `test ... ok/FAILED/ignored` lines kept from both commits
         in scratch, and their `diff` empty.
    2. The `runtime` feature and `LOOK_AHEAD` (section 15).
  - Until the kernel's asks of section 8 land, the kernel lays processes out inside `address_space`: the
    back end's ceiling 0xFF00_0000 and the stand-in's 4 GiB. So layouts and the told memory size differ
    between the two in J1. This is recorded in the status file; no test of J1's bar prints them.
  - What this covers besides hello: exec (`sh -c "exec ..."`, `nice`, `setpriv`, `ionice`), a call that waits
    and `Idle` (`sleep 1`), and every file call parking until the backend answers (`busybox::compare` runs
    each command with the root in memory and on the backend). All of these are the loop's leaving and the
    test outside's work, so none needs fork, outside threads or a timer.
  - Bar, first step: `cargo test -p wcontainers-kernel-test --features runtime --test hello -- --test-threads
    3` passes, then again with `LOOK_AHEAD=none` and with `LOOK_AHEAD=bounded` in front.
  - Bar: `cargo test -p wcontainers-kernel-test --features runtime --test hello --test busybox --
    --test-threads 3` passes at full look-ahead. That is the same output, exit status and files as the
    reference, with the root in memory and on the backend. The checks of step 1 also hold.
  - Recorded: the CPU side's warnings per command; the time to translate and compile busybox and musl in the
    runtime; the time of a second container in the same process (cache hit); how many pieces each command
    needed translated at run time; how many starts each range got from each source. What counting at every
    stop check costs against full speed moves to J3, which brings the free turn it is measured against.
- **J2, processes, and the rest of the boundary.** First, each as its own commit:
  - the stand-in counts the guest's own transfers, and the loop counts the kernel's changes of pc
    (section 9), together, since both change the stand-in's unit. Checked: the crate's verdicts without
    the feature are unchanged, by the same `diff` as J1's step 1. Not built until the owner rules on the
    unit (the brief: the count's definition is not ruled; stop checks stand in for it, warned, and no
    test compares progress between the back ends until it is);
  - the cache's disk level for the tests' directory. The bars' none and bounded runs span many test
    binaries, and busybox at none compiles in 8.45 s (section 18), so it should compile once per build;
  - the kernel's clock method in the clock hook, if the kernel track has landed it (it has not: a counter read
    on a back end brought back is counted and warned, and tests/counterclock.rs's test across saves is the
    back end's known failure until it lands).
  Then fork with the exact copy of section 8 (macOS `mach_vm_copy`, Linux `pagemap`), signals and frames,
  job control and terminals. Linux is checked for host-supplied memory, shared blocks and the fork copy,
  including a page the system has swapped out (`MADV_PAGEOUT`) being seen by the child. A test translates
  and compiles new code in a save's copy and finishes (section 11): the back end's own, in a fork copy of
  its test process with rayon's global pool started, since the kernel tests' copies find their
  translations in the cache's directory.
  Bar: with `--features runtime`, the tests `shell`, `script`, `scriptline`, `processes`, `zombie`,
  `orphans`, `procsignals`, `sigframe`, `faults`, `align`, `memory`, `mapflags`, `privatemap`, `sharedmap`,
  `sharedviews`, `memfd`, `sysvshm`, `loader`, `loaderrors`, `terminal`, `write` and `unknown` all pass as
  they do with the stand-in (or, where the stand-in is the known failure, against the reference), at full,
  none and bounded look-ahead. `sharedviews` (added after J2's third check) shows one shared file at two
  places of one process. Layer 1 with `CpuSide` passes 10,000 seeded sequences, and
  the one-million run is one command. System call and fork cost measured (section 15), with both sides of the
  fork copy (the copy, and the first writes after it).
- **J3, the user surface.** The library surface of section 14 as the owner ruled it: `oci_spec::runtime::Spec`
  with every field honoured or refused by name, `TranslatorConfig` and `GuestMachine` apart from it, images from an OCI image
  in the OCI layout (by the image specification's conversion) and from a plain tar, the on-disk cache, the
  shipped outside (free turns with its timer thread, output streams; standard input waits for the kernel's
  interface, asked of the kernel track). Last, once the library bar passes, the command line program: thin,
  calling only the library, with nothing the library lacks (its name is the owner's; suggested
  `wcon`). Free turns need the back end to run a turn with no stop-check hook: `Turn` gains a free
  kind and the back end a `StopFlag` any host thread raises (section 9).
  Measured: what counting at every stop check costs a busybox `echo` and a busybox shell loop against free
  turns (moved from J1; the static hello is not in the alpine image the surface runs).
  Bar: tests (crates/wcontainers-wasmtime/tests/surface.rs) run `sh -c` with a pipeline in `alpine:3.22`
  through the public surface, once from the image in the OCI layout (`docker save`) and once from a plain tar
  (`docker export`), and match the reference's output, error and status; one test per refused field
  (tests/refused.rs) fails naming that field; a second run of the same container in a new process starts
  from the disk cache (every range's translation from disk; measured); the command line program runs the
  same pipeline from both and fails naming a refused field.
- **J4, threads and turns**, split by the orchestrator's ruling of 2026-11-18 into J4a, built now on the unit
  the back end has today (stop checks passed, warned "progress counted by stop checks" as the interim it
  is), and J4b, which waits for the guest-defined count (section 9) with the stand-in's count and the loop's
  count of the kernel's changes of pc.
  - **J4a.** Many threads per process, a dropped thread's block and instances given to the process's next
    new thread (section 9), futexes, timers, and the stop flag raised from another host thread in free turns.
    The local system's one such source today is its slice timer; arrivals from a file backend, the
    network (J5) and input (the kernel track's streams) raise it the same way once they exist.
    Bar: `threads`, `turns`, `timers`, `deadlines`, `cputime`, `cpulimit`, `clocks`, `lockwaits`,
    `rseqabort`, `priorities`, `schedattr` and `shell::runs_repeat_for_a_seed` pass with `--features
    runtime` at full, none and bounded; a seeded run repeats exactly at each look-ahead taken alone
    (the kernel test crate's `repeats`: output, warnings, counters, the CPU side's warnings, the turn
    count and the files left, with and without saves, `REPEAT_RUNS` times per seed; programs whose threads
    switch only at system calls, and one whose threads only compute, so that where the seed ends a turn
    decides which thread runs next, which it checks happened); a test runs a threaded
    program through the public surface in free turns and matches the reference
    (crates/wcontainers-wasmtime/tests/surface.rs `a_threaded_program_matches_the_reference`, at the
    look-ahead `LOOK_AHEAD` names); a new thread given a dropped thread's block runs on its instances
    (the back end's unit test `a_dropped_threads_instances_serve_the_next_thread`).
  - **J4b.** The same count at every exit for a seed across look-ahead settings and against the stand-in.
    Starts once the owner rules on the unit and the translator emits it.
- **J5, files outside, sockets, network, and the whole suite.** The local system's backend and network
  (taken from what the kernel test host does, written for shipping; section 14), with one bind mount of the
  configuration honoured, and `root.path` refused with why. Bar: the whole kernel test crate with
  `--features runtime` gives the same verdicts as without, test by test, at full, none and bounded
  look-ahead, except the tests listed by name in the status file as waiting (the count
  comparison of J4b, and the K6b runtimes that J6 brings, whose processes reserve more than the back end's
  address ceiling); the surface tests of the file backend and the network match the reference
  (`a_host_directory_is_served_through_a_bind_mount`, `a_tcp_connection_from_the_host_reaches_a_published_port`),
  and, once the streams track's ends are merged, standard input and the terminal match `docker run
  --interactive` and `--tty` (`standard_input_matches_docker_interactive`, `a_terminal_matches_docker_tty`,
  `the_command_line_program_passes_its_input`);
  the Linux Test Project's verdicts, at full,
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
  operation (a browser's memory has no residency to ask about, and every page of it is already backed). A
  browser's ceilings are what its memories allow (4 GiB for a 32 bit memory, 16 GiB for a 64 bit one, as the
  brief says), computed by the same layout code from the output's widths; the container's size and the
  kernel's placement do not change. The
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
  locks (section 8). x86's array of slots takes four bytes per byte of address, so with today's contract the
  same layout code gives x86 a ceiling of 1020 MiB, and the wider form the translator is asked for (table and
  array in 64 bit memory, section 8) lifts it as it does ARM64's. The container's address-space size is x86
  Linux's own (2^47), held by the kernel like ARM64's, and placement low first keeps a typical process far
  under either ceiling. The
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
  were not seen by the other). The parent rewriting all its written pages after the copy (each first write a
  fault the system settles): busybox sh 266.3 µs, python-sized 1575.6 µs, java-sized 8916.5 µs, node-sized
  9576.1 µs. 2000 copies of one parent in a row, the parent writing between them: the mean of the first ten
  20.3 µs, of the last ten 18.0 µs, all exact. The independent check of run 3 reran it (at load 45.75, as that check
  reported; the log holds no load) with the same picture (busybox 15.8 / 97.7 / 225.7 µs for copy / `memcpy` / rewrite; check-j0-run3/forkcopy-recheck.log).
- Relocations of the translator's M5 binaries (scratch check-j0-run2/relr.py, reading each file's dynamic
  table; relr.log): busybox no relative `RELA`, 1074 `RELR` places (1046 into code); musl none and 80 (45);
  python 12,063 relative `RELA` (10,353 into code) and no `RELR`; git none and 4253 (3858); rg none and 14,146
  (13,198). The other four of the list, from J0's fourth check (check-j0-run4/relr-run4.log): java's libjvm
  118,737 relative `RELA` (112,799 into code), no `RELR`; node (node:22-alpine) 96,357 (91,968), no `RELR`;
  bun neither; go not read.
- Whole binaries at look-ahead none from M5's starts, and reading those starts from the file (`Binary::read`,
  median of 7), speed setting, wasmtime compiling on all cores (test g of the scratch runtime_measure.rs;
  load 27.1 to 26.2 over the 1 minute figure; measure-none.log; peak memory is the process's largest resident
  size so far): busybox starts read in 0.07 ms, translated in 0.12 s to 135,972 pieces in one module
  (19,505,404 bytes), compiled in 8.45 s, peak 3,120 MB; musl 0.08 ms, 0.08 s, 85,954 pieces (12,668,018
  bytes), 5.09 s, peak still 3,120 MB then 3,485 MB; libpython 1.37 ms, 0.71 s, 703,235 pieces (106,375,194
  bytes), 113.92 s, peak 16,918 MB.
- System call through the translator's runner, at full look-ahead (load 39.78, 33.56, 28.45;
  measure-translator.log lines 150-171): answered in the hook
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
  88.45, 56.60, 45.15, read once after the runs; retired-gzip.log): 2,716,771,743 to 2,721,493,991 instructions (0.17% apart); 0.25 to 0.29 s.
- Address room on this Mac (macOS 27.2; scratch ceiling/reserve.c, `cc -O2`; load 38.34, 53.88, 53.59;
  ceiling/reserve.log): the largest single `PROT_NONE` reservation one process can make is 130,578 GiB
  (2^46.99), and all its reservations together 130,596 GiB: 2^47 bytes of address in all.
- The reference's address layout (`alpine:3.22`, ARM64, `/proc/self/maps`; load 41.28;
  ceiling/reference-maps.log): libraries near 0xffff_bc84_9000 and the stack at 0xffff_d4a3_b000, so the
  reference's processes use 48 bit addresses.
- J2, through the runtime (release; the kernel tests' counted turns; status file, J2, with every log): a
  `getppid` loop 163 to 228 ns an iteration, of which the loop's stop check 9 ns, against the reference's
  140 to 335 ns in Docker's Linux VM; fork, exit and wait 339 and 740 µs in two runs (reference 179 to 980
  µs); the back end's copy of J0's busybox sh layout 125 to 131 µs whole, and the parent's first writes to
  its 446 written pages after it 221 to 237 µs against 11 µs with no copy (loads 42 to 65). With the
  records in shared chunks (J2 run 2, five alternating pairs, load 45): the same copy 71 to 95 µs against
  110 to 151 µs before, and a memory with two written pages 29 to 37 µs against 70 to 76 µs. On Linux
  (Docker, linux/arm64) the back end's tests pass, the `pagemap` copy included, with a page `MADV_PAGEOUT`
  left swapped out copied exactly.
- From the brief, not measured here: unkernel's `getppid` 56 ns and fork 20 µs; the benchmark set's times
  native in the translator's runner (fib 5.8, sort 3.4, interpret 3.6, sieve 1.4, matrix 2.6, search 1.9).

## 19. Not yet measured

- Where instantiation time goes, and fork with it brought down (J7).
- A lookup in a 64 bit array of slots and permission table, the wider form of section 8 (the translator
  and x86 tracks).
- How many processes one runtime holds at a given ceiling in the wider form, and the cost of moving a
  process to a higher ceiling (with the move).
- What emitting the guest-defined progress count costs (the translator track; the map measured +1.2 to
  15.5% instructions retired for a cruder counter at today's check sites).
- How many pieces real programs need translated at run time with this plan's starts: busybox's single
  programs measured at J1 (median 5 a run, at most 51; status file); the runtimes at J6.
- What a stack loss costs real programs after a call that waits (J6).
- A JIT's store growth (J6).
- What counting every stop check costs the tests: needs a turn that counts nothing, which comes with J3's
  free turns (J3).
- Creating a per-process 4 GiB state memory on its own (J1 measured only whole runs that include it: 4.6 ms
  the fastest).
- The final form's extra cost per system call (J9).

## 20. Decisions for the owner

- **Names**: approved for the crates and `Runtime`, `Image`; and, by the owner's rulings of
  2026-11-18 (built in J4a run 3 and J5 run 3): the trait the embedder implements `System` (first `Outside`), the
  shipped one `LocalSystem` (first `System`), the execution side's trait `Executor` (first `BackEnd`; its
  getter `Container::executor`, first `back_end`), long-lived
  configuration types ending in `Config` (`RuntimeConfig`, first `RuntimeSettings`; how the translator works
  `TranslatorConfig`, first `Tuning` and then `TranslatorSettings`), what machine the guest is told it runs on
  `GuestMachine` (first `Machine`), and the command line program `wcon`. The per-process
  `ExecutionConfig { thread_ordering, replay_determinism, translator }` comes with the threads plan and the
  progress count; until then the translator's configuration is given to `Runtime::container` alone. Still yours: `StopFlag` (perhaps
  `Interrupt`, and whether it stays in this form) and the kernel's own configuration (`base_spec`).
- **The user surface**: ruled (section 14) and built in J3, the OCI image layout included as J3's task asked.
  Added in J5, names suggested: `System::serve_directory` (a system serving a host directory for the one bind
  mount), `LocalSystem::publish` (a container port reached from the host), `bind_mount`, and wcon's
  `--volume` and `--publish` (Docker's names); and the reading that a bind mount's files keep their host
  owners (no id mapping), with `root.path` refused until owners are mapped.
  Yours: the host name a configuration without `hostname` gets (built: `localhost`; runc would keep the
  host's, which the container never sees).
- **Address ceilings** (section 8), the owner's direction taken as written there: the container's
  address-space size is the guest's Linux size (2^48 for ARM64), not the 4 GiB of the kernel plan's written
  list of differences; the proposed tiers (4 GiB, 16 GiB, the container's size) and the placement rule are
  the owner's and the kernel track's to fix; and the memory size every container is told (proposed 4 GiB, one
  value, given in `Start`), including whether a user may set it.
- **The progress unit** (section 9): the plan carries the map's proposal as a placeholder; the ruling, and
  the speed cost of emitting it, are yours.
- **How the benchmark margin is checked**: 5% in instructions retired, with times reported beside it, since
  times under this machine's load vary more than the margin.
- **The shipped form on native machines**: the linked kernel or the kernel as WebAssembly (section 13); the
  plan keeps both first-class.
- **Interfaces of other tracks** this plan asks for: the kernel exposes its monotonic clock (one method),
  holds the container's address-space size (the guest's Linux size) and takes its memory size in `Start`,
  places mappings low first by one rule
  with each process's highest address in use, fails a crossing of the ceiling as running out of memory with a
  counter and a warning, and settles standard input in the abi before J3; the kernel test crate's driver
  loop is replaced by the runtime's `step` and `drive`, its stand-in implements `Execution` (counting
  Unicorn's instructions as its progress until the owner rules on the unit; its count of the placeholder
  unit is built with J4b, section 9), its test host implements `System`, with a `runtime` feature and
  `LOOK_AHEAD` (made by this track in J1 and J2); K7's module exports the kernel's six methods
  and imports `Cpu` and `Host`; the translator emits the guest-defined progress count with a limit word
  (before J4b), and gives a form of its output with the table and the array of slots in 64 bit memory,
  its widths in the cache key (before x86 runs here, and before any ARM64 process needs more than 4080 MiB).
