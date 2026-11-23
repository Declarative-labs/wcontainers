# x86-64 native output: plan

Status: plan (XN0, run 1, 2026-11-23), not yet checked. What the translator's native output (docs/native.md
section 3, built for ARM64) becomes for x86-64 guests on x86-64 hosts, in the three ways of running natively:
inside a virtual machine (KVM, docs/native.md section 5, "the machine"), each guest process as a host process
("P"), and the kernel in process with inline memory checks ("I"; both from the owner's ruling of 2026-11-23,
docs/native-without-virtual-machine.md when it exists). The native output is one; what differs between the
ways is named in section 2 and lives in two places of the emitter (the access and the call into the inside).

Paths: `x86/` is crates/wcontainers-x86/src, `gcpu/` crates/wcontainers-guest-cpu/src, `tr/`
crates/wcontainers-translator/src, `x86 plan` docs/phase-2-guest-cpu-and-x86.md. Measurements of this run are
in pyro-parked/wcontainers-scratch/x86-native-output (probes/ and results/), taken on the rented x86 machine
(Xeon Platinum 8488C, Sapphire Rapids, 96 processors, Linux 7.0.0-1018-aws, 5-level paging, user FSGSBASE,
vsyscall execute-only), load 1.3 to 7.2.

## 1. What is shared with ARM64, and what is new

- Shared unchanged: the native emitter's piece finder and count placement (tr/native.rs `translate_native`,
  `reach`, `counted_ahead`), `counted` and `count_placement` (gcpu/native.rs), `HostCode`, the position map
  (gcpu/position.rs) and `Translation::moved`. x86 instructions have any length and any byte may start one
  (`G::ALIGN` is 1); the finder already takes lengths from `G::decode`.
- New, in wcontainers-x86 behind its `translate` feature: `impl Native for X86_64` (module `native`), its
  carrying code (module `carry`, holding the one `access` function and the one `call` function) and its
  encoder use (iced-x86's `Encoder`, MIT, already the crate's decoder; the feature `encoder` is added).
  The route table is per iced `Code`, like the claim table.
- New, in the x86 crate's no_std core: `answer`, the plain Rust answers the inside gives for the Kernel route
  (section 3.5), sharing every table and mask with the meanings by construction (the CPUID table of
  x86/cpuid.rs, the rflags masks), so the inside of every way can run them.
- Changes to shared code, each its own small commit keeping ARM64's suites passing, listed with code in
  section 9.

## 2. The ways of running, as the native output sees them

