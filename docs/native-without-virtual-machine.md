# Native without a virtual machine: the plan for both forms

Status: plan (W0), 2026-11-23. Brief: the owner's ruling of 2026-11-23 and brief-native.md (including its section
on register groups). It builds on docs/native.md (the virtual-machine form, "the VM form" below) and reuses its
native output, count, position map and replay check; where this plan says "as native.md", that section holds.

Words used here:
- **P**: each guest process is a real host process (a **stub**); our kernel runs in the embedder's process.
- **I**: our kernel and every guest process of a container run in the embedder's process; translated code checks
  every guest memory access.
- **floor**: our code that runs on a guest thread's host thread between translated code and the kernel: entry and
  exit routines, signal handlers, the slot table (the "mirror"). In P it is the stub; in I a part of the executor.
- **round trip**: one guest kernel entry in P: the guest thread tells the kernel's runner and waits; the runner
  answers and wakes it.
- **kernel entry**: anything that leaves translated code for the kernel: a system call, a routed instruction
  (`Route::Kernel`, the time counter among them), a fault the kernel must see, a stop.
- Measurements are cited as "W0 probe" (this plan's own, section 11) or by the orchestrator's file.

## 1. What the two forms deliver, and what they share

Both run a guest of the host's own architecture as translated native code (native.md section 3) on a Linux host
with no hardware virtualization, as an ordinary unprivileged process. Both give the guest exactly what the VM form
gives (identical behaviour to the WebAssembly output instruction for instruction, exact count, replay), except the
named differences of section 9.

- **P** buys memory protection from the host's page tables, so translated code is the VM form's code with no
  check per access, and pays a round trip per kernel entry: 1.7 to 5.1 µs measured on server chips
  (costs-round-3-and-cloud-partial.json, "cloud | intel-process", "cloud | arm", "cloud | amd"), and more under
  load (section 4.3).
- **I** pays an exact range check on every guest access (+19.4% wall on Graviton4, +12.5% on Granite Rapids,
  +10.0% on Zen 4 for the measured exact form with the stack exempt, X32-e / X32-nosp, same file) and makes a kernel
  entry a plain call (2.1 ns call, 12.0 ns with the general-register switch, 16 ns with q0 to q31, Graviton4; 1.7 /
  10.1 / 12.7 ns Granite Rapids).

Shared by both, built once (section 3): the address layout, the floor's entry and exit, position resolution, the
slot table, the code area, copy-on-write and written marks, the stop protocol, the register-group rules and the
`Cpu` side's bookkeeping. They differ only where the kernel is (another host process, or the same one) and in the
one place a memory access is emitted.

The crates: `crates/executor/implementation/host-process` (P, package `wcontainers-executor-host-process`) and
`crates/executor/implementation/in-process` (I, package `wcontainers-executor-in-process`). What both use lives in
host-process and is a public module that in-process depends on (one copy, no third crate: the owner's layout allows
an implementation to depend on another; if he prefers a shared crate, it is a move of files). The stub is a binary
target of host-process.

## 2. Rules that shape this plan

- Guest code always passes through the translator; its native output is the only code that runs for the guest.
  Every page of guest memory is mapped without host execute permission, in both forms.
- No inline stop check: a thread is stopped by a real host signal (section 3.6). The count is the VM form's count
  (definition E, one add, only under `ReplayDeterminism::Exact`).
- Protection keys are never relied on (not used at all here).
- Nothing needs root, a capability, ptrace, user namespaces, userfaultfd, process_vm_readv/writev, perf events,
  /dev/kvm or a kernel module. Section 6 lists every host call each form makes.
- The kernel is driven only through its existing interface and runners (section 8 says what is missing there).
- One primitive per job: one transition (a futex slot), one copy-on-write mechanism, one stop mechanism, one
  layout formula for both forms.
- Unsafe code: both crates deny `unsafe_code`; every use carries its own `#[expect(unsafe_code, reason = "..")]` on
  the smallest item, rustix first for every host call (MIT/Apache-2.0), libc only where rustix has no call. Expected
  uses, each to be listed in the status file as built: running translated code (a jump into generated code), the
  floor's assembly (`global_asm!` entry, exit and signal routines), mapping guest memory and the code area
  (`mmap` with a fixed address through rustix is `unsafe`), reading and writing through those mappings, and reading a
  signal frame. Nothing for speed.

## 3. What both forms share

### 3.1 The address layout

Measured (W0 probe, section 11): the host's user space is 48 bits on the Graviton4 machine (4 KiB pages) and 56 bits
on the Xeon machine (five-level paging, where Linux maps above 2^47 only when asked with a hint); a static PIE
leaves [2^16, 2^45) free on both; on ARM64 the host puts the binary, the vDSO and the stack in [2^47, 2^48) and
leaves [2^45, 2^47) free; on x86-64 it puts them just under 2^47.

One layout, with every figure a named constant derived from the fact it describes (these are for the reader):
- **The guest area**: [`LOWEST_ADDRESS`, 2^44). P: the guest's own addresses, unchanged (guest address = host
  address). I: the container's guest processes, each in its own **slot** of 2^r bytes, rebased (host address =
  slot base + guest address), the slots starting at 2^32 so that nothing a non-PIE embedder or the host's low
  mappings hold is inside.
- **The slot table** (ARM64; the VM form's entry mirror): a 16-byte slot for every host address H of the guest
  area at `SLOTS | (H << 2)`, `SLOTS` = 2^46, so the table fills [2^46, 2^47). Slot pages are mapped only where a
  guest page holds code. The jump through a register is the VM form's six instructions
  (native.md section 3.2) with the mask's `eor` replaced: P computes the slot from G itself (`ubfiz`, `bfxil`,
  `orr` of the single bit 2^46, a logical immediate); I first adds the slot base (`add t, base, xN`), which is the
  one more instruction it pays per jump. The reach test is against the process's ceiling (P: 2^44; I: 2^r).
  x86-64 has no slot table (its native output uses a lookup cache, native.md section 3.2; not built yet).
