# x86-64 native output: plan

Status: plan (XN0, run 8, 2026-11-23), revised after its seven independent checks, and at XN1's start for the
eighth check's findings. What the translator's native output
(docs/native.md section 3, built for ARM64) becomes for x86-64 guests on x86-64 hosts, in the three ways of running
natively: inside a virtual machine (KVM, docs/native.md section 5, "the machine"), each guest process as a host
process ("P"), and the kernel in process with inline memory checks ("I"; both from the owner's ruling of
2026-11-23, docs/native-without-virtual-machine.md, "the P/I plan"). The native output is one; what differs between
the ways is named in section 2 and lives in three places of the emitter (the thread area's segment, the access, the
call into the inside).

Paths: `x86/` is crates/wcontainers-x86/src, `gcpu/` crates/wcontainers-guest-cpu/src, `tr/`
crates/wcontainers-translator/src, `x86 plan` docs/phase-2-guest-cpu-and-x86.md. Measurements are in
pyro-parked/wcontainers-scratch/x86-native-output (probes/ and results/), taken on the rented x86 machine (Xeon
Platinum 8488C, Sapphire Rapids, 96 processors, Linux 7.0.0-1018-aws, 5-level paging, user FSGSBASE, vsyscall
execute-only), load given beside each.

## 1. What is shared with ARM64, and what is new

- Shared unchanged: the native emitter's piece finder and count placement (tr/native.rs `translate_native`,
  `reach`, `counted_ahead`), `counted` and `count_placement` (gcpu/native.rs), `HostCode`, the position map
  (gcpu/position.rs) and `Translation::moved`. x86 instructions have any length and any byte may start one
  (`G::ALIGN` is 1); the finder already takes lengths from `G::decode`.
- New, in wcontainers-x86 behind its `translate` feature: `impl Native for X86_64` (module `native`), its
  carrying code (module `carry`, holding the one `access` function, the one `area` function that gives the thread
  area's segment, and the one `call` function) and its encoder use (iced-x86's `Encoder`, MIT, already the crate's
  decoder; the feature `encoder` is added). The route table is per iced `Code`, like the claim table.
- New, in the x86 crate's core (the part built without the `translate` feature, `no_std` with `alloc`, which the
  machine's floor, built for `x86_64-unknown-none`, can link): `answer`, the plain Rust answers the inside gives for
  the Kernel route (section 3.5), sharing every table and mask with the meanings by construction (the CPUID table
  of x86/cpuid.rs, the rflags masks), so the inside of every way can run them. What `answer` and the `int3` hit
  need that is translate-only today moves into the core first, as a change of shared code (section 9): the
  `Condition` enum with a plain `holds` (today in x86/flags.rs, which emits WebAssembly and is compiled only with
  `translate`), and the x87 unit and the transcendental answers (x86/x87.rs, x86/transcendental.rs, x86/root.rs,
  also translate-only, transcendental.rs using std's `thread_local!`). Measured (run8/nostd, built in the
  checkout's Nix shell on the rented x86 machine with Rust 1.99.0): their dependencies rustc_apfloat 0.2.3 and
  astro-float-num 0.3.7 (default features off, as the crate takes it today) build for `x86_64-unknown-none` only
  once lazy_static, which astro-float-num uses, has its `spin_no_std` feature (without it: "could not compile
  `lazy_static`", E0463, no std); with it they build, adding spin 0.9.9 (MIT).
- Changes to shared code, each its own small commit keeping ARM64's suites passing, listed with code in section 9.

## 2. The ways of running, as the native output sees them

The native output takes a `Way` (section 9) from the executor that translates; it is part of the translation's
cache key. Four things depend on it, and nothing else:
- the thread area's segment: GS in the machine and P, FS in I;
- where the guest's FS base lives: in the hardware in the machine and P (so a guest `fs:` operand and `rdfsbase`
  are copied, and `wrfsbase` where the host's canonical rule is the guest's, section 3.1), in the register area in
  I (every guest access is rewritten there anyway);
- the access form: none (machine, P), wrapping or exact (I; chosen by the in-process executor's
  `TranslatorConfig`, the owner's `Executor::TranslatorConfig` as it stands);
- the call into the inside (section 3.6).
The guest's GS base is in the register area in every way (guest `gs:` operands: 0.00 per 1000 instructions over 13
files, 0.10 in libcrypto, results/shapes86.out). In I, GS holds the guest window's base B (section 4).

The thread area is reached at `seg:[field]`, `field` a field offset of one `#[repr(C)]` type in the x86 crate's
core (no literal; a disp32), the segment base being the area itself, set by the way for each host thread. Machine:
one area per virtual processor, its first level pointed at the running process's by the floor at a process switch.
I: one area per runner, the running process's first level mapped into it (one `mmap` of that process's memfd range,
0.9 µs on the Xeon by the P/I plan's W0 probe, which did not have other runners busy: such an `mmap` changes the
one host process's page tables and so flushes the other runners' TLBs, which XN2 measures with every runner busy)
when the runner switches to a thread of another process; FS is
written with `wrfsbase` at every entry into guest code and the host's value put back at every exit, as the P/I
plan's 3.8 does, and GS with the process's window base (`arch_prctl` where the host lacks user FSGSBASE, a counted
slow path). P: one area per guest thread, set once. Where an area serves many guest threads (machine, I), the
thread's own words (register area, stop word) are copied in and out at each guest thread switch with its other
registers. It holds:
- read-only to guest code: the lookup's first level (section 3.3), the inside's entry addresses and (I) stack, the
  stop word (section 3.7), the flags table's address and its `pext` mask (P, section 3.6), the count stack's
  address, the window limit, the window base and its negation (I), and in P the resume word the embedder writes
  (section 3.6), the vector image's delta from the signal context and the stop signal's set (section
  3.7);
- read and written by guest code (the register area): the guest values of the withheld registers and of the
  guest's GS base (and FS base in I), the guest's FIP (section 3.1) and rcx before a rep string (section 3.6).
  Nothing read from it is used as a host address or decides where host code goes.