The native output takes a `Way` (section 9) from the executor that translates; it is part of the
translation's cache key. Three things depend on it, and nothing else:
- the access form: none (machine, P), wrapping or exact (I; chosen by the in-process executor's
  `TranslatorConfig`, the owner's `Executor::TranslatorConfig` as it stands);
- the call into the inside (section 3.6): a ring change (machine) or a call on the inside's own stack (P, I);
- the guest window base, I only: the GS base, which no other way uses.

One layout for every way: the FS base reaches the thread area (below); the guest's own FS and GS bases are
guest state kept in the register area, never in the hardware. Why: form I needs GS for the window (the
measured cheapest wrapping access is one segment prefix, section 4), and keeping the guest's FS in hardware
in the other ways would make two layouts. Cost: a guest `fs:` operand gains one load (`mov t, fs:[guest fs]`)
and, with both a base and an index, one `lea`. Measured here (static, 13 files of this Ubuntu: glibc, ld.so,
libm, libstdc++, libcrypto, libz, bash, python 3.12, git, perl, xz, gcc's cc1 and Docker's daemon, a Go
program; 19.2 million instructions; results/shapes86.out): `fs:` operands 1.22 per 1000 instructions over
all, 12.7 in glibc, 11.3 in libstdc++; `gs:` operands 0.00 (0.10 in libcrypto). This departs from
docs/native.md section 3.3, which gave the guest's FS to the hardware and the thread page to GS.

The thread area, reached at `fs:[place + field]` where `place` is an address place the loader writes (so each
way puts the area where it needs it, a host thread's own static TLS included), and `field` a field offset of
one `#[repr(C)]` type in the x86 crate's core (no literal):
- read-only to guest code: the lookup's first-level table (section 3.3), the inside's entry addresses and
  stack, the window limit (I, exact form);
- read and written by guest code (the register area): the guest values of the withheld registers, the guest
  FS and GS bases, the saved guest rsp of a call into the inside. Nothing read from it is used as a host
  address or decides where host code goes.
What each way must give (recorded for the other tracks in docs/status-x86-native-output.md): the read-only
part must really be read-only to guest code. In P, guest stores are copied unchecked, so every writable page of
the guest's host process is writable by the guest, its sibling threads included: no host address translated
code jumps to may sit in writable memory, and the inside's stack in P is writable by sibling guest threads.

## 3. Each mechanism of docs/native.md section 3, in x86-64 form

### 3.1 Copy, and what is never copied

Copied as its bytes: an instruction whose host execution does exactly what its meaning does for every input,
with no RIP-relative operand, naming no withheld register, with no `fs:`/`gs:` operand, and (way I) touching no
memory. In practice: integer, SSE, x87 arithmetic and moves, `lock`ed and plain memory forms in the machine and
P, `pushf` (the hardware's TF, AC, DF, NT, ID are kept equal to the guest's, section 3.5), `pause`, the fences,
`movbe`, `adx`, `cmpxchg16b`, `clflush`, and the AVX setting's VEX forms when the container's setting claims
them. The offer is the translator's: a form outside it is not decoded and takes the Trap route (illegal
instruction), whatever the host has (this host runs AVX-512, AMX, `umwait`, `serialize`, `enqcmd`, `wrpkru`;
none is decoded, x86-machine.md). A host lacking an offered feature refuses the native way at the probe
("back end refused by the probe"); nothing is emulated. Protection keys are relied on nowhere: nothing in this
plan needs them, and `rdpkru`/`wrpkru` are outside the offer.

Never copied, by route:
- Rebase: every RIP-relative operand (24.5 `lea` and 22.0 other uses per 1000 here, against the laptop's 23.6
  and 19.3; 0 `lea` in this python, which is built without position independence). `lea r, [rip+d]` becomes
  `mov r, imm64` (an address place, its own commit point); another use becomes `mov t, imm64` and the
  instruction re-encoded on `[t]`. With the address-size prefix (`[eip+d]`) the place holds the 32-bit
  wrapped address, as the meaning computes it. A RIP-relative instruction that also names a withheld register
  takes a stand-in for that one (section 3.4).
- Transfer: every branch, call, return and jump (section 3.3). Never copied because the host address differs
  and because hosts differ: AMD honours a 66-prefixed near branch (16-bit rip, 2-byte pop) where Intel and the
  meaning ignore the prefix; the emitter computes the target as the meaning does.
- Kernel (section 3.5): `syscall`; `cpuid`; `rdtsc`, `rdtscp`, `rdpid`; `xgetbv`; segment register writes and
  far transfers (`mov`/`pop` to a segment, `lss`, `lfs`, `lgs`, far `call`/`jmp`/`retf`, `iretq`); `popf`; the
  user-mode descriptor reads (`sgdt`, `sidt`, `sldt`, `str`, `smsw`, `lar`, `lsl`, `verr`, `verw`; this host has
  UMIP, which emulates three of them, so copying would give the host's answer); the x87 transcendentals; the
  approximate reciprocals `rcpps`, `rcpss`, `rsqrtps`, `rsqrtss` and VEX forms until decision 14 is ruled;
  the XSAVE family (`xsave`, `xsavec`, `xsaveopt`, `xrstor`), whose image depends on the host's XCR0 and
  XINUSE (here XCR0 has AVX-512, AMX and PKRU: an XSAVE area of 11,008 bytes against the guest's 1,088, so a
  copied `xsave` with edx:eax all ones would write ten times past the guest's buffer; a signal frame's
  xstate_bv here is 0x207): the inside writes the image the meaning writes.
- Float status (docs/native.md 3.4, unchanged): `ldmxcsr`, `stmxcsr` and VEX forms, `fxsave`, `fxrstor`, the x87
  environment saves; the second call also writes MXCSR_MASK in an `fxsave` image as the meaning does (it is the
  host's in the hardware's image).
- Trap: `int3`, `int1`, `int n`, `ud2`, `hlt`, `in`/`out`, `cli`/`sti`, every privileged form, and every form
  outside the offer. `int 0x80` and `sysenter` are the meaning's general protection with x86's reason "32-bit
  system call from 64-bit code, not provided" (x86 plan section 5.2).
- Nothing: the forms the meaning makes do nothing (`nop` of every length, `endbr64`, the prefetch hints).
  Prefetches are never copied, so way I cannot be made to touch host memory outside the window with them.
- Undefined results (flags after `mul`, shifts, `bsf` and the rest; a `bsf` destination of zero): which
  behaviour the guest sees is the instruction design's (docs/status-instruction-design.md, a precise
  instruction set per container), not this plan's. The native route of such a form is Copy where the host's
  measured set gives the container's behaviour for it, else Kernel (the inside's answer, or the flag
  computation inline where the finder shows the flags dead). Until an x86-64 precise set exists, the meanings
  are the container's behaviour; this host and the i7-8750H agree with them on 15,728 cases of these forms
  (x86-machine.md, flags-sweep). The route therefore takes the host's match (section 9).