- **Our area**: everything else of ours (code area, floor, register and channel pages, stacks) between 2^44 and
  2^46 on ARM64, and between 2^44 and the host's own mappings near 2^47 on x86-64.
- Each form reserves the guest area and the slot table at start (`mmap` PROT_NONE, MAP_NORESERVE,
  MAP_FIXED_NOREPLACE): the host never places anything inside a reservation, so our code and the guest cannot
  collide. If a reservation is refused, the form is unavailable on that host (the evaluator's probe, section 7).
- **The ceiling**: `Cpu::address_space()` is [LOWEST_ADDRESS, 2^44) in P (the VM form has 2^45: the slot table at
  2^47 would need the host's stack and vDSO moved; 16 TiB is the ruled "lower ceiling", brief 2026-11-18) and
  [LOWEST_ADDRESS, 2^r - guard) in I, the guard being the largest offset an exempt stack access can add (3.3).
  The kernel places mappings low first and never depends on the ceiling (K2), so the guest sees the same
  addresses under every executor; a process that reaches its ceiling is the existing `on_process_outgrown` case.
- r is chosen per container at start, default 36 (64 GiB per process, 4096 slots between 2^32 and 2^44); a
  container that needs more live processes than slots, or more memory per process, is the evaluator's to send
  elsewhere (section 7). Running out of slots is `CpuError::OutOfMemory` at `memory_new`, counted
  ("in-process slots exhausted") and warned.

### 3.2 The four places the native output changes

The native output stays one emitter (native.md section 3); a form is a value the executor gives the translator,
part of the translation's cache key, with three variants (`Vm`, `HostProcess`, `InProcess`). It changes exactly:
1. **`access`** (crates/wcontainers-arm64/src/carry.rs, the single place a guest access is emitted): `Vm` and
   `HostProcess` emit the access as today. `InProcess` emits the exact check and the rebased access:
   `ubfx u, a, #r, #(56 - r); cbnz u, fault; <access> [base, a]` where `a` is the access's first byte in t (or the
   base register itself when the access has no offset), `u` a second withheld scratch, `base` a withheld register
   holding the slot base. The check ignores the top byte as Linux's Top Byte Ignore does, touches no flags, and the
   host's own Top Byte Ignore drops the tag from `base + a`. Pairs and multi-register forms check their last byte
   too (the access's span, as the Aligned route already computes it). The stack pointer: in I, sp holds the host
   address of the guest's stack pointer (the slot base added), because ARM64 cannot index by sp; an instruction
   that reads sp as a value (`mov x29, sp`, `add x0, sp, #n`) is re-emitted with the base subtracted (`sub x29,
   sp, base`: the same length), one that writes sp from another register adds the base and checks the result,
   and one that moves sp by an immediate needs nothing. So accesses through sp with an immediate are exempt (the
   measured "-e" forms), the slot's top and bottom guards being that immediate's reach, and every handler runs on
   its own signal stack (`SA_ONSTACK`). The position map gives the guest sp as the host sp less the base.
   `fault` is out of line and reports the guest address and access with exact registers (the check precedes the
   access, so nothing has changed).
2. **The jump through a register**: the slot table's base and reach (3.1), and I's base add.
3. **The call to the floor** (`Arm64::call`, today `brk #code`): a `brk` in a host process is a host SIGTRAP
   (1.2 to 1.3 µs measured for a signal round trip, W0 probe), so `HostProcess` and `InProcess` emit
   `str x30, [page, #saved_link]; bl entry_<code>`, the entries being one page of floor routines placed in the code
   area at most a direct branch's reach from every piece (one such page per 128 MiB of code area). The position
   map records the saved x30 as a relocated guest value, as it records stand-in moves.
4. **The replay check** (native.md section 5.7): `TPIDRRO_EL0` is the host kernel's in a host process, so the stop
   word is a word of the thread's register page: `ldr t, [page, #stop]; sub t, t, c; cbz t, stop`.
Withheld registers: the VM form's three (count x18, scratch x23, register page x28) in P; two more in I (`base`
and `u`), chosen by the same dynamic-use rule (native.md section 3.3). Everything else of the native output (routes,
pieces, linking, count placement, the position map) is unchanged. These are the translator track's files: section
10 asks for them.

### 3.3 Exits, the floor and position resolution

- An inside call enters the floor's entry for its code with the guest's registers in the hardware. The entry
  saves the general registers, sp, pc (from the call site through the position map) and the condition flags into
  the thread's register block; FP/SIMD stay in the hardware unless the exit needs them (a system call does not:
  the kernel reads them only through `Cpu::registers`, which then saves them). It forms the `Exit` exactly as the
  VM form's inside does.
- A host signal in translated code (SIGSEGV, SIGBUS, SIGILL, SIGTRAP from a guest `brk`, and the stop signal)
  gives the floor the host pc and every register in the signal frame (general, `fpsimd_context`, `sve_context`,
  `esr_context` for the access kind on ARM64). The guest state at a guest instruction boundary comes from the
  position map with the VM form's resolution: `hardware-virtualization-inside`'s `stop::resolve` and
  `stop::guest_state` are target-independent and public, and are used as they are.