What each way must give (recorded for the other tracks in docs/status-x86-native-output.md): the read-only part
really read-only to guest code; no host address translated code or the inside jumps to, returns to or resumes at
may ever sit in memory the guest can write; and an area's writable part reachable only by guest threads of the
process whose thread it holds. In P every writable page of the guest's host process is writable by the guest and
its sibling threads; so is a signal frame on its alternate stack (section 3.7). Translated code reads no data
from the code area (the guest return addresses a call pushes come from the pool, a read-only range outside it,
section 3.3), so a way may map the code area execute-only (P where the host allows it, the P/I plan's 3.4). In the
machine our area is in the upper canonical half, as docs/native.md 3.2 and 7.2 have it, on user-accessible pages
(section 6); the areas sit at one virtual address mapped per virtual processor (each has its own top-level table
entry for it, the rest of its top-level table copied from the running process's), so a guest thread reaches only
the area of the processor it runs on, which holds its own words: with one area per processor mapped at distinct
addresses in a process's tables, a thread of process A on processor 1 could still write processor 2's register
area after processor 2 has switched to a thread of process B.

## 3. Each mechanism of docs/native.md section 3, in x86-64 form

### 3.1 Copy, and what is never copied

Copied as its bytes: an instruction whose host execution does exactly what its meaning does for every input, with
no RIP-relative operand, naming no withheld register, with no operand or effect on the thread area's segment, and
(way I) touching no memory. In practice: integer, SSE, x87 arithmetic and moves, `lock`ed and plain memory forms in
the machine and P, `fs:` operands and `rdfsbase` in the machine and P, `wrfsbase` in the machine (the floor runs the
guest on 4-level tables) and in P on a 4-level host only (on a 5-level host the hardware takes a base the guest's
4-level rule refuses: on this host `wrfsbase` of 2^47, 2^50, 0xff00000000000000 and 0xfff0000000000000 succeeded,
check-xn0b/wrfs.out, where the meaning gives general protection; there it goes to the inside), `pushf` (the
hardware's TF, AC, DF, NT, ID are kept equal to the guest's, section 3.5; AC is always clear, the meaning refusing
it, "alignment checking asked for, not provided"), `pause`, the fences, `movbe`, `adx`, `cmpxchg16b`, `clflush`,
and the AVX setting's VEX forms when the container's setting claims them. The offer is the translator's: a form
outside it is not decoded and takes the Trap route (illegal instruction), whatever the host has (this host
runs AVX-512, AMX, `umwait`, `serialize`, `enqcmd`, `wrpkru`; none is decoded, x86-machine.md). A host lacking an
offered feature, or one the carrying code uses (BMI2 for the lookup's `rorx`, section 3.3; user FSGSBASE in the
machine and P; XSAVEC in I), refuses the native way at the probe ("back end refused by the probe"), and so does a
user shadow stack on P's stub threads (section 3.7); nothing is emulated. Protection keys are relied on nowhere:
nothing in this plan needs them, and `rdpkru`/`wrpkru` are outside the offer.

Never copied, by route:
- Rebase: every RIP-relative operand (24.5 `lea` and 22.0 other uses per 1000 here; 0 `lea` in this python, built
  without position independence). `lea r, [rip+d]` becomes `mov r, imm64` (an address place: the 8-byte immediate,
  its own commit point); another use becomes `mov t, imm64` and the instruction re-encoded on `[t]`. With the
  address-size prefix (`[eip+d]`) the place holds the 32-bit wrapped address, as the meaning computes it. A
  RIP-relative instruction that also names a withheld register takes a stand-in for that one (section 3.4).
- Transfer: every branch, call, return and jump (section 3.3). Never copied because the host address differs and
  because hosts differ: AMD honours a 66-prefixed near branch (16-bit rip, 2-byte pop) where Intel and the meaning
  ignore the prefix; the emitter computes the target as the meaning does.
- Area: the guest's own use of the thread area's segment: `gs:` operands, `rdgsbase`, `wrgsbase` (and in I `fs:`,
  `rdfsbase`, `wrfsbase`). The meaning forms such an address as the segment base plus the operand's offset part
  (base + index*scale + disp) taken at the operand's own address size, so wrapped to 32 bits under the guest's
  0x67 before the base is added (x86/operand.rs `address`). In the machine and P (the hardware holds the
  guest's FS base there) such an instruction runs on FS while FS holds the guest's GS base: `rdfsbase u` (the
  guest's FS base, which the position map then gives as held in u), `mov t, gs:[guest GS base]`, `wrfsbase t`, the
  guest's own bytes with each 0x65 among its prefixes made 0x64 (every other prefix and their order kept), and
  `wrfsbase u`; no flag is written. So the access is the CPU's own with the guest's base: under 0x67 each part of
  an access is wrapped as the CPU wraps it (the ninth check: `fxsave64 gs:[r10d+r9d*4+disp]` at offset
  0xffffffd6 faults at GS + 0x1d5, where an address rebuilt as `[t + u]` faulted 2^32 higher), a bit test's
  register offset is added and wrapped by the CPU, a vector index needs no rebuild, an implicit rsi, rdi or rbx
  (`movs`, `cmps`, `lods`, `xlat`, `maskmovq`, `maskmovdqu`, `vmaskmovdqu` with `gs:`) needs none either, a base of
  rsp or rbp keeps the guest's fault (#GP, an override naming no stack segment), and the CPU picks the segment
  among several prefixes as it does for the guest (measured on the Xeon: cs, ds, es and ss overrides are
  ignored and the last of fs and gs wins, so `2e 65 36` is gs and `65 64` fs; iced decodes the same, so `65 64`
  is not this route). `stos`, `scas` and `ins` reach es:rdi, which no override changes: copied. An instruction
  that names a withheld register takes its stand-ins around the window, re-encoded by iced on `fs:` (its several
  segment prefixes become the one iced decodes, the CPU's own choice); a RIP-relative operand on the area's segment
  is `mov t, imm64` (an address place) inside the window and the access on `fs:[t]`, `fs:[t32]` under 0x67. The
  in-process way (I), where FS is the thread area, keeps the inside for the implicit forms (XN2). A RIP-relative
  operand with the guest's 0x67 off the area's segment is made on `[t32]` likewise, so the CPU wraps a bit offset
  added to it (the ninth check: `bt [eip+disp], rax` with rax 2^35 lands). A stop, fault or signal inside the
  window finds the guest's FS base in u through the position map, and whoever resumes the guest elsewhere puts
  it back on FS (P's handler and the machine's floor: recorded for their tracks). The `rd*base r`
  form is `mov r, seg:[guest base]` (the 32-bit form `mov r32`, as `rdfsbase r32` zero-extends); `wr*base` goes to
  the inside (Kernel), which refuses a non-canonical base with general protection as the meaning does, since its
  canonical check would write flags inline. A copied `wrgsbase` would let the guest choose where the lookup reads
  its first level, and a copied `rdgsbase` or `rdfsbase` (in I) would hand it a host address: the static routing
  test (section 7) fails if any of them is routed Copy in the way concerned.
- Kernel (section 3.5): `syscall` in P and I (in the machine it is Copy: the floor owns LSTAR, section 3.6);
  `cpuid`; `rdtsc`, `rdtscp` (`rdpid` is never offered: Trap); `xgetbv`; segment register reads and writes
  and far transfers (`mov` from and to a segment register, `push` and `pop` of `fs` and `gs`, `lss`, `lfs`, `lgs`, far
  `call`/`jmp`/`retf`, `iretq`; a read gives the meaning's selector, never the host's or the floor's); `popf`; the
  user-mode descriptor reads (`sgdt`, `sidt`, `sldt`, `str`, `smsw`, `lar`, `lsl`, `verr`, `verw`; this host has
  UMIP, under which its kernel emulates all five of `sgdt`, `sidt`, `sldt`, `str` and `smsw` for a 64-bit process,
  measured by the check (check-xn0e/umip.c: each ran, none raised a signal), so copying would give the host's
  answers); the x87 transcendentals; the
  approximate reciprocals `rcpps`, `rcpss`, `rsqrtps`, `rsqrtss` and VEX forms until decision 14 is ruled; and, when
  the offer has XSAVE, `xsave` and `xrstor` (64-bit forms included), whose image depends on the host's XCR0 and
  XINUSE (here XCR0 has AVX-512, AMX and PKRU: the host's XSAVE area is 11,008 bytes against the guest's
  832 with AVX or 576 without (x86/cpuid.rs), so a copied `xsave` with edx:eax all ones would write about thirteen
  times past the guest's buffer; x86-machine.md): the inside writes the image the meaning writes for the offer.
  The route keys on the offer's XSAVE and AVX bits apart, as the offer does: XSAVE and AVX are features told per
  program (`choose_processor_features`, the x86 plan's section 7.2), and `Offer::new` refuses AVX without XSAVE but
  takes XSAVE without AVX (x86/offer.rs `missing_prerequisite`). So: XSAVE offered, the two are Kernel, with the
  x87 and SSE components (576 bytes) and AVX's too (832) when AVX is offered (and `xgetbv` answers the offer's
  XCR0); XSAVE not offered, they and `xgetbv` are outside the offer and take the Trap route. `xsavec` and `xsaveopt` are outside the offer in every container (never told),
  and `xsaves` and `xrstors` are privileged: Trap.
- Float status (docs/native.md 3.4, unchanged): `ldmxcsr`, `stmxcsr` and VEX forms, `fxsave`, `fxrstor`, the x87
  environment saves; the second call also writes MXCSR_MASK in an `fxsave` image as the meaning does. An operand
  on the area's segment is rebuilt between the two calls as the Area route rebuilds it (run 3 found it on the
  hardware's GS, the area itself).
- The x87 pointers (FIP, FDP, FOP) that native.md 3.4 has the second call write "as the meaning writes them": the
  hardware's are never read. They hold host addresses (FIP is in the code area, and in I FDP is B plus the guest's),
  and hosts differ: AMD saves them only with an exception pending, Intel parts with FDP_EXCPTN_ONLY change FDP only
  at an exception. The meaning's rule (x86 plan 5.8, x86/fpu.rs): FIP is the guest address of the last non-control
  x87 instruction; FDP and FOP change only with an unmasked exception. So:
  - FIP lives in the register area. After the last non-control x87 instruction of a straight-line run (and before
    any call into the inside that follows such an instruction in the run), `mov qword
    seg:[x87 ip], imm32` (or `mov t, imm64` and the store for an address at or above 2^31; no flag); a position
    between the run's first such instruction and that store carries the fix-up `X87Pointer` with the guest address
    of the last one before it (section 9), so a stop, a fault or a call into the inside sees the meaning's FIP. The
    value is a guest address written at translation, so a piece dropped since leaves nothing to look up. Cost: one
    store a run that holds x87 arithmetic (expected small; XN1 measures it on `long double` code).
  - FDP and FOP: copied x87 arithmetic raises an unmasked exception only when the guest's control word unmasks one.
    Every way the control word is loaded is watched: `fldenv`, `frstor`, `fxrstor` and `xrstor` are already the
    inside's; a copied `fldcw` is followed, where the flags are dead, by `fnstcw seg:[x87 control]; movzx t, word
    seg:[x87 control]; not t; test t, 0x3f; jnz <call into the inside, kind X87Control>`, which reads back the
    control word the hardware loaded (a word of the register area) rather than the guest's operand; where the flags
    may be live, by that call alone (counted "x87 control check with live flags"); `fninit` leaves every exception
    masked. In the machine and I that word is reachable by no other thread (the processor's area, the area outside
    every window), so the check is exact. In P it is not: the register area is writable by sibling threads (section
    2), so a sibling's store between `fnstcw` and `movzx` can still hide an unmasked control word, and the hardware
    then raises its #MF with FDP and FOP the block does not hold. That changes only that thread's x87 pointers and
    which instruction reports the exception, inside the guest's own process; it is P's named difference (section 5),
    since in P every word a check could read is writable by siblings. A thread whose control word unmasks any
    exception runs pieces in the request's `x87_unmasked` mode (section 9, a cache key bit as `step` is), counted and
    warned "x87 exceptions unmasked". In that mode the x87 register stack and tags stay in the hardware, whose
    control word the inside keeps with every exception masked; the guest's control word, status word, FDP and FOP
    live in the block. Every x87 instruction takes the Kernel route, answered by `answer`'s x87 part: the meaning's
    x87 unit (x86/x87.rs `Unit`, plain Rust over the block's FXSAVE area, moved into the core, section 9) does only
    the arithmetic the x87 hook does, and the rest of each instruction's meaning is WebAssembly in the
    translate-only x86/fpu.rs `emit`: the check for a pending unmasked exception (`pending`), the memory operand's
    address, its split and permission checks, the read, and the store with NOT_STORED and `fstp m80`'s order of
    its two parts. So `answer` gains a plain-Rust twin of that instruction-level part (new code, x86/x87_answer.rs
    in the core, section 9) which reads and writes guest memory through the inside's own access to it, and the
    differential test (section 7) proves it equal to fpu.rs's output form by form, faults across a page included.
    In P it runs in the embedder, since the stub is assembly. The inside moves the register stack from the hardware
    into the block (`fxsave`), runs the twin, which sets FDP, FOP and the pending exception as the meaning does,
    and moves it back (`fxrstor`, the masked control word kept). MMX instructions stay copied, since they raise no x87 exception of
    their own and their meanings are WebAssembly (x86/simd.rs) with no plain-Rust twin; each is preceded by a call of
    kind X87Control, which raises the meaning's #MF when the block holds a pending unmasked exception (as an MMX
    instruction does on the reference) and else returns, so the copied instruction works on the hardware's stack as
    in the normal mode. The mode ends when the inside's x87 unit loads a control word that masks every exception.
    With every exception masked the hardware raises none, so FDP and FOP stay as the block holds them.
  - Every save of x87 state (the Float status route, the kernel's signal frames, the parent's image at `clone`,
    the floor's and I's saves at a thread switch) writes FIP from the register area (or the fix-up) and FDP and FOP
    from the block; in the `x87_unmasked` mode also the control word and the status word (its exception, ES and B
    bits included) from the block, since the hardware's control word is the masked one there. Every load of x87
    state in that mode (a thread switched back in, the frame `rt_sigreturn` loads, a `clone` child's first run, a
    Float status restore) puts the image's control and status words in the block and keeps the hardware's masked,
    and the thread (the child too) runs in the mode while the block's control word unmasks an exception. So an
    unmasked exception survives a switch, a handler that returns and a `clone`: section 7 has the shapes. In P the
    host kernel's save across host switches holds the hardware's masked word, which is what the mode keeps there;
    the guest's is in the block.
- Trap: `int3`, `int1`, `int n`, `ud2`, `hlt`, `in`/`out`, `cli`/`sti`, `swapgs` and every privileged form, and every
  form outside the offer. `int 0x80` and `sysenter` are the meaning's general protection with x86's reason "32-bit
  system call from 64-bit code, not provided" (x86 plan section 5.2). The inside raises the trap `native::trap`
  gives (the meaning's `emit` in plain Rust; native layer 1 checks it equal to the meaning's at every Trap call).
- Nothing: the forms the meaning makes do nothing (`nop` of every length, the prefetch hints, and the whole
  reserved-NOP space, which the decoder reads as `Reservednop` (x86/decode.rs, `FORCE_RESERVED_NOP`): `endbr64`,
  `rdsspq`, `incsspq` and the other shadow-stack forms). Prefetches are never copied, so way I cannot be made to
  touch host memory outside the window with them; `rdsspq` is never copied, since on a host thread with a user
  shadow stack it would hand the guest a host address. The routing test fails if any of these is routed Copy.
- Undefined results (flags after `mul`, shifts, `bsf` and the rest; a `bsf` destination of zero): which behaviour
  the guest sees is the instruction design's (docs/status-instruction-design.md, a precise instruction set per
  container), not this plan's. The native route of such a form is Copy where the host's measured set gives the
  container's behaviour for it, else Kernel (the inside's answer, or the flag computation inline where the finder
  shows the flags dead). Until an x86-64 precise set exists, the meanings are the container's behaviour; this host
  and the i7-8750H agree with them on 15,728 cases of these forms (x86-machine.md, flags-sweep). The route therefore
  takes the host's match (section 9). AMD does not match (XN1 run 10, the same sweep on an EPYC 9R14): 9,447 of
  its 15,644 distinct cases differ, every one in rflags alone (results equal): OF of shifts and rotates by more than
  one, AF of shifts, mul, imul, div and idiv, SF, ZF and PF of mul, imul, div and idiv, and tzcnt's and lzcnt's
  undefined ones; the logic forms, the bit tests, bsf and bsr agree. So on AMD these forms take the Kernel route
  (or the inline flags) wherever the finder shows a flag they leave undefined live, until the instruction design
  names a precise set the host can give; where every such flag is dead, Copy is exact on any host (the route does
  not look at liveness yet: `route` sends every such form to Kernel on a host that differs, and their answers are
  not built, so an AMD host cannot run x86-64 guests natively until they are).
- Partial writes the host model leaves (section 3.6): the same rule, by the probe's match.

### 3.2 Pieces, layout and patching with variable lengths

- Instructions are laid out in address order as on ARM64. An x86 piece may hold instructions that overlap (a direct
  branch into the middle of a held instruction): the finder keys instructions by offset, so the two decodings are
  separate parts, and the first's fall-through goes on by an exit. XN1 has a test of exactly this shape (`jmp` into
  the immediate of a `mov`).
- An instruction may straddle the end of the start's page. The emitter is given the page and up to the longest
  instruction less one (14) bytes of the next page when that page is executable; a piece whose last instruction
  straddles records it, so the write lock of both pages drops it (as the WebAssembly output locks
  `NEXT_PAGE_WRITE`, x86 plan section 5.7); a fetch from a next page that cannot run code is the meaning's fault
  there, which the inside reports as the WebAssembly output does.
- Exits the emitter writes are rel32 (`jmp`, `jcc`, 5 and 6 bytes), never relaxed to rel8, so every exit has one
  length and a link changes only its rel32 (only branches inside one instruction's carrying code, such as the
  replay check's `jrcxz`, are short). An unlinked exit's rel32 points at its own path into the inside, which finds
  the piece and goes on.
- Linking changes code other threads may be running, which x86 allows only with a protocol: on ARM64 a concurrent
  change of one branch is architecturally allowed (docs/native.md 3.2), but the Intel SDM's cross-modifying rule
  (volume 3, 9.1.3) asks every processor that runs modified code to serialize between the change and running it,
  and running bytes while another processor changes them has no defined result; Linux patches live x86 code only
  through `int3` for this reason (arch/x86/kernel/alternative.c: `smp_text_poke_batch_add` and
  `smp_text_poke_batch_finish`, with `smp_text_poke_int3_handler` taking the hits). Links are made by that
  protocol, in batches: (1) the batch's new pieces written, and an `int3` stored over the first byte of every exit
  the batch relinks; sync; (2) those exits' rel32 written; sync; (3) their first bytes written back; sync; then the
  batch's mirror entries stored (ordinary data). A thread reaching such an exit between (1) and (3) takes the
  `int3` (a host SIGTRAP in P and I, vector 3 in the machine); an `int3` in translated code is only ever this, since
  a guest `int3` takes the Trap route, so the handler finds the exit in the position map and does what the exit's
  branch would have done, with nothing of the guest's changed: a `jmp` exit resumes at the exit's unlinked path; a
  `jcc` exit evaluates its condition (the exit's record, `DirectExit::condition`, section 9) on the interrupted
  rflags, and resumes at the unlinked path when it holds and at the host byte after the `jcc` (the next run's
  start) when it does not, as Linux's handler emulates a `jcc` (`int3_emulate_jcc`). The meaning's conditions are
  WebAssembly emitters (x86/flags.rs `condition_from_flags`, `condition_direct`), so the evaluation has two forms
  of its own over the same `Condition` enum: in the machine's floor and I's handler, plain Rust in the x86 core,
  `Condition::holds(self, rflags: u64) -> bool`, with the enum moved there from the translate-only flags.rs
  (section 9); in P's handler, which is assembly with no call, the
  copy's rflags put through the flags table (`pext`, `popfq` of the table word: the word keeps CF, PF, ZF, SF and
  OF, all a condition reads) and then one of a read-only table of 16 entries, entry k being `j<k> 1f; <not taken>;
  1: <taken>`, each of the two choosing the exit's word for that path in the batch's read-only page as the word the
  handler's tail leaves through (section 3.7), so the hardware's own `jcc` decides. The differential test (section 7) runs all three, the meaning's
  through the WebAssembly output, on every condition and every combination of the five flags, and they must agree. Both are host addresses of ours fixed by the map; the rflags only choose between them, so in
  P, where they come from the frame's copy, a sibling's edit changes only which way that guest goes. Resuming every
  hit at the unlinked path sent a thread whose condition was false along the branch: measured by the check
  (check-xn0e/jcc.c, a `jnz` exit relinked 20,000 times, 4 and 16 workers alternating parity), 50,983 and 142,005
  wrong arrivals. Measured with the condition evaluated (run6/jcc16.c, jcc16.out; load 0.13 before; every one of
  the 16 condition codes in turn, each turn loading seeded arithmetic flags and checking its arrival against the
  condition computed apart, 20,000 links each): 16 workers, about 321,000 traps a condition, and 4 workers, about
  92,000, seeds 0x5eed and 0x2b17: 0 wrong arrivals for every condition. The handler takes a SIGTRAP as such a
  hit only when its `si_code` is `SI_KERNEL` and rip less one is the first byte of an exit of the batch being
  made (in P the batch's exits, each with its unlinked path, its condition and the host byte after it, are in a
  page read-only to the stub that the embedder writes before step (1)). In P the handler's choice also reads three
  things a sibling can write, all from the frame on the alternate stack: the copy's rflags, its rip and the
  `si_code`. None can send the thread outside our map: an edited rflags only picks the other of the exit's two
  paths, an edited rip that is the first byte of another exit of the batch picks that exit's paths (both ours,
  that guest's state otherwise unchanged), and any other rip or `si_code` makes the hit no link's, so the thread
  is ended as below.
  In P nothing else can raise SIGTRAP or an alignment SIGBUS, since the hardware's TF and AC are never set there
  (every `popfq` reads the flags table, section 3.6): a `TRAP_TRACE` SIGTRAP or a `BUS_ADRALN` SIGBUS in P is a
  fault of ours, as the P/I plan's 3.6 treats a host signal in the stub it does not name: reported, the thread
  ended, counted "unexpected host trap". In the machine and I a step is the guest's TF, resolved as section 3.6
  says. New code is reached only through a
  link or a mirror entry stored after sync (1), so every processor has serialized after the code was written and
  before it can run it, which is the SDM's rule; code space a dropped piece leaves is reused only through such a
  batch, and only once every saved return point into it has been converted (section 3.7, "Saved return points"). The sync, per way: the machine, an interprocessor interrupt to every virtual processor running a thread of
  the process, each serializing before it answers (docs/native.md 3.2's); I,
  `membarrier(MEMBARRIER_CMD_PRIVATE_EXPEDITED_SYNC_CORE)` in the process (registered once with
  `MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_SYNC_CORE` at the executor's start); P, where the embedder writes the
  code area through its own mapping of the memfd and the guest's threads run in the stub's process: the call
  serializes only the caller's process, so it is a command to the stub, made by a stub thread in a kernel entry
  (waiting for its answer); when none is, the embedder first stops one with the stop signal, as the P/I plan's 4.2
  makes any command on a memory none of whose threads is in a kernel entry. The stub's first thread registers before
  any guest code runs (refused: "links made with threads stopped", below), and the embedder goes to the next step
  only when the stub has answered. Measured in one process, the I case (run5/patch2.c, plink.out, run 4's probe
  with an atomic trap count; load 0.4 before, the probe's own threads after; a writer linking one exit 20,000 times
  between two targets while 0, 4, 16 or 64 threads run it in a loop, each on its own processor, each checking at
  its end that every turn reached a target): one membarrier 0.74, 1.50, 2.11, 5.88 µs; one link by the protocol
  1.97, 5.91, 16.69, 69.99 µs, the `int3` traps included (0, 115,878, 321,673, 1,191,515, every one resumed at the
  unlinked path). Measured across processes, the P case (run5/plink.c, plink.out; the embedder writes through a
  shared memfd mapping, the threads run it read-execute in a forked stub, three futex requests a link): a stub
  thread already waiting, 0, 4, 16, 64 running threads: 14.92, 19.37, 38.83, 79.79 µs a link; no thread waiting, so
  each request first stops a running one, whose handler makes the call: 1, 4, 16, 64 threads: 15.81, 29.84, 83.96,
  231.34 µs; every check passed. A batch costs three syncs whatever its size, so a translation's links go in one
  batch, and the traps arise only for a thread at that exit during the batch. Unlinking, when a piece is dropped,
  is done with the threads that may run it stopped (section 3.7). A sandbox that refuses `membarrier` leaves P and
  I linking only while the process's other threads are stopped: named "links made with threads stopped".
- An address place is the 8-byte immediate of `mov r, imm64` (10 bytes, the immediate at byte 2) or a word of the
  piece's pool (section 3.3): one length wherever the code sits, so the code moves by its places alone; the pool
  sits at a fixed distance from its code and moves with it. The thread area needs no place (its segment base
  is the area). `movable()` is every page of our area (section 6), since no host instruction addresses a guest
  address relatively.
- The entry of a piece reached through the lookup is its first byte, aligned to 16 (padding is outside every span,
  so no stop lands there).

### 3.3 Calls, returns and jumps through a register: the two-level mirror

What any x86 sequence must meet (docs/native.md 3.2): a guest call puts its 64-bit guest return address on the
guest stack; the flags at an unknown target cannot be shown dead, so the lookup writes none; the host's
return-address predictor cannot be used (a host `ret` would read a guest-written address).

The design, built at XN1: three withheld registers, c (the count), t and u (scratch). A guest `ret imm` is
```
mov t, [rsp]                    ; the read; a fault here is before the instruction
rorx u, t, 32
mov u32, u32                    ; u = bits 32..63 of t
mov u, seg:[l1 + u*8]           ; first level: a fault here is before the instruction (t at or above 2^47)
lea rsp, [rsp + 8 + imm]        ; commit
jmp qword ptr [u + t*8]         ; second level: the host entry of the piece at t, or the miss routine
```
The first level is 2^32 words indexed by bits 32..63 of t: a reservation of 32 GiB in each thread area, of which
only the 256 KiB for indices below 2^15 (targets below 2^47) is mapped, one memfd range per guest process shared by
its threads' areas. Index m's word holds the second-level table of guest region m (4 GiB) less m << 35, or, for a
region without code, a 32 GiB reservation without access less m << 35, so the second read faults. The second-level
table of a region is a 2^35-byte reservation whose 4 KiB pages hold eight bytes per guest byte (512 guest bytes a
page), mapped where a target was entered; an entry with no piece holds the host address of the miss routine (the
counterpart of ARM64's `udf`).

So every t either reaches the entry of its own piece, enters the miss routine with t intact, or faults with t
intact, and the host address jumped to is always one of our table's words:
- t at or above 2^47: the first-level read faults, before the instruction has changed anything. The inside answers
  with `answer::near_transfer` (the kind from `native::far_transfer`, the target t), as the meaning does
  (x86/guest.rs `branch_target`, 4-level canonical rule): a target that is not canonical is general protection at
  the transfer, no address, rip on it, rsp not popped, but a call's return address already written below rsp (the
  reference's `call rax` to 2^63 writes it, round 3 run 3, and so does the meaning); a canonical target in the
  upper half (the vsyscall address 0xffffffffff600000 included) is handed on, so the inside completes the pop (or a
  call's push) and reports a page fault at the target with rip = the target, as Linux does (also under
  `vsyscall=none`). A direct `jmp`, `call`, `jcc`, `loop` or `jrcxz` whose target is at or above 2^47 (a rel32
  wrapping below zero, or code near 2^47) calls the inside instead of exiting, with `Question::NearBranch` (the
  condition or count, the offset): taken, the same rule; not taken, the next instruction (a loop's count written).
  Measured on this host (check-xn0/mirror2b.out, refjump): a `ret` to 0xffffffffff700000 or 0xffff800000001000 is
  SIGSEGV SEGV_MAPERR at the target with rip the target and rsp popped; a `ret` to 0x8000000000401000 is SIGSEGV
  SI_KERNEL, address 0, rip on the `ret`, rsp not popped (this host's 5-level paging takes 0xfff0000000401000 as a
  page fault, where the guest's 4-level rule gives general protection; x86-machine.md judges such forms).
- t below 2^47 with no piece: the miss routine (or the second read's fault in a region or page not yet mapped)
  enters the inside with t, after the commit: the inside translates if the guest page at t can run code, else
  reports the meaning's page fault at t.
Measured with exactly 2^15 first-level words mapped (run4/calls.c, calls-resume.out; load 0.05): over 37 targets
(a valid one, each of bits 32..63 set alone, 0xfff0000000401000, 0xffffffffff600000, -1, 0xe000000000401000) this
form reaches no wrong piece and keeps t at every fault; the 21 targets at or above 2^47 fault on the first-level
read and the 15 below it with no piece on the second. (Run 2's probe, probes/mirror3.c and results/mirror3.out,
mapped 2^16 words, so its targets in [2^47, 2^48) faulted on the second level instead; the timings below are its.)
The run-1 form indexed by bits 32..47 alone reached the piece of 0x401000
for four of them (bit 61, 62 or 63 set). Timing, recursive fib(30), 3 runs of 21 alternating rounds, `rdtsc`, one
pinned processor, against plain `call`/`ret`: this form 1.065 to 1.070, the run-1 form 1.112 to 1.117, a `pext`
index over bits 32..47 and 61..63 1.119 to 1.123 (exact too, but slow on AMD before Zen 3), and the run-1 form with
t saved first and a non-wrapping top-bits bound 1.265 to 1.272. Earlier (results/returns.out): a 256-entry cache
compared with `cmp` (flags clobbered, a lower bound for every compare form) 1.17 to 1.39, the same compared without
flags (rcx borrowed, `jrcxz`) 1.52 to 1.62; so the compare forms docs/native.md 3.2 listed are rejected. Not
measured: a return site the indirect predictor cannot learn (an interpreter's), and the memory the second level
takes on busybox and python (XN1 records both).

Calls: `call r/m` is the load into t (its own fault, before), the three instructions of the first level (before, so the inside's answer makes the push and the target's test in the reference's order), `push qword ptr [rip + d]` of the guest return address from
the piece's pool (the commit point; its fault is before), and the second-level `jmp`. The pool is a read-only,
non-executable range at one fixed distance from the code area (the pool page of each code page at the same
distance), holding guest addresses only, so the code area is never read as data and may be execute-only; the
check's probe (check-xn0c/xonly.c) showed a pool inside an execute-only code page faults (SEGV_PKUERR on this
host, whose kernel makes execute-only from a protection key of its own). Measured (run4/calls.S and calls.c,
calls-resume.out; recursive fib(30) whose every guest call is `call r`, 21 rounds, one pinned processor, load
0.05), the median against plain `call`/`ret`: pool in the code page (read-execute) 1.119, pool in a read-only page
outside an execute-only code page 1.120, a stand-in register saved to the area around `mov imm64; push` 1.146, and
`push imm32` then `mov dword ptr [rsp + 4]` for the high half 3.271 (the return's 8-byte load cannot be forwarded
from two stores). A direct `call` is the same `push` and a direct exit (a target at or above 2^47 is known when
translating: Kernel route). A jump
through a register or memory is the load into t, the first level, and the second-level `jmp`. The count's add is
at the start of the run (definition E unchanged).

Costs of the first level: 32 GiB of address space per thread area, nothing more until written. Areas: per virtual
processor (machine), per runner (I), per guest thread (P). In P on a 4-level host our area between the guest's
ceiling 2^44 and the host's own mappings near 2^47 (about 112 TiB) holds about 3,000 thread areas beside the code
area and the second levels: a further guest thread is refused as Linux refuses one past its limit (`clone`
EAGAIN), counted "P thread areas exhausted" and warned; on a 5-level host P places areas above 2^47 (by address
hint) and the limit does not arise. Second levels: 32 GiB of address space per 4 GiB region holding code a lookup
reached; when our area is full, the least recently filled region's table is dropped (its first-level word back to
the reservation without access, so its targets miss again), counted "mirror region dropped". XN1 records the
second level's memory on busybox and python.

### 3.4 Registers kept for the translated code

Every x86 register is live in guest code, so hooks use none of the guest's: every call into the inside saves and
restores everything the inside touches (section 3.6). Withheld: c, t and u, chosen among r8 to r15 (no form names
them implicitly except `syscall`'s r11, which is the Kernel route), by the dynamic count of XN1 on the benchmark set
and the X9 programs. The static count is only a first guess, and it does not agree across files (per 1000
instructions, results/shapes86.out): over all 13 files r10 13.9, r15 15.0, r11 17.1, r9 19.1, r14 21.3, r13 21.8,
r8 28.6, r12 30.8; glibc r11 14.0, r10 17.9, r15 31.5; cc1 r11 4.6, r10 8.0, r15 26.6; but libm r15 2.9, r14 4.4,
r11 5.0, r10 5.3, libz r15 50.7, and the Go program r15 2.9, r11 24.9 (Go keeps r14 for its goroutine). First guess:
r10 (c), r11 (t, so `syscall`'s write of r11 only clobbers a scratch register), r15 (u). Stand-ins follow
docs/native.md 3.3 unchanged: one load into t, or a guest register the instruction does not name saved to the
register area, both through `seg:` moves, which touch no flag. Counted "withheld register named by the guest".
Where an instruction names one withheld register, as a whole operand only, and has no memory operand, that
operand becomes the register's slot of the register area (`seg:[slot]`, the form's r/m operand, or the same
operation's form with the r/m and register operands' roles exchanged, `mov r/m32, r32` for `mov r32, r/m32`, from
iced's opcode tables): one host instruction, no stand-in. Not where the memory form acts otherwise: a 32-bit write
(which zero-extends only in a register), memory of another size than the register (`pextrb r32/m8`), `xchg`
(which locks memory), a bit test with a register offset (which reaches past its operand in memory), or a form
whose results Intel leaves undefined (the probe's match covers the register form only: native layer 1 found
`rol r/m, imm8`'s OF differing between the two forms on the reference). A stand-in is loaded only where the guest reads the register
(a write of fewer than 32 bits keeps the rest, so reads it) and stored back only where it writes it. Measured
(XN1 run 4, the benchmark set at full on the Xeon): matrix from 6.2 to 3.8 times the host's with the slot form and
to 2.8 with the lighter stand-ins, search from 4.1 to 2.0 and 1.4, the geometric mean from 1.64 to 1.41 and 1.27.
No fixed choice of c, t and u meets the bar (XN1 run 4 measured five), so a loop holds the withheld registers it
names in homes of its own: a straight-line part that loops to its own start (the translator's loop copy,
`Native::loop_invariant`) and names withheld registers, with as many general registers it never names (implicitly
either; never rsp or rbp) free, runs its copy with each withheld register renamed to one of them; the moves before
the copy put each home's own guest value in the register area's home words and the withheld value in the home,
and every way out of the copy (the branch back not taken, an exit's call into the inside after its linkable
`jmp`) moves them back first. Inside the copy the position map holds each withheld register in its home and the
home's own value in its word, from each instruction's first byte (the count's add before it included,
`HostCode::relocate_throughout`). An instruction that calls the inside or rebuilds its operand in a register of the
first eight (Kernel, Trap, FloatStatus, Area, a repeated string, `fldcw`, a RIP-relative operand with a high byte)
is written with the homes left around it. Where fewer registers are free than withheld ones are named, no homes:
the loop keeps the slot form and stand-ins. Measured (XN1 run 5): matrix from 2.8 to 1.04 times the host's, the
geometric mean from 1.27 to 1.15.

### 3.5 The Kernel route: who answers

The inside answers with `answer` (section 1): `cpuid` from the same table the meaning emits (both settings),
`xgetbv` with the offer's XCR0, `rdtsc`/`rdtscp` through the clock and the processor answer the meaning
gives, segment reads, writes and far transfers by the x86 plan's rules (a selector of 32-bit code, 0x23, is refused
by name, "far transfer to 32-bit code, not provided"), `wrfsbase`/`wrgsbase` where they are routed, `popf` and
`iretq` taking TF, DF, NT and ID and leaving AC clear, the descriptor reads' designed answers, the transcendentals
through the meaning's astro-float-num function (in the core once moved, section 9), the reciprocals by the meaning's formula, the XSAVE family by the
meaning's image. Pure answers (all but the clock, the system call and the far transfers' kernel part) need no
kernel; whether P answers them in the guest's host process is P's choice. Where an answer is WebAssembly logic in
the meaning and Rust logic here, the differential test (section 7) proves them equal case by case.
`answer(question, block, offer, constants, memory) -> Result<Notes, Fault>` (x86/answer.rs) takes a `Question`,
which the translate half's `native::question` gives for a decoded instruction (the inside finds the instruction by
the position map's guest pc), so the core needs no decoder; a memory operand comes as the shared `Operand`, whose
address the core forms from the block (`answer::address`), and the guest's memory as `answer::Memory` (a read or
write of a range, all or nothing, or the first address it may not reach). On `Ok` the caller moves rip past the
instruction unless `Notes::transferred` says a far transfer already set it; on a `Fault` nothing has changed but
the slots a far `call` wrote before the one that faulted (as on the CPU), and `Fault::Memory(address)` goes to the
guest's `memory_fault` as every other fault does (page, stack or general protection); `Fault::FarTransferTo32Bit`
is general protection counted by its name. `Notes` names what the meaning warns of (`cpuid` asking for AVX while
told no, `popf` or `iret` setting TF, which starts stepping, or AC, which is never held). The memory also checks a
range without reaching it (`Memory::check`, for `xsave`'s order of faults), and the clock is `answer::Clock`, the
inside's nanoseconds. Built: `cpuid`, `xgetbv`, the transcendentals, the reciprocals (by `answer::estimate`, the
meaning's formula in plain Rust, with libm's correctly rounded square root and rounding as WebAssembly's), `popf`,
the descriptor reads (from the table both share, x86/user_segments.rs), the segment register moves, pushes and
pops, `lfs`/`lgs`/`lss` and far `jmp`/`call` through memory (the selector's address wrapping by itself under
0x67, as the meaning's), `retf`, `iret`, `wrfsbase`/`wrgsbase`, `rdtsc`/`rdtscp`, `xsave`/`xrstor` (the frame
slots, the XSAVE constants and `xsave_bytes` shared with the meaning), and the partial-write forms of section 3.6:
`enter` of any level (each push faulting by itself, then the final stack pointer's check, the registers changed
only at the end) and every x87 store of ST(0) to memory (`Question::X87Store`: the unit's store, then the value
written in the meaning's two parts, the status and tag words taken only once memory took it; the hook's value
split is the core's, x87.rs `VALUE_LOW`, which fpu.rs reads). An answered x87 instruction that is not a control
one sets the block's FIP to its own address, as the meaning does: the inside puts the register area's FIP in the
block before an answer and writes it back after, as for the second call. Native layer 1 requires every answered
case's outcome by `answer` equal to the meaning's; forms layer 1 leaves out (`cpuid`, `xgetbv`, whose table is
ours; `rdtsc`, `rdtscp`, the clock; `xsave`, `xrstor`, XCR0 and XSTATE_BV by design) and the operands its random
cases rarely reach have edge tests of their own against the meaning (tests/layer1.rs `the_inside_answers_*`).

The Float status route's second call (section 3.1) is `answer::float_status_after(status, block, tracking,
memory) -> Result<Notes, Fault>`, with `native::float_status` giving the `FloatStatus` of a decoded instruction
(`ldmxcsr`, `stmxcsr`, the FXSAVE image, the x87 environment, each with its operand). The block it is given is the
hardware's state just after the copied instruction but for FIP (the register area's word, or the fix-up), FDP and
FOP, which are the meaning's from before it; its rip is the instruction's own. A save's image gets FIP, FDP, FOP,
the selectors and MXCSR's mask as the meaning writes them (the environment's layout is the core's, which the
meaning's x86/fpu.rs reads too); a load leaves the image's FIP, FDP and FOP in the block as the meaning loads them;
`fnsave` clears them, as the initialisation after its store does; `ldmxcsr` keeps MXCSR's flags from then on; a
read of MXCSR while its flags are not kept is a note. Its caller writes the register area's FIP from the block on
return. A fault is the image's, made unreachable by a sibling since the hardware reached it. Native layer 1 makes
the call at every run that reaches it, and `the_second_call_writes_and_takes_the_x87_pointers_as_the_meaning_x86_64`
runs every save and load form after none, one, two, a memory and a control x87 instruction (one piece for the
stretch), on a register, rip, fs, the area's gs and a 32-bit address, against the meaning, the pointers the block
holds after included. The reference runner runs one instruction, so with an x87 instruction before the save only
the meaning is compared.

### 3.6 System calls, faults, the count

- The call into the inside, one function `call(way, kind)`, writing no flag and touching no guest byte (the red
  zone and any rsp the guest holds, including one that is not a stack, stay untouched):
  - the machine: `int VECTOR + kind`, a DPL 3 gate the floor owns; the ring change switches to the floor's stack by
    the TSS, and the floor returns by `iretq` to the host address the processor saved there, which no guest code can
    reach. A guest `syscall` is copied as `syscall` (the floor owns LSTAR). The hardware's rcx and r11 are host
    values there (the host address after the instruction, and rflags with t holding nothing of the guest's), so the
    floor writes the guest's: rcx with the guest's next address from the span, and the guest's r11, which lives in
    the register area's slot while r11 is withheld (t in the first guess), with the guest rflags as the reference
    does; when it returns, t is a scratch again.
  - P and I: `lea t, [rip]` (the host address of the next instruction) and `jmp qword ptr seg:[entry + 8*kind]`.
    Nothing is pushed: no host address goes to any stack. In I the routine keeps t in the inside's own memory, out
    of every guest's reach, switches to the inside's stack (the area's read-only part names it) before any Rust code
    runs, and resumes by jumping to t, or through the lookup at a guest pc when the kernel changed where the guest
    goes (a signal, `rt_sigreturn`, `execve`) or when the piece holding t was dropped while the thread was in the
    inside (section 3.7, "Saved return points").
  - P never jumps to a t that a stop may have replaced. The stub's routine (assembly; t is r11 and c is r10 in the first guess, and a host `syscall`
    overwrites r11 and takes its fourth argument in r10, so it moves both to registers the host kernel keeps) hands
    t to the embedder in the request, and every way out of the stub is `jmp qword ptr gs:[resume]`, a word of the
    thread area's read-only part that only the embedder writes (through its own mapping of the area's memfd),
    after the request and before its answer. The embedder writes there the host address t names only once its
    position map shows t is the return point of a call into the inside of the kind the request names, in a live
    piece of this process's code (it validates at the request, recording the guest pc and count the map gives at
    t), and else the lookup's entry with the guest pc in the register area: when the kernel changed where the guest
    goes, or when the piece was dropped after the request was validated (section 3.7, "Saved return points"). A t
    in no live piece and in no request it validated earlier gets nothing: the thread is ended as corrupted ("call
    from an unknown host address").
    The miss routine's t is the guest target, a guest value, and its way out is always the lookup's entry.
    A stop in the stub lands while t is in a register, so the host kernel writes t into the frame, which a sibling
    can edit, and the stub's state after a stop comes from the handler's copy (section 3.7): a t from there is
    validated in the same way, so the only addresses a sibling can choose are other call return points of its own
    process, and a resume at one of them is the guest going on at that call's guest instruction with its own
    registers, since at every call position the guest's state is the registers and the register area alone (no
    fix-up pending, nothing relocated: the FIP store of section 3.1 comes before a call, the replay check puts rcx
    back first, and XN1's position-map unit test checks this for every call the emitter writes). Every other word
    the stub uses after a stop (its pages, the answer word, the futex address) is read from the read-only part
    again, never from the copy. A call the stub answers in its own process (a pure answer, section 3.5) may go back
    by `jmp t` from the register it arrived in; one a stop has landed in goes to the embedder all the same, since
    only the embedder can validate t. Cost: the jump reads a word where it
    would read a register, and the embedder writes that word once a call (expected nothing measurable against a
    call into the stub of about a microsecond; measured at XN1).
  - TF and AC across the call: `int n` clears TF itself and the floor restores rflags by `iretq` from its own
    stack. The `jmp` of P and I keeps the hardware's flags, so in I the routine, having saved rflags on the inside's
    stack, clears TF and AC (`popfq` of the saved value with both cleared) before any Rust code, and its last `popfq`
    before the jump back reads the guest's rflags from that stack as saved (or as the inside changed them), TF and
    AC the guest's; a step the
    hardware takes in the routine's first instructions is resumed silently, as for the carrying code (below); the
    inside's stack is out of every guest's reach in I, so that saved word cannot change under it. In P the stub's
    state after a wait is read from pages sibling threads can write (the P/I plan's 3.7), and clearing TF and AC in
    a word of such a page before `popfq` reads it does not keep them clear: a sibling's store lands between the two.
    Measured (check-xn0d/popf.c by the check, and run5/flags.c, flags.out; load 0.3 to 0.5; a sibling on another
    processor storing 0x40302 into the word in a loop, 20,000,000 rounds, 3 runs): `andq` then `popfq` of the word
    set the hardware's TF 6, 6 and 6 times and AC 6, 6 and 6 times (the check's runs: 31 to 1,254). So P puts the
    guest's rflags back only through the flags table (next item): P never runs with the hardware's TF or AC set.
  - The flags table, P's alone: every `popfq` P's carrying code and P's stub run reads a word of a read-only page of 512 words,
    word k holding the flags that `pdep` of k over CF, PF, AF, ZF, SF, DF, OF, NT and ID gives, with IF and bit 1
    set and TF and AC clear. The value to put back is read once into a register (t in the carrying code), `pext`
    with that mask (a read-only word of the area, or an immediate in the stub) gives k, rsp is pointed at word k
    (the table's address a read-only word of the area; the stub's own read-only page, reached rip-relative), and
    `popfq` loads it. Nothing `popfq` reads can be written by a guest, so no store can set TF or AC. The machine and
    I never use it: there the hardware's TF and AC are the guest's (the TF item below), and a table word would clear
    them, so the first rep string instruction a stepping thread ran would end its stepping (measured by the check,
    check-xn0e/tf.c: with TF set, a `popfq` of the guest's word gave 10 steps over the ten `nop`s after it, of the
    table's word 0). Their carrying code's `popfq` reads the word its `pushfq` wrote, in memory no other thread
    reaches: the machine's per-processor area (section 2) and I's area outside every window. The carrying code is
    emitted per way already (`carry` takes the way). Measured (run5/flags.c, flags.out): with the same sibling,
    the table form set TF 0 and AC 0 times in each of 3 runs of 20,000,000; without the sibling, 9.12 to 9.18 ns a
    round against 8.25 to 8.26 for `andq` and `popfq` (about 0.9 ns, 3 rounds in each of 3 runs, one pinned
    processor).
  - What is saved: the inside's routine saves the general registers and rflags (then `cld`); the register groups
    follow the native brief's last section, per way: in the machine the floor is built for `x86_64-unknown-none`
    (no SSE or AVX in its code) and saves groups only at guest thread switches; in P the stub's routines use no
    vector register and the host kernel saves per thread; in I the inside's Rust code runs on a host whose C library
    picks AVX2 or AVX-512 forms of `memcpy` and `memset` that end in `vzeroupper` (which would clear the upper
    halves of the guest's ymm registers), so every call into the inside that runs Rust code saves the guest's
    offered groups with `XSAVEC` and restores them with `XRSTOR` (52 to 78 ns, the feature-switch research), the same
    save the thread switch needs. XN2's hostile-guest test fills ymm upper halves before a call that copies 64
    bytes or more and compares after. A new guest thread's registers of every offered group are loaded, in every
    way, from the image our kernel gives it (the parent's at `clone`, as Linux copies them; the initial state after
    `execve`), by `XRSTOR` with the offered groups as the mask; in P the stub does this before the thread's first
    guest instruction, so nothing its zygote or an earlier thread left in a vector register reaches the guest.
- Faults: the hardware's code is never the guest's answer. A fault in translated code is resolved by the position
  map; for an access its guest address comes from the span's operand, which gains the segment and the address size
  (section 9). So: a non-canonical address is general protection with no address, as the reference, also on this
  5-level host, which reports SEGV_MAPERR at the address for 2^47 to 2^56; a misaligned `movaps` or `cmpxchg16b` is
  general protection (P and I read the trap number from the signal context's `REG_TRAPNO`); division faults are the
  meaning's. A locked access across a cache line is slowed by this host's split-lock detection and would be SIGBUS
  under `split_lock_detect=fatal`: named for the owner (section 10), not handled.
- Partial writes behind a fault. Measured on this host (results/partial.out, every offset across a page that
  refuses writes or reads; load 2.4): none for `mov` of 8 bytes (21 cases), `movdqu` (45), `vmovdqu` of 32 (93),
  `fxsave64` (93), `xsave64` (24), `fnsave` (107), the masked stores `vmaskmovps` xmm and ymm, `vpmaskmovq` ymm and
  `maskmovdqu` with every lane enabled (15, 31, 31, 15); loads changed no register. But three forms do leave part of
  their write: `enter` with a nesting level (each push its own fault: 6, 14, 22 of 8, 16, 24 bytes written), `fstp
  m80` (its first 8 bytes written before the last 2 fault) and `fbstp` (one byte, the checker's run and this one
  agree). The meaning models these as the reference does (x86/fpu.rs `write`, x86/meaning.rs `enter`), so copying
  matches on a host that behaves like this one; decision 14's question does arise for them, as for `rep` strings
  and the AVX2 gathers. Route: Copy where the probe at start finds the host's pattern for each equal to the
  meaning's, else Kernel (the inside writes as the meaning does), counted per site as the shared slow path "host-model
  result routed" (`NativeReason::HostModelResultRouted`), as is every form routed Kernel only because the probe
  found the host different (`native::host_differs`: these, the undefined results, `wrfsbase` in P on a 5-level host). `enter` with a level is rare; `fstp m80` is common in `long double` code, so a host that differs pays.
  Measured on AMD (XN1 run 10, an EPYC 9R14 of a c7a.large, the same three probes): `fstp m80` and `fbstp` write
  nothing behind the fault (0 of 9 each), `enter 0, 3` writes as here (6, 14, 22), every other form none. So an AMD
  host differs from the meaning on two of the three and they take the Kernel route there; `HostMatches` carries one
  `partial_writes` for the three, so `enter` with a level goes with them (rare, and the answer is the meaning's).
  The inside's answers are built (section 3.5); tests/layer1.rs
  `the_inside_answers_enter_and_the_x87_stores_as_the_meaning_x86_64` checks them against the meaning and runs
  the three forms natively for a host whose probe finds them different.
  XN1 runs the probe on an AMD machine too.
- The count: `lea c, [c + k]` at each run's start (no flag). A rep string instruction is an open question between
  two plans: docs/native.md section 4 says E does not count its rounds; the x86 meaning names a jump to itself, so
  `counted` counts every pass, and a pass is one element or a bulk run of at most 1 MiB, chosen by the direction
  flag, the trap flag, overlap and the range's permissions (x86 plan 5.6). Native code can match only by computing
  that choice again after the hardware ran the instruction. Proposed for the owner (section 9): a rep string's
  events are its elements, which both outputs know (rcx before less rcx after), `lea c, [c + t]` plus a flag-free
  subtraction natively, and which make every element boundary a distinct position on both outputs. Until ruled,
  XN1 counts the meaning's passes natively, inline and with no call into the inside: before the copied
  instruction `mov seg:[rep count], rcx` (rcx before, in the register area; no flag); after it, out of line, rsp
  moved to the area's count stack (`mov seg:[saved rsp], rsp; mov rsp, seg:[count stack]`, the stack named by the
  read-only part, so the guest's stack and red zone stay untouched), `pushfq`, the passes computed from rcx before
  and after, DF, TF and the distance between rdi and rsi, added to c by one `lea c, [c + t]`, rflags put back.
  The pass rule is the meaning's, and the meaning's is WebAssembly in the translate-only meaning.rs
  (`repeated_string`, `bulk_or_element`, `repeated_compare`); no plain form of it exists, so this output carries
  a second implementation of it, in two forms of its own: `string_passes` in the x86 core (plain Rust, which the
  inside uses for the `RepPasses` fix-up and the twin) and the emitted after-code, both tested against the
  WebAssembly output's count (section 7, Count). The rule, for an instruction that ran to its end (its ranges'
  permissions need no test, since natively every page was allowed, and a fault inside it is the inside's to
  count): `repe`/`repne` `cmps` and `scas`, `lods`, and any string with the guest's 0x67 (Reason::StringAddress32)
  go one element a pass, so the passes are rcx before less rcx after; `movs` and `stos` with DF or TF set likewise;
  otherwise each pass takes n = min(rcx, 2^20 / element bytes) elements (`BULK_STRING_BYTES`) as one bulk pass,
  except a `movs` whose destination lies 1 to n * bytes - 1 bytes above its source (`rdi - rsi - 1 < n * bytes -
  1`, unsigned, each pass with its own n), which takes one element. rsi and rdi advance together, so the distance
  d = rdi - rsi is the same at every pass and the passes have a closed form: one element a pass while d >= 1 and
  min(rcx, cap) * bytes > d, then bulk passes of min(rcx, cap) until rcx is spent. For a forward `rep movsb`
  with rdi = rsi + 10 and rcx = 100 that is 90 element passes, then one bulk pass of 10: 91. The after-code
  computes that form flag-free where it can and with the flags saved on the count stack where it compares; its
  length is not estimated here (XN1 counts its instructions and measures it, below) (in P the saved word read once into t and put back through the flags table, since sibling
  threads can write the count stack; in the machine and I by `popfq` of the saved word itself, TF and AC the
  guest's), rsp put back. The count stack's word is at a fixed offset of the register page (rsp is set to the same
  address each time), so the position map can name both moved values with what exists: from the store of rsp to its
  return, `Relocated { guest: rsp, held: Held::Page(saved rsp) }`, and from the `pushfq` to the `popfq`, also
  `Relocated { guest: RFLAGS, held: Held::Page(count word) }` (RFLAGS is the x86 crate's number for rflags in the
  map, after the sixteen general registers; section 9). The span from the instruction's commit point to that `lea`
  carries the fix-up `RepPasses` ("the rep string's passes owed"), so a stop or fault there is resolved by the
  inside computing the same passes from rcx before and the registers by `string_passes`, and the span after the
  `lea` none. Cost not estimated: XN1 counts the after-code's instructions, measures it on the benchmark set and on
  glibc's `memcpy` above its `rep movsb` threshold, and the elements form beside it for the owner.
- TF: in the machine and I as docs/native.md 3.4 (routed `popf`/`iretq`, hardware TF equal to the guest's, steps
  resolved through the map); every host instruction of the carrying code steps too, and the handler resumes those
  silently at the host instruction ("single step through host instructions"), which is safe there because the
  frame it resumes from is out of every guest's reach. Not in P: a silent resume in the middle of a sequence takes
  t and u from a frame a sibling thread can edit (section 3.7), and a u it chose would make the second-level `jmp`
  go anywhere. In P the hardware TF stays clear, and a guest thread whose TF is set runs stepping pieces, one guest
  instruction each with every exit to the inside and the inside's call kind Step after it, which raises the
  meaning's debug trap (the request's `step`, section 9; counted "guest single step"); `pushf` in them goes to the
  inside, since the hardware's TF is not the guest's there.

### 3.7 Stops, replay, self-modifying code, signals

- Stops: the machine by an interrupt, P and I by a host signal on the thread (run on an alternate signal stack,
  since the guest's rsp is the guest's). A stop at any host instruction of translated code resolves by the position
  map, exactly as on ARM64 (docs/native.md section 4). The handler first classifies the interrupted rip (and copies
  the frame once where it holds the guest's state): translated code is looked up in the position map; our own
  code (P's stub, I's routines) by the P/I plan's 3.6 cases (the entry, the resume sequence, the wait, and the handler's own tail after its unblock,
  which takes the same exit the interrupted handler had already written); any other value ends the thread as
  corrupted, counted. A stop in translated code resumes at a guest instruction boundary through the lookup, one in
  our own code by restarting that code's step (in P every way out of the stub validated, section 3.6), never at a
  host address read from the frame. In P the rip classified is itself in the copy a sibling can edit, so the
  classification only ever chooses among our own resume paths, each of which is safe for any register values.
  - P never resumes through `rt_sigreturn`: the host kernel reads the frame's rip back from the alternate stack,
    which sibling threads can write. Measured (check-xn0b/psig.c, psig.out; load 0.17): with a sibling thread
    storing one address into the frame's rip slot in a loop, 198,432 of 200,000 signals returned by `rt_sigreturn`
    to that address. P's handlers are installed as the P/I plan's 3.6 has them (`SA_SIGINFO | SA_ONSTACK |
    SA_NODEFER | SA_RESTORER`, `sa_mask` the stop signal, `sa_restorer` a routine of the stub's that is never
    reached, since no handler returns and the stub's filter refuses `rt_sigreturn`). The stub has no C library and
    installs them by the raw `rt_sigaction`, and on x86-64 the host kernel delivers no signal to a handler installed
    without `SA_RESTORER`: it kills the process (the check's bt16.c: raw `rt_sigaction` without it, then a signal,
    killed by signal 11; with it, the handler ran). This plan's own probes installed through glibc's `sigaction`,
    which adds a restorer of its own, so they did not show it. A stub thread blocks every signal but the stop signal and the
    synchronous faults. The handler is the stub's assembly and makes no call and no push, so nothing of ours is
    written to the alternate stack, which siblings can write. It leaves by its own jump instead: it copies the
    frame's general registers (from the context address the host kernel passes in rdx, a register no sibling can
    edit) and the offered groups' part of its vector image (576 bytes, 832 with AVX offered, the standard layout's
    x87, SSE and AVX components) to its own memory, the image read at rdx plus a fixed delta, never through the
    frame's `fpregs` pointer. The host kernel places the image 64-aligned and the context below it at a fixed size
    and alignment, so the delta is one constant a host, learnt from the stub's first frame before any guest code
    runs and kept in the read-only part. Measured (run7/altdelta.c, btseq-altdelta.out; load 2.1, other users on
    the machine): 448 bytes on this host, for an interrupted rsp on the program's stack and for 128 rsp values
    inside the alternate stack (64 KiB below its top, every alignment mod 128), where the image's distance from the
    top ranged over -68,544 to -68,672 bytes. A place fixed below the stack's top, as run 6 had it, is wrong once
    the interrupted rsp, a guest value in P, points into the alternate stack (the check's altrsp.c): the host
    kernel then builds the frame below that rsp. Such an rsp also lets the guest make the host kernel's frame not
    fit: with rsp 512 bytes above the alternate stack's bottom the process was killed by SIGSEGV (measured, same
    probe). That ends only the guest's own process, and only for a guest whose rsp points into memory outside its
    mappings (our area); named in section 5. Its tail then points rsp at the flags table's word for the copy's
    rflags (off the alternate stack), unblocks the stop signal alone with one `rt_sigprocmask(SIG_UNBLOCK)` whose
    set is a word of the stub's read-only page (never the frame's mask, which a sibling can edit to block the stop;
    a stub that blocks it all the same is killed after 10 s by the P/I plan's 3.6), restores the vector state by
    `XRSTOR` from its copy (the host kernel starts a handler with the vector state reset: ymm7 read zero at entry,
    set before the signal) with the offered groups as the mask, never the frame's `xfeatures` (a sibling can write
    those, and the host's 0x207 includes PKRU), `popfq`s the table word, loads the general registers and jumps
    through a read-only word (the resume word of section 3.6, or the lookup's entry with the guest pc from the copy,
    a guest value; never a register value from the copy). A stop that
    lands in the tail, once unblocked, finds rsp off the alternate stack, so the host kernel puts its frame at the
    stack's top again over the old one, already copied, and the new handler takes the same exit from the same copy:
    the stack holds one frame however many stops arrive (two when the first stop's interrupted rsp, a guest value,
    pointed into the stack). Measured (run5/flags.c, flags.out; load 0.3 to 0.5; 128
    KiB alternate stack, as run 4's), a sibling sending 200,000 stops back to back while another ORs TF and AC into
    the copy's rflags, 3 runs: 2 to 4 stops taken in guest code and 53,145 to 62,085 in the tail, the deepest
    handler frame 3 KiB below the stack's top, and the guest loop (which checks its arithmetic flags, DF, NT, ID and
    that TF and AC are clear after every round) kept its flags, r12..r14 and ymm7. Run 4's handler, which unblocked
    by the frame's mask with rsp on the alternate stack, piled a frame per nested stop (the check's popf.c: 58,309
    KiB deep at 200,000; with 128 KiB the process died of SIGSEGV). Run 5's figures came from a handler in C that
    read the image through `fpregs`, ran calls on the alternate stack and jumped through a writable word. Run 6's
    probe (run6/presume.c) was still not the handler specified: it never loaded the copy's r10 and r11 (it used
    them for its own jump), so the sibling's stores into them could not have changed anything. The handler as
    specified was measured in run 7 (run7/presume7.c, presume7.out; load 3.1 to 3.2 from other users of the 96
    processors; all in assembly, no call or push, the image at rdx plus its delta, 832 bytes, every word the tail
    decides by in a read-only page; the copy's pc, a guest value, written to the register area at `gs:8`, all
    sixteen general registers loaded from the copy, r10 and r11 included, and the exit by `jmp` through a read-only
    word to a lookup entry that reads that pc from the register area and goes on, flag-free, through a read-only
    table standing for the mirror): the same flood of 200,000 stops while a sibling ORs TF and AC into the copy's
    rflags and stores host addresses of the handler into the copy's r10 and r11, 3 runs: 8 to 10 stops taken in
    guest code, 54,952 to 57,568 in the tail, the deepest frame 3 KiB below the top, the guest loop's flags,
    r12..r14 and ymm7 kept, no jump to a stored address, exit 0; a round trip, `tgkill` to itself, 3 rounds of
    200,000 in each of 3 runs: 924 to 931 ns against 1,069 to 1,079 ns with an empty handler returning by
    `rt_sigreturn` (the thread pinned to one processor; run 6's lighter handler at load 0.03: 930 to 942 against
    1,078 to 1,097). With PKRU edited to
    deny key 0 and bit 9 set in the frame's `xfeatures` and header (run4/resume.c, a sibling's edit made by the
    handler itself): the offered mask left PKRU at
    0x55555554; the frame's own mask restored the edit and the process died of SIGSEGV. A frame whose vector
    image a sibling made invalid faults that `XRSTOR`, which ends the thread as corrupted, counted. Guest values
    in the copy may have been edited by a sibling, which changes only that guest's registers. So t may be read
    from the copy as a guest value (the target a first-level fault reports, the guest's rcx over the replay
    check), or, after a stop in the stub, as the host address the embedder validates before it writes the resume
    word (section 3.6); never as an address host code jumps to unvalidated; u is never read. c is loaded from the
    copy too: the live count is only in the interrupted r10, so nothing else holds it. A sibling's store there
    changes that thread's count, and so the counts its process's stops and replay log record and where an exact
    turn end falls, never which instructions run (a turn is still ended by the real stop) and nothing outside the
    guest's own process: P's named difference (section 5).
  - Resuming by jump never pops the token the host kernel pushes on a user shadow stack at delivery, so P's
    stub threads must have none: the stub carries no shadow-stack marking and never asks for one, and the probe
    refuses P by name ("user shadow stack on a guest thread") if `arch_prctl(ARCH_SHSTK_STATUS)` reports one on a
    stub thread (this host has user_shstk). I's frame is the inside's memory, out of every guest's reach, so I's
    handler resumes by `rt_sigreturn` with the frame's rip set by the handler to the lookup's entry and t to the
    guest pc, which keeps a shadow stack the embedder runs with consistent.
  - I: the host kernel starts a handler with the interrupted FS base (measured: the value set before the signal),
    which during guest code is the thread area, not the runner's thread-local storage. The handler's first
    instructions, before any Rust code, set the host's FS base from the runner's record, found by the alternate
    stack's address (as the P/I plan's handler finds its pages), and FS is set to the area again before guest code
    resumes, since the frame does not hold the FS base.
- A stop inside a `rep` string instruction: the hardware leaves rip on the instruction with rsi, rdi and rcx
  advanced, which is a guest instruction boundary with the instruction's progress in its registers; the thread
  resumes there in the instruction's twin, a piece holding that one instruction followed by a call into the
  inside, so the instruction ends and the stop is taken at its end ("stop deferred to the end of a string
  instruction"). The twin's start adds nothing to c (the original run's add stands) and it does not repeat `mov
  seg:[rep count], rcx`: the register area keeps rcx as the original piece stored it before the whole instruction,
  and the twin's after-code, the original's, computes the passes of the whole instruction from that word and the
  final rcx, rsi and rdi, so no pass done before the stop is lost or counted twice, and c at the deferred stop
  counts them all. A second stop inside the twin resumes in the same twin. A fault inside the instruction (in the
  original or the twin) is resolved with the fix-up `RepPasses`: the inside adds the passes the meaning completes
  before the faulting element, from the same word and the frame's rcx. This replaces docs/native.md section 4's
  DR0 breakpoint, which P and I cannot set without `perf_event_open` (refused here at `perf_event_paranoid` 4, costs-cloud.json).
- Replay to a logged position (docs/native.md 5.7): to stop a thread at (count N, guest pc P), its stop word in the
  area's read-only part becomes N (every other thread's holds a value no count reaches), and the inside asks for P's
  piece again with a stop point at P (the request's `stop` of section 9, part of the cache key). The new piece is
  loaded into the target thread's process only (its mirror entries and links pointed at it); the process's other
  threads run it too, and the check being against the running thread's own stop word is what makes only the target
  stop, as on ARM64. The stop point starts a run, and the check is placed before that run's count add, so c holds
  the count before P's instruction. The check, flag-free, with rcx borrowed into t, which is free at an
  instruction boundary (`jrcxz` has only an 8-bit displacement, so the stop's call sits right after it, the one
  short branch the emitter writes):
  ```
  mov t, rcx
  mov rcx, seg:[stop]
  not rcx
  lea rcx, [rcx + c + 1]      ; rcx = c - stop word
  jrcxz 1f                    ; equal: the stop
  mov rcx, t
  jmp 2f
  1: mov rcx, t
     <call into the inside, kind Stop>
  2: <the run's count add, then P's instruction> ...
  ```
  The position map holds "rcx in t" over the check. A replay target at a `rep` string instruction with N inside its
  passes: in the replay piece that instruction goes to the inside, which runs it pass by pass with the meaning's
  shared pass function, adding each pass to c and stopping when c equals the stop word. Once stopped, the process
  goes back to the original translation. The check runs only in a piece made for one replay, so its cost does not
  matter. Across outputs as docs/native.md 5.7 says.
- A translated page is write-locked (the machine: its page tables; P and I: the host's protection of that guest page
  in every view of every memory that maps it, shared blocks included, as the P/I plan's 3.4 does). The lock is set
  before the page's bytes are read for translation, so a racing write either lands before the lock (and is in the
  bytes read) or faults. The first write faults to the inside, which stops the memory's other threads (the P/I
  plan's 4.2; the machine by interprocessor interrupt), drops the page's pieces and their mirror entries and links
  (each exit and second-level entry is recorded), unlocks, runs the faulting instruction alone from fresh bytes, lets
  the threads go and goes on; the next translation locks again. Intel guarantees that a store changing the very next
  instruction takes effect: here the store faults first, so the next instruction is translated from the new bytes.
  Writes that do not pass through a guest view: the kernel's own writes (`Cpu::write`, `zero`, and every write
  through the kernel's view of a memfd) check the written range against the translated pages and drop them as above
  (`Cpu::code_changed` is honoured as in the machine). `munmap`, `mprotect` taking execute away, `mremap` and a
  mapping placed over a range drop every piece any of whose bytes is in the range, a piece straddling onto it from
  the page before included.
- Saved return points. A thread in the inside holds its call's return point t: in the machine as the rip of the
  `iretq` frame the floor keeps for it, in P in the embedder's record of its request, in I in the inside's record
  of the thread. It may stay there long (a `futex` wait from a `syscall`) or briefly (a Float status call on its
  runner while another runner drops code). Resuming at t after its piece was dropped would run the old
  translation of bytes that have changed, where Linux runs the new ones (the return from a system call is
  serializing), and once the space is reused it would land in the middle of other code. So every drop above
  converts, before the piece's position map goes and while the memory's other threads are stopped, every saved t
  in the piece into its guest resume: the guest pc and the count the map gives at t. That is the map's ordinary
  resolution, since at every call position the guest's state is its registers and register area alone (section
  3.6; XN1's map test checks it for every call the emitter writes), a call before its instruction (Float
  status's first call, the replay check, a Trap, and the X87Control call before an MMX instruction in the
  `x87_unmasked` mode) resuming at that instruction and one after it (an answer, a step, and the X87Control check
  after a copied `fldcw`) at the next. A kind therefore does not say which: the map records each call position's
  guest pc and count as that position resolves, and XN1's map test checks both X87Control positions. The thread then leaves the inside through the lookup's entry at that pc, never
  at t, and runs translations of the bytes as they are when it leaves. Code space is reused only after the
  conversion (section 3.2), so no saved t can name reused code. How the stop covers a thread in the inside, per
  way: in the machine the floor converts under the lock the drop holds, and a thread leaves the floor only after
  reading its frame under that lock; in P the embedder holds every request and writes the resume word only at the
  answer, and the drop's stop waits for every stub thread's report (each t validated while its piece is live), so
  it converts its own records and answers with the lookup's entry; in I a thread leaves the inside only after
  reading its record past the memory's drop gate (the P/I plan's 4.2 stop waits for threads in the inside to
  reach it, and a stop that lands in the routine's resume sequence restarts that step, its 3.6 case). Counted
  "return point moved by a dropped piece".
- Code reading its own bytes reads guest memory, which holds the guest's bytes (translated code lives elsewhere and
  never reads them after translating). A jump table whose entry lands in the middle of an instruction goes through
  the lookup and translates from that byte.
- A signal handler that edits the saved registers: the frame is built by the kernel from the thread's block (the
  withheld registers' guest values merged from the register area), sigreturn loads it, and the thread goes on at its
  guest rip through the lookup, wherever that is. `call next; pop` and unwinders see guest return addresses, since
  every call pushes the guest address.
- vsyscall and vDSO: no x86-64 vDSO exists today. Our kernel's vDSO (crates/kernel/implementation/linux/vdso.rs)
  is ARM64's one-function image, `__kernel_rt_sigreturn` in ARM64 code words, which its loader (loader.rs) hands
  a program through `AT_SYSINFO_EHDR`, and the P/I plan (its 4.3: "our vDSO has only `__kernel_rt_sigreturn`,
  so `clock_gettime` is a system call") builds on that. For an x86-64 guest the kernel must give either no vDSO (no
  `AT_SYSINFO_EHDR`) or an x86-64 one; which is the kernel track's, asked in the status file. Without one, glibc's `clock_gettime` is a `syscall` (Kernel route in P and I, copied
  in the machine), as the P/I plan says. With one, its code is guest code translated like any other, its `rdtsc`
  the Kernel route, so each clock read is one call into the inside where Linux runs it in the process (in P the
  stub answers it in its process only if the clock's answer is pure there); XN1 records the cost of
  `clock_gettime` in P and the machine either way beside the speed bar. The legacy vsyscall page is not provided
  (x86 plan: as under `vsyscall=none`), so a call or jump to 0xffffffffff600000 is a page fault at that address
  with rip there (section 3.3), as Linux gives under `vsyscall=none`. The host's own
  vDSO, vvar and vsyscall pages (here vvar and vdso at the top of the 47-bit space, vsyscall execute-only) are
  P's: they sit in the guest's host process above the guest's ceiling 2^44, out of the guest's mappings; a guest
  read there succeeds where Linux faults, which P names or prevents (P's plan).
- 32-bit code is never run: the decoder is 64-bit only; far transfers and `iretq` are answered and refuse 0x23 by
  name; `int 0x80` and `sysenter` are the meaning's general protection; a 32-bit ELF is the kernel's refusal; and
  `rt_sigreturn` with the frame's cs edited to 0x23 (which on Linux switches the thread into compatibility mode) is
  refused by our kernel's sigreturn with SIGSEGV, named "32-bit code segment in sigreturn, not provided"; any other
  cs but 0x33 is the SIGSEGV Linux gives when its return to user mode faults (the kernel's code, shared with the
  WebAssembly output: asked of the kernel track, since x86/lib.rs keeps no cs in the block today).

## 4. The three memory-access forms (way I only changes them)

The one place is `carry::access(way, code, written, resume)`: every host instruction that may reach guest memory
is written through it (a copied instruction's bytes, a rebuilt one, a return's read of its target, a call's push,
the load of a jump through memory), implicit ones included; a withheld register's slot form reaches the register
area, not guest memory, and is not one. Guest memory in I is a window at B, the GS base (per guest process), inside a slot of the P/I plan's
layout (2^36 bytes from 2^32; section 6). The guest's ceiling there is 2^32 in the first build (a larger exact
window needs the bound in a register, measured at XN2 if asked).
- None (machine, P): the operand as the guest wrote it (only the area segment's operands rewritten, section 3.1).
- Wrapping: the instruction re-encoded with the address-size prefix and `gs:`, so the hardware forms the address in
  32 bits and adds B: no instruction added. A guest `fs:`/`gs:` operand is `mov t, fs:[guest base]`, `lea t, [t +
  base]` when it has a base, and the access on `gs:[t + index*scale + disp]` in 32-bit form (the sum modulo 2^32
  is the meaning's address modulo 2^32 with or without the guest's 0x67, so this form needs no case for it;
  section 3.1 has the meaning's rule). Measured earlier
  (costs-cloud.json, stack exempt): +0.0% instructions, +1.8 to +4.3% wall on Granite Rapids, -7.8 to +5.0% cycles on
  AMD Zen 4, +2 to +4% on the laptop. Its named difference: an address at or above 2^32 lands at its value modulo
  2^32 inside the guest's own window, where the WebAssembly output faults (it cannot reach anything not the
  guest's).
- Exact: the check decides the fault and the access is the wrapping form's, cut to the window by its addressing
  mode (address-size prefix, `gs:[t32]`), so no access, run or only speculated, reaches outside the window (the
  Speculation item below). Where the finder shows the flags dead within the straight-line run (static: 79% of
  memory operands here, a conservative count), `lea t, [operand]` (64-bit), `cmp t, fs:[limit]`, `jae out`, and the
  access on `gs:[t32]`; where they may be live (21%), a flag-free probe with a stand-in v (u is the lookup's and free
  here): `lea t, [operand]; rorx v, t, 32; mov v32, v32; lea v, [v*8]; not v; mov v, gs:[v + 1 - 2*PAGE]`, which
  reads B - 2 PAGE (a readable page of ours that holds only zeros) when bits 32..63 of t are zero and else an
  address 8 to 2^35 bytes below it, in a reservation without access, then the access on `gs:[t32]`; counted "exact
  check with live flags". The limit is at most 2^32, so a t that passes either is below 2^32 and t32 is t: the cut
  changes nothing a check let through. A guest operand with its own address-size prefix takes `lea t32,
  [operand]`, so t is the meaning's wrapped address; on a segment (`fs:`, `gs:`) the segment base is added after
  the wrap, as the meaning forms it (section 3.1; an access whose parts cross offset 2^32 under 0x67, the ninth
  check's `fxsave64`, is XN2's to answer in this form): `mov t, fs:[guest base]; lea u32, [base32 + index32*scale + disp]; lea t, [t +
  u]`, then the check on t, so a base at or above 2^32 faults as the meaning does (`lea t32` over the whole
  operand would wrap the base too). A fault on either is the guest's fault at t, from the span.
  Measured earlier (stack exempt): X32 +11.0 to +15.2% wall (Granite Rapids), +10.0 to +25.4% cycles (Zen 4), +14
  to +32% on the laptop. Whether the cut (one prefix byte an access, no instruction) changes
  them is not measured; XN2's speed bar measures the cut form itself.
- As built (XN2 runs 1 and 2; the status file's "Departures from the plan (XN2)"): the exact form takes the
  flag-free probe for every access, since a fault after the `cmp` would show the check's flags; it probes the last
  byte too of the forms the CPU makes in parts (`fxsave`, `fxrstor`, `fnsave`, `frstor`, `fnstenv`, `fldenv`),
  whose parts would otherwise wrap past 2^32 into the window; a push and a pop to memory commit at their store, rsp's
  move then `Fixup::Offset`, a pop to a register at rsp's move after its load into t; a call through a register or
  memory probes rsp - 8 before the lookup's first level, so no second stand-in; `pushf` and `enter` go to the inside
  in I.
- Bit offsets: `bt`, `bts`, `btr` and `btc` on memory with a register bit offset reach A = t + (w/8) * (offset SAR
  log2 w), t the operand's address and w the operand size, anywhere within about 2^60 bytes of t (the check's
  btreach.c: `btsq %rcx, (%rax)` with rax a checked t and rcx 0x200000005 set a host byte 1 GiB away). Intel
  accesses the w/8 bytes at A, AMD may access one byte of them; either lies in [A, A + w/8). The meaning computes
  exactly A (x86/meaning.rs `bit_test`: the offset added before the address is wrapped, then the check of w/8 bytes
  at A). With an immediate offset the hardware takes it modulo w, so those forms reach only their operand. So:
  - wrapping: nothing more; under the address-size prefix the hardware wraps t plus the offset's part to 32 bits
    before adding B (the check's btreach.c: with 0x67, base 0xff001000 and a 16 MiB byte offset the fault was at
    0x1000, the wrapped address), as the meaning does;
  - exact: the check and the probe are made on A, not t, computed flag-free in u (free in the exact form): 64-bit,
    `mov u32, 6; sarx u, r, u; lea u, [t + u*8]`; 32-bit, `mov u32, 5; sarx u32, r32, u32; movsxd u, u32; lea u,
    [t + u*4]`; then `cmp u, fs:[limit]; jae out` where the flags are dead, or the flag-free probe on u (with v)
    where they may be live, and the instruction re-encoded with the address-size prefix on `gs:[t32]` with its own
    offset register. Under that prefix the hardware adds the offset's part to t32 and wraps the sum to 32 bits
    (the check's btreach.c), which is A mod 2^32, and A is checked below 2^32, so the hardware reaches B + A even
    when t itself is at or above 2^32 and the offset negative; a fault is the guest's at A. A guest bit test with
    its own address-size prefix wraps A to 32 bits as the meaning does (x86/meaning.rs `bit_test`: `at &=
    u32::MAX` after the offset is added), so its sequence ends in a 32-bit `lea` (`lea u32, [t + u*8]`, `[t +
    u*4]`, `[t + u*2]`): for `67 0f a3 08` (`bt dword [eax], ecx`) with eax 0xfffffff0 and ecx 0x100, A is 0x10,
    not 0x1_0000_0010. The 16-bit form needs its offset sign-extended first and so a second register for the
    count: `movsx u, r16; sar u, 4; lea u, [t + u*2]` where the flags are dead, and with the probe's stand-in v as
    the count where they may be live (`movsx u, r16; mov v32, 4; sarx u, u, v`, then the `lea`). Measured
    (run7/btseq.c, btseq-altdelta.out; load 2.1; 2,000,000 seeded cases each with seeds 0x5eed and 0x2b17, half
    64-bit and half 32-bit, offsets small, negative, with bit 62 set or any): the sequence gave the meaning's A
    every time and changed no flag. The 16-bit sequences, measured by the check (check-xn0g/bt16.c, bt16.out;
    10,000,000 cases, seed 0xdecaf): 0 wrong A in the dead and the live form, the live form changed no flag, and
    `lock btsw` landed on the bit at B + A every time. The cut access, measured in run 8 (run8/btcut.c,
    btcut-specx3.out; load 0.1 to 0.3; 2,000,000 seeded cases each with seeds 0x5eed and 0x2b17, a quarter 16-bit,
    a quarter 32-bit, half 64-bit, A anywhere in a 1 MiB buffer at B and t = A less the offset's part, at or above
    2^32 in 625,255 and 625,142 of them): `lock bts` on `gs:[t32]` with the offset register set exactly the bit at
    B + A plus (offset mod w) / 8 every time; the 32-bit `lea` gave the meaning's wrapped A every time, flag-free
    where meant; and the check's case above reached bit 0 of B + 0x10.
- Guards: below B, one page without access, the probe's readable page under it, and 2^35 bytes without access under
  that; above B + 2^32, one page without access. Every access but the strings starts inside the window (in both
  forms by its 32-bit address; in the exact form the check, made on the address the access reaches, A for the bit
  tests, first decides whether the guest faults) and reaches
  at most its own size past that start, and no access is larger than a page (the largest, the guest's XSAVE image
  of 832 bytes, is the inside's), so one page catches a run over the top. The routing test (section 7) fails if any
  form with a memory operand whose reach is not its operand's address alone (the bit tests above, strings, `xlat`,
  `enter`, the gathers, `maskmovq`, `maskmovdqu`, `vmaskmovdqu`) takes the plain exact sequence, if any string
  with the guest's own address-size prefix is routed other than Kernel (below), and if any guest access either
  form emits is on an address not cut to 32 bits (a `gs:` operand without the address-size prefix, or a string
  without its `mov esi, esi` or `mov edi, edi`; Speculation, below).
- Implicit accesses: `push`, `pop`, `call`, `ret`, `enter`, `leave` become explicit accesses through `access` on rsp
  (`mov gs:[esp - 8], src` then `lea rsp, [rsp - 8]`, the store the commit point and the `lea` its fix-up, section 9).
  The source must be a register: `push m` and `pop m` go through t; a `call` pushes its return address from a
  register too (the machine's and P's `push qword ptr [rip + d]` has no store form), and in `call r/m` t holds the
  target and u the first level, so the return address takes a stand-in (one guest register saved to the register
  area and put back, docs/native.md 3.3), and in the exact form the check of rsp - 8 takes one more (two stores and
  two loads a `call r/m` more than the machine's sequence); `pushf` cannot be re-encoded, so it reads the flags
  on the area's count stack and pushes them as a register: `mov fs:[saved rsp], rsp; mov rsp, fs:[count stack];
  pushfq; pop t; mov rsp, fs:[saved rsp]` (the map holding rsp in its page word meanwhile, as the rep string count
  does, section 3.6; in I the count stack is outside every window), then the push of t through `access` like any
  other (the 16-bit `pushf` stores t's low half). Every implicit access is so on a 32-bit address (`gs:[esp - 8]`).
  `xlat` is re-encoded with the address-size prefix on `gs:[ebx + al]`, in the exact form after the check of `movzx
  u, al; lea u, [rbx + u]`, or with the guest's 0x67 (`67 d7`) `lea u32, [rbx + u]`, wrapping as the meaning does
  (wrapping: its named difference, the address modulo 2^32). The AVX2 gathers in the exact form go to the inside (rare; their lanes cannot be
  bounded by one probe), counted.
- Strings, `maskmovq` and `maskmovdqu`: a string instruction with the guest's own address-size prefix (0x67) goes to
  the inside in both forms (Kernel, counted "string with address-size prefix answered by the inside"). Under that
  prefix the hardware forms the rdi side as edi plus the low half of the rebase, wrapped to 32 bits, on ES's base
  0, so no rebase of rdi can reach B + edi: measured by the check (check-xn0h/str67.c, the sequence below with
  guest edi 0x100000), `67 aa` stored at host 0x102000 and left the guest's rdi 0xfffffff700100001 (the meaning:
  0x100001), and `67 f3 aa` with rcx 16 stored its 16 bytes there. The meaning runs these one element a pass
  already (x86/meaning.rs `repeated_string`, Reason::StringAddress32), so the inside's answer is the same slow
  path. Every other string: its start is bounded first, then rdi is rebased. For each of rdi and rsi the instruction
  uses, the flag-free probe above on that register (wrapping and exact alike: a start at or above 2^32 faults on the
  probe, and the inside performs the instruction as the meaning does in that form: modulo 2^32, or the guest's
  fault; counted "string start outside the window"). Then each register so probed is cut to 32 bits, `mov esi, esi`
  and `mov edi, edi` (no flag; a start that passed the probe is below 2^32, so this changes nothing a probe let
  through, but a probe that faults does not stop the instructions after it running speculatively, and the cut keeps
  those inside the window), then `mov t, fs:[base]; lea rdi, [rdi + t]`, the copied
  instruction with a `gs:` override on its rsi side (the rdi side is ES, which cannot be overridden), and `mov t,
  fs:[negated base]; lea rdi, [rdi + t]`; the position map holds "rdi less B" while rebased (`Held::Rebased`). A run
  from a start inside the window moves one element at a time, so upward it faults on the guard page above B + 2^32
  and downward on the page below B; that fault goes to the inside, which in the wrapping form goes on modulo 2^32 as
  the meaning does and in the exact form is the guest's fault there. `maskmovq` (8 bytes at rdi, x86/simd.rs
  `masked_store_mmx`) and `maskmovdqu` and `vmaskmovdqu` (16 bytes at rdi) store at DS:[rdi], which a segment
  override can change, so each is re-encoded on `gs:` with the address-size prefix (so on edi; the guest's own
  0x67 changes nothing there) in both forms, after the probe in the exact one. Measured by the
  check (check-xn0/partial.out): with the run-1 sequence and no bound, a guest rdi of 2^40 wrote host bytes at B +
  rdi; section 7's way I tests hold this case.
- Speculation. A check is a prediction until it resolves: after `cmp; jae` a mispredicted branch, and after the
  faulting probe the fault itself, which retires only after the access that follows has run, let that access run
  speculatively with the refused address (a bounds-check bypass, Spectre variant 1). So no form trusts its check
  for this: every access either form emits is on a 32-bit address under `gs:` (the address-size prefix, `gs:[esp]`
  for the stack, `mov esi, esi` and `mov edi, edi` before a string), so even speculatively it reaches only B plus a
  32-bit value plus its own size: the window, the guard page above, and (a string run downward) the guard page, the
  probe's page of zeros and the reservation without access below. The cut is data, not a prediction, so it needs no
  barrier, as the P/I plan's 3.2 cuts ARM64's accesses with `uxtw`. What a speculative access past the window's top
  can still reach is our slot's reservation without access (a string runs at most a reorder buffer's worth of
  elements speculatively); nothing of ours or another guest's is mapped there. The lookup's reads index our own
  tables by construction (section 3.3). Measured (run8/specx3.c, built on the check's specx.c; btcut-specx3.out;
  load 0.3 to 0.8; processor 5; a secret byte at B + 2^32 + 0x1000, each gadget's speculative byte or bit indexing
  an in-window probe array that is then timed, 100,000 rounds each with seeds 0x1234567 and 0xbeef01): the uncut
  forms read the secret: the live-flags probe then `gs:[t]` 99,999 times in each run, the probe then `lods
  gs:[rsi]` 99,997 and 99,997, the dead-flags `cmp; jae` then `gs:[t]` 4 and 7, and a dead-flags bit test checked
  on A then `bt r, gs:[t]` read the secret's bit 7 and 5 times; the cut forms read it 0 times in every case (`gs:[t32]` after
  either check, `bt r, gs:[t32]`, the probe then `mov esi, esi` and `lods`), reading the window's own byte at B +
  0x1000 instead (99,999 and 99,996 times after the probe). The check's figures for the uncut live-flags probe were
  2,000 of 2,000 and 99,943 and 99,994 of 100,000. What the cut does not cover is named in the P/I plan's
  security section (branch target injection, shared caches between guests on one core).
- The stack's cost is not measured: the earlier figures exempted the stack, and this form rewrites every implicit
  stack access. XN2 measures it, and the alternative the P/I plan uses on ARM64 (rsp holding B plus the guest's rsp,
  so `push`, `pop`, `call` and `ret` run unchanged inside the window, a read of rsp as a value less B, and a write of
  rsp from another register rebased and checked), and keeps the cheaper that passes the way I tests. Measured in
  XN2 run 3 (the status file has the figures): with every stack access cut, the wrapping form costs 1.115 times the
  unchecked form over the benchmark set (fib 1.355); with the stack's accesses as the guest wrote them and rsp
  holding B plus the guest's rsp, 1.032 (fib 1.000). The second form is chosen; a guest rsp written outside the
  window is cut by the wrapping form and faults at the write in the exact form, both named in section 5 once built.
- The page permissions inside the window are the host's protection of the window's pages, written by the kernel's
  mapping calls, 4 KiB as the guest's.

## 5. What the guest sees: differences named

- Way I, wrapping: the modulo landing above (section 4).
- The machine: our area in the upper canonical half, on user-accessible pages (section 6): guest code can read the
  code area, the pool, the mirror's levels and its processor's thread area there, and write that area's register
  part (which holds only the running thread's own words), where Linux faults on every upper-half address. This is
  docs/native.md 7.2's named difference, accepted as decision 4: the pages are in the half Linux never gives a
  user program, and the guest's own half up to 2^47 is as Linux's.
- P: the host pages in the guest's host process (section 3.7), the pool and the read-only thread area: a guest read
  there succeeds where Linux faults, unless P keeps the guest away from them (P's plan decides and counts). A guest store
  into a sibling thread's register area changes that thread's values of the withheld registers, where Linux lets
  no thread write another's registers (it changes nothing outside the guest's own process). Two more of that kind,
  each inside the guest's own process: a sibling's store into a thread's x87 control word slot between the check's
  `fnstcw` and its read can leave an unmasked exception unwatched, so its #MF carries FDP and FOP the meaning would
  not give (section 3.1); and a guest rsp pointing into its thread's alternate signal stack, which is in our area
  outside every guest mapping, puts the next stop's frame below it, and near the stack's bottom makes the frame not
  fit, which kills the process by SIGSEGV (measured, section 3.7) where Linux would go on; and a sibling's store
  into a stopped thread's frame changes the count c that thread resumes with (section 3.7), so that process's
  recorded counts, its replay and its exact turn ends, never its instructions. P on a 4-level
  host: at most about 3,000 guest threads per process (section 3.3).
- P and I on a host with split-lock detection: a split locked access is slowed (or fatal) where the reference runs
  it.
- Everything else is the WebAssembly output's behaviour; the tests of section 7 hold it.

## 6. Ceilings and layout

For P and I, the P/I plan's layout (its 3.1), with x86's parts: the guest area [LOWEST_ADDRESS, 2^44) in P; our
area from 2^44 to the host's own mappings near 2^47 (thread areas, code area and its pool, the mirror's second
levels), and above 2^47 on a 5-level host where P and I place by hint; in I the slots of 2^36 bytes from 2^32, each
holding one guest process's window (2^32), its guards (2^35 and two pages below, one page above) and nothing else.
No slot table: x86 has the mirror (section 3.3). For the machine, docs/native.md's: the guest's half below 2^47 and
our area (code area and pool, thread areas, the mirror's levels) in the upper canonical half beside the floor, on
user-accessible pages (writable only for the register part of the thread area) so that guest-mode code reaches it;
the floor's own pages there are supervisor-only, and the floor reaches ours with SMAP's `stac` or through a
supervisor alias.
- The machine: 2^47, as docs/native.md.
- P: 2^44.
- I: 2^32 in the first build.
Each is `Cpu::address_space` of that way; a request above it fails, counted, as the brief allows.

## 7. Tests

- Routing, static, per way: every decoded x86 form through the recording emitter, with this plan's classes (section
  3.1; docs/native.md 3.4's class "held by the hardware as presented" does not hold the thread area's segment or its
  base instructions here): a form added to the x86 crate is routed or the test fails, and the area segment's
  operands, `rdgsbase`, `wrgsbase` (and in I `rdfsbase`, `wrfsbase`, `fs:`, and in P on a 5-level host
  `wrfsbase`) routed Copy fail it.
- Routing, differential: every decoded form run natively as routed and through the WebAssembly output from the same
  seeded state (all 64 bits of every register, every register collision, each access placed across a page with
  either page refusing, after seeded float flags with tracking on and off, TF and AC in the values popf takes),
  comparing every register, flag, control, memory byte and trap. Seed and cases per form are arguments. Beside it,
  the `int3` hit's three condition forms (section 3.2: `Condition::holds`, P's flags-table and `jcc`-table
  assembly, and the meaning's emitted condition through the WebAssembly output) on all 16 conditions and all 32
  combinations of CF, PF, ZF, SF and OF, each with seeded values of the other flags: all three must agree.
- Rep string passes: `string_passes` and the emitted after-code against the WebAssembly output's count, seeded
  over every string kind, element size, prefix (rep, repe, repne, 0x67), DF, TF, rcx (0, 1, the cap and either
  side of it) and rdi - rsi (0, 1 to the length, negative, above the cap), with the check's case (`rep movsb`,
  rdi = rsi + 10, rcx 100: 91 passes); seed and cases arguments.
- Count: the same count at every stop across look-ahead none, bounded and full on the benchmark set and layer 2's
  programs, the kernel test's hash with a stop at every counted event; the every-offset test with a stop at every
  host instruction of every piece, the lookup and the twins.
- Replay: a logged (count, pc) from the WebAssembly output reached natively and the reverse, on the benchmark set,
  at pcs in the middle of pieces, at a return site, and inside `rep movsb` passes.
- The shapes this plan was broken with, kept as tests: a direct branch into the middle of an instruction in the
  same piece; a jump table into the middle of an instruction; code reading its own bytes and code storing into the
  next instruction, from the same and from a sibling thread; a write to a code page through a shared mapping from
  another process and through the kernel; a thread waiting in a `futex` from a `syscall` in a piece on a writable
  and executable page while a sibling rewrites that page (and, in further cases, unmaps it, takes execute away
  with `mprotect`, and maps new code over it and runs enough to reuse the dropped space), then wakes it: the woken
  thread runs the new bytes with its count exact, and the same with the thread in a Float status call on another
  runner (I) and stopped in the stub's entry (P); a signal handler editing rip, r10, r11, r15 and rflags; a jump
  and a call to 0xffffffffff600000 and, once the kernel gives an x86-64 vDSO, to it; a far transfer to 0x23, `int 0x80`, `sysenter`, `rt_sigreturn` with cs
  0x23; `ret`, `jmp r` and `call r` to 2^47, 2^48, each of bits 48..63 set alone over a valid target,
  0xfff0000000401000, 2^63 and -1, with signal, si_code, address, rip and rsp compared; a stop inside `rep movsb`,
  `rep stosq` and `repe cmpsb`; an instruction straddling into a page that cannot run code; a 66-prefixed `jmp`,
  `call` and `ret`; `xsave` with edx:eax all ones beside a guest buffer of the offered size; `wrfsbase`,
  `wrgsbase`, `rdgsbase` and `mov r, gs` (each way); `enter 0, 3` and `fstp m80` across a page that refuses writes;
  `rdsspq` and `incsspq` (the reserved-NOP results); `rep movsb`, `xlat`, `maskmovq` and `maskmovdqu` with a `gs:`
  override; a `gs:` operand with the guest's 0x67 and a GS base at or above 2^32 (`65 67 8b 00`, GS base
  0x300000000, rax 0x100000010, reading at 0x300000010), and with a RIP-relative operand; `popf` setting
  TF followed by a `ret` and a `jmp r` (each way, P on stepping pieces); a thread with TF set running `rep movsb`
  and then ten one-byte instructions, each step reported (each way: in the machine and I the hardware TF must
  survive the rep string count's `popfq`, as check-xn0e/tf.c showed a table word would not).
- Hostile guest, per way: in P a sibling thread storing into the running thread's alternate signal stack and
  register area in a loop while the thread takes stops, faults and calls into the inside (as check-xn0b/psig.c),
  no host instruction run but translated code, the stub and the lookup's targets; in the machine a thread of one
  process storing into every address of our area while a thread of another process runs on another processor,
  the other's registers unchanged; in I a stop taken in guest code with the handler's Rust code reading its
  thread-local storage. In P also a sibling setting TF, AC and bit 9 (PKRU) and AMX bits of `xfeatures` in the
  frame, the register area and the count stack, editing the frame's `fpregs` pointer and signal mask, and flooding
  the thread with stops while it is in the handler's tail and the resume sequence (as run6/presume.c): the hardware's
  TF and AC stay clear, PKRU and the groups outside the offer unchanged, the stop signal never blocked once a
  handler has left, and the alternate stack never holds more than one frame. In P also a sibling storing into the
  copy's t, while the thread is stopped in each step of the stub (its entry, its wait, its resume), a host address
  of the stub, of the handler, of guest memory, of a piece's middle and of another call's return point: the thread
  goes on only at a call return point the embedder validated, or is ended as corrupted, and never at the others.
- x87 pointers, per way: `fnstenv`, `fnsave`, `fxsave` and `xsave` after x87 arithmetic in the same piece, in an
  earlier piece, after a stop between the two, and after the earlier piece was dropped; FIP, FDP and FOP compared
  with the WebAssembly output's; `fldcw` unmasking division by zero, then `fdiv` by zero, `fwait` and the save, with
  the signal and the saved pointers compared; the same with an MMX instruction in place of `fwait` (its #MF), and
  MMX arithmetic on values the x87 stack left, in that mode, compared register by register; and the same with the
  flags live after the `fldcw`. The unmasked mode across a save and a load: `fldcw` unmasking division by zero,
  then a forced switch (two threads on one runner or processor, `sched_yield`), a SIGUSR1 to itself whose
  handler returns, or a `clone`, then `fdiv` by zero and `fwait`: SIGFPE with FPE_FLTDIV in the thread and in the
  clone's child, as the reference gives; the saved control word in each image unmasked. The position map at
  both X87Control positions: a drop converting a saved return point at each resumes before the MMX instruction
  and after the `fldcw`.
- Links, per way: threads running an exit in a loop while it is linked and unlinked by the protocol (as
  run4/patch.c), every turn reaching one of its targets; a `jcc` exit of each of the 16 conditions so linked while
  the threads run it with seeded flags, every arrival as the condition says (as run6/jcc16.c); a new piece reached
  from another thread in the same batch.
- Register groups, per way (the native brief's last section): guest threads each filling every offered group (x87,
  SSE, AVX with the setting, MXCSR, the x87 control and status) with values of their own, interleaved on one
  processor (machine), one runner (I) or a few host processors (P), and in a test build an assertion at every
  switch and every return from the inside that the registers hold the incoming thread's values, or the initial
  state for a new thread after `execve` and the parent's after `clone`; and in P a new thread's vector registers
  checked against what its zygote left in them.
- Way I: no access outside the window succeeds in either form, through every operand shape: one byte past a
  mapping, 2^32 - 1 with 8 bytes, a SIB with a large index, `fs:` with a negative offset, a downward and an upward
  string with a large rcx, a string, `maskmovq` and `maskmovdqu` with rdi and rsi at 2^32, 2^32 + 2^35, 2^40 and
  -1, `stos`, `movs`, `cmps` and `scas` with the guest's 0x67, plain and `rep`, with edi inside the window and the
  upper half of rdi set (the check's str67.c: edi 0x100000, `67 aa` writes the window's byte B + 0x100000 and
  leaves rdi 0x100001, nothing at host 0x102000), `fs:` and `gs:` operands with the guest's 0x67 and a base at or
  above 2^32 in the exact form (the meaning's fault), `67 d7` with rbx's upper half set (the wrapped address), `bt`,
  `bts`, `btr` and `btc` (plain and `lock`ed) of each operand size with a register bit offset of 0, -1, w, -w - 1,
  2^31 - 1, -2^31, 2^35, 2^60 and -2^63 (each taken at the operand size) from operands at the window's first and last bytes and in its middle, with
  the flags dead and live after them (a seeded sweep beside these, seed and cases arguments), the same bit tests
  with the guest's address-size prefix where t plus the offset's part carries past 2^32 (`67 0f a3 08` with eax
  0xfffffff0 and ecx 0x100 reaches 0x10, as the reference), a push at
  the bottom of the stack, `enter` with a deep level, `call r/m` and `pushf` with rsp at 2^32 and at the window's
  first byte; and the ymm upper halves kept across a call into the inside. Speculation, in both forms: the
  bounds-check-bypass test of the P/I plan's W2 made with this output's sequences (as run8/specx3.c): a guest
  trains each form's check with in-window addresses, then gives one outside the window (another slot's byte, our
  area's, a byte of the embedder), with its access's value indexing its own probe array, which it times; for the
  plain access with dead and live flags, a bit test, a string from rsi and from rdi, `maskmovdqu`, `xlat`, `pop`
  and `ret`; seed and rounds arguments. It reads nothing outside the window under I, and the same gadget built as
  a host program without the cut reads the byte, which shows the test can see a leak.

## 8. Milestones and pass bars

- XN1, the output with no memory check (machine and P). Built: `Native` for x86-64, the routes, the encoder, the
  stand-ins, the mirror, the count, the replay check, the twins, `answer`, the changes of section 9. Runs
  translations through the first native x86 executor that exists (P's, the P/I plan's W1, or the KVM floor); until
  one can run a translation, the static routing test, encoding round trips through iced's decoder and the position
  map's unit tests run, and the rest waits on it (coordinated in the status file). Bar, on the rented x86 machine
  against the real x86-64 reference: the translator's layer 1 (every form, 1,000 cases a form or more, seed
  recorded; the 5-level forms judged as x86-machine.md says), layer 2 (10,000 programs, seed recorded), layer 3, the
  count test, the replay test and the every-offset test, the shapes of section 7, all with no failure; the X9
  binaries natively with the verdicts they have under WebAssembly; and speed: the benchmark set's geometric mean
  under P, against the same programs run plainly on the host (cycles from `perf_event_open` where allowed, else
  wall time, best of 5, machine and load recorded), at most 1.25 (ARM64's N4 measured +21.2%), with no program above
  1.6. Section 7's link, register-group and hostile-guest tests for the way XN1 runs on, with no failure. The
  dynamic register count, the mirror's memory on busybox and python, the rep string count's cost and the links'
  batches and traps on the benchmark set are recorded.
- XN2, the two checked forms (way I). Bar: section 7's way I tests pass for both forms, the speculation test among
  them (nothing outside the window read, the uncut control reading the byte), and its link,
  register-group and hostile-guest tests in I; faults carry exact
  registers; the stack form is chosen by measurement (section 4); and each form's geometric mean over the benchmark
  set against the unchecked form at most 1.10 (wrapping) and 1.35 (exact), with every workload's figure and the X9
  programs' in the status file.

## 9. Changes to shared code (as code, for the owner and the translator's owners)

In gcpu/native.rs, the trait gains the way and what x86 needs; ARM64's implementation takes `Way` and ignores what
does not apply to it:
```rust
pub trait Native: Guest {
    /// How translated code reaches guest memory, the thread area and the inside; part of the cache key.
    type Way: Copy + Eq + core::hash::Hash + Serialize;
    /// Forms whose host execution gives the container's chosen behaviour, found by the probe (paging depth included).
    type HostMatches;
    fn route(way: &Self::Way, i: &Self::Instruction, host: &Self::HostMatches) -> Route;
    /// As today, with the way and the rest of the straight-line part after it (flag liveness).
    fn carry(way: &Self::Way, before: &[(u64, Self::Instruction)], i: &Self::Instruction, bytes: &[u8],
        address: u64, after: &[(u64, Self::Instruction)], invariant: &Self::LoopInvariant, code: &mut HostCode);
    fn call(way: &Self::Way, code: &mut HostCode, call: InsideCall);
    /// As today, with the way: an unlinked exit's path into the inside is the way's call.
    fn exit(way: &Self::Way, code: &mut HostCode, target: u64);
    /// The replay check of docs/native.md 5.7, placed before the instruction at the stop point.
    fn stop_check(way: &Self::Way, code: &mut HostCode);
    fn movable(way: &Self::Way) -> core::ops::Range<u64>;
    /// Replaces `link`: the bytes that point an exit at `distance` bytes from the end of its branch, for the
    /// emitter (inside one piece) and the loader (between pieces) alike; none where the branch cannot reach.
    fn retarget(exit: &DirectExit, distance: i64) -> Option<Patch>;
    /// Bytes past the start's page the emitter must be given (the longest instruction less one).
    const PAST_PAGE: usize;
    // the other methods unchanged
}

/// A change to code other threads may be running, written by the way's protocol (section 3.2).
pub struct Patch {
    /// Offset of the instruction's first byte in the code.
    pub at: u32,
    /// The instruction's bytes after the change (x86 writes them by the `int3` protocol, ARM64 in one store).
    pub bytes: [u8; 8],
    pub length: u8,
}
```
The translation request gains the stop point of docs/native.md 5.7, part of the cache key: `stop: bool`, whether
`stop_check` goes before the piece's first instruction (section 3.7 asks for the stop pc's own piece, so the stop
point is always its start; built as `NativeRequest` beside `translate_native_for`, whose callers ask for neither,
with `stop_check` and `steps` provided methods that answer false for a guest that has none, the translator refusing
such a request); and, for P's guest single step (section 3.6), `step: bool`,
also in the cache key: every instruction its own piece, every exit to the inside, `call(way, code, Step)` after
each; and, for a thread whose x87 control word unmasks an exception (section 3.1), `x87_unmasked: bool`, also in
the cache key: every x87 instruction on the Kernel route and every MMX instruction after a call of kind
`X87Control`. `InsideCall` (gcpu/position.rs, eight kinds today) gains three, and `ALL` becomes `[Self; 11]`:
```rust
    /// A copied `fldcw` unmasked an x87 exception, or an MMX instruction follows in the unmasked mode: the inside
    /// enters that mode, or raises the pending exception as the meaning does (section 3.1).
    X87Control,
    /// After each instruction of a stepping piece: the meaning's debug trap (section 3.6).
    Step,
    /// The replay check found the thread's stop word equal to its count (section 3.7).
    Stop,
```
In the x86 crate, so that the machine's floor (`x86_64-unknown-none`) and I's inside can run them (sections 1, 3.1,
3.2, 3.5), the core gains what is translate-only today, each move its own small commit keeping the x86 and ARM64
suites passing:
- x86/condition.rs (new, core): the `Condition` enum, moved out of x86/flags.rs (which keeps its WebAssembly
  emitters `condition_from_flags` and `condition_direct` and takes the enum from there), with its plain form below.
- x86/x87.rs's unit (`Unit`, `Code`, `Operation`, `Format`, `Arithmetic`, `Extended`, `Wide` and what they use),
  x86/transcendental.rs and x86/root.rs move into the core; the two functions of x87.rs that take
  `iced_x86::Mnemonic` (`transcendental`, `above_exact`) stay behind `translate`. transcendental.rs's
  `thread_local!` cache of astro-float's `Consts` becomes a `&mut Consts` its functions and `Unit::new` take: the translate half
  keeps today's per-thread cache around them, and the floor and I's inside keep one per virtual processor or
  runner.
- x86/x87_answer.rs (new, core): the plain-Rust twin of x86/fpu.rs's instruction-level part for the unmasked x87
  mode (section 3.1): the pending-exception check, the memory operand (address, split, permissions, read, store
  with NOT_STORED and `fstp m80`'s order) around the unit, over a guest-memory access the inside passes in.
- x86/strings.rs (new, core): `string_passes`, the plain form of the meaning's rep string pass rule (section 3.6),
  for the `RepPasses` fix-up and the twin; the after-code is its emitted form.
- Cargo.toml: rustc_apfloat (Apache-2.0 WITH LLVM-exception) and astro-float-num (MIT, default features off) move
  from `translate` to the core's dependencies, and the core gains `lazy_static = { version = "1.5", features =
  ["spin_no_std"] }` (MIT OR Apache-2.0, bringing spin 0.9.9, MIT) only to give astro-float-num's lazy_static that
  feature, without which it does not build for `x86_64-unknown-none` (measured, section 1). The kernel, which
  depends on the core, then compiles these too (no run-time cost; XN1 records the build time it adds).
```rust
impl Condition {
    /// Whether the condition holds for these rflags, as `condition_from_flags` computes it.
    pub fn holds(self, rflags: u64) -> bool;
    /// The condition a `jcc`'s low four bits name.
    pub fn from_code(code: u8) -> Self;
}
```
`Route` gains `Area` (the guest's own use of the thread area's segment, section 3.1), which ARM64 never gives (its
carrying code's match takes it as unreachable). In gcpu/position.rs: `Operand` gains `segment: Option<Segment>`
(`Fs`, `Gs`) and `address_bits: u8` (32 or 64); `DirectExit::retarget` and `BranchTable::retarget` hold the branch's
displacement as `{ at: u8, bits: u8 }` in bytes, not ARM64's instruction field; `DirectExit` gains
```rust
    /// The condition under which the exit's branch is taken, as the architecture numbers it (x86: the low four bits
    /// of a `jcc`); none for an unconditional one. A hit on a link's `int3` evaluates it (section 3.2).
    pub condition: Option<u8>,
```
(ARM64 leaves it `None`: its links change a branch in one store and take no hit); the rflags in a moved slot are
`Relocated { guest: RFLAGS, .. }` with the x86 crate's map number for rflags, 16, after the general registers
(the rep string count, section 3.6); `Fixup` gains `Offset { register:
u8, by: i32 }` (rsp after a split push), `RepPasses` (the rep string's passes owed, section 3.6, until the count
is ruled) and `X87Pointer(u64)` (the guest's FIP not yet stored, section 3.1); `Held` gains `Rebased(u8)` (way
I's strings: rdi
holding the guest value plus B); rcx in t over the replay check is the existing `Relocated { guest: rcx, held: Held::Register(t) }`. `HostCode` gains `pool: Vec<u8>` (the piece's
read-only words, loaded at the code area's fixed distance) and `AddressPlace` says which of the two holds it;
`Translation` gains `past_page: bool`.
For the count's rule on rep strings (section 3.6), the proposal: `counted` becomes
```rust
pub enum Events { None, One, Elements { count: u8 } }
pub fn events<R>(address: u64, flow: &Flow<R>) -> Events;
```
with the x86 meaning adding the elements of each pass. This changes the WebAssembly output's count: the
orchestrator's and the x86 track's to agree. The owner's reviewed interfaces (executor, processor-architecture,
precise-instruction-set) are used as they stand: no executor here; the native x86 executors implement `Executor`
with an x86-64 `ProcessorArchitecture` that does not exist yet (the instruction design's).

## 10. For the owner

- The rep string count (section 3.6, 9).
- Split-lock detection on cloud hosts in P and I (section 3.6): leave it as a named difference, or have the inside
  complete such an access itself.
- Decision 14 for x86: the partial-write forms (`enter` with a level, `fstp m80`, `fbstp`, `rep` strings, the AVX2
  gathers) and the undefined-flag forms routed by the probe's match on each host, not on every host (section 3.6).
- P on a 4-level host: about 3,000 guest threads per process, from the mirror's first level (section 3.3).
