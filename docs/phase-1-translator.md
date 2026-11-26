# wcontainers, phase one: the translator

Status: agreed plan, 2026-11-14. Nothing is built yet.

Words used here:
- **guest**: the ARM64 program being run.
- **host**: whatever runs our WebAssembly (wasmtime, a browser, later Cloudflare).
- **engine**: the host's own WebAssembly compiler.
- **look-ahead**: how much code the translator sees at once. It is the one dial.
- **piece**: the WebAssembly function produced by one translation. It starts at one guest address.

Every figure below says whether it was measured or is only expected.

## 1. What phase one delivers

A Rust translator that turns a blob of ARM64 machine code into WebAssembly which behaves exactly like
the original, plus the test system that proves it.

The translator's only concern is ARM64 machine code in, WebAssembly out. It knows nothing about ELF,
files or Linux. Reading program files is a separate job that calls it.

Scope is any ARM64 Linux program. Nothing in the design is chosen for one program. The translator
covers every instruction of the CPU we present to the guest, taken from Arm's own specification.
Figures below come from five sample binaries (busybox, the C library, ripgrep, git and Node). Node is
quoted most often only because it is the largest of the five and so the hardest test of size and
engine limits.

Done means:
- An ARM64 program built as one flat blob, which only computes and prints, gives byte-identical output
  under wasmtime and Chrome as on real ARM64 Linux.
- The three test layers in section 9 pass with zero mismatches, at every look-ahead.
- The code of the five real binaries in section 10 translates and loads, and the size and time numbers
  are recorded.

Out of phase one, but the design must leave room for each:
- The kernel. Translated code reaches it through one generic system call entry point (section 6).
  In phase one a stand-in sits behind that entry point and answers five call numbers: `write`,
  `writev`, `exit`, `exit_group` and `brk`. Every other number returns "not implemented" and bumps a
  counter. The real kernel later plugs into the same entry point and the translator does not change.
  (The five are expected; the first hello world will show the true minimum.)
- Starting real program files. Reading ELF, placing a program in memory and running the guest's own
  loader for shared libraries all belong to the kernel phase. The translator's output must already
  work at any load address.
- Threads, signals, fork.
- Code a program generates while running.
- Hosts that cannot load new code while running (Cloudflare).

## 2. Rules already decided

1. Our own translator, straight to WebAssembly. No LLVM and no optimizer of our own. The engine does
   the heavy optimization. We only make layout choices the engine cannot make because it does not know
   ARM64's conventions.
2. The guest's whole state is in guest memory or in the saved register block at every point where it
   can stop. WebAssembly's own call stack is a cache of the guest stack. Losing it costs some
   translation work, never correctness.
3. The translator has one operation: translate from this address, with this look-ahead. Look-ahead is
   none, bounded (N instructions) or full (the whole blob). Layout choices are not settings. They are
   what the translator does with what it can see.
4. Every instruction's meaning is written once, as the WebAssembly it becomes. There is no second copy
   of instruction logic anywhere, including in any future interpreter.
5. Every fast path has a slow twin, and every slow twin has a named reason, a counter and a warning.
6. Every place the runtime touches the host, or has a cheaper future version, is a hook from day one.
   First implementations are naive.
7. Look-ahead none is kept forever as the reference. Every other look-ahead must give the same result.
8. Written in Rust. Comments are brief and say what the thing is for.

## 3. Parts (suggested crate names)

- `wcontainers-arm64`: everything that is only true of ARM64: the instruction type, the decoder
  wrapper, the meaning of every instruction, the registers and flags, and the compiler habits the
  translator recognises. It knows nothing about look-ahead or layout.
- `wcontainers-translator`: takes a blob of code, start addresses and a look-ahead; emits WebAssembly.
- `wcontainers-abi`: what the translator and the kernel must agree on: the saved register block
  layout, the system call entry and the hook definitions. Shared with the kernel plan.
- `wcontainers-translator-test`: a small runner for wasmtime and a small one for browsers, which put
  a flat blob in memory, start at its first instruction, and hold the stand-in system calls. Also the
  case descriptions for `proptest`, the native reference runner, and the comparison.

Reuse, checked 2026-11-14:
- `yaxpeax-arm` 0.5.0 (0BSD, works without the standard library) as the first decoder. Its README lists
  scalable vectors as missing and makes no claim about newer extensions. Keep it behind our own
  instruction type so it can be replaced.