### 3.2 Pieces, layout and patching with variable lengths

- Instructions are laid out in address order as on ARM64. An x86 piece may hold instructions that overlap (a
  direct branch into the middle of a held instruction): the finder keys instructions by offset, so the two
  decodings are separate parts, and the first's fall-through goes on by an exit. XN1 has a test of exactly this
  shape (`jmp` into the immediate of a `mov`).
- An instruction may straddle the end of the start's page. The emitter is given the page and up to the
  longest instruction less one (14) bytes of the next page when that page is executable; a piece whose last
  instruction straddles records it, so the write lock of both pages drops it (as the WebAssembly output locks
  `NEXT_PAGE_WRITE`, x86 plan section 5.7); a fetch from a next page that cannot run code is the meaning's fault
  there, which the inside reports as the WebAssembly output does.
- Branches the emitter writes are rel32 (`jmp`, `jcc`, 5 and 6 bytes), never relaxed to rel8, so every exit
  has one length and a link is one patch. A linkable exit's `jmp rel32` sits inside one aligned 8-byte word
  (the emitter pads with a multi-byte `nop`), so linking and unlinking are one atomic 8-byte store, the form
  JITs use for cross-modifying code on x86; in P and I, where other host threads of the process may run the
  piece, the patching thread then makes `membarrier(MEMBARRIER_CMD_PRIVATE_EXPEDITED_SYNC_CORE)` (registered at
  process start), so every thread serializes before it runs the new bytes (Intel SDM volume 3, 9.1.3,
  cross-modifying code); in the machine the serializing exit and an interprocessor interrupt do it (docs/
  native.md 3.2). A sandbox that refuses `membarrier` leaves P and I with patching only while the process's
  other threads are stopped: named "links made with threads stopped".
- An address place is the 8-byte immediate of `mov r, imm64` (10 bytes, the immediate at byte 2): one length
  wherever the code sits, so the code moves by its places alone. `movable()` is every page from 2^32 to the
  ceiling (section 6), since no host instruction addresses a guest address relatively.
- The entry of a piece reached through the lookup is its first byte, aligned to 16 (padding is outside every
  span, so no stop lands there).

### 3.3 Calls, returns and jumps through a register: the two-level mirror

What any x86 sequence must meet (docs/native.md 3.2): a guest call puts its 64-bit guest return address on the
guest stack; the flags at an unknown target cannot be shown dead, so the lookup writes none; the host's
return-address predictor cannot be used (a host `ret` would read a guest-written address).

