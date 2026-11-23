# x86-64 native output: plan

Status: plan (XN0, run 2, 2026-11-23), revised after its independent check. What the translator's native output
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
- New, in the x86 crate's no_std core: `answer`, the plain Rust answers the inside gives for the Kernel route
  (section 3.5), sharing every table and mask with the meanings by construction (the CPUID table of x86/cpuid.rs,
  the rflags masks), so the inside of every way can run them.
- Changes to shared code, each its own small commit keeping ARM64's suites passing, listed with code in section 9.

## 2. The ways of running, as the native output sees them

The native output takes a `Way` (section 9) from the executor that translates; it is part of the translation's
cache key. Four things depend on it, and nothing else:
- the thread area's segment: GS in the machine and P, FS in I;
- where the guest's FS base lives: in the hardware in the machine and P (so a guest `fs:` operand, `rdfsbase` and
  `wrfsbase` are copied), in the register area in I (every guest access is rewritten there anyway);
- the access form: none (machine, P), wrapping or exact (I; chosen by the in-process executor's
  `TranslatorConfig`, the owner's `Executor::TranslatorConfig` as it stands);
- the call into the inside (section 3.6).
The guest's GS base is in the register area in every way (guest `gs:` operands: 0.00 per 1000 instructions over 13
files, 0.10 in libcrypto, results/shapes86.out). In I, GS holds the guest window's base B (section 4).

The thread area is reached at `seg:[field]`, `field` a field offset of one `#[repr(C)]` type in the x86 crate's
core (no literal; a disp32), the segment base being the area itself, set by the way for each host thread. Machine:
one area per virtual processor, its first level pointed at the running process's by the floor at a process switch.
I: one area per runner, the running process's first level mapped into it (one `mmap` of that process's memfd range,
0.9 µs on the Xeon by the P/I plan's W0 probe) when the runner switches to a thread of another process; FS is
written with `wrfsbase` at every entry into guest code and the host's value put back at every exit, as the P/I
plan's 3.8 does, and GS with the process's window base (`arch_prctl` where the host lacks user FSGSBASE, a counted
slow path). P: one area per guest thread, set once. Where an area serves many guest threads (machine, I), the
thread's own words (register area, stop word) are copied in and out at each guest thread switch with its other
registers. It holds:
- read-only to guest code: the lookup's first level (section 3.3), the inside's entry addresses and (I) stack, the
  stop word (section 3.7), the window limit, the window base and its negation (I);
- read and written by guest code (the register area): the guest values of the withheld registers and of the
  guest's GS base (and FS base in I). Nothing read from it is used as a host address or decides where host code
  goes.
What each way must give (recorded for the other tracks in docs/status-x86-native-output.md): the read-only part
really read-only to guest code; no host address translated code or the inside jumps to, returns to or resumes at
may ever sit in memory the guest can write. In P every writable page of the guest's host process is writable by the
guest and its sibling threads; so is a signal frame on its alternate stack.

## 3. Each mechanism of docs/native.md section 3, in x86-64 form

### 3.1 Copy, and what is never copied