- `wasm-encoder` 0.263.0 (Apache-2.0 or MIT) to write WebAssembly.
- Arm's machine readable specification (JSON files, BSD licence) as the list of every instruction form.
  It is the source of truth for "have we covered everything", and the decoder is checked against it.
- No ELF reader of our own. Reading program files uses the existing `object` crate (Apache-2.0 or MIT,
  717 million downloads, updated 2026-11-10). Phase one uses it only as test tooling for milestone 5,
  to pull code and start addresses out of real binaries. It is never a dependency of the translator. The
  kernel phase uses the same crate for "start a program".
- elfconv (Apache-2.0) as the benchmark to beat, run on the same binaries.

## 4. The translator's input

- **A blob of bytes** holding ARM64 machine code, and where in the blob the code is.
- **One or more start addresses, and a look-ahead.** The caller passes every start it knows. One is
  enough to be correct. More starts mean more is translated ahead of time and less at run time.
  Measured on Node: 52% of its functions (48,700 of 93,410) are never the target of a direct call or
  jump, so the translator cannot find them from the code alone.
- There is no other kind of hint. In particular the translator is never told "these bytes are data":
  it only follows code from its start addresses, so it never reaches data.
- The same interface serves code translated ahead of time, on first use, and code a program generates
  while running, which never had a file.
- The blob's bytes also sit in guest memory unchanged, code included, because programs read their own
  code for constants and jump tables. Placing them there is the caller's job.
- Output never contains an absolute address. The load address arrives as an imported value and every
  address is "load address plus offset".
- The cache key is the blob's content hash, the start addresses, the look-ahead, what to optimise
  for (section 11), the translator version and its tuning numbers. Caching is the caller's job.

## 5. Look-ahead: the one dial

### What a piece is

One translation produces one piece: a WebAssembly function that starts at one guest address. Every
piece, at every look-ahead, is entered and left the same way, so pieces made at different look-aheads
can call each other freely:

- A few registers arrive as parameters and leave as results: the first argument registers and the
  stack pointer. How many is a tuning number (section 11).
- One more parameter says where the caller expects control to come back.
- Every other register is in shared state.
- A table maps guest addresses to pieces (section 5, "Looking up an address").

### None

The translator sees one instruction. Each instruction is its own piece. It reads its inputs from
shared state, works out all four flag bits, writes its results back, and passes control to the piece
for the next address. Nothing is merged and nothing is assumed about what comes next.

This is slow and very easy to reason about. It is the reference for testing, and it is the model for
any future interpreter (section 7). It is not meant for large blobs: Chrome allows a million functions
in a module and Node alone has 9.13 million instructions (both measured).

### Bounded (N)

The translator follows the code from the start address until it has seen N instructions. That stretch
becomes one piece. Inside it the translator can do everything in "What seeing more allows" below.
Where the stretch ends, the piece passes control to the piece for the next address.

This is what unknown jump targets, code generated at run time, and continuing after the stack is lost
all use. The default N is a tuning number.

### Full

The translator sees the whole blob. It cuts the code into functions and makes one piece per function,
so loops and calls stay inside pieces. This is the normal mode for code known ahead of time.

### What seeing more allows

- **Registers are locals.** Each register the piece uses is a WebAssembly local, written back to shared
  state only around calls and at the piece's edges. Measured: this takes loops from up to 2.9 times
  slower than native to about 1.3.
- **Flags are kept as the last comparison.** A compare followed by a conditional branch becomes one
  WebAssembly comparison. Measured: 0 to 15% faster than working out all four bits every time.
- **Jumps inside the piece** become WebAssembly blocks and loops.
- **Direct calls** become WebAssembly calls to the target's piece.
- **Calls through a register** look up the target address and make an indirect call.
- **A jump that leaves the piece** becomes a WebAssembly tail call, direct when the target is known
  and through the lookup when it is not.
- **Returns** check that the return address is where the caller expects control back, then return.
  Measured: free in wasmtime, up to 14% on a test that does nothing but call, in Chrome. If the check
  fails, the piece leaves through "go to address" (section 7).
- **Switch statements.** Compilers emit a table lookup followed by a jump through a register. Recognise
  the patterns and emit a WebAssembly jump table. An unrecognised jump through a register simply
  leaves the piece through the lookup. Counter: "jump through a register not recognised". How much
  recognition gains is unmeasured.