The design, built at XN1: three withheld registers, c (the count), t and u (scratch). A guest `ret` is
```
mov t, [rsp]              ; fault here: before, as the reference's pop
lea rsp, [rsp + 8 (+imm)] ; commit: after, the guest is at the address in t
rorx u, t, 32             ; u's low half: bits 32..63 of t
movzx u32, u16            ; u = bits 32..47
mov u, fs:[place + l1 + u*8]   ; first level, per thread: region u's table less (u << 35)
jmp qword ptr [u + t*8]   ; second level: the host entry of the piece at t
```
No flag is written, nothing is compared, and the host address jumped to is always read from our tables: for
any t the read lands in a second-level table of ours (bits 32..47 select it, the stored base cancels them),
or, when bits 48..60 of t are not all zero, a multiple of 2^51 beyond it, which on a 4-level host is not
canonical and on a 5-level host is above every mapping we and the guest make (the guest's ceiling is under
2^47), so it faults. So it is safe for every t. It is exact only below 2^61: the scale drops bits 61..63, so a
t with those set and bits 48..60 clear would land on the piece of t mod 2^61, where the reference refuses the
jump. XN1 therefore puts the top bits' bound in front, measured there (the figures below do not include it):
`rorx t, t, 48; movzx u32, t16; mov u, fs:[place + top + u*8]; rorx t, t, 16; lea t, [t + u]`, where `top` is
2^16 words, 0 at index 0 and 2^52 elsewhere (one read-only region shared by every thread), so a t with any of
bits 48..63 set is moved to where the second read faults. A target the reference refuses (not canonical, or at
or above 2^47) is answered as the meaning answers it, with rsp put back by the span's fix-up when the
reference faults before the pop. First-level pages not yet needed and second-level pages not yet needed are not mapped (reserved
without access): the fault is the miss, resolved by the inside from the faulting address and t (still in its
register), which fills the entry or reports the jump as the meaning does (for an address at or above 2^47,
its general protection). Each second-level table is a reservation of 2^35 bytes per 4 GiB region of guest
addresses that holds code a jump reached; its pages hold eight bytes per guest byte, materialized per 4 KiB
(512 guest bytes). A call is `mov u, imm64` (G + length, an address place), `push u` (the commit point; a fault
there is before), then a direct exit; `call r/m` first loads the target into t (its own fault, before), as the
reference reads the operand before it pushes. A jump through a register or memory is the load into t, then the
four words above. The count's add is at the start of the run (definition E unchanged).