- Resuming is always at a guest instruction boundary through the guest pc's slot, never at a host pc in the middle
  of a piece, except an instruction retried after the floor itself resolved its fault (copy-on-write, written
  marks, a code page's first write) in I, where the frame is out of the guest's reach (3.7). `Resume::Continue`
  after a stop is therefore re-entry at the resolved boundary with the count set from the map.
- Loading pieces, filling slots, linking exits and the kept translations are the VM form's
  (hardware-virtualization-inside `translation`, the monitor's `Translator`); section 10 asks for the monitor's kept
  translations to be usable without the hypervisor crates.

### 3.4 The code area and code a program writes

- One memfd per guest memory holds its code area: mapped read-write in the embedder's process (the translator
  writes there) and read-execute in the guest's view (P: in the stub; I: a second mapping in the same process).
  Measured (W0 probe): a memfd written in one process and run from a read-execute mapping in another runs, on both
  machines; a memfd made with `MFD_NOEXEC_SEAL` still maps executable. After writing, the writer does the
  architecture's cache maintenance on its own mapping (ARM64 `dc cvau`/`ic ivau`, which act on the physical
  line); the guest thread runs the new code only after leaving the host kernel (a futex wait or a signal return),
  which is context synchronizing on ARM64. x86-64 needs a serializing instruction on the running core for code
  another core wrote; this is settled with the x86 native output (section 10).
- A read-execute mapping without read is execute-only on both machines (W0 probe: a load from it faults; on the
  Xeon the host kernel makes execute-only from a protection key of its own, which we neither use nor rely on). The
  code area and the slot table are mapped so wherever the host allows it; where it does not, they are readable by a
  P guest (section 9).
- Code a program writes: every guest page that holds translated code is write-protected in the guest's view of
  every memory that maps it (shared blocks included); the first write faults to the floor, which drops the
  translations of that page in every process holding them, gives the page its guest permission back, and retries.
  `Cpu::code_changed` is honoured as in the VM form.

### 3.5 Memory: frames, copy-on-write, written marks

- Guest memory is in memfds, never anonymous host memory: P needs the kernel's process to see it; I needs a second,
  read-write view for the kernel (`Cpu` reads and writes ignore page permissions). Each guest memory has its own
  memfd whose offset is the guest address (sparse: holes cost nothing); each shared block (`shared_new`) has its
  own memfd. The guest's view maps a memfd range with the guest's permissions, one host mapping per guest mapping,
  so the number of host mappings follows the guest's as on Linux.
- The kernel's view: I maps each memfd once, read-write, beside the slot (so a guest address is one add away);
  P maps, in the embedder's process, each guest mapping's range of its memfd when the kernel first touches it,
  found through a range map per memory. `Cpu::read`, `write` and `zero` (a hole punched with `fallocate`) go through
  it in both forms and are seen at once by every view.
- **Fork** (`memory_duplicate`) is one copy-on-write mechanism, the same in both forms: the child gets its own
  memfd; the parent's writable private pages are write-protected in its view before the fork returns to it; the
  child's view of a page not yet copied is inaccessible. A page is copied into the child's memfd exactly once, by
  whichever comes first: the child's touch, the parent's first write after the protection, or the child's end
  (nothing to copy). Copies are made in units of 2^u pages (u by measurement in W1, from 0 to 9, against the number
  of host mappings it creates: mprotect splits a mapping at the unit's edges, and the host's vm.max_map_count is
  65530 by default, 1048576 on the Ubuntu machines here). Each page's copy is taken at one instant, so the child sees
  every write the forking thread made before the fork, and a sibling thread's racing write either before or after,
  as on Linux. The same write protection and first-write fault give `take_dirty_memory`'s written marks (ARM64 Linux
  has no soft-dirty bit, and userfaultfd's write protection is withheld by sandboxes).
- Shared blocks map the same memfd in every view, so two guest processes writing a shared mapping see each other's
  writes through the host's coherent memory, with the hardware's ordering; the kernel's writes into one show in
  all. The kernel sets the written mark for its own writes through `Cpu::write` and `zero`, and checks watches
  before them, as native.md section 5.3 says.
- Measured costs of the pieces (W0 probe, Graviton4 / Xeon): mprotect of one page 391 / 306 ns, mmap of one memfd
  page over another 1.0 to 1.5 µs / 0.9 µs, a hole punched in one page 0.68 to 0.78 µs / 0.51 to 0.55 µs, a fault
  handled in process and returned from 1.31 / 1.31 to 1.40 µs.

### 3.6 Stopping a thread, slices and events

- A stop is a real-time signal sent with `tgkill` to the guest thread's host thread, after the stop request is
  written in the thread's block. Measured (W0 probe): from the call to the handler running on a spinning target,
  1.72 µs median in the same process and 1.63 µs in another process (Graviton4); 2.28 / 1.90 µs (Xeon).
- The handler: in translated code, it resolves the position (3.3) and leaves as `Exit::Stop`. In the floor's
  entry, the thread is already leaving: nothing to do. In the floor's resume sequence (after the decision to enter
  guest code, before the guest's first instruction), it moves the pc back to the sequence's start, which reads the
  request word again and leaves as `Exit::Stop`: the window is restartable, so no signal mask is changed on any
  path (a mask change is a host system call, 120 ns, per entry).
- A slice (`Next::Run { slice_nanoseconds }`): P: the runner's futex wait carries the slice as its time-out and
  sends the stop on expiry (nothing is paid when the thread comes back first). I: a per-runner POSIX timer
  (`timer_create` with SIGEV_THREAD_ID) re-armed only when a new deadline is earlier than the armed one.
- A turn that must end at an exact count is the VM form's open decision 10; until the owner rules it runs one host
  instruction at a time, which in a host process is not available (no single-step without ptrace): such a turn is
  refused as the VM form refuses it without the count (`Failure`, "turn limit without stepping"), counted.

### 3.7 What protects us from the guest

- Control flow: the guest reaches host code only through pieces we translated and their slots; a slot without a
  piece holds `udf`. Nothing read from memory the guest can write is ever used as a host code address.
- I: every guest access is checked against its slot, so no guest load or store reaches our area, the kernel, the
  embedder, other guest processes or the frames of a signal. This is the WebAssembly output's guarantee made
  natively.