Passing a few registers as parameters was measured on small tests: four argument registers plus the
stack pointer was about 20% faster on calls than none, and eight was worse than none in wasmtime.

### Finding functions (full look-ahead)

Measured on Alpine's builds: busybox and the C library have no symbols and almost no unwind tables
(132 and 868 bytes), while ripgrep, git and Node have full unwind tables. So the caller will often
know few starts, and nothing may depend on having them.

What the translator finds by itself, from the code alone:
- the start addresses it was given,
- targets of direct calls,
- the target of any direct jump that leaves its function,
- addresses the code builds that point into the blob's code.

Where the caller's extra start addresses come from, when the file has them: symbols, unwind tables,
init and fini lists, and addresses in data that point into code.

The rule that makes this safe: correctness never depends on a guess. If the guest jumps to an address
that has no piece, one is translated from that address (section 7). A wrong guess costs speed and
bumps a counter. The same instruction may appear in more than one piece.

### Looking up an address

Two designs, both to be built and measured:
- A WebAssembly table with one slot per 4 bytes of code. Fastest, largest.
- A compact table with one slot per piece, plus an array in memory from code offset to slot. One extra
  memory read per call through a register.

Measured limits that force this choice: Node has 9.13 million instruction slots and Chrome allows
10 million table entries. So the first design alone is not safe.

### Engine limits (read from V8's source, 2026-11-14)

A module may be 1 GiB, hold 1,000,000 functions, and a function body may be 7.65 MB with 50,000
locals. Node has 102,003 functions by its symbol table. The translator must be able to split one blob
into several modules when a limit is near. Calls across modules go through the lookup.

## 6. Saved register block and the module contract

- One WebAssembly instance per guest thread. Registers that are not in locals live in that instance's
  globals. The cost of creating an instance for a large module is unmeasured.
- Each module exports "save state" and "load state", which copy every register to and from a block in
  memory with a fixed layout: x0 to x30, stack pointer, program counter, flags, v0 to v31, the two
  floating point control registers, and the thread pointer. One more word in the block is the thread's
  stop flag (section 7).
- Imports: the guest memory, the page permission table, the load address, the lookup table, and the
  hooks it calls (system call, stop requested, memory fault, load new code, warn).
- Exports: a "run from this address" entry, save and load state, and a description section listing the
  pieces, the look-ahead used, and the blob's hash.
- A system call instruction writes changed registers back first, like a call, then calls the system
  call hook with the number and six arguments.

System calls are layered:
- **Translated code to kernel: one generic entry.** It takes the call number and six raw values and
  returns one. The number can be a run-time value, so the translator knows nothing about individual
  system calls.
- **Inside the kernel: one typed Rust function per system call.** A single dispatch table maps the
  number to its function and turns the six raw values into proper types. All the ergonomics live here,
  in code we control.
- **Kernel to host: the hooks in section 12.** Typed and few.

How the kernel reaches guest memory is a kernel-phase decision. The translator only emits imports, so
it does not need to know.

### Address space

The process gets the largest address space the host offers: 64 bit memory where the host has it,
32 bit where it does not. A request for an address above the host's cap is refused, with a counter.

- **32 bit memory**: a process has at most 4 GiB of address space and the kernel must place
  everything below that. An address above it is a fault, never quietly cut down (section 7).
- **64 bit memory**: measured free in Chrome and 5 to 8% slower in wasmtime on small tests. Safari has
  it only in Technology Preview. Chrome caps it at 16 GiB.

Measured: untouched space costs no real memory. A 4 GiB and a 16 GiB memory in V8 each added 2 MB
until touched, and a 256 GiB memory in wasmtime ran in 15 MB. V8 refuses anything above 16 GiB. So the
limit is the engine's cap, not cost.

Measured on real ARM64 Linux with the address space capped (`ulimit -v`), running a one-line script:
Node 22 reserves 412 MB and runs under a 512 MB cap. Bun 1.4.4 reserves 9.8 GiB when uncapped, but
under a 4 GiB cap it reserves 3.7 GiB and runs, and it still runs under 1 GiB. So both fit in 32 bit
memory. Real workloads under a cap are untested.

## 7. The rare paths

All of them are the same operation: translate from an address.

### The bottom loop