Measured (probes/returns.S, recursive fib(30), three alternating runs of 21 rounds, cycles by `rdtsc`, one
pinned processor; results/returns.out and returns2.out): against plain `call`/`ret`, a one-level mirror
indexed by t alone 1.05 to 1.13 times (not bounded: shown for what the bound costs), the two-level mirror
above 1.120 to 1.133, a 256-entry cache compared with `cmp` (flags clobbered, so not usable: a lower bound
for every compare form) 1.17 to 1.39, and the same cache compared without flags (rcx borrowed, `not`, `lea`,
`jrcxz`) 1.52 to 1.62. So the compare forms docs/native.md 3.2 listed are rejected. Not measured: a return
site the indirect predictor cannot learn (an interpreter's), and the memory the second level takes on busybox
and python (XN1 records both). Rejected also: a one-level mirror (its 2^50 bytes do not fit a 4-level host's
2^47 user space beside the guest, and its index needs a bound).

### 3.4 Registers kept for the translated code

Every x86 register is live in guest code, so hooks use none of the guest's: every call into the inside saves
and restores everything the inside touches (section 3.6). Withheld: c, t and u, chosen among r8 to r15 (no
form names them implicitly except `syscall`'s r11, which is the Kernel route), by the dynamic count of XN1 on
the benchmark set and the X9 programs. The static count here, per 1000 instructions over all 13 files: r10
13.9, r15 15.0, r11 17.1, r9 19.1, r14 21.3, r13 21.8, r8 28.6, r12 30.8; in the C files alone r10 and r11 are
the fewest (glibc 17.9 and 14.0; cc1 8.0 and 4.6) and r15 is 26 to 45, while the Go program names r15 2.9 and
r11 24.9 (Go keeps r14 for its goroutine). Expected choice: r10 (c), r11 (t, so `syscall`'s write of r11 only
clobbers a scratch register), r15 (u). Stand-ins follow docs/native.md 3.3 unchanged: one load into t, or a
guest register the instruction does not name saved to the register area, both through `fs:` moves, which touch
no flag. Counted "withheld register named by the guest".

### 3.5 The Kernel route: who answers

The inside answers with `answer` (section 1): `cpuid` from the same table the meaning emits (both settings),
`xgetbv` with the offer's XCR0, `rdtsc`/`rdtscp`/`rdpid` through the clock and the processor answer the
meaning gives, segment writes and far transfers by the x86 plan's rules (a selector of 32-bit code, 0x23, is
refused by name, "far transfer to 32-bit code, not provided"), `popf` and `iretq` taking TF, DF, NT and ID and
leaving AC clear, the descriptor reads' designed answers, the transcendentals through the meaning's
astro-float-num function, the reciprocals by the meaning's formula, the XSAVE family by the meaning's image.
Pure answers (all but the clock, the system call and the far transfers' kernel part) need no kernel; whether
P answers them in the guest's host process is P's choice. Where an answer is WebAssembly logic in the meaning
and Rust logic here, the differential test (section 7) proves them equal case by case.

### 3.6 System calls, faults, stops, the count

- The call into the inside, one function `call(way, kind)`: in the machine, `int VECTOR + kind` (a DPL 3 gate
  the floor owns; the ring change switches to the floor's stack by the TSS, so no guest byte is touched), and
  a guest `syscall` is copied as `syscall` (the floor owns LSTAR; it writes rcx with the guest's next address
  from the span and r11 with the guest rflags, as the reference does; r11 is t, a scratch); in P and I,
  `mov fs:[place + saved rsp], rsp; mov rsp, fs:[place + inside stack]; call qword fs:[place + entry + 8*kind]`,
  three moves and a call, no flag written, no guest byte touched (the red zone below the guest's rsp and any
  rsp the guest holds, including one that is not a stack, stay untouched). The inside saves the general
  registers, rflags (then `cld`, as its ABI requires), and the vector, MXCSR and x87 state it may change, and
  restores them all: what it does not touch it need not save (a Rust kernel built for x86-64 uses xmm0 to
  xmm15 and MXCSR). The register-group rule of the native brief holds at thread switches, which are the
  inside's.
- Faults: the hardware's code is never the guest's answer. A fault in translated code is resolved by the
  position map; for an access its guest address comes from the span's operand, which gains the segment and
  the address size (section 9). So: a non-canonical address is general protection with no address, as the
  reference, also on this 5-level host, which reports SEGV_MAPERR at the address for 2^47 to 2^56; a
  misaligned `movaps` or `cmpxchg16b` is general protection (P and I read the trap number from the signal
  context's `REG_TRAPNO`); division faults are the meaning's. A locked access across a cache line is slowed by
  this host's split-lock detection and would be SIGBUS under `split_lock_detect=fatal`: named for the owner
  (section 10), not handled.
- x86's architecture leaves no partial write behind a fault for one non-string instruction: a fault restores
  the state before the instruction (Intel SDM volume 3, 6.5, Faults). Measured again on this host
  (probes/partial86.c, every offset across a page that refuses writes or reads): `mov` of 8 bytes,
  `movdqu`, `vmovdqu` of 32, `fxsave64` and `xsave64` wrote nothing in the first page, and loads changed no
  register, as on the i7-8750H. So x86 takes no Split route and no page test; decision 14's question does not
  arise for x86 except `rep` strings (below) and the AVX2 gathers, whose progress the hardware and the meaning
  both record per element.
- Stops: the machine by an interrupt, P and I by a host signal on the thread (run on an alternate signal stack,
  since the guest's rsp is the guest's). A stop at any host instruction resolves by the position map, exactly
  as on ARM64 (docs/native.md section 4). A stop that lands inside a `rep` string instruction does not use a
  debug register: every rep string instruction has an out-of-line twin, the same instruction followed by a call
  into the inside, and the stop resumes the thread at its twin with the hardware's registers, so the
  instruction ends and the stop is taken at its boundary ("stop deferred to the end of a string instruction").
  This replaces docs/native.md section 4's DR0 breakpoint, which P and I cannot set without `perf_event_open`
  (refused here at `perf_event_paranoid` 4, costs-cloud.json).
- The count: `lea r10, [r10 + k]` at each run's start (no flag). A rep string instruction is an open question
  between two plans: docs/native.md section 4 says E does not count its rounds; the x86 meaning names a jump to
  itself, so `counted` counts every pass, and a pass is one element or a bulk run of at most 1 MiB, chosen by
  the direction flag, the trap flag, overlap and the range's permissions (x86 plan 5.6). Native code can match
  only by computing that choice again after the hardware ran the instruction. Proposed for the owner
  (section 9): a rep string's events are its elements, which both outputs know (rcx before less rcx after),
  `lea c, [c + t]` plus a flag-free subtraction natively, and which make every element boundary a distinct
  position on both outputs. Until ruled, XN1 builds the pass count from one shared function of the x86 core
  that the meaning's constants feed.
- TF: as docs/native.md 3.4 (routed `popf`/`iretq`, hardware TF equal to the guest's, steps resolved through
  the map). In P and I every host instruction of the call into the inside steps too until the inside clears
  TF: the handler resumes those silently ("single step through host instructions").

### 3.7 Self-modifying code, code reading itself, signals that edit registers

- A translated page is write-locked (the machine: its page tables; P and I: the host's protection of that guest
  page); the first write faults, the inside drops the page's pieces and their mirror entries and links (each
  exit and second-level entry is recorded), unlocks, runs the faulting instruction alone from fresh bytes, and
  goes on; the next translation locks again. Intel guarantees that a store changing the very next instruction
  takes effect: here the store faults first, so the next instruction is translated from the new bytes.
- Code reading its own bytes reads guest memory, which holds the guest's bytes (translated code lives
  elsewhere and never reads them after translating). A jump table whose entry lands in the middle of an
  instruction goes through the lookup and translates from that byte.
- A signal handler that edits the saved registers: the frame is built by the kernel from the thread's block
  (the withheld registers' guest values merged from the register area), sigreturn loads it, and the thread
  goes on at its guest rip through the lookup, wherever that is. `call next; pop` and unwinders see guest return
  addresses, since every call pushes the guest address.
- vsyscall and vDSO: our kernel gives an x86-64 vDSO, guest code translated like any other (its `rdtsc` is the
  Kernel route); the legacy vsyscall page is not provided (x86 plan: a call faults, as under `vsyscall=none`),
  so a jump to 0xffffffffff600000 is the lookup's general protection there. The host's own vDSO, vvar and
  vsyscall pages (here vvar, vdso at the top of the 47-bit space, vsyscall execute-only) are P's: they sit in
  the guest's host process at addresses the guest may name, so P must unmap the first two and keep the guest's
  layout clear of them, or count the difference (P's plan).
- 32-bit code is never run: the decoder is 64-bit only, far transfers and `iretq` are answered and refuse 0x23
  by name, `int 0x80` and `sysenter` are the meaning's general protection, and a 32-bit ELF is the kernel's
  refusal.

## 4. The three memory-access forms (way I only changes them)

The one place is `carry::access(way, operand, code)`; every route that emits a guest access calls it, implicit
ones included. Guest memory in I is a window of the host process at B, the GS base (per process); the guest's
ceiling there is 2^32 (wrapping) and, for exact, at most 2^32 as well in the first build (a larger exact window
needs the bound in a register, measured at XN2 if asked).
- None (machine, P): the operand as the guest wrote it (only `fs:`/`gs:` rewritten, section 2).
- Wrapping: the instruction re-encoded with the address-size prefix and `gs:`, so the hardware forms the
  address in 32 bits and adds B: no instruction added. A `fs:`/`gs:` operand is `mov t, fs:[guest base]`, `lea
  t, [t + base]` when it has a base, and the access on `gs:[t + index*scale + disp]` in 32-bit form. Measured
  earlier (costs-cloud.json, round 2's tooling, stack exempt): +0.0% instructions, +1.8 to +4.3% wall on
  Granite Rapids, -7.8 to +5.0% cycles on AMD Zen 4, +2 to +4% on the laptop. Its named difference: an address at
  or above 2^32 lands at its value modulo 2^32 inside the guest's own window, where the WebAssembly output
  faults (it cannot reach anything not the guest's).
- Exact: where the finder shows the flags dead within the straight-line run (static: 79% of memory operands
  here, results/shapes86.out, a conservative count), `lea t, [operand]` (64-bit), `cmp t, fs:[place + limit]`,
  `jae out`, and the access on `gs:[t]`; where they may be live (21%), a flag-free probe with a stand-in v (u is
  the lookup's and free here): `lea t, [operand]; rorx v, t, 32; mov v32, v32; lea v, [v*8]; not v; mov v,
  gs:[v + 1 - 2*PAGE]`, which reads B - 2 PAGE (a readable page of ours) when bits 32..63 of t are zero and else
  an address 8 to 2^35 bytes below it, in a reservation without access, then the access on `gs:[t]`; counted
  "exact check with live flags". A fault on either is the guest's fault at t, from the span. Measured earlier:
  X32 +11.0 to +15.2% wall (Granite Rapids), +10.0 to +25.4% cycles (Zen 4), +14 to +32% on the laptop.
- Guards around the window: below B, one page without access (so a downward string or access that leaves the
  window faults before it reaches ours), the probe's readable page under it, and 2^35 bytes without access
  under that; above B + 2^32, 2^35 bytes without access, so an access that starts below 2^32 and runs over, and
  a 64-bit `fs:` sum, fault.
- Implicit accesses in I: `push`, `pop`, `call`, `ret`, `enter`, `leave` become explicit accesses through
  `access` on rsp (`mov gs:[esp - 8], src` then `lea rsp, [rsp - 8]`, the store the commit point and the `lea`
  its fix-up, section 9); string instructions and `maskmovdqu` add B to rsi and rdi around the copied
  instruction (`mov t, fs:[place + base]; lea rdi, [rdi + t]`, and back with the negated base), the guard
  regions catching a run past the window, the position map holding "rdi less B" while they are rebased;
  `xlat` is re-encoded on `gs:`. The AVX2 gathers in the exact form go to the inside (rare; their lanes cannot
  be bounded by one probe), counted.
- The page permissions inside the window are the host's protection of the window's pages, written by the
  kernel's mapping calls, 4 KiB as the guest's.

## 5. What the guest sees: differences named

- Way I, wrapping: the modulo landing above (section 4).
- P: the host pages in the guest's address space (section 3.7) and the read-only thread area: a guest read
  there succeeds where Linux faults, unless P keeps the guest away from them (P's plan decides and counts).
- P and I on a host with split-lock detection: a split locked access is slowed (or fatal) where the reference
  runs it.
- Everything else is the WebAssembly output's behaviour; the tests of section 7 hold it.

## 6. Ceilings

- The machine: 2^47, the guest's own (the floor chooses 4-level paging for the guest's half).
- P: 2^47, less what P reserves for its own tables (the mirror's second levels, 32 GiB per region with code).
- I: 2^32 in the first build.
Each is `Cpu::address_space` of that way; a request above it fails, counted, as the brief allows.

## 7. Tests

- Routing, static: every decoded x86 form through the recording emitter, classes as docs/native.md 3.4; a form
  added to the x86 crate is routed or the test fails.
- Routing, differential: every decoded form run natively as routed and through the WebAssembly output from the
  same seeded state (all 64 bits of every register, every register collision, each access placed across a page
  with either page refusing, after seeded float flags with tracking on and off, TF and AC in the values popf
  takes), comparing every register, flag, control, memory byte and trap. Seed and cases per form are arguments.
- Count: the same count at every stop across look-ahead none, bounded and full on the benchmark set and layer 2's
  programs, the kernel test's hash with a stop at every counted event; the every-offset test with a stop at every
  host instruction of every piece, the lookup and the twins.
- The shapes this plan was broken with, kept as tests: a direct branch into the middle of an instruction in the
  same piece; a jump table into the middle of an instruction; code reading its own bytes and code storing into
  the next instruction; a signal handler editing rip, r10, r11, r15 and rflags; a jump and a call to
  0xffffffffff600000 and to the vDSO; a far transfer to 0x23, `int 0x80`, `sysenter`; `ret` to 2^47, 2^48, 2^63 and
  -1; a stop inside `rep movsb`, `rep stosq` and `repe cmpsb`; an instruction straddling into a page that cannot
  run code; a 66-prefixed `jmp`, `call` and `ret`; `xsave` with edx:eax all ones beside a guest buffer of the
  offered size.
- Way I: no access outside the window succeeds in either form, through every operand shape: one byte past a
  mapping, 2^32 - 1 with 8 bytes, a SIB with a large index, `fs:` with a negative offset, a downward and an upward
  string with a large rcx, a push at the bottom of the stack, `enter` with a deep level.

## 8. Milestones and pass bars

- XN1, the output with no memory check (machine and P). Built: `Native` for x86-64, the routes, the encoder, the
  stand-ins, the mirror, the count, the twins, `answer`, the changes of section 9. Runs translations through
  the first native x86 executor that exists (P's, docs/native-without-virtual-machine.md W1, or the KVM floor);
  until one can run a translation, the static routing test, encoding round trips through iced's decoder and the
  position map's unit tests run, and the rest waits on it (coordinated in the status file). Bar: on the rented
  x86 machine against the real x86-64 reference, the translator's layer 1 (every form, 1,000 cases a form or
  more, seed recorded; the 5-level forms judged as x86-machine.md says), layer 2 (10,000 programs, seed
  recorded), layer 3, the count test and the every-offset test, the shapes of section 7, the X9 binaries
  natively with the verdicts they have under WebAssembly, and the benchmark set's speed against the same
  programs run plainly on the host (cycles from `perf_event_open` where allowed, else wall time, machine and
  load recorded; the bar's figure is the owner's to set from N4's +21.2%, proposed the same). The dynamic
  register count and the mirror's memory on busybox and python are recorded.
- XN2, the two checked forms (way I). Bar: section 7's way I tests pass for both forms, faults carry exact
  registers, and each form's cost against the unchecked form per workload on the benchmark set and the X9
  programs is in the status file, beside the earlier +11 to +21% (exact) and +2 to +4% (wrapping).

## 9. Changes to shared code (as code, for the owner and the translator's owners)

In gcpu/native.rs, the trait gains the way and what x86 needs; ARM64's implementation takes `Way` and ignores
what does not apply to it:
```rust
pub trait Native: Guest {
    /// How translated code reaches guest memory and the inside; part of the translation's cache key.
    type Way: Copy + Eq + core::hash::Hash + Serialize;
    /// Forms whose host execution gives the container's chosen behaviour, found by the probe.
    type HostMatches;
    fn route(i: &Self::Instruction, host: &Self::HostMatches) -> Route;
    /// As today, with the way and the rest of the straight-line part after it (flag liveness).
    fn carry(way: &Self::Way, before: &[(u64, Self::Instruction)], i: &Self::Instruction, bytes: &[u8],
        address: u64, after: &[(u64, Self::Instruction)], invariant: &Self::LoopInvariant, code: &mut HostCode);
    fn call(way: &Self::Way, code: &mut HostCode, call: InsideCall);
    fn movable(way: &Self::Way) -> core::ops::Range<u64>;
    /// Points a loaded exit at `distance` bytes from the end of its branch, as the loader links it.
    fn retarget(code: &mut [u8], exit: &DirectExit, distance: i64) -> bool;
    /// Bytes past the start's page the emitter must be given (the longest instruction less one).
    const PAST_PAGE: usize;
    // route's other methods unchanged
}
```
In gcpu/position.rs: `Operand` gains `segment: Option<Segment>` (`Fs`, `Gs`) and `address_bits: u8` (32 or 64);
`DirectExit::retarget` and `BranchTable::retarget` hold the branch's displacement as `{ at: u8, bits: u8 }` in
bytes, not ARM64's instruction field; `Fixup` gains `Offset { register: u8, by: i32 }` (rsp after a split push);
`Held` gains `Rebased(u8)` (rdi or rsi holding the guest value plus B); `Translation` gains `past_page: bool`.
For the count's rule on rep strings (section 3.6), the proposal: `counted` becomes
```rust
pub enum Events { None, One, Elements { count: u8 } }
pub fn events<R>(address: u64, flow: &Flow<R>) -> Events;
```
with the x86 meaning adding the elements of each pass. This changes the WebAssembly output's count: the
orchestrator's and the x86 track's to agree. The owner's reviewed interfaces (executor, processor-architecture,
precise-instruction-set) are used as they stand: no executor here; the native x86 executors implement
`Executor` with an x86-64 `ProcessorArchitecture` that does not exist yet (the instruction design's).

## 10. For the owner

- The rep string count (section 3.6, 9).
- Split-lock detection on cloud hosts in P and I (section 3.6): leave it as a named difference, or have the inside
  complete such an access itself.
- Decision 14 for x86: the architecture already guarantees no partial write (section 3.6); routing the
  undefined-flag forms by the probe's match, not on every host.
- The thread area's fixed layout for all three ways (section 2), which costs `fs:` operands one load.