Copied as its bytes: an instruction whose host execution does exactly what its meaning does for every input, with
no RIP-relative operand, naming no withheld register, with no operand or effect on the thread area's segment, and
(way I) touching no memory. In practice: integer, SSE, x87 arithmetic and moves, `lock`ed and plain memory forms in
the machine and P, `fs:` operands, `rdfsbase` and `wrfsbase` in the machine and P, `pushf` (the hardware's TF, AC,
DF, NT, ID are kept equal to the guest's, section 3.5), `pause`, the fences, `movbe`, `adx`, `cmpxchg16b`,
`clflush`, and the AVX setting's VEX forms when the container's setting claims them. The offer is the translator's:
a form outside it is not decoded and takes the Trap route (illegal instruction), whatever the host has (this host
runs AVX-512, AMX, `umwait`, `serialize`, `enqcmd`, `wrpkru`; none is decoded, x86-machine.md). A host lacking an
offered feature (user FSGSBASE included, in the machine and P) refuses the native way at the probe ("back end
refused by the probe"); nothing is emulated. Protection keys are relied on nowhere: nothing in this plan needs
them, and `rdpkru`/`wrpkru` are outside the offer.

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
  `rdfsbase`, `wrfsbase`). An operand becomes `mov t, seg:[guest base]` and the access on `[t + base + index*scale +
  disp]` (`lea t, [t + base]` first when it has both a base and an index); `rd*base r` is `mov r, seg:[guest base]`
  (the 32-bit form `mov r32`, as `rdfsbase r32` zero-extends); `wr*base` goes to the inside (Kernel), which refuses a
  non-canonical base with general protection as the meaning does, since its canonical check would write flags
  inline. A copied `wrgsbase` would let the guest choose where the lookup reads its first level, and a copied
  `rdgsbase` or `rdfsbase` (in I) would hand it a host address: the static routing test (section 7) fails if any of
  them is routed Copy in the way concerned.
- Kernel (section 3.5): `syscall`; `cpuid`; `rdtsc`, `rdtscp`, `rdpid`; `xgetbv`; segment register reads and writes
  and far transfers (`mov` from and to a segment register, `push` and `pop` of `fs` and `gs`, `lss`, `lfs`, `lgs`, far
  `call`/`jmp`/`retf`, `iretq`; a read gives the meaning's selector, never the host's or the floor's); `popf`; the
  user-mode descriptor reads (`sgdt`, `sidt`, `sldt`, `str`, `smsw`, `lar`, `lsl`, `verr`, `verw`; this host has
  UMIP, which emulates three of them, so copying would give the host's answer); the x87 transcendentals; the
  approximate reciprocals `rcpps`, `rcpss`, `rsqrtps`, `rsqrtss` and VEX forms until decision 14 is ruled; the
  XSAVE family (`xsave`, `xsavec`, `xsaveopt`, `xrstor`), whose image depends on the host's XCR0 and XINUSE (here
  XCR0 has AVX-512, AMX and PKRU: the host's XSAVE area is 11,008 bytes against the guest's 832 with the AVX setting
  and 576 without (x86/cpuid.rs), so a copied `xsave` with edx:eax all ones would write about thirteen times past
  the guest's buffer; x86-machine.md): the inside writes the image the meaning writes.
- Float status (docs/native.md 3.4, unchanged): `ldmxcsr`, `stmxcsr` and VEX forms, `fxsave`, `fxrstor`, the x87
  environment saves; the second call also writes MXCSR_MASK in an `fxsave` image as the meaning does.
- Trap: `int3`, `int1`, `int n`, `ud2`, `hlt`, `in`/`out`, `cli`/`sti`, `swapgs` and every privileged form, and every
  form outside the offer. `int 0x80` and `sysenter` are the meaning's general protection with x86's reason "32-bit
  system call from 64-bit code, not provided" (x86 plan section 5.2).
- Nothing: the forms the meaning makes do nothing (`nop` of every length, `endbr64`, the prefetch hints).
  Prefetches are never copied, so way I cannot be made to touch host memory outside the window with them.
- Undefined results (flags after `mul`, shifts, `bsf` and the rest; a `bsf` destination of zero): which behaviour
  the guest sees is the instruction design's (docs/status-instruction-design.md, a precise instruction set per
  container), not this plan's. The native route of such a form is Copy where the host's measured set gives the
  container's behaviour for it, else Kernel (the inside's answer, or the flag computation inline where the finder
  shows the flags dead). Until an x86-64 precise set exists, the meanings are the container's behaviour; this host
  and the i7-8750H agree with them on 15,728 cases of these forms (x86-machine.md, flags-sweep). The route therefore
  takes the host's match (section 9).
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
- Branches the emitter writes are rel32 (`jmp`, `jcc`, 5 and 6 bytes), never relaxed to rel8, so every exit has one
  length and a link is one patch. A linkable exit's `jmp rel32` sits inside one aligned 8-byte word (the emitter
  pads with a multi-byte `nop`), so linking and unlinking are one atomic 8-byte store. Both the old and the new
  bytes are correct to run (an unlinked exit enters the inside, which finds the piece and goes on), so a thread
  that runs the old bytes is right; what the store needs is the Intel SDM's cross-modifying rule (volume 3, 9.1.3):
  every processor that may run the bytes serializes before it relies on them. Who serializes, per way: the
  machine, the floor's interprocessor interrupt and the serializing exit (docs/native.md 3.2); I, the patching
  thread's own `membarrier(MEMBARRIER_CMD_PRIVATE_EXPEDITED_SYNC_CORE)`, which covers its process; P, where the
  embedder writes the code area through its own mapping of the memfd and the guest's threads run in the stub's
  process, a thread of the stub makes that call in the stub's own process at the embedder's request, once per batch
  of links (the call serializes only the caller's memory). A link the stub has not yet confirmed is still correct to
  run as either bytes. Dropping a piece other threads may be inside is done with those threads stopped (section
  3.7). A sandbox that refuses `membarrier` leaves P and I linking only while the process's other threads are
  stopped: named "links made with threads stopped".
- An address place is the 8-byte immediate of `mov r, imm64` (10 bytes, the immediate at byte 2): one length
  wherever the code sits, so the code moves by its places alone. The thread area needs no place (its segment base
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
  as the meaning does (x86/guest.rs `branch_target`, 4-level canonical rule): a target that is not canonical is
  general protection at the transfer, no address, rip on it, rsp not popped; a canonical target in the upper half
  (the vsyscall address 0xffffffffff600000 included) is handed on, so the inside completes the pop (or a call's push)
  and reports a page fault at the target with rip = the target, as Linux does (also under `vsyscall=none`).
  Measured on this host (check-xn0/mirror2b.out, refjump): a `ret` to 0xffffffffff700000 or 0xffff800000001000 is
  SIGSEGV SEGV_MAPERR at the target with rip the target and rsp popped; a `ret` to 0x8000000000401000 is SIGSEGV
  SI_KERNEL, address 0, rip on the `ret`, rsp not popped (this host's 5-level paging takes 0xfff0000000401000 as a
  page fault, where the guest's 4-level rule gives general protection; x86-machine.md judges such forms).
- t below 2^47 with no piece: the miss routine (or the second read's fault in a region or page not yet mapped)
  enters the inside with t, after the commit: the inside translates if the guest page at t can run code, else
  reports the meaning's page fault at t.
Measured (probes/mirror3.S and mirror3.c; results/mirror3.out; load 9.4 then 2.4): over 37 targets (a valid one,
each of bits 32..63 set alone, 0xfff0000000401000, 0xffffffffff600000, -1, 0xe000000000401000) this form reaches no
wrong piece and keeps t at every fault; the run-1 form indexed by bits 32..47 alone reached the piece of 0x401000
for four of them (bit 61, 62 or 63 set). Timing, recursive fib(30), 3 runs of 21 alternating rounds, `rdtsc`, one
pinned processor, against plain `call`/`ret`: this form 1.065 to 1.070, the run-1 form 1.112 to 1.117, a `pext`
index over bits 32..47 and 61..63 1.119 to 1.123 (exact too, but slow on AMD before Zen 3), and the run-1 form with
t saved first and a non-wrapping top-bits bound 1.265 to 1.272. Earlier (results/returns.out): a 256-entry cache
compared with `cmp` (flags clobbered, a lower bound for every compare form) 1.17 to 1.39, the same compared without
flags (rcx borrowed, `jrcxz`) 1.52 to 1.62; so the compare forms docs/native.md 3.2 listed are rejected. Not
measured: a return site the indirect predictor cannot learn (an interpreter's), and the memory the second level
takes on busybox and python (XN1 records both).

Calls: `call r/m` is the load into t (its own fault, before), the three instructions of the first level (before,
as the reference checks the target before it pushes), `push qword ptr [rip + d]` with the guest return address in
the piece's constant pool (the commit point; its fault is before), and the second-level `jmp`. A direct `call` is
the same `push` and a direct exit (a target at or above 2^47 is known when translating: Kernel route). A jump
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

### 3.5 The Kernel route: who answers

The inside answers with `answer` (section 1): `cpuid` from the same table the meaning emits (both settings),
`xgetbv` with the offer's XCR0, `rdtsc`/`rdtscp`/`rdpid` through the clock and the processor answer the meaning
gives, segment reads, writes and far transfers by the x86 plan's rules (a selector of 32-bit code, 0x23, is refused
by name, "far transfer to 32-bit code, not provided"), `wrfsbase`/`wrgsbase` where they are routed, `popf` and
`iretq` taking TF, DF, NT and ID and leaving AC clear, the descriptor reads' designed answers, the transcendentals
through the meaning's astro-float-num function, the reciprocals by the meaning's formula, the XSAVE family by the
meaning's image. Pure answers (all but the clock, the system call and the far transfers' kernel part) need no
kernel; whether P answers them in the guest's host process is P's choice. Where an answer is WebAssembly logic in
the meaning and Rust logic here, the differential test (section 7) proves them equal case by case.

### 3.6 System calls, faults, the count

- The call into the inside, one function `call(way, kind)`, writing no flag and touching no guest byte (the red
  zone and any rsp the guest holds, including one that is not a stack, stay untouched):
  - the machine: `int VECTOR + kind`, a DPL 3 gate the floor owns; the ring change switches to the floor's stack by
    the TSS, and the floor returns by `iretq` to the host address the processor saved there, which no guest code can
    reach. A guest `syscall` is copied as `syscall` (the floor owns LSTAR; it writes rcx with the guest's next
    address from the span and r11 with the guest rflags, as the reference does; r11 is t, a scratch).
  - P and I: `lea t, [rip]` (the host address of the next instruction) and `jmp qword ptr seg:[entry + 8*kind]`.
    Nothing is pushed: no host address goes to any stack. The routine at the entry keeps t in a register (P: the
    stub's assembly, which waits in the host kernel with its state in registers, as the P/I plan's 3.7 requires) or
    in the inside's own memory (I, out of every guest's reach), and resumes by jumping to it, or through the lookup
    at a guest pc when the kernel changed where the guest goes (a signal, `rt_sigreturn`, `execve`); t is never read
    back from memory a guest can write. In I the routine switches to the inside's stack (the area's read-only part names
    it) before any Rust code runs.
  - What is saved: the inside's routine saves the general registers and rflags (then `cld`); the register groups
    follow the native brief's last section, per way: in the machine the floor is built for `x86_64-unknown-none`
    (no SSE or AVX in its code) and saves groups only at guest thread switches; in P the stub's routines use no
    vector register and the host kernel saves per thread; in I the inside's Rust code runs on a host whose C library
    picks AVX2 or AVX-512 forms of `memcpy` and `memset` that end in `vzeroupper` (which would clear the upper
    halves of the guest's ymm registers), so every call into the inside that runs Rust code saves the guest's
    offered groups with `XSAVEC` and restores them with `XRSTOR` (52 to 78 ns, the feature-switch research), the same
    save the thread switch needs. XN2's hostile-guest test fills ymm upper halves before a call that copies 64
    bytes or more and compares after.
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
  meaning's, else Kernel (the inside writes as the meaning does), counted "partial write form answered by the
  inside". `enter` with a level is rare; `fstp m80` is common in `long double` code, so a host that differs pays.
  XN1 runs the probe on an AMD machine too.
- The count: `lea c, [c + k]` at each run's start (no flag). A rep string instruction is an open question between
  two plans: docs/native.md section 4 says E does not count its rounds; the x86 meaning names a jump to itself, so
  `counted` counts every pass, and a pass is one element or a bulk run of at most 1 MiB, chosen by the direction
  flag, the trap flag, overlap and the range's permissions (x86 plan 5.6). Native code can match only by computing
  that choice again after the hardware ran the instruction. Proposed for the owner (section 9): a rep string's
  events are its elements, which both outputs know (rcx before less rcx after), `lea c, [c + t]` plus a flag-free
  subtraction natively, and which make every element boundary a distinct position on both outputs. Until ruled,
  XN1 builds the pass count from one shared function of the x86 core that the meaning's constants feed.
- TF: as docs/native.md 3.4 (routed `popf`/`iretq`, hardware TF equal to the guest's, steps resolved through the
  map). In P and I every host instruction of the call into the inside steps too until the inside clears TF: the
  handler resumes those silently ("single step through host instructions").

### 3.7 Stops, replay, self-modifying code, signals

- Stops: the machine by an interrupt, P and I by a host signal on the thread (run on an alternate signal stack,
  since the guest's rsp is the guest's). A stop at any host instruction resolves by the position map, exactly as on
  ARM64 (docs/native.md section 4). The handler copies the frame once; the interrupted rip it holds is only looked
  up in the position map (a value the map does not know ends the thread as corrupted, counted), and the thread
  resumes at a guest instruction boundary through the lookup, never at a host address read from the frame. A stop
  inside a `rep` string instruction: the hardware leaves rip on the instruction with rsi, rdi and rcx advanced, which
  is a guest instruction boundary with the instruction's progress in its registers; the thread resumes there in the
  instruction's twin, a piece holding that one instruction followed by a call into the inside, so the instruction
  ends and the stop is taken at its end ("stop deferred to the end of a string instruction"). This replaces
  docs/native.md section 4's DR0 breakpoint, which P and I cannot set without `perf_event_open` (refused here at
  `perf_event_paranoid` 4, costs-cloud.json).
- Replay to a logged position (docs/native.md 5.7): to stop a thread at (count N, guest pc P), its stop word in the
  area's read-only part becomes N (every other thread's holds a value no count reaches), and the inside asks for P's
  piece again with a stop point at P (the request's `stop` of section 9, part of the cache key). The new piece is
  loaded into the target thread's process only (its mirror entries and links pointed at it); the process's other
  threads run it too, and the check being against the running thread's own stop word is what makes only the target
  stop, as on ARM64. Before P's instruction the check, flag-free, with rcx borrowed into t, which is free at an
  instruction boundary:
  ```
  mov t, rcx
  mov rcx, seg:[stop]
  not rcx
  lea rcx, [rcx + c + 1]      ; rcx = c - stop word
  jrcxz 1f                    ; equal: the stop
  mov rcx, t
  <P's instruction> ...
  1: mov rcx, t
     <call into the inside, kind Stop>
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
- Code reading its own bytes reads guest memory, which holds the guest's bytes (translated code lives elsewhere and
  never reads them after translating). A jump table whose entry lands in the middle of an instruction goes through
  the lookup and translates from that byte.
- A signal handler that edits the saved registers: the frame is built by the kernel from the thread's block (the
  withheld registers' guest values merged from the register area), sigreturn loads it, and the thread goes on at its
  guest rip through the lookup, wherever that is. `call next; pop` and unwinders see guest return addresses, since
  every call pushes the guest address.
- vsyscall and vDSO: our kernel gives an x86-64 vDSO, guest code translated like any other (its `rdtsc` is the
  Kernel route); the legacy vsyscall page is not provided (x86 plan: as under `vsyscall=none`), so a call or jump to
  0xffffffffff600000 is a page fault at that address with rip there (section 3.3), as Linux gives under
  `vsyscall=none`. The host's own vDSO, vvar and vsyscall pages (here vvar and vdso at the top of the 47-bit space,
  vsyscall execute-only) are P's: they sit in the guest's host process above the guest's ceiling 2^44, out of the
  guest's mappings; a guest read there succeeds where Linux faults, which P names or prevents (P's plan).
- 32-bit code is never run: the decoder is 64-bit only; far transfers and `iretq` are answered and refuse 0x23 by
  name; `int 0x80` and `sysenter` are the meaning's general protection; a 32-bit ELF is the kernel's refusal; and
  `rt_sigreturn` with the frame's cs edited to 0x23 (which on Linux switches the thread into compatibility mode) is
  refused by our kernel's sigreturn with SIGSEGV, named "32-bit code segment in sigreturn, not provided" (the
  kernel's code, shared with the WebAssembly output: asked of the kernel track, since x86/lib.rs keeps no cs in the
  block today).

## 4. The three memory-access forms (way I only changes them)

The one place is `carry::access(way, operand, code)`; every route that emits a guest access calls it, implicit ones
included. Guest memory in I is a window at B, the GS base (per guest process), inside a slot of the P/I plan's
layout (2^36 bytes from 2^32; section 6). The guest's ceiling there is 2^32 in the first build (a larger exact
window needs the bound in a register, measured at XN2 if asked).
- None (machine, P): the operand as the guest wrote it (only the area segment's operands rewritten, section 3.1).
- Wrapping: the instruction re-encoded with the address-size prefix and `gs:`, so the hardware forms the address in
  32 bits and adds B: no instruction added. A guest `fs:`/`gs:` operand is `mov t, fs:[guest base]`, `lea t, [t +
  base]` when it has a base, and the access on `gs:[t + index*scale + disp]` in 32-bit form. Measured earlier
  (costs-cloud.json, stack exempt): +0.0% instructions, +1.8 to +4.3% wall on Granite Rapids, -7.8 to +5.0% cycles on
  AMD Zen 4, +2 to +4% on the laptop. Its named difference: an address at or above 2^32 lands at its value modulo
  2^32 inside the guest's own window, where the WebAssembly output faults (it cannot reach anything not the
  guest's).
- Exact: where the finder shows the flags dead within the straight-line run (static: 79% of memory operands here, a
  conservative count), `lea t, [operand]` (64-bit), `cmp t, fs:[limit]`, `jae out`, and the access on `gs:[t]`; where
  they may be live (21%), a flag-free probe with a stand-in v (u is the lookup's and free here): `lea t, [operand];
  rorx v, t, 32; mov v32, v32; lea v, [v*8]; not v; mov v, gs:[v + 1 - 2*PAGE]`, which reads B - 2 PAGE (a readable
  page of ours) when bits 32..63 of t are zero and else an address 8 to 2^35 bytes below it, in a reservation without
  access, then the access on `gs:[t]`; counted "exact check with live flags". A fault on either is the guest's fault
  at t, from the span. Measured earlier (stack exempt): X32 +11.0 to +15.2% wall (Granite Rapids), +10.0 to +25.4%
  cycles (Zen 4), +14 to +32% on the laptop.
- Guards: below B, one page without access, the probe's readable page under it, and 2^35 bytes without access under
  that; above B + 2^32, one page without access. Every access but the strings starts inside the window (wrapping: by
  the 32-bit address; exact: by the check) and reaches at most its own size past it, and no access is larger than a
  page (the largest, the guest's XSAVE image of 832 bytes, is the inside's), so one page catches a run over the top.
- Implicit accesses: `push`, `pop`, `call`, `ret`, `enter`, `leave` become explicit accesses through `access` on rsp
  (`mov gs:[esp - 8], src` then `lea rsp, [rsp - 8]`, the store the commit point and the `lea` its fix-up, section 9).
  `xlat` is re-encoded on `gs:`. The AVX2 gathers in the exact form go to the inside (rare; their lanes cannot be
  bounded by one probe), counted.
- Strings and `maskmovdqu`: their start is bounded first, then rdi is rebased. For each of rdi and rsi the instruction
  uses, the flag-free probe above on that register (wrapping and exact alike: a start at or above 2^32 faults on the
  probe, and the inside performs the instruction as the meaning does in that form: modulo 2^32, or the guest's
  fault; counted "string start outside the window"). Then `mov t, fs:[base]; lea rdi, [rdi + t]`, the copied
  instruction with a `gs:` override on its rsi side (the rdi side is ES, which cannot be overridden), and `mov t,
  fs:[negated base]; lea rdi, [rdi + t]`; the position map holds "rdi less B" while rebased (`Held::Rebased`). A run
  from a start inside the window moves one element at a time, so upward it faults on the guard page above B + 2^32
  and downward on the page below B; that fault goes to the inside, which in the wrapping form goes on modulo 2^32 as
  the meaning does and in the exact form is the guest's fault there. `maskmovdqu` (16 bytes at rdi) is re-encoded on
  `gs:` with the address-size prefix (wrapping) or after the probe (exact). Measured by the check
  (check-xn0/partial.out): with the run-1 sequence and no bound, a guest rdi of 2^40 wrote host bytes at B + rdi;
  section 7's way I tests hold this case.
- The stack's cost is not measured: the earlier figures exempted the stack, and this form rewrites every implicit
  stack access. XN2 measures it, and the alternative the P/I plan uses on ARM64 (rsp holding B plus the guest's rsp,
  so `push`, `pop`, `call` and `ret` run unchanged inside the window, a read of rsp as a value less B, and a write of
  rsp from another register rebased and checked), and keeps the cheaper that passes the way I tests.
- The page permissions inside the window are the host's protection of the window's pages, written by the kernel's
  mapping calls, 4 KiB as the guest's.

## 5. What the guest sees: differences named

- Way I, wrapping: the modulo landing above (section 4).
- P: the host pages in the guest's host process (section 3.7) and the read-only thread area: a guest read there
  succeeds where Linux faults, unless P keeps the guest away from them (P's plan decides and counts). P on a 4-level
  host: at most about 3,000 guest threads per process (section 3.3).
- P and I on a host with split-lock detection: a split locked access is slowed (or fatal) where the reference runs
  it.
- Everything else is the WebAssembly output's behaviour; the tests of section 7 hold it.

## 6. Ceilings and layout

The P/I plan's layout (its 3.1), with x86's parts: the guest area [LOWEST_ADDRESS, 2^44) in the machine and P; our
area from 2^44 to the host's own mappings near 2^47 (thread areas, code area, the mirror's second levels), and above
2^47 on a 5-level host where P and I place by hint; in I the slots of 2^36 bytes from 2^32, each holding one guest
process's window (2^32), its guards (2^35 and two pages below, one page above) and nothing else. No slot table: x86
has the mirror (section 3.3).
- The machine: 2^44 (the floor maps our area above the guest's half, user-accessible, so thread areas are reached
  from guest mode).
- P: 2^44.
- I: 2^32 in the first build.
Each is `Cpu::address_space` of that way; a request above it fails, counted, as the brief allows.

## 7. Tests

- Routing, static, per way: every decoded x86 form through the recording emitter, with this plan's classes (section
  3.1; docs/native.md 3.4's class "held by the hardware as presented" does not hold the thread area's segment or its
  base instructions here): a form added to the x86 crate is routed or the test fails, and the area segment's
  operands, `rdgsbase`, `wrgsbase` (and in I `rdfsbase`, `wrfsbase`, `fs:`) routed Copy fail it.
- Routing, differential: every decoded form run natively as routed and through the WebAssembly output from the same
  seeded state (all 64 bits of every register, every register collision, each access placed across a page with
  either page refusing, after seeded float flags with tracking on and off, TF and AC in the values popf takes),
  comparing every register, flag, control, memory byte and trap. Seed and cases per form are arguments.
- Count: the same count at every stop across look-ahead none, bounded and full on the benchmark set and layer 2's
  programs, the kernel test's hash with a stop at every counted event; the every-offset test with a stop at every
  host instruction of every piece, the lookup and the twins.
- Replay: a logged (count, pc) from the WebAssembly output reached natively and the reverse, on the benchmark set,
  at pcs in the middle of pieces, at a return site, and inside `rep movsb` passes.
- The shapes this plan was broken with, kept as tests: a direct branch into the middle of an instruction in the
  same piece; a jump table into the middle of an instruction; code reading its own bytes and code storing into the
  next instruction, from the same and from a sibling thread; a write to a code page through a shared mapping from
  another process and through the kernel; a signal handler editing rip, r10, r11, r15 and rflags; a jump and a call
  to 0xffffffffff600000 and to the vDSO; a far transfer to 0x23, `int 0x80`, `sysenter`, `rt_sigreturn` with cs
  0x23; `ret`, `jmp r` and `call r` to 2^47, 2^48, each of bits 48..63 set alone over a valid target,
  0xfff0000000401000, 2^63 and -1, with signal, si_code, address, rip and rsp compared; a stop inside `rep movsb`,
  `rep stosq` and `repe cmpsb`; an instruction straddling into a page that cannot run code; a 66-prefixed `jmp`,
  `call` and `ret`; `xsave` with edx:eax all ones beside a guest buffer of the offered size; `wrfsbase`,
  `wrgsbase`, `rdgsbase` and `mov r, gs` (each way); `enter 0, 3` and `fstp m80` across a page that refuses writes.
- Way I: no access outside the window succeeds in either form, through every operand shape: one byte past a
  mapping, 2^32 - 1 with 8 bytes, a SIB with a large index, `fs:` with a negative offset, a downward and an upward
  string with a large rcx, a string and `maskmovdqu` with rdi and rsi at 2^32, 2^32 + 2^35, 2^40 and -1, a push at
  the bottom of the stack, `enter` with a deep level; and the ymm upper halves kept across a call into the inside.

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
  1.6. The dynamic register count and the mirror's memory on busybox and python are recorded.
- XN2, the two checked forms (way I). Bar: section 7's way I tests pass for both forms; faults carry exact
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
    /// Forms whose host execution gives the container's chosen behaviour, found by the probe.
    type HostMatches;
    fn route(way: &Self::Way, i: &Self::Instruction, host: &Self::HostMatches) -> Route;
    /// As today, with the way and the rest of the straight-line part after it (flag liveness).
    fn carry(way: &Self::Way, before: &[(u64, Self::Instruction)], i: &Self::Instruction, bytes: &[u8],
        address: u64, after: &[(u64, Self::Instruction)], invariant: &Self::LoopInvariant, code: &mut HostCode);
    fn call(way: &Self::Way, code: &mut HostCode, call: InsideCall);
    /// The replay check of docs/native.md 5.7, placed before the instruction at the stop point.
    fn stop_check(way: &Self::Way, code: &mut HostCode);
    fn movable(way: &Self::Way) -> core::ops::Range<u64>;
    /// Points a loaded exit at `distance` bytes from the end of its branch, as the loader links it.
    fn retarget(code: &mut [u8], exit: &DirectExit, distance: i64) -> bool;
    /// Bytes past the start's page the emitter must be given (the longest instruction less one).
    const PAST_PAGE: usize;
    // route's other methods unchanged
}
```
The translation request gains the stop point of docs/native.md 5.7, part of the cache key: `stop: Option<u64>`, the
guest pc before whose instruction `stop_check` goes. In gcpu/position.rs: `Operand` gains `segment: Option<Segment>`
(`Fs`, `Gs`) and `address_bits: u8` (32 or 64); `DirectExit::retarget` and `BranchTable::retarget` hold the branch's
displacement as `{ at: u8, bits: u8 }` in bytes, not ARM64's instruction field; `Fixup` gains `Offset { register:
u8, by: i32 }` (rsp after a split push); `Held` gains `Rebased(u8)` (rdi holding the guest value plus B) and
`InScratch { register: u8, scratch: u8 }` (rcx in t over the replay check); `Translation` gains `past_page: bool`.
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