A small loop sits at the bottom of the WebAssembly stack. It reads the program counter, looks up the
piece for that address, translates one if there is none, and calls it. It is how a program starts and
how it continues after anything below.

### "Go to address"

Used when a return goes somewhere other than where its caller expects (longjmp, C++ exceptions). The
piece saves its registers, records the target and throws a WebAssembly exception. The bottom loop
catches it and continues at the target.

This is always correct. The functions that were in progress are then continued by pieces translated
from the addresses where they resume. Those pieces are ordinary fast code that happens to start in the
middle of a function. Counter: "left to an address".

A later improvement, "landing regions", lets a piece catch the exception itself when the target is one
of its own known landing points. Measured cost of such a region: none in Chrome, up to 60% on a
call-only test in wasmtime, so it goes only where landing points are known.

### The stack is lost

After a fork, or when resuming from a saved snapshot, there is no WebAssembly stack. The bottom loop
starts at the saved program counter, and each function that was in progress is continued by a piece
translated from its resume address. Counter: "continued after stack loss".

### Adding code to a running program

A missing piece is translated with a bounded look-ahead and handed to the "load new code" hook, which
loads a module that shares the guest memory and the lookup table and adds the new piece to the table.
Phase one must prove this once in wasmtime and once in Chrome. Counter: "translated at run time".

### Hosts that cannot load new code (shelved)

There the "load new code" hook is answered by a small WebAssembly interpreter running inside our own
module, which executes the piece the translator just produced. So the interpreter is the translator
plus something that runs its output, with no ARM64 logic of its own. An existing crate may serve:
`wasmi` 2.0.0 (MIT or Apache-2.0, checked 2026-11-14). Not built in phase one.

### The stop check

WebAssembly code cannot be interrupted from outside, so translated code has to ask. Without this a
thread in a loop never gives other threads a turn, and Ctrl-C or a timer signal cannot reach it.

- **The check.** Translated code reads the thread's stop flag, one word of memory. If it is zero the
  code carries on. If not, it saves its registers and calls the "stop requested" hook. The kernel
  behind that hook may deliver a signal, switch threads, or simply clear the flag. Afterwards the code
  loads its registers back and continues, or leaves through "go to address".
- **Where.** At every backward jump, and at the entry of every piece that calls or jumps to another
  piece. A piece that does neither cannot be part of endless recursion, so it needs no entry check.
  Measured: checking every entry made a sort with a tiny comparison function about 40% slower in
  Chrome; checking only pieces that call out did not.
- **Who raises the flag.** Something outside the guest thread, through memory both can see: the page's
  own thread in a browser, a host thread in wasmtime. So the guest memory is created shareable.
  Measured in Chrome: a page raised the flag 20,000 times while a worker ran a tight loop; all 20,000
  were noticed, in 230 ns on average and 30 microseconds at worst. A shareable memory ran at the same
  speed as a plain one.
- **Cost, measured on the toy translator.** Nothing on loops that do real work. On the tightest loop
  (three instructions a turn) about 4% in Chrome and 5 to 15% in wasmtime. On call-heavy code 0 to 6%
  in Chrome. wasmtime's own built-in check was no cheaper and cost 20 to 40% on call-heavy code, so it
  is not used.
- **Hosts with no second thread (shelved, with Cloudflare).** Nothing can raise a flag there. The
  fallback is a countdown: the code counts its own loop turns and calls the hook every so many.
  Measured: 0 to 12% on the tightest loop in Chrome, 10 to 20% in wasmtime. Not built in phase one.

In phase one the stand-in behind the hook clears the flag and returns.

### The progress count

For exact replay (`ReplayDeterminism::Exact`) translated code keeps a count defined on the guest's own
instructions, the same at every look-ahead and on every output: one event for every executed direct jump
to its own address or below (taken or not), call, return, and jump through a register or memory. Asked for
per translation (`ProgressCount::Kept`); without it the output is what it was.