- P: a guest load or store can reach anything in its stub that is mapped readable or writable: its register and
  channel pages, its sigaltstack. So the stub is built so that this gives the guest nothing it does not have:
  - its routines are assembly that keep their own return and state in registers while they wait (the host kernel
    preserves registers; another guest thread cannot change them), and use no stack in guest-writable memory;
  - everything the stub reads from its pages after a wait is untrusted: guest register values (the guest's own to
    change), and mapping commands, which it checks itself before obeying (the range inside the guest area, the
    fd its own memory's memfd, no execute permission);
  - the signal handler finds its thread's pages from its sigaltstack's address (the host kernel's, unforgeable),
    copies what it needs from the frame once, and never returns through the frame to a host pc: it resumes through
    a slot (3.3);
  - the position resolution, translation and every decision are in the embedder's process; the stub only reports.
  Defence in depth if all of that fails: the stub runs under a seccomp filter installed by itself at start
  (`PR_SET_NO_NEW_PRIVS`, then `SECCOMP_SET_MODE_FILTER`, both unprivileged) that allows only its own calls
  (section 6) with their arguments bounded (no `PROT_EXEC`, addresses inside the guest area), and it holds no fd but
  its memory's memfd and its channel. A compromised stub can then reach only its own guest's memory and its
  channel, through which every request is validated by the kernel as a system call is.
- The kernel's side treats every word from a stub as untrusted (read once, bounds-checked), like a system call's
  arguments.

### 3.8 Register groups (brief-native.md, 2026-11-23)

- The start-up probe of which groups the host offers runs on its own short-lived host thread, which then exits.
- P: each guest thread is its own host thread, so the host kernel saves and restores every group lazily; we add
  nothing. The stub's routines use no FP/SIMD register. Before an embedder runner thread blocks (x86-64), it hands
  back the upper vector state it dirtied (`vzeroupper`, and zmm16 to zmm31 cleared when the host has AVX-512 and
  `XGETBV` with ECX=1 shows them in use).
- I: we switch guest threads on a runner, so the floor saves eagerly at every guest thread switch, the incoming
  thread's values or zeros loaded before its first instruction: ARM64 general, FP/SIMD and SVE in the host's vector
  length (4 / 15 / 22 ns measured, feature-switch research), SME streaming mode and ZA left off and so discarded;
  x86-64 `XSAVEC` and `XRSTOR` over the offered groups (52 to 78 ns), which skip and reset unused groups themselves.
  No learning of whether a thread used a group. The floor also swaps the thread register the host's runtime uses for
  its own thread-local storage (ARM64 `TPIDR_EL0`, x86-64 FS base) at every entry and exit, since the guest's value is
  its own (ARM64 `msr`/`mrs`; x86-64 `wrfsbase` where the host enables FSGSBASE, else `arch_prctl`, a counted slow path).
- Tests (hostile-guest part of each suite): guest threads each fill every offered group with distinct values,
  interleaved in every order of the bounded search, each reads back exactly its own and an unused group reads zero;
  in a test build, the floor asserts at the moment a guest thread is about to run that the registers hold its values
  or zero.

### 3.9 Count and replay

The count is in x18 as in the VM form; the host kernel preserves it across its calls and signals (in I the floor
saves it, since the kernel's Rust code may use x18 on Linux). Positions are resolved with the VM form's map, so a
position logged here is one the WebAssembly output and the VM form reach and the reverse (native.md section 5.7).
Replay to (count, pc) uses the check of 3.2 item 4, against the target thread's own stop word. Under
`ThreadOrdering::Parallel` P runs guest threads truly in parallel on the host's cores; I runs as many as it has
runners. The order of memory accesses between threads is the embedder's, through watches, as in the VM form.

## 4. Form P: each guest process a host process

### 4.1 Processes and threads

- A guest memory (`MemoryId`) is one stub process. A guest thread (`ThreadId`) is one host thread of it.
- Stubs are made by a **zygote**: one small stub forked from the executor at start, before it runs anything else,
  which unmaps everything but its own code and waits for requests. `memory_new` and `memory_duplicate` ask it for a
  child (measured, W0 probe: a request to a zygote until its new child runs, 89 to 103 µs Graviton4, 62 to 96 µs
  Xeon; a plain fork of a small static process 97 to 130 / 64 to 121 µs). The child maps its memory's memfd, its code area (read-execute, then
  closes that fd), its slot table, installs its handlers, sigaltstack and seccomp filter, and becomes the memory's
  stub. Exec is `memory_new` plus `memory_drop`: a fresh stub, as Linux gives a fresh address space.
- `thread_new` on a memory: the stub's thread that is in a kernel entry for that memory (the `clone` caller) makes
  the host thread with `clone` before it resumes; the first thread of a new memory is the stub's first thread.
- A stub has no thread of its own for commands: every change to a memory's guest view is a command the kernel's
  side queues and the next thread of that memory to resume applies before running guest code (4.2).
- One embedder process serves every stub of every container it runs. Stubs are children of the zygote; the
  executor learns of a stub's death through a pidfd (or, where `pidfd_open` is refused, the zygote's SIGCHLD over its
  channel) and ends the process as killed by SIGKILL, counted ("stub ended outside the kernel").

### 4.2 Memory

- Layout and frames as 3.1 and 3.5: the guest's own addresses, the memfd per memory.
- Mapping commands (`set_permissions`, `shared_map`, `shared_unmap`, a protection for copy-on-write or for code)
  change only the guest view, which no one but the memory's own threads can observe. They are queued per memory and
  applied by the resuming thread in the order made, before it runs guest code, so a system call that maps or
  protects costs no extra round trip, and the effect is in place when the call returns, as on Linux. A command on a
  memory none of whose threads is in a kernel entry (a fork's parent protections are not: the forking thread is in
  one) rides on the resume of one of its threads, which the kernel's side first stops (3.6). The kernel's side counts
  the host mappings each command creates, so `CpuError::OutOfMemory` is returned up front instead of a host failure.
- Storage changes (`write`, `zero`) are made by the kernel's side on the memfd at once, so the kernel reads what it
  wrote and every view sees it. One ordering rule joins the two: storage of a range is never changed while a
  queued command still has to take a view's access to it away (an unmapping, `mmap` with `MAP_FIXED` over a
  mapping); the queue is first applied by a round trip to the thread in the kernel entry, counted ("mapping applied
  before a storage change"). Otherwise a sibling thread could read the new storage through the old permission,
  which no order of the two calls gives on Linux.
- Dropping translations while other threads of a memory run (code changed, a range unmapped) stops those threads
  first, as the VM form's group stop does (docs/threads.md sections 7.5 and 10); slot table and code area are memfds
  the kernel's side writes directly.

### 4.3 The system call and its return

- Each guest thread has a channel page (shared memfd, mapped in its stub and in the embedder's process): a request
  word, an answer word, the exit and the register block. The runner (`Runner::run` on an embedder thread) writes the
  `Resume` and the queued commands, wakes the thread's futex word, and waits on its own with the slice as time-out;
  the guest thread's exit writes the `Exit` and wakes it back. No spin: on one CPU spinning only adds its bound
  (measured, cloud arm and intel rows), and across CPUs it burns a core per waiting side.
- Measured round trips for this exact shape (futex slot, ppid-like request): Graviton4 c8g virtual machine 2.20 µs
  one CPU, 5.58 to 5.64 µs two CPUs; Granite Rapids 1.72 / 4.18 to 4.25 µs; Zen 4 5.07 to 5.14 / 8.46 µs (the cloud
  rows). W0 probe, bare metal, unpinned as an embedder runs it: 2.40 µs on Graviton4 (the scheduler kept both sides
  on one CPU), 4.23 µs on a Xeon 8488C (section 11). When the CPU the woken side needs is busy with any other task, a round trip waits
  for the next scheduler tick: 1.99 to 2.00 ms measured on Graviton4 and Granite Rapids. P is only for hosts with
  idle cores (an evaluator input, section 7).
- Rejected: seccomp user notification on an untranslated `svc` (as fast on one CPU and 0.4 µs cheaper across CPUs,
  but it gives only the number and six arguments, so exits that need the whole register file would need a second
  path; and it is not offered by every sandbox); pipes, sockets and ptrace (all dearer, measured).
- Everything the guest reads that the kernel answers is a round trip in P, the time counter included (our vDSO has
  only `__kernel_rt_sigreturn`, so `clock_gettime` is a system call): this is what the evaluator counts as kernel
  entries.

### 4.4 Fork, exec, shared mappings, signals

- Fork while another thread of the parent is in a system call or running: the kernel's `memory_duplicate` is made
  during the forking thread's kernel entry; the parent's protections are applied by the forking thread before its
  fork returns (4.2); the other threads keep running until those protections land, and each page's copy is taken
  once (3.5): the child holds only the forking thread, as on Linux, whose host thread is the child stub's first thread.
- A shared mapping written by two guest processes: one memfd mapped in both stubs (3.5).
- Guest signals are the kernel's: it writes the frame and resumes the thread `FromRegisters`; nothing host-level.
- A host signal arriving inside the transition: 3.6 (the entry is already leaving; the resume window restarts).
- A host signal in the stub's own code that is not the stop signal is a fault of ours: the stub reports it if it
  can and ends; the process ends as killed, counted ("stub fault").

### 4.5 What is saved at a guest thread switch, and by whom

The host kernel, lazily, at every switch of host threads (3.8). At an exit the floor saves the general registers
to the channel; FP/SIMD and SVE stay live in the hardware across the futex wait (the host kernel preserves them;
Linux's own system call rule, which our kernel follows, already makes SVE's P and FFR registers zero at a system
call). `Cpu::registers` and `set_registers` read and write the channel; a thread resumed `FromRegisters` loads
everything from it.

### 4.6 How the kernel's runners drive it

`Kernel::new(config, parallelism)` gives N runners; the embedder runs each on a thread of its own. A runner's `run`
asks the kernel for the next thread and hands it to that thread's host thread through the channel, blocks until it
comes back, gives the kernel the `Exit`, and repeats; N is the number of guest threads that run at once. This
needs one call the executor interface lacks (section 8).

## 5. Form I: the kernel in process

### 5.1 Processes and threads

- Every guest memory of a container is a slot in the embedder's process (3.1); every guest thread is a register
  block. Runners are the embedder's threads, each running whichever guest thread the kernel gives it: the kernel's
  own model (a call that cannot finish parks its thread and returns), with no host thread per guest thread.
- Fork, exec and thread creation make no host process or thread: a slot and a memfd, or a block.

### 5.2 Memory

- The guest's view is its memfd mapped into its slot with the guest's permissions; host page tables enforce the
  guest's permissions inside the slot, the check of 3.2 keeps every access inside it. A fault inside the slot is a
  host SIGSEGV whose address less the slot base is the guest address.
- Mapping commands are made at once in the same process (no queue). Copy-on-write, written marks and code pages are
  3.5 and 3.4, with the faults handled in process (1.31 µs measured for the handler's round trip, plus the copy).
- Exact against wrapping checks: built exact (3.2 item 1): every access Linux faults on faults here at the same
  address with the same registers. The wrapping form (`and t, a, #(2^r - 1)` before the access, or 32-bit
  addressing with a 4 GiB slot) measured cheaper (+9.2% against +19.4% on Graviton4, +1.8 to +4.3% against +11.0
  to +15.2% on Granite Rapids) but sends an access outside the slot to the process's own memory at that address
  modulo 2^r, where Linux faults: a behaviour difference no counter can see. It is the owner's to allow (section 12).

### 5.3 The system call and its return

A plain call: `bl entry` (3.2 item 3) saves the guest registers (12 ns measured for the general file on Graviton4),
switches to the runner's host stack in our area (the guest's stack is guest memory), swaps the thread register, and
returns `Exit::Syscall` to the runner's loop as a function return; `Resume::Return` is a return into the call site
with x0 set. No host system call is made on the path.

### 5.4 Fork, exec, shared mappings, signals

As 4.4 with every step in one process: the forking runner protects the parent's view itself before returning; a
racing sibling on another runner is as in 4.4. A shared mapping is one memfd mapped into both slots. A host signal
inside the transition: 3.6. A signal arriving while the runner is in the kernel or the embedder's code sets the
request bit only; the floor sees it before its next entry into guest code.

### 5.5 What is saved, by whom; the runners

The floor, eagerly, at every guest thread switch (3.8). The runners are `Runner::run` on the embedder's threads,
each calling the executor's run-a-thread call in a loop (section 8).

## 6. What each form needs from the host

Both (embedder's process): `memfd_create` (or, where refused, an unlinked file in a tmpfs at /dev/shm), `ftruncate`,
`fallocate` with `FALLOC_FL_PUNCH_HOLE`, `mmap` (fixed addresses, `MAP_FIXED_NOREPLACE`, `MAP_NORESERVE`, shared file
mappings, read-execute), `munmap`, `mprotect`, `futex` (wait with time-out, wake; shared futexes on memfd mappings),
`tgkill`, `rt_sigaction`, `rt_sigprocmask` at start only, `sigaltstack`, `rt_sigreturn`, `clone` for threads, and a
4 KiB host page.
- P adds: `clone` without namespace flags for the zygote and its children (or `fork`), `exit`, `exit_group`,
  `pidfd_open` (optional), `prctl(PR_SET_NO_NEW_PRIVS)` and `seccomp(SECCOMP_SET_MODE_FILTER)` in the stub, and an
  idle core per running guest thread for the fast round trip. The stub's own filter allows: `futex`, `mmap`,
  `munmap`, `mprotect` (bounded as 3.7), `clone` with thread flags, `rt_sigreturn`, `sigaltstack`, `exit`,
  `exit_group`, and `fork` in the zygote only.
- I adds: `timer_create` and `timer_settime` for slices; `arch_prctl` on x86-64 hosts without FSGSBASE. No process
  is ever made, so I runs where `fork` is refused.
- Neither needs: ptrace, user namespaces, mount, chroot, capabilities, `process_vm_readv/writev` (withheld without
  CAP_SYS_PTRACE by Docker's default seccomp profile), `userfaultfd` (likewise), `perf_event_open`, /proc (nothing
  is read from it), execve of a memfd. Docker's default seccomp profile allows every call listed (it withholds
  only namespace flags of `clone`, which are not used); gVisor implements each of them, expected and not yet checked.
  W1 and W2 run their suites inside a container started with Docker's default profile, and under gVisor's runsc where
  it can be installed on the rented machine (if not, named as unchecked).
- P makes one host process per guest process, so a host limit on processes (a pids cgroup, `--pids-limit`) is also
  the guest's; I makes none.
- macOS: P and I need a 4 KiB host page to give the guest its own page permissions; Apple silicon has 16 KiB, so
  there neither form is offered (the VM form, Hypervisor.framework, works on every Mac with no entitlement beyond
  an ad hoc one). Intel Macs would need `os_sync_wait_on_address` for the futex (macOS 14.4 on; measured, confinement
  costs), `shm_open` for the memfd, Mach exception ports or signals for faults, `thread_suspend` for stops; not planned.
- Windows: no fork (our copy-on-write is our own, so none needed); a section per memory mapped with
  `MapViewOfFile3` into the stub process from outside (Windows maps into another process without its help, which
  would remove 4.2's queue), 64 KiB view granularity (the identity layout of 3.5 maps a whole slot as one view; a
  shared block at a 4 KiB boundary would need copying, a named slow path); `VirtualProtectEx` for permissions,
  `SuspendThread`/`GetThreadContext` for stops, a vectored exception handler in the stub, events for the wake.
  Not planned (Windows hosts are left out, brief-native.md decision 8).

## 7. The evaluator's inputs and the crossover

What the embedder can know before choosing, all without running the workload:
- The host, probed once at start (about 50 ms): whether KVM is usable (then the VM form; nothing here replaces it);
  the user-space width and page size; whether each form's reservation and calls of section 6 succeed (a refused call
  removes the form); the P round trip measured on this host (2,000 round trips unpinned); I's check cost, from a
  fixed translated loop run in both forms (or the per-CPU figure measured below); the number of idle cores now.
- The container's configuration: its memory and process limits (pids), which bound I's slot size and count.
- What earlier runs of the same workload recorded, as the embedder defines "the same" (image digest and command):
  kernel entries per second of guest CPU time, processes started per second, peak live processes, peak memory per
  process. Our kernel does not report kernel entries yet (section 8).

The crossover. Per second of guest CPU time, P adds k·c (k kernel entries per second, c the round trip less I's
call) and I adds f (its check's share). I beats P when k > f / c. From the measured files (cloud rows, medians):
- Graviton4 (c8g): f = 0.194, c = 2.2 µs on one CPU -> 88,000 entries per second; c = 5.6 µs across CPUs -> 35,000.
- Granite Rapids (c8i): f = 0.125, c = 1.72 / 4.2 µs -> 73,000 / 30,000.
- Zen 4 (c7a): f = 0.100, c = 5.1 / 8.5 µs -> 20,000 / 12,000.
- On a host whose cores are all busy, c is about 2 ms: I wins above about 100 entries per second.
- With the bare-metal round trips of section 11 the same arithmetic is given there.
Process creation adds to P only: 89 to 103 µs (Graviton4) or 62 to 96 µs (Xeon) per new guest process (W0 probe), against
a slot and a memfd in I.
The workload split measured on the laptop (confinement-costs.json, "measure workload-mix"): a shell loop starting
2,000 programs makes about 256,200 system calls in 0.73 s of user time (about 350,000 per second) and 2,000
processes: I by a factor; a Python start with 70 imports makes about 3,460 calls in 0.9 s (about 4,000 per second):
P (0.9% against I's 10 to 19%); a compute benchmark makes 9: P. These are expected outcomes from the arithmetic;
W2 measures them on both forms.
Rule given to the embedder (it chooses, not us): with no history, P (the owner's default for agentic workloads);
with history, the form whose arithmetic above is lower at the recorded rates on this host's measured c and f. The
form is chosen per container at start and fixed for its run. Changing it at an exec of a guest process (the kernel is
in the embedder's process in both forms, so one executor could hold memories of both) is possible later and not
built (section 12).

## 8. The interface question

These executors implement, as they stand:
- `wcontainers_executor::Executor` (one instruction, `execute_single_instruction_in_place`): each translates the
  instruction at look-ahead none and runs it in a scratch memory (P: a scratch stub; I: a scratch slot) with the
  given processor state and memory deltas, through the executor conformance suite.
- `wcontainers_abi::Cpu`, which `wcontainers-kernel-linux` requires of its executor (`C: Cpu + Executor`), every
  method as 3.5, 4.1 and 5.1 say.
- Not `PerThreadWatch` (either trait): threads of one host process share one set of host page tables, so one
  thread's narrower permissions cannot be expressed in either form. Watches per process (`Process::watch_memory`)
  work through the guest view's protections. Named in section 9.
- `Runtime`, `Container`, `Interrupter` are the runtime's and need nothing new: the embedder picks the form by which
  runtime it makes.

Where an interface would have to change, the smallest change, for the owner (nothing here is edited by this track):
1. Running a thread. `KernelRunner::run` is `unimplemented!` because "the executor interface has no call that runs
   a thread" (crates/kernel/implementation/linux/executing.rs). Every executor needs it, these two included:
   ```rust
   pub trait Executor {
       // ...as today...
       /// Runs the thread until it comes back, raising its stop after the slice if one is given.
       fn run_thread(&mut self, thread: ThreadId, resume: Resume, slice: Option<Duration>) -> Exit;
   }
   ```
   (`ThreadId`, `Resume`, `Exit` from wcontainers-abi). With it, `Runner::run` is the loop of 4.6 and 5.5.
2. The evaluator's count of kernel entries, so that the embedder can record the rate section 7 needs:
   ```rust
   pub trait ProcessInfo {
       // ...as today...
       /// Returns how many times the process's threads have entered the kernel, ended ones included.
       fn kernel_entries(&self) -> u64;
   }
   ```
3. Stopping a running thread from outside the runner. The kernel's `suspend_tasks` and the runtime's `Interrupter`
   already exist; the executor needs from the runner only the thread's host thread, which it keeps itself. No change.
4. Nothing else: `Cpu::address_space` carries the ceiling, `ProcessInfo::address_ceiling_distance` and
   `on_process_outgrown` cover growth, `CpuError::OutOfMemory` covers slots running out.
Considered and not asked: a clock the system publishes for the floor to read without a kernel entry (as Linux's
vDSO does) would remove P's round trip per time read, but it changes what the system decides (every read is the
system's answer today); listed for the owner in section 12 instead.

## 9. Named differences from the WebAssembly output and Linux

- P: a guest load or store to our area in its stub (register and channel pages, sigaltstack; the code area and
  slot table where the host has no execute-only) succeeds where Linux and the other outputs fault. Only a guest that
  uses an address it never mapped can see it; it changes nothing of ours (3.7). This extends the VM form's accepted
  "our reachable pages" difference (native.md decision 4) to these pages. Counted: no (a successful access cannot
  be seen); listed for the owner.
- P and I: no per-thread watches (8). The VM form's single-step turns are not available (3.6).
- P and I: pointer authentication keys and memory tags are the host kernel's in a host process; ARM64 features
  whose results depend on them (FEAT_PAuth, FEAT_MTE) are not offered to the guest by these executors until the
  owner rules (section 12). They are reported as absent the way a CPU without them is.
- I with the wrapping form, if the owner allows it (5.2).
- P's ceiling is 2^44 against the VM form's 2^45; I's is 2^r per process.

## 10. Asked of other tracks (written up in the status file; not made here)

- Translator and ARM64 crates: the form value of 3.2 with its four places, keyed in the translation cache; `REACH_BITS`
  and `MIRROR` (crates/wcontainers-arm64/src/mirror.rs) made per form; two more withheld registers for I.
- Native (VM) track: the monitor's kept translations (`Translator` in hardware-virtualization's translate.rs)
  usable from a crate that does not pull the hypervisor crates; `stop` and `translation` of the inside are already
  target-independent and public.
- Kernel: K2 as native.md asks (placement low first, the memory size given at start); `kernel_entries` if the owner
  takes 8.2.
- x86 track: the x86-64 native output (N9), with the same four places and its cross-modifying-code rule (3.4).

## 11. Measurements behind this plan (W0 probe)

Program: scratch native-without-virtual-machine/w0/probe.c (gcc 13.3 -O2 -static; 21 batches, median and minimum
of the batch means), outputs probe2-arm.out and probe2-x86.out beside it. Machines: the rented ARM64 machine
(Graviton4, Neoverse V2, 96 cores, bare metal, Linux 7.0.0-1018-aws, 4 KiB pages), load average 24 to 26 from other
tracks during the run; the rented x86-64 machine (Xeon Platinum 8488C, 96 processors, bare metal, intel_idle), load
average 1.8 to 3.8. A first run is discarded: a spinning thread of its stop test was left on CPU 3 and slowed the
futex figures (probe-arm.out, probe-x86.out, kept); a second hung on a test bug (probe2-*.out, kept for its
address-space part, which is right). Results: probe3-arm.out, probe3-x86.out, and probe4-arm.out for the ARM64
cross-CPU futex figures (load average 28 to 38; CPU 61 was 96% busy with another track's job, CPU 60 50%, 62 42%,
63 4%).
- Address space: user space 48 bits (ARM64) and 56 bits (x86-64). A static PIE leaves [2^16, 2^45) free on both;
  [2^45, 2^47) is free on ARM64; the binary, vDSO and stack sit in [2^47, 2^48) on ARM64 and just under 2^47 on
  x86-64. A non-PIE static binary sits at 0x400000 and blocks a reservation from 2^16 (hence the slots from 2^32 in I).
- Code: `PROT_EXEC` alone runs and a load from it faults (execute-only) on both; a memfd written in one process and
  mapped read-execute in another runs on both; a memfd with `MFD_NOEXEC_SEAL` maps executable.
- Faults and signals in one process, Graviton4 / Xeon: a fault on a PROT_NONE page whose handler skips the
  instruction, 1.31 to 1.32 / 1.31 to 1.40 µs; `tgkill` to itself with an empty handler 1.20 to 1.24 / 1.02 to 1.06 µs;
  `mprotect` of one page 391 to 394 / 301 to 306 ns; `mmap` of one memfd page over another 1.04 to 1.49 / 0.90 µs; a
  hole punched in one memfd page 0.68 to 0.78 / 0.51 to 0.55 µs.
- Stop latency, from the sender's `tgkill` to the target's handler, target spinning on another CPU: same process
  1.72 to 1.82 µs (p90 2.0 to 2.9) / 1.85 to 2.28 µs; another process 1.63 to 1.89 µs / 1.79 to 1.90 µs.
- Process creation: a fork of this small static process until the child runs and is reaped 97 to 130 µs / 64 to
  121 µs; a request to a zygote until its new child runs 89 to 103 µs / 62 to 96 µs (the lower figure with the
  parent pinned, the higher unpinned).
- The futex slot between two processes, ppid-like: Graviton4 one CPU 2.34 µs (min 2.33, p90 2.35), two CPUs 4.55 to
  4.92 µs (min 2.19 to 2.49), unpinned 2.40 µs (the scheduler put both on one CPU); Xeon one CPU 1.90 µs, two CPUs
  3.98 µs (min 0.36), unpinned 4.23 µs (min 2.82). Both pinned to a CPU that another job keeps busy (CPU 61): no
  batch of 2,000 round trips finished in 60 s, so above 1.4 ms per round trip, as the cloud rows' 2 ms.
- So with these round trips, by section 7's arithmetic: Graviton4 unpinned c = 2.4 µs with f = 0.194 (c8g's) gives
  81,000 kernel entries per second; Xeon 8488C unpinned c = 4.23 µs with Granite Rapids's f = 0.125 (the nearest
  measured; this CPU's f is measured at W2) gives 30,000.

## 12. Decisions for the owner

1. Names: the two crates (`host-process`, `in-process`) and the form value's variants.
2. I's wrapping check (5.2): about 10 points faster on Graviton4, and a guest access outside its slot lands in its
   own memory instead of faulting.
3. P's reachable stub pages (9).
4. PAuth and MTE under P and I (9).
5. A clock the system publishes for reads without a kernel entry (8).
6. Changing the form at a guest exec (7).
7. The two interface additions of section 8.

## 13. Milestones

**W1: form P on ARM64 Linux.** Needs the translator's `HostProcess` form (10). Builds: the shared layout, floor,
code area, memory with copy-on-write, stops (section 3), the zygote and stubs, the channel and the runners
(section 4), the probe and the seccomp filter. Pass bar, every item on the rented ARM64 machine:
- the executor conformance suite and the abi `Cpu` behaviour the kernel relies on (the kernel test crate's
  programs: hello, the fork, exec and signal program, and busybox's `text` and `files` commands, as N3's bar) give
  the reference's output and status; exits, registers and memory equal the WebAssembly output's at every look-ahead,
  with and without the count (the VM form's differential test, seeded, with its case knob; cases run recorded);
- the count at every stop is identical to the WebAssembly output's (forced interrupts at every counted event);
- the adversarial cases, each a kept test: a fork while a sibling thread is in a system call and another runs; a
  shared mapping written by two guest processes, checked against the reference; a stop signal landing at every
  instruction of the floor's entry and resume windows (forced by a seeded delay); a guest that writes its own
  channel and register pages with forged commands and registers (it changes only itself; no host code runs that we
  did not translate); a stub that tries a host call outside its filter is killed; the register-group isolation
  tests of 3.8;
- the suites pass inside a container started with Docker's default seccomp profile;
- measured and recorded with machine and load: the guest's getppid round trip unpinned, under 1.25 times the W0
  probe's unpinned futex slot on the same machine; busybox `sh` starting 1,000 programs, its time against the
  reference; every slow path with its counter.

**W2: form I on ARM64 Linux, and the crossover.** Needs the translator's `InProcess` form. Builds: slots, the check,
in-process entry and eager switch, the slice timer (section 5). Pass bar:
- the same suites as W1 (conformance, kernel test programs, busybox, differential, count, adversarial,
  register groups, Docker's default profile);
- exactness of the check: a seeded random test (knob for cases; count recorded) of accesses at, just inside and
  just outside the slot edges, tagged and untagged, every access form, each faulting exactly where the reference
  does with the reference's registers; slot exhaustion gives ENOMEM at fork, counted;
- speed: the translator's benchmark set under I against the same programs run plainly, alternating pairs, within
  the measured X32-e figure plus 3 points on the same machine (+19.4% on Graviton4 c8g; re-measured on the rented
  machine at W2 start); the getppid entry under 100 ns median;
- the crossover measured with both forms built, on the ARM64 and x86-64 machines where each runs: the compute
  benchmark, a Python start, and busybox `sh` starting 1,000 programs, each in both forms with its kernel entries
  per second, and the measured k at which the two cost the same, against section 7's arithmetic.
x86-64 hosts follow when the x86-64 native output exists; the plan's x86 items are listed in 3.4, 3.8 and 10.