- **Where it lives.** The u64 at the stop flag's place becomes the limit, and the count is the u64 after it
  (offsets 800 and 808 on ARM64, `contract::progress_limit_offset` and `progress_count_offset`). Whenever
  registers are saved the block holds the count. In translated code the count is a piece's last parameter
  and last result, so a call passes it and gets it back with no memory access, at every look-ahead. A tail
  call off the hot path (to a stop, to a loop's checked copy, to exact code) passes it through the block,
  and code of the full emitter (full and bounded look-ahead) that carries on after a stop or a system call
  loads it from there. At none it stays in the piece's local across both: none's system call and stop check
  save it with the registers and never change it, so nothing is loaded back.
- **The check.** The stop check stays where it is, and tests "count >= limit" instead of the flag. Raising
  the stop writes a limit of 0.
- **Positions.** A stop is at (count, pc), the count being the events before the instruction at pc. Between
  two events the pc only grows within the guest's own control flow. A transfer the kernel makes does not
  count: delivering a signal to a handler and `rt_sigreturn` back move the pc down with no event, so a
  position inside a handler before its first event is not ordered by (count, pc). The stand-in kernels
  deliver no signals, so the test crate never meets one; replaying programs that take signals needs such a
  transfer counted as an event, or positions ordered by it too. A position logged at any look-ahead is
  reached again from any other: run
  with the position's count as the limit (lowered until the first stop is at or before the position), then
  carry on at look-ahead none, which checks at every instruction, until count and pc are the position's
  (`progress::reach` in the test crate).

### Page permissions

Linux lets a program say what each 4 KiB page of its memory allows: reading, writing, both or
neither. The CPU enforces it, and programs rely on that:

- Address zero is forbidden, so a null pointer stops the program. Java and Go leave out their own
  null checks, let the access fail, catch the failure and carry on.
- A forbidden page sits under every stack, so runaway recursion stops instead of overwriting memory.
- Java pauses its threads by making one page unreadable and letting compiled code fail on it.

WebAssembly memory has no permissions, so translated code checks them itself.

- **The table.** One byte per page, per process, saying whether reading and writing are allowed on
  that page, and also whether they are allowed on the page after it. The guest cannot reach the table.
- **Accesses that cross onto the next page are exact.** If the byte is clean, this page and the next
  both allow the access, so it is safe even if it runs over, and the normal path is still one lookup.
  Only on a page that sits right before a restricted one does the code take a slower path, which
  works out whether the access really crosses and faults at the exact address if it does.
- **The check.** Before a memory access, translated code looks up the byte for the page. If the access
  is not allowed, it saves its registers and calls the "memory fault" hook with the address and
  whether it was a read or a write. Registers are exact, so the program's own handler can run and the
  access can be tried again.
- **Who writes the table.** The kernel, through the "set page permissions" hook, with what the program
  asked for. The host may lock pages further for its own use, such as sharing pages between processes
  (kernel plan). A fault on a page the host locked is settled by the host and never reaches the kernel.
- **Checks the translator drops.** A check is skipped only where an earlier check in the same piece
  already covers the same page: the base register has not changed and the offset is small and known.
  For the stack, the page is checked when the stack pointer moves down, which covers the function's
  frame, so accesses inside the frame need none. A frame larger than a page checks each page. This
  works on any stack, including ones a program makes for itself. Apart from that frame check,
  nothing proved before a call, a system call or a stop is relied on after it, because the table may
  have changed. Measured on the five sample binaries,
  looking only within one straight run of instructions: 44 to 58% of memory accesses go through the
  stack pointer and 10 to 22% reuse a base already checked, which leaves 32 to 43% needing a check,
  or 9 to 13% of all instructions. Reasoning over a whole function should drop more; unmeasured.
- **Running code.** Whether a page may be run is checked when code is translated from it, not at every
  jump. When a range stops being runnable its pieces are dropped.
- **Cost, measured on the toy translator with no checks dropped.** 0 to 11% in Chrome and 0 to 24%
  in wasmtime. One result is unexplained: the call-only test was 29 to 37% slower in one of the two
  layouts tried in Chrome, and unaffected in the other. Re-measured at M2.
- **Addresses above the address space.** An address beyond what the host gives us must never be cut
  down to a valid one. The engine's own bounds check stops it. The process is killed as Linux would
  kill it for a bad address, but its handler cannot run, because the registers are lost when the
  engine stops WebAssembly code. Counter: "fault without exact registers".
- **What still gets through.** A permission change to a page holding a running function's own frame
  is noticed at the next function entry, not at once.

Measured and rejected:
- **Comparing every address with 64 KiB.** Cheaper (0 to 7% in Chrome, 0 to 11% in wasmtime) but it
  only catches null pointers.
- **Sliding every address down by 64 KiB**, so the engine's bounds check catches null. 0 to 8% in
  Chrome, 2 to 5% in wasmtime, but the registers are lost, so no handler can run.
- **A table that also says where each page really lives**, which is what a CPU does in hardware and
  would allow pages to be shared and copied on first write everywhere. In Chrome the tightest loop was
  6 to 19% slower, a sort 3 to 37% and call-only code 34 to 65%; in wasmtime 0 to 27%. No check could
  be dropped, stack accesses included, and a whole container would have to fit in one memory.
- **Comparing the stack pointer with a limit inside the stop check.** It cost the same as the plain
  flag, but it is wrong for programs that move to stacks they made themselves.

### Counters and warnings

Each has a name, a count, the time spent, and the address concerned. A warning is raised once per
address past a threshold, through the warn hook.

Phase-one reasons: translated at run time; left to an address; continued after stack loss; jump
through a register not recognised; software fused multiply-add; unsupported instruction reached;
system call not implemented; exact floating point requested; address above the host's cap; stopped by
the flag; memory fault; fault without exact registers.

### Two workarounds for what WebAssembly lacks

Both are large costs that a future WebAssembly could remove. Each stays behind a small set of named functions
in the emitters, so taking it out is a local change.

- **The inline stop check**, because nothing can interrupt running WebAssembly code from outside. If the
  platform gains a way to interrupt it (as native code already has, with a signal), the check goes. Where
  the code changes: `push_stop_wanted` in `code.rs` (the test, flag or count), called by `stop_check` in
  `code.rs` (look-ahead none) and `stop_check_or_deep` in `full.rs` (full and bounded), whose entry check is
  chosen by `FullEmitter::entry_check`. With the progress count of `ReplayDeterminism::Exact`, the count is
  merged into this same check ("count; if count >= limit, leave"), so the count would stay where the flag
  test goes. Where nothing else can run while
  the guest runs (one thread, no shared memory), the count is always emitted, as the only bound.
- **The per-page permission check**, because WebAssembly memory has no page protection with catchable
  faults. If the platform gains it (the "memory control" proposal is the one to watch), the check goes.
  Where the code changes: `permission_fast`, `permission_exact`, `permission_range` and
  `permission_checker` in `code.rs`, called from both emitters, and the reasoning in `full.rs` that drops
  checks (kept pages, frame checks and proven loops), which would go with them. `Omit::permissions`
  already builds the output without the checks, for measuring.

## 8. Where ARM64 and WebAssembly differ

Each item needs its own tests.

- **Division.** ARM64 gives 0 for division by zero and wraps on overflow. WebAssembly traps. Guard both.
- **Number to integer conversion.** ARM64 saturates. Use WebAssembly's saturating conversions.
- **Fused multiply-add.** The result must be exact, and WebAssembly has no guaranteed form. Probe the
  host once to see whether its relaxed form really fuses. If not, use software, with a counter.
  Measured: 900 such instructions in Node, 13 in the C library.
- **Not-a-number results.** ARM64 fixes which bit pattern comes out and WebAssembly does not. Results
  are always exact; there is no loose mode and no setting. On a host whose CPU already follows ARM64's
  rules the engine's output is used as it is. Measured: 26 of 26 cases bit-identical between native
  ARM64 and V8 on an ARM Mac. On other hosts the translator adds a fix-up after floating point
  operations. Which applies is found by a probe at start-up, and it is part of the cache key. The
  fix-up's cost on Intel and AMD hosts is unmeasured. Tests compare not-a-number results bit for bit,
  with reference results recorded on ARM64 so they can be replayed on other hosts.
- **Rounding modes and status flags.** WebAssembly has one rounding mode and no status flags. A program
  that changes the mode gets floating point done in software, with a warning. Changing over
  automatically when a program sets a mode is a runtime job for the kernel phase.
- **Atomics.** The older two-instruction style becomes a compare-and-exchange. The single-instruction
  style maps directly. Measured: both styles are rare (240 in the C library, 105 in Node).
- **CPU features.** We decide what the guest is told the CPU has. Told: the base set, floating point,
  vectors, checksum instructions and single-instruction atomics. Never told: the encryption extensions
  and scalable vectors, so programs use their fallback code. These are never emulated in software,
  because that would advertise hardware speed and deliver the opposite. An optional feature is offered
  only if WebAssembly can do it at close to hardware speed. An instruction we do not offer raises
  "illegal instruction", as a real CPU without it would. Measured in Node: 8,819 encryption and 1,237
  scalable vector instructions out of 9.13 million.
- **Pointer signing and branch marks.** Measured: 20,362 in Node, all of the kind that older CPUs treat
  as doing nothing. Translate them as nothing.
- **System instructions.** System call goes to the hook. Reading the thread pointer, the floating point
  control registers, the timer and the CPU identity registers is answered directly. The "code changed"
  instruction calls a hook, which is how generated code will be noticed later. Memory barriers do
  nothing in single-thread mode. Breakpoint raises a trap through a hook.
- **Memory.** Unaligned access and byte order already match.

## 9. Testing

The reference is real ARM64 Linux, run in Docker on the Mac (present: Docker 29.9.3, arm64 Linux).
Every failure is replayable from a seed and shrunk automatically to the smallest failing case.

Existing tools do the generating and shrinking; we do not write our own:
- `proptest` 1.12.0 (MIT or Apache-2.0) for layers 1 and 2: random
  cases, shrinking, and saved failing seeds.
- Csmith for random C programs in layer 3, and C-Reduce (2.10.0 in Homebrew) to shrink a failing one.
- What we write is only what no tool provides: the descriptions of what a valid instruction and a
  valid small program look like, the runner that executes a case on the real CPU, and the comparison.

**Layer 1: one instruction at a time.** For every instruction form in Arm's JSON that we tell the guest
it has: random operands, random register and memory contents weighted toward edge values. Run on the
real CPU and through the translator at look-ahead none. Compare every register, flag and touched byte.
This proves the meanings. It does not prove anything that spans instructions.

**Layer 2: random small programs.** Generated ARM64 programs built to always finish: loops with
counters, calls, recursion with a bound, tail calls, calls and jumps through registers, compiler-style
jump tables, flags carried across several instructions, memory use inside a fixed area. The final state
is hashed and printed.

**Layer 3: real compiled programs.** Csmith (2.3.0 is in Homebrew) for random C. Hand-picked programs
for the C library's number printing and sorting, setjmp and longjmp, C++ exceptions and Rust panics.
Built with clang and gcc at every optimisation level, then written out as flat blobs, both for a
fixed address and position-independent.

**Across layers 2 and 3:**
- Every look-ahead must give the same result as none: full, and bounded with random N. Both runs are
  in the same engine, so every register and all of memory can be compared, not only the output. The
  smallest N that fails points at the fault.
- Pieces of different look-aheads are mixed at random within one run.
- Rare events are injected at random points: "the stack is lost here" at a random call (save state,
  start a fresh instance, load state), "this address has no piece yet" for a random target, and "the
  stop flag is raised" at a random check.
- Programs that read or write a page that does not allow it, at random points and with the table
  changed at random points, must reach the "memory fault" hook with the same registers as look-ahead
  none. This includes null pointers and running off the end of a stack.

**Proposed pass bar:** zero mismatches over 10,000 cases per instruction form, one million random
programs and 100,000 Csmith programs in wasmtime, with a sample of each in Chrome.

**Coverage reported:** share of instruction forms exercised, line coverage of the translator, pass rate
per look-ahead.

## 10. Milestones and what is measured at each

Measured throughout: translation speed, output size against input size (raw and compressed), the
engine's compile time, and run time against native and against elfconv.

- **M0, skeleton.** Crates, the hook interface, the register block, the piece convention and the
  lookup. Look-ahead none working for the base integer instructions, enough to run a small C
  program. Layer 1 running against it. Full instruction coverage is M3, and the list of what "full"
  means is Arm's specification, not any sample binary. (For scale: 359 instruction names appear
  across the five samples.)
- **M1, hello world at none.** A C program built as one flat blob prints and exits in wasmtime and
  Chrome. The runner puts the blob in memory and starts at its first instruction.
- **M2, full look-ahead.** Functions, registers in locals, merged compares, tail calls, jump tables,
  the stop check and the permission check, each timed with and without on the benchmark set. Hello world
  and the benchmark set give the same results as none. First size and speed numbers. Run speed is
  goal number one. Output size is recorded but is not a goal: it is never traded for speed, and it
  matters only against the engine limits in section 5 and for how long the engine takes to compile.
  For scale, the toy translator produced about 5 bytes of WebAssembly per byte of ARM64, which would
  make the largest sample (38 MB of code) about 190 MB, well under the 1 GiB module limit.
- **M3, every instruction.** Integer, floating point, vectors, atomics and system instructions. Layer 1
  complete, Csmith passing.
- **M4, bounded look-ahead and the rare paths.** Translating from any address, the bottom loop, "go to
  address", adding code while running, injection tests. setjmp, C++ exception and Rust panic programs
  pass.
- **M5, real code at scale.** Test tooling built on the `object` crate pulls the code and start addresses out of
  busybox, the C library (which tests load-address independence), ripgrep, git and Node from the Alpine
  image, and out of other large runtimes of different kinds: Python, Bun, a Java runtime and a Go
  program. The translator turns each into WebAssembly that loads. Lookup design and module splitting are
  settled against the engine limits. None of these run until the kernel exists; this milestone is
  about size, speed of translation and engine limits. The "optimise for" setting of section 11 is
  built here, in its order: first everything that makes the output smaller at no cost in run speed,
  then the size mode. Recorded for each binary in each mode: output size, the memory each engine uses
  for the compiled code, and what a call between two modules costs.

## 11. Tuning numbers

Internal constants, fixed per build and part of the cache key. They are not settings.

- How many registers are passed as parameters. Measured on small tests only; to be re-measured on real
  programs, including whether to pass floating point argument registers.
- The default N for bounded look-ahead.
- Thresholds at which each warning is raised.

### Optimise for speed or for size

This is the one setting the translator has, and it works like a compiler's. It is given with each
translation, so different blobs in one program can use different values, and it is part of the cache
key.

- **Speed** (the default). Nothing is traded away for size. Run speed stays goal number one.
- **Size.** The translator may give up run speed to make the output smaller. What each such choice
  saves and what it costs is measured and recorded.

Output size matters for two reasons only: the engines' limits (a module may be 1 GiB in Chrome, so a
large program is split, and calls between modules have a cost), and the memory an engine uses for
compiled code. Measured at M2: about 23 bytes of WebAssembly per byte of ARM64, where the toy made 5.

The order of work:
1. Everything that makes the output smaller with no measurable loss of run speed is done in both
   modes. This is measured first, by itself, to show how far size can come down for free.
2. Then the size mode, one trade at a time.
3. Mixing the two within one program (hot code for speed, the rest for size) is an experiment for
   after both are measured. It is not built in phase one.

Both modes give the same results as look-ahead none, and the tests of section 9 run in both.

## 12. Hooks present from day one

Naive or "not supported" bodies are fine; the interface is not optional.

New memory. Duplicate memory (plain copy). Set page permissions. Shared pages: make a block, map it
into a memory, drop it. Start a thread. Pause and wait until a time or an event. Stop requested.
Memory fault. Can this host load new code, and load it. Clock. Randomness. Network:
connect, send, receive, close, look up a name. File backend. Warn. Read all counters.

## 13. Not yet measured

- Calls and jumps through registers on real programs, and interpreter-style dispatch loops.
- The two lookup designs: speed and memory.
- Output size and how small it can be made.
- How long the engine takes to compile a module of tens of megabytes, and to create an instance of it.
- How long it takes to translate and load one small piece while a program is running.
- Programs that switch stacks themselves (Go, Java virtual threads, fibers, coroutine libraries). Each
  switch is expected to take the "go to address" path, which could make such programs slow all the
  time. Untested. The lasting fix is WebAssembly's stack switching feature, which browsers do not ship.
- The stop check and the permission check on real programs, how many permission checks
  whole-function reasoning drops, and what the check when the stack pointer moves costs by itself.
- What refusing an address above a 32 bit address space costs.
- Why catch regions cost so much in wasmtime.
- What 64 bit memory costs on real programs.
- The not-a-number fix-up's cost on Intel and AMD hosts.

## 14. Where the evidence is

The numbers in this document came from a throwaway benchmark that is not in this repository: a toy
translator for four small C programs, timed in wasmtime 50 and Chrome 157 on an Apple M5 Max, plus
instruction counts of five Alpine binaries. Timings were taken with the machine at load 5 to 9, and
identical runs varied by 10 to 15%. Every number is to be re-measured with the real translator.
