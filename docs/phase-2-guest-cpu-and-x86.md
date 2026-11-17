# wcontainers, phase two: one guest CPU interface, and x86-64

Status: plan written 2026-11-17 from the owner's brief (decisions of 2026-11-16) and the map of today's
ARM64 assumptions, and revised after each of the four independent checks of X0 (2026-11-17). Nothing of it
is built yet. The research and the two experiments of milestone X0 are done; what they found is in section 2.

Words used here (phase one's words still hold: guest, host, engine, look-ahead, piece):
- **guest CPU**: the kind of processor whose machine code is translated: ARM64 today, x86-64 next, RV64GC
  later.
- **CPU crate**: the crate that holds everything true of one guest CPU (`wcontainers-arm64`,
  `wcontainers-x86`, later `wcontainers-riscv`).
- **the interface**: the crate `wcontainers-guest-cpu`, which says what a CPU crate supplies. The
  translator, the test harness and later the kernel are written once against it.
- **reference CPU**: a real processor of the guest's kind. For x86-64 it is an Intel Core i7-8750H
  (Coffee Lake) on Ubuntu, reached over SSH; later also an i9-9900K (also Coffee Lake).
- **the seam map**: `pyro-parked/wcontainers-scratch/orchestrator/guest-cpu-seam-map.json`, the list of
  187 places where code assumes ARM64 today. Appendix A lists every place of kind b and c in its
  translator and tests areas and where this plan covers it; appendix B does the same for every item of
  those areas' interface surface lists.

Every figure below says whether it was measured or is only expected.

## 1. What phase two delivers

1. **The interface** (section 3): one Rust trait a CPU crate implements, with associated types and
   constants, resolved at compile time. The translator becomes generic over it. One build of the
   translator per guest CPU; nothing is chosen at run time, so the browser downloads only the translator
   for the guest it runs and run speed does not change.
2. **ARM64 moved onto the interface first**, with its output byte for byte the same as before (the
   benchmark set and all nine real binaries of M5, in both settings of "optimise for"), the whole test
   suite passing and the benchmarks unchanged within noise. No x86 code is written before that.
3. **A shared test harness** (section 7): the three test layers, the runners, the reference transport and
   the browser build written once against a test-side interface, with ARM64's tests unchanged in what they
   generate and check.
4. **x86-64 user-mode code**, 64-bit only (section 5), in stages, proven against the real CPU at every
   stage with the same three layers as ARM64.
5. **The kernel's side designed** (section 6), and built as the last milestone, after the kernel track's
   own milestones.

Out of phase two: 32-bit x86 (deferred with Direct3D 9), AVX-512, memory ordering between guest threads
running in parallel (section 8), and building RISC-V (it is designed on paper only, section 4).

Done means: every milestone of section 9 has met its pass bar, and the long runs at the end of the track
(layer 1 at 10,000 cases a form, layer 2 at one million programs, layer 3 at 100,000) have zero mismatches
for x86-64, with ARM64's own long runs still passing.

## 2. What was settled by looking (milestone X0)

### The decoder: `iced-x86`

- Version 1.22.0 on crates.io (checked 2026-11-17). Licence MIT, read from the crate's own `Cargo.toml`
  (`license = "MIT"`) and from `cargo info`. Its README says it supports `#![no_std]` and WebAssembly.
- Its features: `std` or `no_std` (one of the two must be on), `decoder`, `encoder`, `block_encoder`,
  `op_code_info`, `instr_info`, `code_asm`, the formatters (`gas`, `intel`, `masm`, `nasm`, `fast_fmt`),
  and `no_vex`, `no_evex`, `no_xop`, `no_d3now`, `mvex` to leave encodings out. `no_std` pulls in
  `lazy_static` with `spin_no_std`.
- Built for `wasm32-unknown-unknown` (Rust 1.99.0, release, `panic = "abort"`) from a `#![no_std]` library
  with `default-features = false, features = ["no_std", "decoder", "instr_info", "encoder",
  "op_code_info"]`: it builds. So it builds without the standard library.
- What it adds to a wasm32 release build, measured on 2026-11-17 (Mac load 15 to 21) with a small cdylib
  that decodes a buffer and uses the decoded fields, after `wasm-tools strip`, against the same cdylib
  without it:
  - `no_std` + `decoder`: 227,228 bytes raw, 60,705 gzip -9, 47,058 brotli -q 11.
  - `no_std` + `decoder` + `instr_info`: 353,374 raw, 82,721 gzip, 64,947 brotli.
  - For scale, today's browser translator (`wcontainers-browser`, ARM64, release, stripped) is 2,170,386
    bytes raw, 412,059 gzip and 312,302 brotli (measured the same day, load 21 to 28). So the decoder alone
    adds about 15% to the compressed download, before anything ARM64-only leaves the x86 build.
- Decision: the shipped x86 crate uses `no_std` + `decoder` only. The translator learns what an
  instruction reads and writes by recording its meaning (as for ARM64), so `instr_info` is not shipped.
  `encoder`, `op_code_info` and `code_asm` are used by the tests only (forms list, case generation, layer 2
  programs).
- With `decoder` only, iced has no `Instruction::encoding()`, `cpuid_features()` or their `Code` forms
  (they sit behind `#[cfg(feature = "instr_info")]`, `instruction.rs` line 2815 and `code.rs` line 44560 of
  1.22.0). The shipped crate still needs, per `Code`, which claim group it belongs to (section 5.10).
  That comes from iced's own tables at build time, never a hand copy: the
  build script of `wcontainers-x86` has iced-x86 as a build-dependency with `std` and `instr_info`, and for
  each of the 4,936 `Code` values writes one byte: its claim group (default, the AVX setting, never) from
  its `cpuid_features()` against the claim lists. The claim lists are
  one source file that the build script and the crate both include. Cargo's resolver 3 (the workspace's)
  keeps a build-dependency's features apart from the normal dependency, so the shipped decoder stays
  `decoder` only. The table is about 5 KB before compression (expected; measured at X3). A test checks the
  table against iced's `instr_info` answers for every `Code`.

### Fused multiply-add in WebAssembly

- Read in the relaxed SIMD proposal's overview (copy in `pyro-parked/wcontainers-scratch/x86/sources`):
  `f32x4.relaxed_madd`, `f64x2.relaxed_madd` and the `nmadd` forms give either the fused result (one
  rounding) or the unfused one (two roundings), "if supported by hardware". There is no scalar fused
  multiply-add in WebAssembly; the scalar form is lane 0 of the vector one.
- The same overview quotes the specification's rule for such operators: "each environment globally
  chooses a fixed projection for each operator". So within one engine on one machine the choice is fixed:
  a start-up probe that sees it fuse may rely on it fusing everywhere in that environment. This is the
  probe phase one already has (`Host::fused_multiply_add`), part of the cache key.
- So WebAssembly cannot promise an exact fused multiply-add; a host can be found to give it. Decision, as
  the brief says for that case: the x86 FMA instructions are a software slow path wherever the probe says
  the host does not fuse ("software fused multiply-add", counted and warned, as on ARM64), and the FMA
  feature is claimed only with the AVX setting (section 5.10).

### Stopping a real x86 CPU after exactly one instruction

How others do it, read in their sources or descriptions:
- **sandsifter** (Domas, 2017; `injector.c`, BSD-3): sets the trap flag (`TF`, rflags bit 8) with a
  `pushfq; or; popfq` preamble just before the instruction, and also places the instruction at the end of
  a page so its length shows as a page fault on the next page.
- **RISU** (QEMU's random instruction tester; EPL-1.0, read only): its x86 port marks each operation with a
  `UD1 r, eax` instruction (`0F B9 C0+r`, the operation number in the reg field) placed after the test
  code; the `SIGILL` handler reads it and steps over its 3 bytes (`risu_i386.c`, `get_risuop` and
  `advance_pc`, read in run 3 from the mirror github.com/rth7680/risu at commit eed2249, 2024-11-16, since
  git.linaro.org answered 504). That is the way our ARM64 harness relies on zeros being `udf #0`; like any
  filler it does not catch jumps.
- **Debuggers** (`ptrace(PTRACE_SINGLESTEP)`) use the same trap flag.

Zero bytes are a real instruction on x86 (`add [rax], al`), so the ARM64 trick does not carry over, and a
trap filler after the instruction does not catch jumps. The method chosen and proven (experiment below):
**the trap flag set in the signal frame the reference runner returns through.** The runner enters the guest
state from a signal handler (as on ARM64): it writes the guest's registers into the frame, sets `TF` in the
saved rflags and returns. The CPU runs one instruction and raises a debug trap, which arrives as `SIGTRAP`
with `si_code` `TRAP_TRACE` (2) and rip at the next instruction, or at the target of a jump. The handler
reads the registers back and returns to the runner's own saved state.

Two kinds of instruction do not stop that way, both seen on the reference (below), and the runner handles
both (section 7.3):
- `mov ss, r/m16` (valid in 64-bit user mode) holds the debug trap back for one instruction, as the manual
  says. The `int3` filler after it then runs and stops the CPU with `SIGTRAP`, `SI_KERNEL` (128), rip at
  CODE + length + 1. So a stop by `int3` exactly one byte past the instruction's end means "the instruction
  ran, fell through, and its trap was held back".
- A system call (`syscall`, and `int 0x80`, Linux's 32-bit system call gate, which also works from 64-bit
  code) runs the call and gives no `TRAP_TRACE`; the filler stops it the same way. The runner never lets a
  case's system call run: a seccomp filter turns it into `SIGSYS` first.

### The two experiments on the x86 machine (scratch, not in the repository)

Machine: Intel i7-8750H, 12 threads, Ubuntu 20.04.3 with a mainline Linux 7.3.4 kernel. Load 0.03 at the
first run; it is not always idle: another track's measurement (`~/ordering-cost`, a wasmtime job) runs
there at times (load about 1.0 seen on 2026-11-17), so every timing there records `/proc/loadavg`. Its CPU
reports `avx2`, `fma` and `fsgsbase`, and no AVX-512. This track installed nothing on it; Rust (`~/.rustup`,
`~/.cargo`, rustc 1.100.0) was already there from before the track and is not used by it. The experiments
are static binaries copied to `~/wcontainers-x86/proof`.

1. **A static x86-64 program built on the Mac runs there.** `zig cc -target x86_64-linux-musl -static`
   (zig 0.16.0 from the Nix shell) built `hello.c`; `scp` copied it; `ssh ... ~/wcontainers-x86/proof/hello`
   printed "hello from a static x86-64 program on x86_64 7.3.4-070304-generic" and the exit status 7 came
   back. Files: `pyro-parked/wcontainers-scratch/x86/reference-proof/hello.c`.
2. **One arbitrary instruction from given registers, stopped right after it.** `stepper.c` in the same
   folder takes the instruction's bytes and any register values, runs it by the method above and prints
   every register back. 22 cases (`cases.sh`, `cases2.sh`; outputs `cases.out`, `cases2.out`) gave what
   the Intel manual says, among them:
   - `add rax, rbx` with -1 and 1: rax 0, rflags CF PF AF ZF set, rip +3, `TRAP_TRACE`.
   - `jmp rel32 +0x100`: rip at the target, `TRAP_TRACE`; nothing at the target ran.
   - a load from 0x1000: `SIGSEGV`, `SEGV_MAPERR`, si_addr 0x1000, rip at the instruction.
   - a load from 0x8000000000000000 (not canonical), a `ret` to it, and `movaps` from an address that is
     not a multiple of 16: each `SIGSEGV` with `SI_KERNEL` (128) and si_addr 0, rip at the instruction,
     rsp not moved by the `ret`.
   - `div rcx` with rcx 0: `SIGFPE`, `FPE_INTDIV`; `ud2`: `SIGILL`, `ILL_ILLOPN`; `int3`: `SIGTRAP` with
     `SI_KERNEL` and rip after it (so the trap flag's stop and a breakpoint are told apart by `si_code`).
   - the two zero bytes `00 00`: one `add [rax], al`, rip +2.
   - `addps`, and `vextractf128 xmm0, ymm1, 1` with the upper half of ymm1 set: vector registers and the
     upper halves of the ymm registers written and read back through the XSAVE area of the frame.
   - `mov rax, fs:[0x28]` with the FS base set by `arch_prctl(ARCH_SET_FS)` in the handler just before
     returning to the guest, and the runner's own FS base set back first thing in the second handler.
   - every one of the 16 general registers, rflags (with DF) and MXCSR set and read back unchanged by a
     `nop`.
   - `rep movsb` with rcx 3: one element per step (rcx 2, rip unchanged): the trap flag stops a repeated
     string instruction after each element.
   - `pushfq`: the pushed word has `TF` set (0x100) as well as IF.
   - `syscall` (getpid): the call ran (rax the process id, rcx the address after it) and no `TRAP_TRACE`
     came at all; the `int3` filler after it stopped the CPU (`SI_KERNEL`, rip CODE + 3). So system calls
     must be stopped by seccomp before they run, as the ARM64 reference already does.

   Repeated in run 2 (2026-11-17, x86 load 0.07), after the independent check found them:
   - `mov ss, eax` with 0x2b: `SIGTRAP`, `SI_KERNEL`, rip CODE + 3: the trap was held back and the filler
     stopped the CPU.
   - `int 0x80` with eax 20 (i386 getpid): the call ran (eax the process id) and the filler stopped the CPU
     at CODE + 3, as for `syscall`. `int 0x81`: `SIGSEGV`, `SI_KERNEL`, rip at the instruction (general
     protection).
   - `int1` (`F1`): `SIGTRAP` with `TRAP_BRKPT` (1), rip after it. `int 3` (`CD 03`): `SIGTRAP`, `SI_KERNEL`,
     rip after it, as `int3`.

   Run 3 (2026-11-17, x86 load 0.00 to 0.89), after the second independent check, with `stepper3.c` (the
   same stepper, whose code area can be mapped elsewhere through `CODE_BASE` in the environment), the
   check's seccomp variant `stepper_sc.c`, and `approx.c`, `approx_dump.c`, `approx_formula.c` and
   `approx_mxcsr.c` for the approximate reciprocals:
   - A `call` to a non-canonical target pushes first: `call rax` (`FF D0`) with 0x8000000000001000 or
     0x800000000000, `call r8`, `call [rdi]` with the target 0x8000000000000000 in memory, and `call rel32`
     to 0x800000000000 from code at 0x7fffffff8000 each give `SIGSEGV`, `SI_KERNEL`, rip at the
     instruction, rsp unchanged, and the return address written at rsp - 8. `jmp rax` and `jmp rel32` with
     the same targets write nothing. With rsp 0x1008 the push's page fault at 0x1000 comes instead; `call
     [rdi]` with rdi 0x3000 and rsp 0x1008 faults at 0x3000, so the target's read comes before the push.
   - Accesses that reach the non-canonical range from a canonical start: an 8-byte load at 0x7ffffffffffc
     and a 4-byte load at 0x7ffffffffffe give `SI_KERNEL` (general protection); an 8-byte load at
     0x7ffffffff000 and a 1-byte load at 0x7fffffffffff give `SEGV_MAPERR` at that address.
   - `syscall` (stopped by seccomp's `SIGSYS`) from rflags 0x2C3: r11 0x3C3, so the copy of rflags in r11
     carries the trap flag the stepping set.
   - `div rcx` with rdx:rax 1:0 and rcx 2: rax 0x8000000000000000, rdx 0; with rdx 2: `SIGFPE`,
     `FPE_INTDIV`, rip at the instruction.
   - `rcpps` of 3.0 gives 0x3EAAA000 in every lane; `rsqrtss` of 2.0 gives 0x3F34F800. Over all 2^32 inputs:
     `rcpss` results depend only on the top 11 mantissa bits, `rsqrtss` results on the top 10 and the
     exponent's parity, both have their low 11 result bits zero, the packed forms equal the scalar ones in
     every lane, and the closed formula of section 5.8 gives every one of the 2^32 results of each, in MXCSR
     0x1F80 and in 0xFFC0 (24 s for the analysis, 73 s and 145 s for the two formula checks).

   Run 4 (2026-11-17, x86 load 0.00 to 0.07, except 36.5 from another job during the AC cases, which time
   nothing), after the third independent check, with `stepper4.c` (`stepper3.c` that also writes and reads
   the x87 component of the frame; made by `scripts/make_stepper4.py`), `cases4.sh`, `cases5.sh` and
   their outputs, and `cases6.out`:
   - The x87 component written into the frame (FCW, FSW with TOP, the abridged tag byte, the eight
     register slots, `xstate_bv` bit 0) is what the CPU runs from, and it reads back: `fadd st0, st1` with
     1 and 2 gives 3; `fdiv` 1 / 3 gives 0x3ffd:aaaaaaaaaaaaaaab at precision 64 and
     0x3ffd:aaaaab0000000000 with FCW 0x007F (precision 24), FSW 0x220 (precision exception, C1); `fsqrt`
     of 2 gives 0x3fff:b504f333f9de6484 and, at precision 24, 0x3fff:b504f30000000000; `fld1` on an empty
     stack leaves TOP 7 and tag byte 0x80, and from TOP 3 leaves TOP 2, tag 0x0c and the old ST0 in slot 1,
     so the frame's slots are in stack order and its tag byte in physical order; a `nop` with every slot,
     FCW 0x0C7F and FSW 0x3800 set gives them all back.
   - Far transfers: `call far [rdi]` with selector 0x33 pushes 16 bytes (return address, then 0x33) and
     lands; selectors 0, 0x2b (data) give general protection with nothing pushed; 0x23 lands, in 32-bit
     mode; a non-canonical offset with 0x33 gives general protection after the 16-byte push. `jmp far`
     with 0x33 lands, with 0x2b faults. `retfq` with 0x33 lands with rsp up 16, with 0x2b faults with rsp
     unchanged. `iretq` with rip, CS 0x33, rflags 0x8C3, rsp, SS 0x2b lands with rsp loaded and rflags
     0xAC3 (IF kept); with SS 0x33 it faults.
   - With AC (rflags bit 18) set, an 8-byte load at an odd address gives `SIGBUS`, `BUS_ADRALN`, rip at
     the instruction, and the same load at an aligned address runs: Linux enables alignment checking in
     user mode.

   Run 5 (2026-11-17, x86 load 3.6 to 5.4 from another job, nothing timed), after the fourth independent
   check, with `stepper4`, `cases7.sh`, `cases8.sh`, `sweep15.sh` and their outputs, and `brk.c` run in
   Docker on the Mac:
   - The 15-byte limit wins over every other decoding rule. Fifteen bytes run or trap as their body says:
     `66`×14 + `nop` lands at CODE + 15, `rex`×12 + `mov rax, rbx` lands, and `66`×14 + `06` (push es, no
     instruction in 64-bit mode), `66`×12 + `ud0 eax, eax` and `f0`×14 + `nop` (lock of a nop) give
     `SIGILL` at the instruction. One more prefix byte in each of them, and 17 bytes of repeated segment
     prefixes before `mov rax, rbx`, give general protection (`SIGSEGV`, `SI_KERNEL`, address 0, rip at the
     instruction). iced reports all of the too-long ones as `INVALID` with length 15, the same as the
     15-byte `66`×14 + `06`; section 5.2 says how the two are told apart.
   - The CPU counts an invalid opcode's length by its own tables, not iced's (`cases8.sh`, and
     `sweep15.sh`: every opcode of the one-byte, `0F`, `0F 38` and `0F 3A` maps placed after `26` prefixes
     so it ends at byte 15, against the opcode alone; 1,024 cases, output `sweep15.out`, classed against
     iced in `sweep15.classes`). `66`×13 + `0F 24 C0` (16 bytes if `C0` were its ModRM) gives `SIGILL`, so
     `0F 24` counts no ModRM; `66`×12 + `0F 38 FF` then the filler gives general protection, so it counts
     one. Of the opcodes iced decodes as `INVALID` there, 24 count nothing after the opcode (listed in
     section 5.2) and 539 count at least one byte.
   - `clflush [rax]` and `clflushopt [rax]` with nothing mapped at rax give a page fault (`SEGV_MAPERR`,
     the address); with a mapped address they run; with a non-canonical one, general protection.
     `prefetchnta [rax]` (`0f 18 00`) with nothing mapped runs.
   - An ARM64 `brk #0x123` under Linux gives `SIGTRAP` with `TRAP_BRKPT` (1).

   What this teaches the reference runner (section 7.3): write every vector register and the whole x87
   component (FCW, FSW, the abridged tag byte, FOP and the last-instruction pointers as zero, the eight
   slots in stack order) (else the runner's own values leak in), set the x87, SSE and AVX bits of the
   frame's XSAVE header (`xstate_bv` bits 0, 1 and 2) or the kernel loads their first state, read a
   component as its first state when its `xstate_bv` bit is clear, find the
   upper ymm halves at the offset CPUID leaf 0xD sub-leaf 2 gives, mask `TF`, `RF` (bit 16) and `IF` out of
   the comparison of rflags, and mask `TF` out of every copy of rflags an instruction makes (the word
   `pushf` stores, r11 after `syscall`).
3. **Round trips.** A new SSH connection from the Mac: 0.24 to 0.32 s, five runs of `time ssh ... true`
   (Mac load 9.2, 2026-11-17, run 3; the first run saw about 344 ms at Mac load 57; the fourth independent
   check 0.24 to 0.40 s at Mac load 34). Run 5, with the x86 machine at load 5 to 7 from another job and
   the Mac at 28: 0.92 to 1.19 s in one set of five, 0.27 to 0.48 s in the next. A new process on the
   x86 machine: `bash -c 'time (for i in $(seq 1000); do ./stepper 90; done > /dev/null)'` took 0.93,
   1.25 and 1.25 s, so about 1 ms a process; the same loop over the trivial static `hello` took 0.86, 0.88
   and 0.94 s (x86 load 0.89, raised by the experiment just before; CPU governor `powersave`). The first
   run's figure of 410 ms for 1,000 processes had no recorded method and could not be repeated; it is
   withdrawn. So the reference must keep one connection open and stream cases through it, and run many
   cases per process (section 7.3).

## 3. The guest CPU interface

### 3.1 Shape

`wcontainers-guest-cpu` holds one trait, `Guest`, the `Emitter` trait meanings write to, and the plain
types both use (register sets, accesses, flow, stack moves). A CPU crate defines one zero-sized type and
implements `Guest` for it: `wcontainers_arm64::Arm64`, `wcontainers_x86::X86_64`, later
`wcontainers_riscv::Rv64`. The translator's public functions take the guest as a type parameter:

```rust
pub fn translate<G: Guest>(blob: &[u8], code: Range<u64>, starts: &[u64], look_ahead: LookAhead,
    optimise: Optimise, host: G::Host, offer: G::Offer) -> Result<Translation, Error>;
pub fn thread_module<G: Guest>() -> Vec<u8>;
```

`translate_omitting` and `translate_in_modules` take the same type parameter and the same `host` and `offer`
arguments. Every use is monomorphised; there is no `dyn` anywhere in the translator, so nothing is looked up
at run time and nothing is paid at translate time either beyond what ARM64 pays today. The translator crate
depends on `wcontainers-guest-cpu` only, never on a CPU crate. The program that ships (the browser's cdylib,
the host runner) names the one guest it is built for. The kernel never depends on a CPU crate (section 6):
it stays `no_std` and CPU-free, and what it needs of a CPU is its own architecture personality.

The interface crate depends on `wasm-encoder` (meanings write WebAssembly through its `InstructionSink`),
`zerocopy` (the `Block` bound) and `wcontainers-abi` (only for `PAGE_SIZE`), all already in the workspace.
Like `wcontainers-arm64` and the translator today it is a `std` crate. Besides the trait and the plain types
it holds what is generic over any guest and used by both the translator and the tests: `Registers`, the
`Effects` type and the recorder that finds an instruction's effects by running its meaning against a
recording emitter (today's crates/wcontainers-arm64/src/effects.rs, moved and made generic:
`wcontainers_guest_cpu::effects::<G>(&i) -> Effects<G>`, with `Effects::flags: Option<G::Flags>`).

### 3.2 The trait

Comments say what each item is for; section 3.3 gives the reason for each choice.

```rust
/// Everything the translator needs that is true of one guest CPU only.
pub trait Guest: Sized + 'static {
    // --- identity ---
    /// Names the guest in the description section and the cache key.
    const ID: GuestId; // Arm64 = 1, X86_64 = 2, Rv64 = 3

    // --- the instruction unit ---
    /// Every instruction starts at a multiple of this many bytes: 4, 1, 2.
    const ALIGN: u64;
    /// The longest instruction in bytes: 4, 15, 4.
    const MAX_LENGTH: u64;
    type Instruction: Copy + core::fmt::Debug;
    /// The instruction at the start of `code` (which runs to the end of the code range), with its length;
    /// `None` when it would run past the end. An encoding of no instruction is an instruction whose
    /// meaning traps.
    fn decode(code: &[u8]) -> Option<(Self::Instruction, u8)>;
    /// The detail a trap reports for the instruction at the start of `code`: the 32-bit word on ARM64,
    /// the first 8 bytes on x86 (fewer past the end, zero-filled), the 16- or 32-bit unit on RV64GC.
    fn trap_detail(code: &[u8]) -> i64;

    // --- registers ---
    /// A general register as meanings name it.
    type Reg: Copy + Eq + core::fmt::Debug;
    /// A register of the second file (SIMD and floating point, MMX, ymm upper halves).
    type Vector: Copy + Eq + core::fmt::Debug;
    /// One entry per register bit used, sorted by `Bit::bit`; bits not listed are unused (ARM64 36 to 63).
    const BITS: &'static [Bit];
    /// The register's bit; `None` for a hard zero register (ARM64 xzr, RISC-V x0).
    fn reg_bit(r: Self::Reg) -> Option<u32>;
    fn vector_bit(v: Self::Vector) -> u32;
    /// The register a general-register bit names.
    fn bit_reg(bit: u32) -> Self::Reg;
    /// The stack pointer's bit: ARM64 31 (sp), x86 4 (rsp), RV64 2 (x2, by the ABI only).
    const SP_BIT: u32;
    /// Registers passed into and out of pieces, in order, each list including the stack pointer.
    const PASSED_IN: &'static [u32];
    const PASSED_OUT: &'static [u32];

    // --- flags (all empty on a CPU without flags, which compiles the flag machinery away) ---
    type Flag: Copy + Eq + core::fmt::Debug;
    /// The flags the translator tracks lazily, each with its bit in the saved flags word and its register
    /// bit, in the order the flags word is put together. The flags word holds these and nothing else
    /// (x86's DF and the bits rflags always has are not in it, section 5.1).
    const FLAGS: &'static [(Self::Flag, u32, u32)];
    /// How flags were last set: a shape with the locals holding its operands.
    type Flags: Copy + core::fmt::Debug;
    /// A shape without its locals: the key of the out-of-line flags-word functions.
    type Shape: Copy + Eq + core::hash::Hash + core::fmt::Debug;
    /// The shapes worth an out-of-line flags-word function, in function order.
    const SHAPES: &'static [Self::Shape];
    fn shape(flags: Self::Flags) -> Self::Shape;
    /// The local types a shape keeps, and the shape rebuilt over fresh locals of those types.
    fn shape_locals(shape: Self::Shape) -> &'static [ValType];
    fn with_locals(shape: Self::Shape, locals: &[u32]) -> Self::Flags;
    /// The inverse of `with_locals`: the locals holding a shape's operands, in `shape_locals` order. The
    /// translator pushes them as the arguments of the shape's flags-word function, and copies them pair by
    /// pair into the shape's canonical locals. ARM64: an addition gives `a`, then the value subtracted or
    /// `b`, then the carry when it is a local; logic gives the result (today's full.rs:2176-2193 and
    /// 2664-2690, which push and copy exactly these).
    fn locals_of(flags: Self::Flags) -> Locals;
    /// Work out into `temporaries` (two locals of the type of the shape's first local) what a shape keeps
    /// only implicitly, and return the shape with every local filled in (ARM64: `b` when only the value
    /// subtracted was kept, and the result; today's `completed`).
    fn complete(sink: &mut InstructionSink<'_>, flags: Self::Flags, temporaries: [u32; 2]) -> Self::Flags;
    /// The flags a shape sets; the others keep their earlier value (x86 inc and dec keep CF).
    fn sets(flags: Self::Flags) -> Registers;
    /// Push one flag, as an i32 0 or 1, for flags set this way.
    fn flag_value(sink: &mut InstructionSink<'_>, flags: Self::Flags, flag: Self::Flag);
    type Cond: Copy + core::fmt::Debug;
    /// Push whether a condition holds as one comparison of the shape's operands, or return false.
    fn condition_direct(sink: &mut InstructionSink<'_>, flags: Self::Flags, cond: Self::Cond) -> bool;
    /// Push whether a condition holds, from the flags one by one.
    fn condition_from_flags<E: Emitter<Self>>(e: &mut E, cond: Self::Cond);
    /// Marks "the caller reads the flags" in a piece's expected-return parameter: a bit no return
    /// address can have. ARM64 1; x86 1 << 62; 0 for a CPU without flags.
    const FLAGS_WANTED: i64;

    // --- the thread block and controls ---
    /// The saved register block the kernel sees (`wcontainers_abi::RegisterBlock` on ARM64).
    type Block: zerocopy::FromBytes + zerocopy::IntoBytes + zerocopy::Immutable + Copy;
    /// Where pc and the flags word sit in the block, and the stop flag after it.
    const PC_OFFSET: usize;
    const FLAGS_WORD_OFFSET: usize;
    const STOP_FLAG_OFFSET: usize;
    const BLOCK_SIZE: usize;
    /// Bytes after the block that only the CPU side uses (ARM64: FPSR tracking and the exclusive monitor).
    const PRIVATE_BYTES: usize;
    /// A register kept only in the thread's state, never in a local.
    type Control: Copy + Eq + core::fmt::Debug;
    fn control_ty(c: Self::Control) -> ValType;
    /// Byte offset from the start of the block (into the block or the private bytes).
    fn control_offset(c: Self::Control) -> usize;
    /// The registers a thread starts with when made for a flat blob (x86: MXCSR 0x1F80, FCW 0x037F,
    /// rflags 0x202).
    fn initial_block(pc: u64, sp: u64) -> Self::Block;

    // --- floating point modes ---
    /// Push an i32 not zero when floating point must take the exact path now.
    fn float_slow<E: Emitter<Self>>(e: &mut E);
    /// Controls whose write makes an earlier `float_slow` answer stale.
    const FLOAT_CONTROLS: &'static [Self::Control];

    // --- hooks the meanings call ---
    type Hook: Copy + Eq + core::fmt::Debug;
    /// Name, parameter and result types, and the register bits the hook reads or writes in the thread
    /// block (written back before the call and loaded after). Function indices follow the translator's
    /// own hooks, in this order.
    const HOOKS: &'static [HookType];
    fn hook_index(h: Self::Hook) -> usize;

    // --- traps ---
    type Trap: Copy + core::fmt::Debug;
    /// The generic stop a trap is, and where its detail comes from.
    fn trap_kind(t: Self::Trap) -> (TrapKind, TrapDetail);
    /// Slow paths of this CPU's own, numbered after the translator's (from 64), with fixed names.
    type Reason: Copy + Eq + core::fmt::Debug;
    const REASONS: &'static [(Self::Reason, &'static str)];

    // --- what the host's engine does where WebAssembly leaves it open ---
    type Host: Copy + Eq + core::hash::Hash + core::fmt::Debug;
    fn probe_module() -> Vec<u8>;
    fn host_from_probe(answer: i32) -> Self::Host;
    /// Bytes of the host answer for the cache key.
    fn host_key(host: Self::Host) -> u32;

    // --- what the container offers beyond the default claims ---
    /// A container setting that changes what the CPU claims, given with every translation: x86 the AVX
    /// setting (`Offer { avx: bool }`), `()` for ARM64 and RV64GC.
    type Offer: Copy + Eq + core::hash::Hash + core::fmt::Debug;
    /// Bytes of the offer for the cache key and the description section (0 for `()`).
    fn offer_key(offer: Self::Offer) -> u32;

    // --- the host side of this CPU's own hooks ---
    /// What the runner does when a piece calls a hook whose `HookType::body` is `Cpu`: the work, on the
    /// thread's state (`BLOCK_SIZE + PRIVATE_BYTES` bytes from the block), with the hook's arguments.
    /// ARM64's float hook is `exact::run` behind it; x86's float and x87 hooks likewise. Every runner (the
    /// host runner, the browser's cdylib) calls this and nothing CPU-named. Hooks are a runner's job,
    /// never the kernel's: the runner owns the WebAssembly instance whose imports they are.
    fn run_hook(hook: Self::Hook, args: &[i64], state: &mut [u8]) -> HookDone;

    // --- what a memory fault is on this CPU ---
    /// The memory fault hook reported `address` (the first byte not allowed) for the instruction at the
    /// start of `code`, with the thread's registers exact in `block`: the fault the CPU gives for it.
    /// ARM64 and RV64GC always `Page`; x86 `GeneralProtection` when the access that faulted reaches a
    /// non-canonical address (section 5.4). The runner calls this before it tells the kernel about the
    /// fault, so the kernel needs no CPU crate.
    fn memory_fault(address: u64, code: &[u8], block: &Self::Block) -> MemoryFault;

    // --- meanings ---
    fn emit<E: Emitter<Self>>(e: &mut E, i: &Self::Instruction);

    // --- facts about one instruction, for the analyses ---
    fn flow(i: &Self::Instruction) -> Flow<Self::Reg>;
    /// Up to two memory accesses, in the order the instruction makes them.
    fn accesses(i: &Self::Instruction) -> [Option<MemoryAccess<Self::Reg>>; 2];
    fn stack_move(i: &Self::Instruction) -> StackMove;
    /// The blob offset of an address the instruction `i` at blob offset `at` builds into a register (adr,
    /// adrp + add, lea [rip + d], auipc + addi). `blob`, `code` and `before` as for `jump_table`. ARM64
    /// decodes the word at `at - 4` from the blob whenever its 4 bytes lie in `code`, whatever path
    /// reached the add, exactly as full.rs:561-581 does today (`Code::inside`, `get`), so its output does
    /// not change. x86 needs only `i`. RV64GC walks `before` back one instruction, since with 2- and
    /// 4-byte instructions the one before cannot be found by reading the blob backwards.
    fn built_address(at: u64, i: &Self::Instruction, blob: &[u8], code: Range<u64>,
        before: impl Fn(u64) -> Option<(u64, Self::Instruction)>) -> Option<u64>;
    /// The targets of the jump through `reg` at blob offset `at`, made from a switch statement, as blob
    /// offsets, each once, in table order. The CPU gathers the instructions before the jump itself:
    /// `before(o)` is the function's instruction that falls through into offset `o` (with its offset),
    /// `None` at the start of a block that more than one edge enters. ARM64 does not use `before`: it
    /// decodes the 64 words before `at` that lie in `code` from the blob, as today (full.rs:670-678 and
    /// `habits::jump_table` move into the CPU crate unchanged), so its tables are found exactly as now.
    /// x86 walks `before` back, at most 64 instructions, since x86 code cannot be decoded backwards.
    fn jump_table(at: u64, reg: Self::Reg, blob: &[u8], code: Range<u64>,
        before: impl Fn(u64) -> Option<(u64, Self::Instruction)>) -> Option<Vec<u64>>;
    /// Plain computation, which the flags' liveness may look past: the instruction only computes
    /// registers from registers (no memory, hook, control, trap or leaving). ARM64: exactly today's list
    /// in `flags_needed_after` (`AddSub`, `AddSubCarry`, `Logical`, `MoveWide`, `Bitfield`, `Extract`,
    /// `PcRelative`, `CondCompare`, `CondSelect`, `Unary`, `Binary`, `MulAdd`, `Nothing`); x86 the
    /// register-only forms of the integer instructions.
    fn plain(i: &Self::Instruction) -> bool;

    // --- calls, returns and the stack ---
    /// Where a call leaves the return address.
    const RETURN_ADDRESS: ReturnAddress;
    /// Bytes below the stack pointer a function may use without moving it: x86 128, else 0.
    const RED_ZONE: u64;
    /// The CPU checks the stack pointer's alignment when it is the base of an access (ARM64 under Linux).
    const SP_ALIGNMENT_CHECKED: bool;
    /// Push the address in this i64 local as data accesses use it (ARM64 untags it; others nothing).
    fn data_address(sink: &mut InstructionSink<'_>, local: u32);
    /// On the translator's own slow paths for a jump or call through a register (a switch whose case was
    /// not taken, the lookup's miss): make the target what the CPU would use, in place (ARM64 untags it),
    /// or stop with the fault the CPU gives (x86: general protection when it is not canonical).
    fn branch_target<E: Emitter<Self>>(e: &mut E, local: u32);
    /// The return address the bottom loop passes. The rule a CPU crate keeps: the target a meaning hands to
    /// `Emitter::ret` is always one the CPU would really go to (the meaning has already stopped with the
    /// CPU's fault otherwise), and no such target equals `UNREACHABLE_RETURN & !FLAGS_WANTED`. ARM64 and
    /// x86 i64::MIN, RV64GC 1 (section 3.4).
    const UNREACHABLE_RETURN: i64;

    // --- system calls ---
    const SYSCALL: SyscallRegisters<Self::Reg>;

    // --- code changes ---
    const CODE_CHANGES: CodeChanges;

    // --- loop proofs ---
    /// This instruction adds a step to `reg` (by arithmetic or by an access's write-back), or `None`.
    fn induction(i: &Self::Instruction) -> Option<Induction<Self::Reg>>;
    /// What a conditional branch tests, as one comparison that holds on its taken edge, given the
    /// instructions of its block before it (oldest first, each with its blob offset); `None` when it is
    /// not one comparison of a register with a register or a constant. Serves the counting exit and the
    /// bound a switch's guard puts on an index (today's `counting_exit` and `index_bound` match ARM64
    /// forms for both).
    fn branch_comparison(path: &[(u64, Self::Instruction)], branch: &Self::Instruction)
        -> Option<Comparison<Self::Reg>>;
    /// The largest value this instruction leaves in `reg` when it is a load that zero-extends what it
    /// loads (at most 4 bytes), else `None`.
    fn loaded_max(i: &Self::Instruction, reg: Self::Reg) -> Option<i64>;
    /// How a write narrower than 64 bits leaves the rest of the register, per width in bytes (1, 2, 4);
    /// `None` where the CPU has no register write of that width.
    const NARROW_WRITE: [Option<NarrowWrite>; 3];
}
```

The plain types, also in the interface crate:

```rust
/// A set of register bits (what `Registers` in wcontainers-arm64 is today).
pub struct Registers(pub u128);

/// One register bit: its number in `Registers` (0 to 127), what it is, and its offset in the thread block.
pub struct Bit { pub bit: u32, pub class: Class, pub ty: Ty, pub offset: u16 }
pub enum Class { General, Flag, Vector, Segment }
pub enum Ty { I64, V128 }
/// The locals of a shape (`Guest::locals_of`): the first `len` of `at`. Nine is x86's largest shape, a
/// shift by a run-time count: value, count, result and the six old flags, one local each (section 5.1).
pub struct Locals { pub len: u8, pub at: [u32; 9] }

/// Where control goes (today's `wcontainers_arm64::Target`, moved).
pub enum Target {
    /// This many bytes from the current instruction.
    Relative(i64),
    /// The guest address in this i64 local.
    Address(u32),
}
/// What an access does, as `Emitter::check` and `check_range` take it (today's `Access`, moved).
pub enum Access { Read, Write }

pub struct Flow<R> {
    pub falls_through: bool,
    /// A direct jump or call target, in bytes from this instruction.
    pub target: Option<i64>,
    pub kind: FlowKind<R>,
}
pub enum FlowKind<R> {
    Plain,
    Call,                         // direct, target above
    CallThrough(Through<R>),      // blr x, call rax, call [m], jalr ra, x
    Return,
    JumpThrough(Through<R>),      // br x, jmp rax, jmp [m], jalr x0, x
}
pub enum Through<R> { Register(R), Memory }

pub struct MemoryAccess<R> {
    pub base: Option<Base<R>>,
    /// The index register, its shift, and how it is read (ARM64 `uxtw`, `sxtw`; x86 and RISC-V all 64
    /// bits, `Extend { from_bits: 64, signed: false }`).
    pub index: Option<(R, u8, Extend)>,
    pub displacement: i64,
    /// The address is worked out modulo 2^this: 64, except 32 for x86's 0x67 prefix. The loop proofs and
    /// check grouping leave a 32-bit one alone (it is checked by itself).
    pub address_bits: u8,
    pub write_back: WriteBack,     // None, Before(by), After(by)
    pub bytes: AccessBytes,        // Fixed(n) or Counted (a rep string instruction: rcx elements)
    pub kind: AccessKind,          // Read, Write, ReadWrite
}
pub enum Base<R> { Register(R), Pc, Segment(Segment) }  // Segment::Fs, Segment::Gs: base plus a segment base
/// How a register is read as an index: its low `from_bits` (32 or 64), zero- or sign-extended (today's
/// `wcontainers_arm64::Extend`, moved).
pub struct Extend { pub from_bits: u8, pub signed: bool }

/// A constant or a register, as the loop-proof recognisers name operands.
pub enum Value<R> { Imm(i64), Reg(R) }
/// `reg` becomes `reg + step` (`reg - step` when `negate`) at `bits` (32 or 64), all 64 bits written.
pub struct Induction<R> { pub reg: R, pub bits: u8, pub step: Value<R>, pub negate: bool }
/// On a branch's taken edge, `left relation right` holds, compared at `bits` (8, 16, 32 or 64), signed or
/// not, with `left` and `right` read at the blob offset `at` (the flag setter, or the branch itself for
/// ARM64 `cbz` and every RISC-V branch).
pub struct Comparison<R> { pub left: R, pub right: Value<R>, pub relation: Relation, pub signed: bool,
    pub bits: u8, pub at: u64 }
pub enum Relation { Eq, Ne, Below { inclusive: bool }, Above { inclusive: bool } }
/// What a register write narrower than 64 bits does to the rest of the register.
pub enum NarrowWrite { Zero, Sign, Merge }
/// The fault a memory fault is: a page fault at the address, or general protection (address 0).
pub enum MemoryFault { Page, GeneralProtection }
pub enum StackMove { None, By { delta: i64, before_access: bool }, Unknown }
pub enum ReturnAddress { Register(u32 /* bit */), Stack { bytes: u8 } }
pub struct SyscallRegisters<R> { pub number: R, pub args: [R; 6], pub result: R }
pub enum CodeChanges { Announced, Detected }
pub enum TrapDetail { Instruction, Local(u32), Const(i64) }
pub struct HookType { pub name: &'static str, pub params: &'static [ValType],
    pub results: &'static [ValType], pub registers: Registers, pub body: HookBody }
/// Who does a hook's work on the host side.
pub enum HookBody {
    Clock,        // the runner's clock, in nanoseconds
    CodeChanged,  // the runner drops pieces of the range (ARM64 `ic ivau`, RISC-V `fence.i`)
    Cpu,          // `Guest::run_hook`
}
/// What `run_hook` did: its results (as many as `HookType::results` says), and the slow path it was, by
/// reason number (the translator's or the CPU's), for the runner to count and warn.
pub struct HookDone { pub results: [i64; 2], pub reason: Option<u32> }
```

`TrapKind` moves from the translator's contract into the interface (the contract re-exports it) and gains
the kinds x86 needs, numbered after ARM64's four so ARM64's numbers do not change: `DivideError` (4),
`GeneralProtection` (5), `FloatException` (6, detail the `si_code`), `SingleStep` (7). Each comes with the
signal Linux sends (section 6).

### 3.3 The Emitter

Today's `Emitter` (crates/wcontainers-arm64/src/meaning.rs) becomes `Emitter<G: Guest>`, with the same methods
over the guest's types: `get(G::Reg)`, `set(G::Reg)`, `get_vector(G::Vector)`, `set_vector`,
`set_flags(G::Flags)`, `flag(G::Flag)`, `get_control(G::Control)`, `set_control`, `hook(G::Hook)`, `host() ->
G::Host`, `offer() -> G::Offer` (new: the meanings of CPUID, `xgetbv` and every instruction of the AVX setting
read it), `trap(G::Trap)`, `condition(G::Cond)` (default: `G::condition_from_flags`, the full emitter tries
`G::condition_direct` first, as today), `float_mode` (default from `G::float_slow`), `stack_aligned`,
`this_address`, `jump`, `jump_if`, `call`, `ret`, `syscall`, `next`, `sink`, `local`.
The x86 type of a vector register is per register (`Bit::ty`), so `get_vector` pushes a v128 for xmm and
an i64 for MMX; RISC-V's f registers are i64.

Changes to the methods' contract, each needed by a CPU named here:
- `check(address, bytes, access)` becomes `check(address, bytes, access, which: u8)`: which of the
  instruction's accesses (0 or 1) this is, so plans are kept per access (x86 `movs`, `push [m]`, `call [m]`
  make two). ARM64 always passes 0.
- `check_range(address, length, access) -> i32 local`: whether a run-time length of bytes may be accessed,
  without stopping. Used by the x86 string instructions to choose the bulk path (section 5.6).
- `call(target)` and `ret(target)` follow `G::RETURN_ADDRESS`. With `Register` nothing changes from
  today. With `Stack { bytes }` the meaning does the CPU's own steps itself, in the CPU's order, before it
  calls `call`, so the emitter adds no order of its own: for x86 (section 5.3) it reads a memory target,
  writes the return address at sp - bytes with an ordinary checked store (the push), checks the target
  and stops with the CPU's fault if it is not one the CPU goes to (the return address already written, sp
  not moved, as the reference does), and only then sets sp to sp - bytes. `call` then makes the
  WebAssembly call with the expected return address and, after it returns, checks sp is `bytes` above its
  value at the call. For `ret` the meaning reads the address at sp and checks it; `ret` compares it with
  the expected one and then moves sp up by `bytes` (the meaning adds `ret imm16`'s immediate first). In
  both, the target `ret` is given is one the CPU would go to (see `UNREACHABLE_RETURN`).
- `warn(G::Reason)`: call the warn hook with this CPU's own reason and the instruction's address.
- The rule "`set` is never called inside a part of the meaning that runs only sometimes" stays. Meanings
  whose effect on a register depends on run-time data (x86 `rep` with rcx 0, `cmov`, shifts by zero) write
  it with a `select` and set it unconditionally. A `check` inside a conditional part is planned as exact
  (it neither covers nor is covered).

### 3.4 Why each item, and how it covers today's ARM64 assumptions

Instruction unit. Today the translator steps 4 bytes at a time, decodes a `u32` word and checks 4-byte
alignment (seam map translator T4 to T6, T12). With `decode` returning a length, each block keeps the list
of its instruction offsets in order; "next" is offset + length, "last" is the last offset in the list,
"previous" is the one before in the list, and two blocks join when `p + length(p) == offset`. Alignment
checks use `G::ALIGN`; with `ALIGN == 1` they compile to nothing. x86 code may jump into the middle of
another instruction: the body of a function is a set of offsets, so two overlapping instruction streams are
simply two sets of offsets. A jump target not a multiple of `ALIGN` is an alignment fault only where the
CPU faults: ARM64 yes (bit 0 or 1 set), x86 never, RV64GC never (jalr clears bit 0).

Registers. `BITS` replaces the hard-coded layout of `Registers` (x0 to x30 at bits 0 to 30, sp 31, NZCV 32
to 35, v0 to v31 at 64 to 95: T3) and `Slot::of_bit` (T21). The translator asks the bit's class, type and
block offset; "is the stack pointer" is `bit == SP_BIT`; "is the hard zero" is `reg_bit` returning `None`.
`Registers::FLAGS`, `ALL` and the general mask are computed once from `BITS` as constants. ARM64 keeps its
exact bit numbers, so everything derived from them, and the output, stays the same.

Passed registers (T16). Two ordered lists instead of "x0 and x1, from x0 up, plus sp". A piece's
parameters are `PASSED_IN`, then the expected return address and the depth; its results are `PASSED_OUT`.
ARM64: both [x0, x1, sp]. x86 starts with in = [rdi, rsi, rsp] and out = [rax, rsp]; which and how many is
a tuning number, measured at milestone X5 (section 11). `tracked()`, `register()`, the none emitter's get
and set and `reload(live, passed_from)` go through the lists.

Flags (T22 to T25). The translator keeps its machinery: a current shape, a local per completed flag, the
out-of-line flags-word functions (one per entry of `SHAPES`, which replaces `shapes()` over [W32, X64] and
`FLAGS_WORDS = 11`), the liveness that leaves out flag work nothing reads, and the choice "direct condition
if the shape is known, else from the flags". What it knew about ARM64 moves into the CPU: the shapes, their
locals and types, completing them, each flag's value and the conditions. Two things are new, both for x86:
- **Partial setters.** `sets(flags)` says which flags a shape sets. Setting a shape that does not set every
  flag first completes the other flags of the old shape into their locals, only where they are live. x86
  `inc` and `dec` keep CF; `mul` sets CF and OF and leaves the rest as the reference CPU leaves them
  (section 5.1).
- **Flags kept only sometimes** (x86 shift by a run-time count of 0 changes no flag). The meaning reads the
  old flags it may keep through `flag()` into locals and names them in the shape; `flag_value` selects.
  This is rare (shifts by cl) and needs nothing new in the emitter.
`flags_needed_after` (T25) stops listing ARM64 variants and becomes a test per flag. It starts with the
set of flags still to be set again, `pending`, equal to the `FLAGS` mask, and walks the rest of the block:
an instruction that is not `G::plain`, that reads a flag in `pending`, or that writes the stack pointer
means the flags are needed; otherwise `pending` loses the flags the instruction writes, and when it is
empty they are not. So x86 `inc` (which keeps CF) leaves CF pending, and a later `add` empties the set.
On ARM64 every flag setter writes all four flags, so `pending` is either the whole mask or empty, which
is exactly today's "reads any flag" and "`defs ∩ FLAGS == FLAGS`": the answer, and so the output, does
not change. `plain` is the CPU's (ARM64 today's list, section 3.2) rather than derived from the recorded
effects, because a derived rule would also let ARM64 look past instructions today's list stops at (`crc32`
among them) and change ARM64's output. x86's direction flag is not one of the `FLAGS` (section 5.1), so
string instructions, which read it, do not count as flag readers, no shape has to complete it, and
nothing in this test waits for an arithmetic instruction to set it. The flags-word functions take
`shape_locals(shape)` as parameters, rebuild the shape with `with_locals`, fill it in with `complete` and
put the word together from `flag_value` of each flag of `FLAGS`, shifted to its bit and or-ed in order
(today's `push_word`); the calls to them push `locals_of(flags)`. A CPU with no flags (RISC-V) has empty
`FLAGS` and `SHAPES`, its meanings never call `set_flags`, `flag` or `condition`, its `Cond` is an empty
enum, and `FLAGS_WANTED` is 0, so the flag locals, the flag state and the flags-word functions are never
made. The flag locals are a fixed array of at most 8 (x86 has 6).

Flags wanted (T13). Bit 0 of the expected return address is free on ARM64 because returns are 4-aligned;
on x86 returns can be odd. The CPU names the bit: ARM64 1 (unchanged), x86 bit 62 (no canonical user
address has it), RISC-V none. Every test is `target == L_RETURN & !G::FLAGS_WANTED`.

Address no return reaches (T14) and untagging (T15). The bottom loop calls a piece with the expected
return address `UNREACHABLE_RETURN | FLAGS_WANTED`, and a `ret` whose target equals `UNREACHABLE_RETURN &
!FLAGS_WANTED` returns to the bottom loop as a matched return, with no pc set, so the bottom loop would go
on from a stale pc. So no target `ret` is handed may equal it. A full 64-bit compare can be hit by some
popped value, so each CPU makes the value impossible before the compare, in the meaning that calls `ret`:
- ARM64: the meaning untags every target of `br`, `blr` and `ret` (today's code, meaning.rs:1326), which
  leaves a top byte of 0x00 or 0xFF, never i64::MIN's 0x80. `UNREACHABLE_RETURN` i64::MIN. Nothing changes,
  nothing is added.
- x86: the meanings of `ret`, and of `jmp` and `call` through a register or memory, check the target is
  canonical (bits 63 to 47 all equal: shift left 16, arithmetic shift right 16, compare with the value)
  before they call `ret`, `jump` or `call`, and stop with general protection at the instruction, rsp not
  moved, when it is not, as the CPU does (section 2: `ret` to 0x8000000000000000 gives `SIGSEGV`,
  `SI_KERNEL`, rsp unchanged). A `call`'s check comes after its push, as on the reference: the return
  address is written at rsp - 8 first, a fault on that write wins, and the general protection leaves the
  written slot and rsp unchanged (section 2, run 3; section 5.3). The check must be in the meaning because
  the CPU faults at the branch, not at the target: once the target reaches the lookup (`F_DISPATCH` sets pc
  to it) the branch's address is gone. A canonical value is never i64::MIN, so `UNREACHABLE_RETURN` i64::MIN
  cannot match. Cost: four WebAssembly operations and a branch never taken on every `ret` and every jump or
  call through a register or memory (ARM64 already pays ten for its untag on the same branches; x86's cost
  is measured at X4, section 12). The check also serves the mismatch path, which needs no second one. A
  direct jump or call whose target is not canonical (possible only within 2 GiB of the top of the lower
  half) is known at translation and translates to the same fault, a direct call's after its push (seen on
  the reference with code mapped at 0x7fffffff8000, section 2).
- RV64GC: `jalr` clears bit 0 of its target, so every target has bit 0 clear; `UNREACHABLE_RETURN` is 1
  (FLAGS_WANTED is 0) and costs nothing. A jump to an address the CPU cannot reach finds no piece and
  faults at that address, as the CPU does (RISC-V faults at the target, so no check before the jump).
`untag` is called today at code.rs:1305 (data), full.rs:2561 (a switch's case not taken, where the
translator works the target out again from the guest register) and full.rs:2857 (data); those calls become
`G::data_address` and `G::branch_target`, which on ARM64 emit exactly today's untag and on x86 the data
nothing and the branch the canonical check above.

Control flow (T7). `Flow` gains `FlowKind`, so the translator no longer matches `Branch { link: true }` or
`BranchRegister::Br`. A RISC-V `auipc ra, hi; jalr ra, lo(ra)` pair is a call through a register whose
target the translator knows from `built_address` on the pair; it is treated as a direct call.

Addresses the code builds (T8). `built_address(at, i, blob, code, before)` replaces "adr, or adrp plus
add at the 4-byte slot before". ARM64 keeps reading that slot from the blob, not the instruction before
on the walk's path: the two differ when the add is a jump target the walk first reached by the jump (the
walk records each offset once), and reading the slot is what today's translator does, so the X1 output
stays the same. It finds x86 `lea r, [rip + d]` alone and RISC-V `auipc` plus `addi` or `jalr` (2 or 4
bytes apart) through `before`.

Switches (T9, T10). `jump_table` gets the jump's offset, the blob and `before`, a way to walk the function's
own instructions back from the jump. ARM64 reads its window backwards from the blob exactly as today (so its
output stays the same); x86 cannot decode backwards and walks `before` instead. The patterns stay in the CPU
crate's `habits` module. The jump table the translator emits is indexed by the target address, as today,
scaled by the alignment: `(target - lo) >> log2(ALIGN)`. Decision and reason: indexing by the address the
guest computed is exact even when the guest changed its table in memory at run time (an unknown target leaves
through the lookup), and it needs no second kind of table. On x86 a table then has one entry per byte of the
span its targets cover, so spans reach the 65,520-entry limit sooner; a table over the limit falls back to the
lookup with the existing counter "jump through a register not recognised". Tables whose entries are absolute
addresses (non-PIE x86 code: `jmp [T + rax*8]`) need the load address the translator never sees, so they also
go through the lookup, counted the same way. How often either happens is measured on the x86 real binaries at
X9; only if it is common is indexing by case number brought back for a decision.

The lookup array (T11). One i32 slot per `ALIGN` bytes of guest code: ARM64 one per 4 bytes (as today), x86
one per byte, RISC-V one per 2 bytes. The slot's byte index is `(pc - table_base) << (2 - log2(ALIGN))`, which
is `pc - table_base` on ARM64 (nothing emitted, so the output is unchanged), and the "is it aligned" test is
emitted only when `ALIGN > 1`. `TABLE_BASE` and `SLOTS_COVER` in the contract say "one entry per `G::ALIGN`
bytes". Decision and reason: one memory read per lookup is the fastest lookup there is, and the cost is state
memory, four bytes per byte of x86 code (untouched pages cost nothing, so only code ranges that hold pieces
cost memory). For scale, expected and not measured: 52,050,800 bytes of code, the size of the largest ARM64
sample (bun, docs/status-translator.md), would need about 208 MB of slots if all of it held pieces; the x86
build of the same program is expected to be of a similar size, measured at X9. If the x86 binaries at X9 show
the 4 GiB state memory is at risk, a two-level table is measured then.

Trap detail (T5). `Decoded { word }` becomes `Decoded { length }`; the detail of a trap is
`G::trap_detail(code)`.

Calls and system calls (T17, T18). Covered by `RETURN_ADDRESS` (the length of the return address comes from
the instruction's length) and `SYSCALL`. After a system call the emitter reloads the result register by its
bit, whether or not it is passed (x86's rax is not passed in). The saved pc is offset + length. What the x86
`syscall` instruction does to rcx and r11 is in its meaning.

Traps (T19). `Trap` is the CPU's; `trap_kind` maps it to the generic kinds above.

Controls and the block (T20, T21). The CPU lists its controls with their types and offsets, the block's
pc, flags word and size, and the private bytes after the block. ARM64's are today's (`tpidr_el0`, FPCR,
FPSR, FPSR tracking, the exclusive monitor; private area 32 bytes). `THREAD_STATE_SIZE` in the contract
becomes `G::BLOCK_SIZE + G::PRIVATE_BYTES`, and the stop flag is at `G::STOP_FLAG_OFFSET`. ARM64's block is
`wcontainers_abi::RegisterBlock`, unchanged; x86's is defined in `wcontainers-x86` until the kernel
milestone (section 6).

Floating point slow path (T26). `float_slow` replaces "FPCR or FPSR tracking not zero";
`FLOAT_CONTROLS` replaces "a write to `Fpcr` or `FloatTracking`". Calls and system calls forget the answer,
as today.

Hooks (T27). The CPU lists its hooks; the translator gives them function indices after its own six
(system call, stop, memory fault, load code, warn, trap), so ARM64's float, clock and code-changed hooks
keep indices 6, 7 and 8 and the output does not change. `HookType::registers` is new: the bits a hook reads
or writes in the block (x86's x87 hook and the MMX registers, section 5.8). ARM64's hooks list none.
`HookType::body` says who does the work on the host side. The clock and the code-changed hook are the
runner's own. The software floating point (ARM64's `exact::run`, x86's SSE exact path and x87) is the
CPU's, through `Guest::run_hook` on the thread's state bytes, so no runner names a CPU function. Today the
host runner (runner.rs:903) and the browser's cdylib (browser/src/lib.rs:126) call
`wcontainers_arm64::exact::run` by name and work out FPSR and the reason themselves; at X1 that code moves
into ARM64's `run_hook`, unchanged in what it computes. The browser's cdylib then exports one function,
`cpu_hook(hook, a, b, c, d)`, over a buffer of the cdylib's own into which the JavaScript runner copies the
thread's state before the call and from which it copies it back after, with `cpu_hook_result(i)` and
`cpu_hook_reason()` for the results and the reason; the hook is a counted slow path, so the copy costs
nothing on the fast path. The kernel never runs a hook (the runner owns the instance whose imports they
are), so the CPU crates stay `std` crates and the `no_std` kernel never depends on one; x86's hooks may
use `astro-float` with the standard library (section 5.8). Should a `no_std` user ever need them,
astro-float 0.9.6 does build without the standard library once `lazy_static`'s `spin_no_std` feature is
on (checked in scratch `x87-nostd`: a `no_std` cdylib with its own panic handler links with it, and
without it fails on a duplicate `panic_impl`, because astro-float-num's `lazy_static` then brings std).

Memory faults (from section 5.4). The memory fault hook reports an address and a direction; whether that
is a page fault or something else is the CPU's (x86: general protection for an access reaching a
non-canonical address). `Guest::memory_fault(address, code, block)` answers it from the faulting
instruction's bytes and the exact registers, called by the runner before it tells the kernel, so the
hook's signature and ARM64's output do not change.

Host probe (T28). An associated type with its own probe module. It is part of the cache key.

Offer (from section 5.10, the AVX setting). A container setting that changes what the CPU claims is an
associated type, given to every translation beside the host answer and read by meanings through
`Emitter::offer()`: x86 `Offer { avx: bool }`, `()` for ARM64 and RV64GC. It is part of the cache key
(`offer_key`) and, from X3, of the description section. At X1 ARM64's `()` adds nothing anywhere, so the
output stays the same.

Stack moves and frames (T29 to T31). `stack_move(i)` replaces the match on ARM64 forms. `RED_ZONE`: the
frame a function checks when the stack pointer moves down is `[sp - RED_ZONE, top)` instead of `[sp, top)`,
so leaf functions using the red zone, and the 8 bytes an x86 call pushes, are inside a checked frame; what
is below `sp - RED_ZONE` is no frame. With `Stack` return addresses, the first frame check of a function
also covers the return address slot `[entry sp, entry sp + bytes)`, so the `ret` that reads it needs no
check of its own when the stack pointer is back at its entry value. `SP_ALIGNMENT_CHECKED` is false on x86
and RISC-V, where the "aligned after" analysis is never run.

Memory accesses (T32, T33). `accesses(i)` replaces "the one memory access": up to two, each with base
(register, pc or a segment base), index with scale and extension, displacement, write-back, size or
"counted" (rep), and read, write or both. Check plans are keyed by (offset, which). An access that reads and
writes the same place (x86 `add [m], r`) is one access of kind `ReadWrite`, checked once for writing before
anything changes. The base key of the check analysis becomes "register or segment", so all `fs:[...]`
accesses of a function group under the FS base, which no instruction in the function changes unless it
writes FS (`wrfsbase`).

Code changes (T34). `CodeChanges::Announced` (ARM64 `ic ivau`, RISC-V `fence.i` and the flush system call)
or `Detected` (x86): section 5.7.

Thread pointer (T35). Nothing generic: it is whatever the CPU says. ARM64 a control (`tpidr_el0`), x86 two
register bits of class `Segment` (FS and GS bases, i64, in the block) that accesses use through
`Base::Segment`, RISC-V general register x4.

Loop proofs (T36). The recognisers become CPU functions returning plain descriptions (section 3.2's types):
`induction` ("adds step s to r at width w", today's match on `AddSub` and write-back addresses);
`branch_comparison` ("on the taken edge, r relation v holds at width w, signed or not, read at offset a"),
from which the translator makes both the counting exit (negating the relation, `Eq` and `Ne` swapped and
`Below { i }` and `Above { !i }` swapped, when the loop goes on along the fall-through edge, as today's
`cond ^ 1`) and the bound a switch's guard puts on an index (an unsigned `Below` on the edge into the
block); and `loaded_max` ("a load into r zero-extends to at most m"). How an index is read comes with the
access (`MemoryAccess::index`'s `Extend`), so it needs no function of its own. Comparisons at widths other
than 32 and 64 bits give no proof (the proof module's `Domain` has only those two), which costs only
speed. `NARROW_WRITE` says what a narrow write does to the rest of the register: ARM64 `[None, None,
Some(Zero)]` (only 32-bit writes exist); RISC-V `[None, None, Some(Sign)]` (the `*W` instructions); x86
`[Some(Merge), Some(Merge), Some(Zero)]`. The proof module's `Domain` reasoning takes these instead of
ARM64's "a 32-bit result clears the top half". The generic parts (`Domain`, `Region`, `Counter`, the
steps and turns arithmetic) stay in the translator's proof module.

Identity (T37). `ID` names the guest. To keep X1's proof exact, the description section's format and the
cache key are not changed when ARM64 moves; at X3 the format becomes 4 with a guest byte and the offer key
(unsigned LEB128) after the format byte, and ARM64's output changes in the format byte and those two
bytes only (guest 1, offer 0), checked then.

Vectors, x87, rep and memory ordering (T38 to T41) are x86 decisions: sections 5.5, 5.6, 5.8, 5.10 and 8.

### 3.5 How the three CPUs fill it in

ARM64 (`wcontainers_arm64::Arm64`, everything as today):
- ALIGN 4, MAX_LENGTH 4; `decode` reads one little-endian word (`yaxpeax-arm` behind it, as now); trap
  detail the word.
- Bits: x0 to x30 at 0 to 30, sp at 31, N Z C V at 32 to 35, v0 to v31 (v128) at 64 to 95. `Reg::Zero` has
  no bit. SP_BIT 31. Passed in and out: x0, x1, sp.
- Flags: N Z C V at bits 31 to 28 of the flags word; shapes Add and Logic at 32 and 64 bits, Constant,
  Packed (today's `Flags`); `sets` is always all four. FLAGS_WANTED 1.
- Block: `wcontainers_abi::RegisterBlock`; controls `tpidr_el0`, FPCR, FPSR, FloatTracking, Exclusive*;
  private bytes 32.
- Hooks: Float `(i32, i64, i64, i64) -> i64` (body `Cpu`: `run_hook` reads FPCR and FPSR from the state,
  calls `exact::run`, writes FPSR back and names the reason, today's code in runner.rs and the browser's
  lib.rs moved into the CPU crate), Clock `() -> i64`, CodeChanged `(i64)`.
- RETURN_ADDRESS `Register(30)`; RED_ZONE 0; SP_ALIGNMENT_CHECKED true; data_address and branch_target
  untag (bit 55 rule), as the branch meanings already do; UNREACHABLE_RETURN i64::MIN; Offer `()`; SYSCALL
  x8, x0 to x5, x0; CODE_CHANGES Announced; memory_fault always `Page`; NARROW_WRITE [None, None,
  Some(Zero)] (only 32-bit writes exist).

x86-64 (`wcontainers_x86::X86_64`, section 5 gives the reasons):
- ALIGN 1, MAX_LENGTH 15; `decode` is `iced_x86::Decoder` in 64-bit mode over the bytes up to the end of the
  code range (an instruction that would run past it is `None`); trap detail the first 8 bytes.
- Bits: rax, rcx, rdx, rbx, rsp, rbp, rsi, rdi, r8 to r15 at 0 to 15 (hardware order); CF PF AF ZF SF OF
  at 16 to 21; FS base 22, GS base 23 (class Segment); mm0 to mm7 at 32 to 39 (i64, the mantissas of x87
  registers R0 to R7); xmm0 to xmm15 at 64 to 79 (v128); the upper halves of ymm0 to ymm15 at 80 to 95
  (v128). There is no zero register. SP_BIT 4.
- Flags: the six arithmetic flags at their rflags positions (CF 0, PF 2, AF 4, ZF 6, SF 7, OF 11); shapes
  in section 5.1. FLAGS_WANTED 1 << 62.
- Block: `X86Block` (section 5.9). Controls: `Rflags` (DF and ID, section 5.1), MXCSR, MXCSR tracking, the x87
  control, status (with TOP) and tag words, the x87 last-instruction pointers.
- Hooks: Float (SSE exact path, with MXCSR), X87 `(op: i32, low: i64, high: i64) -> (i64, i64)` reading and
  writing the x87 area and the MMX bits (both body `Cpu`, run by `run_hook` on the state), Clock.
- RETURN_ADDRESS `Stack { bytes: 8 }`; RED_ZONE 128; SP_ALIGNMENT_CHECKED false; data_address nothing (a
  non-canonical data address is past the permission table and faults); memory_fault `GeneralProtection`
  when the faulting access reaches a non-canonical address (section 5.4), else `Page`; branch_target, and
  the meanings of every branch through a register or memory, stop with a general protection fault when
  the target is not canonical (a call after its push); UNREACHABLE_RETURN i64::MIN; Offer `{ avx: bool }`;
  SYSCALL rax, (rdi, rsi, rdx, r10, r8, r9), rax; CODE_CHANGES Detected; NARROW_WRITE [Some(Merge),
  Some(Merge), Some(Zero)].

RV64GC (`wcontainers_riscv::Rv64`, paper only, the check that nothing assumes flags, a link register that
is special, 4-byte instructions or a special stack pointer):
- ALIGN 2, MAX_LENGTH 4; decode reads a 16-bit unit and, when its low two bits are 11, a second; trap detail
  that unit or word.
- Bits: x1 to x31 at 1 to 31 (x0 is the hard zero, no bit), f0 to f31 at 64 to 95 as i64 (single precision
  values are NaN-boxed). SP_BIT 2 (sp is x2 by the ABI only; nothing in the CPU checks it). tp (x4) is an
  ordinary general register, so the thread pointer needs no control. Passed in and out: a0, a1, sp.
- Flags: none. FLAGS, SHAPES empty, `Cond` empty, FLAGS_WANTED 0. Branches compare two registers directly.
- Controls: fcsr (frm and fflags), the LR/SC reservation (the Exclusive* shape again).
- Hooks: Float (fcsr; RISC-V instructions may also carry a static rounding mode, so `float_slow` may be
  answered per instruction), Clock (rdtime), CodeChanged with an "everything" form for `fence.i`.
- RETURN_ADDRESS `Register(1)` (ra, by convention; `jalr` may link to any register, and a call is
  recognised by `FlowKind`, not by the register); RED_ZONE 0; SP_ALIGNMENT_CHECKED false; no untag;
  UNREACHABLE_RETURN 1 (no `jalr` target has bit 0 set); Offer `()`; SYSCALL a7, a0 to a5, a0;
  CODE_CHANGES Announced; memory_fault always `Page`; NARROW_WRITE [None, None, Some(Sign)] (the `*W`
  instructions). Its branches compare two registers, so `branch_comparison` reads them at the branch.

## 4. Design decisions for the translator and the tests

Each decision the seam map lists, decided, with the reason. The brief's own decisions are kept as written.

Translator:
1. **Variable-length, byte-aligned code: lookup and switch tables.** One lookup slot per `ALIGN` bytes, and
   switch tables indexed by target address over the alignment (section 3.4, "The lookup array" and
   "Switches").
2. **Return addresses pushed on the stack.** `RETURN_ADDRESS = Stack { bytes: 8 }`: the meaning pushes
   and checks the target in the CPU's order (push, then the canonical check, then rsp moves) before the
   emitter's `call`, and `ret` moves sp up after its compare; a return moves sp by +8 relative to the call,
   the push is covered by the red-zone-extended frame and the `ret`'s read by the first frame check
   (sections 3.3 and 5.3). "Flags wanted" lives in bit 62.
3. **Lazy flags as a CPU-supplied model.** Section 3.4 "Flags" and section 5.1.
4. **Memory access description.** Up to two accesses, base plus index times scale plus displacement, FS and
   GS segment bases, plans per access (section 3.4).
5. **Passed registers.** CPU-chosen, different in and out (section 3.4).
6. **Unannounced code changes.** Write protection through the permission table (section 5.7).
7. **The red zone.** Part of every frame (section 3.4, "Stack moves and frames").
8. **x87 and AVX registers.** x87 lives in the thread block behind a software hook; ymm upper halves are
   register bits (sections 5.8 and 5.10).
9. **rep string instructions.** Bulk where exact, else one element per pass with a jump to itself (section
   5.6).
10. **The thread pointer.** A segment base on x86, a general register on RISC-V (section 3.4).
11. **Memory ordering between threads.** Not decided: section 8.

Tests (section 7 gives the detail):
12. **Native reference on another machine.** One long-lived SSH connection to the x86 machine, a static
    reference runner uploaded by content hash, cases and blobs streamed through stdin, results and statuses
    reported in-band, layer 1 batched. Reference results are recorded, keyed by the reference CPU's model
    string, the runner's hash and the case, so a replay needs no machine; a recorded result is a real CPU's
    result, never an emulator's.
13. **Instruction unit and boundaries.** From `G::ALIGN`; real boundaries for random starts come from a
    linear decode (with `G::decode` and `G::flow`) from each piece start the translator found.
14. **Stopping the reference after one instruction.** The trap flag (section 2), proven.
15. **Register state and the wire format.** A CPU-described comparable state of named fields with widths
    and the bits that are compared; instructions are length-prefixed.
16. **Bits allowed to differ, and outcomes.** Per instance, from the CPU's test support. x86's undefined
    flags are not masked: they must match the reference CPU, as the brief says. New outcomes: divide error
    (SIGFPE), general protection (SIGSEGV, SI_KERNEL, address 0), float exception, single step.
17. **Unannounced code writes in the runners.** The runners hold a stand-in of the write protection of
    section 5.7, and a program writes code with no announcement and runs it.
18. **Layer 2 generator.** A shared program model with a per-CPU emitter.
19. **Forms list for x86.** From iced-x86's own tables, filtered to what is offered.
20. **Where benchmarks run.** On the x86 machine itself, natively and in wasmtime and Chrome there.
21. **Test program corpus.** Shared C, C++ and Rust bodies; per-CPU support files.
22. **Build shape of the browser translator.** One cdylib per guest, chosen by a cargo feature of the
    browser crate (one build is one guest, nothing chosen at run time), exporting every per-CPU number the
    JavaScript runner needs in its contract JSON.

## 5. x86-64

### 5.1 Flags

- **Lazy, as the brief says.** A flag-setting instruction records its shape and operands in locals; a flag
  is worked out only when something reads it, and with look-ahead nothing is emitted for a flag nobody
  reads. PF (parity of the low byte of the result) and AF (carry out of bit 3) are worked out only on
  demand: `i32.popcnt` of the low byte, and bit 4 of `a ^ b ^ result`.
- **Shapes**, at widths 8, 16, 32 and 64: `Add { a, b, carry_in, result }` (add, adc, inc), `Sub { a, b,
  borrow_in, result }` (sub, sbb, cmp, dec, neg; CF is a borrow, unlike ARM64's carry), `Logic { result }`
  (and, or, xor, test: CF and OF clear), `Shift { kind, value, count, result, old flags }` (the old flags
  kept for a count of 0, each of the six in a local of its own, so the shape has nine locals, the most of
  any shape and the size of `Locals`; one local per flag keeps the per-flag liveness of section 3.4, where a
  packed word would cost a flags-word call at every shift by cl whose flags are live),
  `Multiply { high_nonzero, result }` (CF and OF), `Bits { .. }` for bt, bsf, bsr,
  popcnt, lzcnt, tzcnt, `Constant(word)` and `Packed(local)` (popf, sahf, the system call's return, the
  bottom loop). `inc` and `dec` are `Add` and `Sub` shapes whose `sets` leaves out CF.
- **Undefined flags** (the Intel manual's "undefined" after mul, imul, shifts, rotates, bsf, bsr, bt, the
  BCD instructions which do not exist in 64-bit mode, and others): each meaning defines them as the
  reference CPU leaves them, found by layer 1, which compares them like every other flag. Where the
  reference CPU's value depends on something no meaning can know, the forms concerned are recorded as
  known failures with the reason, never masked.
- **Conditions**: the 16 x86 conditions (O, NO, B, AE, E, NE, BE, A, S, NS, P, NP, L, GE, LE, G) as
  `Cond`. `condition_direct` turns cmp + jcc and test + jcc into one comparison of the operands, at every
  width.
- **DF is a control, not one of the lazily tracked flags.** Only `std`, `cld`, `popf` and `iretq` write
  it and only the string instructions read it, so it gains nothing from lazy tracking, and keeping it out
  of `FLAGS` keeps the liveness test per flag exact (section 3.4): no arithmetic instruction sets DF, so
  with DF among the flags no flag update could ever be found dead, and every string instruction would
  count as a flag reader. It lives in `Control::Rflags`, an i64 in the block holding the bits of rflags
  outside the flags word that user code changes and keeps, at their rflags positions: DF (bit 10) and ID
  (bit 21, which programs toggle to see that CPUID exists). A string instruction reads it with one load
  and a shift per pass.
- **The flags word** in the block holds the six arithmetic flags at their rflags positions and nothing
  else. The user-visible rflags is that word, or `Control::Rflags`, or bits 1 and 9 (IF), which user code
  always sees set: `pushf` puts it together in its meaning; `popf` and `iretq` split it (the six flags as a
  `Packed` shape, DF and ID into the control, IOPL, IF and the other system bits ignored as user mode
  does); the kernel's personality does the same at signal frames and `ptrace` (section 6). So the
  interface needs no item for bits a CPU always has. When `popf` sets TF the instruction after it runs and
  then stops with `SingleStep` (SIGTRAP, `TRAP_TRACE`), as on the CPU; that path is a counted slow path
  ("trap flag set by the program").
- **AC** (bit 18, alignment checking, which Linux's CR0.AM makes work in user mode) is not provided:
  testing every access's alignment would cost every access. It is reported as absent the way a program
  tests for it, which is the 386 test: `popf` and `iretq` leave AC clear, so it reads back 0, as on a
  CPU without alignment checking. Counted and warned, x86's own reason "alignment checking asked for, not
  provided". This differs from the reference by design; layer 1's `popf` and `iretq` cases with AC set
  in the popped value are compared with AC masked and the designed-answers test checks that it reads
  back clear and that the counter fires.

### 5.2 What an instruction is

- `iced_x86::Instruction` is the instruction type, with its length. The meaning of each `Code` the CPU
  offers is written once, against the emitter, in `wcontainers-x86`, from the Intel manual and the
  reference CPU. Nothing of gex (AGPL) is copied or paraphrased.
- iced decodes with `DecoderOptions::NONE`, which follows Intel where Intel and AMD decode differently. An
  encoding iced calls `INVALID` traps `Undefined` (SIGILL), except one longer than 15 bytes, which traps
  `GeneralProtection` (SIGSEGV, `SI_KERNEL`, address 0, rip at the instruction): the CPU's length limit
  wins over every other rule, invalid opcodes and invalid lock prefixes included (section 2, run 5). iced
  reports both as `INVALID` with length 15 (it stops reading at 15 bytes), so `decode` tells them apart:
  when iced gives `INVALID` with length 15, it works out the encoding's full length from the prefix run
  at its start. It keeps the last `66`, the last `67`, the last of `F2` and `F3`, and a REX byte (`40` to
  `4F`) when it is directly before the opcode, and drops every other prefix byte: all segment prefixes
  (`26 2E 36 3E 64 65`) and lock bytes (`F0`), which change no instruction's length, repeats of the kept
  kinds, and REX bytes followed by another prefix, which the CPU ignores. It decodes the rest with iced
  and adds the count dropped to the rest's length. Only the length is used, so the drop may change what
  the rest means (a lock that made it invalid, say) but not how long it is: `66` and `67` are the only
  prefixes that change a length (Intel's "length-changing prefixes"), and the last of `F2` and `F3` stays
  because it picks among mandatory-prefix forms, whose lengths iced may count differently when invalid.
  The longest rest (`F3 66 67` and REX before `0F 3A`, ModRM, SIB, a 32-bit displacement and an
  immediate) is 14 bytes, so iced's limit never cuts the rest short. When the rest is an instruction,
  its length is iced's and the sum is the CPU's length. When the rest is itself `INVALID`, iced's count
  of the bytes after its opcode is iced's own choice, not the CPU's: the CPU's length decoder counts
  bytes by its own tables even for opcodes that do not exist in 64-bit mode (section 2, run 5: `0F 38
  FF` and `9A` count bytes after the opcode; `06` and `0F 24` count nothing after it, where iced counts
  one). So the x86 crate holds a table, purpose-built because iced has none, of what the CPU counts
  after each opcode iced decodes as `INVALID`: nothing, a ModRM (with the SIB and displacement it calls
  for), a ModRM and an immediate of a given size, or an immediate. Run 5's sweep of the four legacy maps
  (`sweep15.sh`: each opcode placed so it ends at byte 15) gives the first answers: nothing after `06 07
  0E 16 17 1E 1F 27 2F 37 3F 60 61 CE D6` and, in the `0F` map, `04 0A 0C 0F 24 25 26 27 36`; at least
  one byte after every other opcode iced calls `INVALID` there. X3 completes the table with the same
  sweep, adding the maps behind `C4`, `C5` and `62`, run with each opcode ending at bytes 14 down to 8
  (the count is how far back it still gives general protection), once with filler whose ModRM asks for
  nothing more and once with a ModRM that asks for a SIB and a 32-bit displacement (which tells a ModRM
  from an immediate); it keeps the sweep's script and output beside the crate's tests, and a test checks
  the table against the recorded output. Over 15, the instruction stays iced's `INVALID` with that
  length set in it (`set_len`, a byte) and its meaning traps `GeneralProtection`; otherwise its meaning
  traps `Undefined`. This runs only for `INVALID` at length 15, so it costs nothing on any other
  instruction; layer 1's no-form cases check it against the reference. A too-long encoding whose rest
  runs past the end of the code range is `None` like any instruction that runs past it (what the CPU
  does there is not seen, section 12).
- An instruction of a feature not offered traps
  `Unsupported` (SIGILL, counted, warned), with x86's own reason when its claim group is the AVX setting
  (section 5.10). Which claim group a `Code` belongs to comes from the table the build script makes from
  iced's own `instr_info` (section 2), since the shipped decoder has no `encoding()` or `cpuid_features()`. A
  `Code` with no meaning written is of a feature never claimed and traps `Unsupported`.
- Kernel-only instructions (hlt, in and out, cli and sti, lgdt and the rest, mov to control registers,
  rdmsr, wrmsr, rdpmc while Linux does not allow it, `int n` other than 3, 0x80 and the `int1` and `int3`
  forms): the fault Linux gives a user program, which for most is general protection (SIGSEGV, SI_KERNEL,
  address 0) and for some #UD (SIGILL); layer 1 decides each against the reference.
- Breakpoints: `int3` (`CC`) and `int 3` (`CD 03`) stop with `Breakpoint`, SIGTRAP with `SI_KERNEL`, rip
  after the instruction; `int1` (`F1`) stops with `Breakpoint`, SIGTRAP with `TRAP_BRKPT`, rip after it
  (section 2). The trap's detail carries the `si_code`, and layer 1 compares it (`Stop::Breakpoint` holds
  it, section 7.2).
- `int 0x80` from 64-bit code is Linux's 32-bit system call gate on the reference (section 2: it ran i386
  getpid). 32-bit code and its system call numbers and layouts are deferred with 32-bit mode, so this
  phase behaves as a Linux kernel built without 32-bit emulation (`CONFIG_IA32_EMULATION` off), where
  vector 0x80 is a kernel-only gate like `int 0x81`: general protection (SIGSEGV, SI_KERNEL), rip at the
  instruction. That is reporting the missing feature the way Linux does, not a pretence. It is counted and
  warned, x86's own reason "32-bit system call from 64-bit code, not provided". Expected from Linux's
  source and not seen on a running kernel without 32-bit emulation; layer 1 excludes `int 0x80` with that
  reason and the designed-answers test checks the fault.
- **Division** (`div` and `idiv` at 8, 16, 32 and 64 bits) stops with divide error (`DivideError`:
  SIGFPE, `FPE_INTDIV`, rip at the instruction, nothing changed) when the divisor is 0 or the quotient
  does not fit the width. Seen on the reference (section 2, run 3): `div rcx` with rdx:rax = 1:0 and rcx 2
  gives rax 0x8000000000000000, rdx 0; with rdx 2 it gives `FPE_INTDIV`. At 8, 16 and 32 bits the dividend
  fits in an i64, so the meaning divides in i64 and checks the quotient's range. At 64 bits the dividend is
  the 128-bit rdx:rax and WebAssembly has no 128-by-64 division. Decision:
  - Fast path, inline: `div` when rdx is 0, `idiv` when rdx is rax's sign (rax shifted right 63,
    arithmetic). One compare, the divisor-zero check, for `idiv` also the check of rax = i64::MIN with
    divisor -1 (divide error on the CPU, a trap in WebAssembly), then `i64.div_u` and `i64.rem_u` (or the
    signed pair). This is what compilers emit for 64-bit `/` and `%` (`xor edx, edx` or `cqo` first).
  - Otherwise, inline in the same meaning behind that compare: a 128-by-64 long division with 64-bit
    operations. For `div`, divide error when rdx >= the divisor (the quotient would not fit; this includes
    divisor 0); else normalise by `i64.clz` of the divisor and divide in two 32-bit digit steps with
    estimate and correction (Knuth's algorithm D for two digits). For `idiv`, the same on the absolute
    values, divide error when the quotient does not fit (above 2^63 - 1, or above 2^63 when negative),
    then the signs (the remainder takes the dividend's sign). About 60 WebAssembly operations (expected).
  - Not a slow path, so no counter: it is the instruction's whole meaning in WebAssembly arithmetic, with
    no hook and no software state, like phase one's guards around ARM64 division and like x86's own flag
    work; the CPU's own 64-bit `div` is a long instruction too. Its cost against native is measured at X3
    on a program that divides 128-bit values (section 12), and reported to the owner if it is far above
    the native cost.
- `mov ss` and `pop ss`: `pop ss` does not exist in 64-bit mode (iced decodes it as `INVALID`); `mov ss`
  with a selector Linux allows changes nothing a user program sees, except that a trap flag the program
  set (section 5.1) stops after the instruction after it, as on the CPU.

### 5.3 Calls and returns

- `call` (direct, through a register, through memory) becomes a WebAssembly call to the target's piece,
  as on ARM64. Its meaning does what the reference CPU does, in the reference's order (section 2, run 3):
  1. `call [m]` reads its target (access 0); a fault there stops first, nothing changed.
  2. It writes the real return address (this instruction's address plus its length) at rsp - 8 (the push:
     access 1 for `call [m]`, else 0); a fault there stops next, nothing changed. The write is inside the
     frame checked when rsp last moved down, so it usually needs no check of its own (below).
  3. It checks the target is canonical; if not, it stops with general protection at the instruction, with
     the return address already written at rsp - 8 and rsp unchanged. That is what the i7-8750H does for
     `call rax` with 0x8000000000001000 or 0x800000000000, `call r8`, `call [rdi]`, and a direct `call
     rel32` to 0x800000000000 from code at 0x7fffffff8000; `jmp rax` and `jmp rel32` to the same targets
     write nothing. With rsp 0x1008 (unmapped below) the push's page fault comes instead of the general
     protection; `call [rdi]` with rdi 0x3000 and rsp 0x1008 faults at 0x3000, so the read comes first.
  4. It sets rsp to rsp - 8 and calls the emitter's `call`, which makes the WebAssembly call.
  Intel's manual's pseudo-code checks the target before the push; the CPU does not, and the CPU decides.
- `ret` reads [rsp] and checks the value is canonical, else it stops with general protection, rsp not
  moved, as on the CPU (section 3.4, T14: this check comes before the compare, so a popped value can never
  match the bottom loop's marker). If the value is the address the caller expects, rsp moves up by 8 and
  the piece returns directly. If not, rsp moves and the thread leaves to the bottom loop through "go to
  address". `ret imm16` also adds its immediate. `jmp` through a register or memory makes the same
  canonical check first, before anything changes; `call` makes it after its push (above).
- **Far transfers** (`call far m16:64`, `jmp far m16:64`, `retf` and `retf imm16` in their 16, 32 and
  64-bit operand sizes, `iretq`; valid in 64-bit user mode, so in layer 1's forms list). Their meanings
  read the selector with the offset and act on it as the CPU does in user mode on Linux. Seen on the
  reference (section 2, run 4): with selector 0x33 (Linux's 64-bit user code segment, the one running)
  `call far [rdi]` reads its 10 bytes, pushes 16 (the return address at rsp - 16, the old CS 0x33
  zero-extended above it) and lands at the offset; `jmp far` lands with nothing written; `retfq` pops the
  offset and the selector, rsp up by 16; `iretq` pops rip, CS, rflags, rsp and SS (40 bytes) and lands
  with rsp and rflags loaded (IF kept set). A selector that names no code segment (0, 0x2b, which is data,
  and SS 0x33 for `iretq`) gives general protection with nothing written and rsp unchanged, so for far
  transfers the selector is checked before the push; a non-canonical offset with selector 0x33 gives
  general protection after the 16-byte push, as a near `call` does. So the meanings: read (access 0,
  checked first), test the selector, push (access 1, for `call far`), test the offset is canonical, then
  move rsp and transfer. A far transfer with selector 0x33 is, for the translator, a jump or call through
  memory (`FlowKind::JumpThrough` or `CallThrough(Memory)`, the return address 16 bytes up for `call
  far`'s matching `retf`) and `retf` and `iretq` are returns that leave to the bottom loop when the target
  is not the one expected; they are rare, so no more than that. Selector 0x23 (Linux's 32-bit user code
  segment) switches the reference to 32-bit mode, which this phase does not have: it stops with general
  protection at the instruction, nothing changed, the fault a CPU gives for a selector it cannot load;
  counted and warned, x86's own reason "far transfer to 32-bit code, not provided", like `int 0x80`
  (section 5.2). Layer 1
  excludes selector 0x23 with that reason; the designed-answers test checks the fault.
- After a WebAssembly call returns, the caller checks rsp is 8 above its value at the call (after the
  push); if not, it leaves to the next instruction through the bottom loop, as ARM64 does for a changed sp.
- Deep recursion leaves to the bottom loop on the stack budget, as on ARM64.
- **The red zone**: 128 bytes below rsp belong to the function's frame (section 3.4). The kernel's signal
  delivery skips them (section 6).
- **Checks**: the push is inside the frame checked when rsp last moved down (which reaches 128 bytes below
  rsp), so a call costs no check; the `ret`'s read is inside the function's first frame check, which also
  covers the 8 bytes at rsp at entry. A leaf function that never moves rsp checks that slot once at its
  `ret`.

### 5.4 Memory operands

- **Addresses**: base + index * scale + displacement, plus the FS or GS base for segment overrides, plus
  rip for rip-relative operands, computed in an i64 local, then checked.
- **Read, change, write** (`add [m], r`, `xchg`, `lock` forms, `inc [m]`): one access of kind `ReadWrite`,
  checked for writing once before anything changes, so a fault leaves exact state.
- **Two memory operands** (`movs`, `cmps`, `push [m]`, `pop [m]`, `call [m]`): both checked before
  anything changes, so a page fault on either leaves exact state (a `call`'s general protection for a
  non-canonical target comes after its push, section 5.3). `pop [m]` computes m with rsp already moved, as
  the manual says.
- **Non-canonical data addresses** stop the thread at the permission table (they are above every page it
  covers). The CPU gives general protection (SIGSEGV, SI_KERNEL, address 0) for any access that reaches a
  non-canonical address, also one that starts at a canonical one: on the reference an 8-byte load at
  0x7ffffffffffc and a 4-byte load at 0x7ffffffffffe give general protection, while an 8-byte load at
  0x7ffffffff000 and a 1-byte load at 0x7fffffffffff give a page fault (`SEGV_MAPERR`) at their address
  (section 2, run 3). So the CPU's check of canonical addresses comes before the page check. The meanings
  add nothing for this (no cost on any access): the permission table already stops every such access,
  since the last page below 2^47 is never mapped: Linux's x86-64 user space ends at 0x7ffffffff000
  (`TASK_SIZE_MAX`), and the x86 personality gives our kernel that same limit (section 6), so no mapping
  reaches the last page and an access reaching 2^47 always faults. The registers are exact at the fault,
  so `X86_64::memory_fault(address, code, block)` works out the instruction's accesses (`accesses` of
  the instruction at `code`, with their start addresses from the block's registers and segment bases, and
  their sizes: an element for `rep` and gather forms). The two accesses of one instruction may differ in
  size (`call far [m]` reads 10 bytes and pushes 16), so it takes the access that faulted: the first, in
  the instruction's order (the order the CPU and the meaning check them), whose range holds the reported
  address (the first byte not allowed). General protection when that range is not wholly canonical (its
  start is not canonical, or its last byte is above 0x7fffffffffff); otherwise a page fault at the
  address. No access is longer than a page (the largest offered, `xsave` with x87, SSE and AVX state, is
  832 bytes in the manual's layout), so no access reaching 2^47 starts below the last page. Layer 1
  places a share of its memory operands' bases, and of its stack pointers, in the last 64 bytes below
  2^47 so that both answers are compared with the reference.
- **Alignment**: x86 allows unaligned access except for a few instructions (`movaps`, `movdqa` and the
  other aligned SSE forms, `cmpxchg16b`, `fxsave`): those meanings test the address and stop with general
  protection, as the reference does (section 2).
- **lock**: one guest thread runs at a time (section 8), so a locked instruction is its plain form.

### 5.5 Segment bases and the thread pointer

FS and GS bases are thread state: two register bits (i64, in the block), so a function that reads
`fs:[0x28]` (the stack protector) many times keeps the base in a local. They are written by `wrfsbase` and
`wrgsbase` (FSGSBASE, offered: Linux enables it and says so in AT_HWCAP2) and by the kernel's `arch_prctl`
through the block at a system call. `rdfsbase` and `rdgsbase` read them. The other segment registers
(cs, ss, ds, es) read as Linux's user values; writing a segment register is general protection unless the
value is one Linux allows, decided by layer 1.

### 5.6 String instructions

- **One element** (no rep prefix): an ordinary instruction with its accesses.
- **rep movs and rep stos, bulk path**: when DF is clear and the ranges do not overlap in a way that changes
  the result (a forward copy onto a destination that starts inside the source), the meaning checks the
  whole source range for reading and the whole destination for writing with `check_range`, and then does
  WebAssembly's `memory.copy` (for movs) or `memory.fill` (for stos with one repeated byte, which includes
  zero; other values store 8 or 16 bytes at a time in a loop over the checked range). At most 1 MiB moves
  per pass; rcx, rsi and rdi are then updated as the CPU would after that many elements, and the
  instruction jumps to itself, which is a backward jump, so the stop check runs between passes.
- **Otherwise, one element per pass**: the meaning moves one element, updates rcx, rsi and rdi with a
  `select` (unchanged when rcx is 0), and jumps to itself while rcx is not 0. Faults and stops are then
  exact to the element. This is the slow twin, counted "string instruction one element at a time" with the
  reason (direction flag set, overlap, range not all allowed).
- **repe and repne cmps and scas** stop on data, so they are always one element per pass.
- What the reference does (section 2): the trap flag stops it after each element, so layer 1 lets the
  reference step on while rip stays on the instruction, up to the case's rcx (kept small by the generator),
  and compares the end state.

### 5.7 Code written without an announcement

x86 programs may write code and run it without telling the CPU. Decision, as the brief says:
- When a page's bytes are first translated, the CPU side clears the page's write bits in the permission
  table (`PAGE_WRITE` of the page and `NEXT_PAGE_WRITE` of the page before) and remembers that it did,
  before it reads the bytes. Translated code is unchanged: it already checks writes, so on the normal path
  nothing is added to the pieces.
- **How this meets the contract for permission changes** (contract.rs: what a piece knows about
  permissions holds until its thread can next stop, except a running function's own frame, noticed only at
  the next function entry; whoever changes permissions first stops every other thread of the process):
  - Other threads: the protecting step is a permission change like `mprotect` and follows the same rule.
    With one host thread, as now and until section 8 is decided, the others are stopped by themselves.
    With threads in parallel it stops them first, once per page (a page is protected once until it is
    written), counted "threads stopped to protect code".
  - The translating thread: translation runs inside `load_code`, which a piece reaches only through a call
    or a jump to the dispatcher (or from the bottom loop), and pieces check permissions again after a call
    returns, so everything they knew is checked again, except proofs over their own frames.
  - Frames: a page that lies at or above a thread's stack pointer inside the mapping that holds that stack
    pointer (code on an executable stack, such as GCC's nested-function trampolines) may be covered by a
    frame proof of that thread, and a write through it would not fault. So when the page being protected
    is such a page for any thread, that thread gives up its WebAssembly frames before it runs on: the
    translating thread's `load_code` returns "leave" once the piece is in place, a stopped thread leaves
    `run` when its stop hook returns, and each carries on from the bottom loop with functions in progress
    continued from their return addresses (as after a lost stack), whose pieces check their frames afresh.
    Counted and warned "code translated from a live stack frame". This is rare (an executable stack), so
    its cost does not matter.
- A write to such a page reaches the memory fault hook with exact registers. The CPU side sees the page is
  only write-protected for code, drops every piece translated from the page, sets the write bits back, and
  continues the thread at the faulting instruction, which writes again. Counter and warning: "code written
  without announcement", with the page.
- Pieces in progress on the WebAssembly stack may have come from that page, including the running one. So
  dropping pieces unwinds to the bottom loop (as after a lost stack); functions in progress continue from
  their return addresses, translated afresh ("continued after stack loss").
- Setting the write bits back is a permission change too: the faulting thread is stopped in its hook,
  and with several threads the same rule (stop the others first) applies.
- **Who keeps the marks.** The CPU side, which owns the memories, the permission table and the shared
  blocks: today the host runner and the browser runner with their stand-in system calls, and at X10 the
  implementation of `wcontainers_abi::Cpu` over translated code under the kernel. A mark is kept per backing
  page, not per address: a private page by (memory, page), a page of a shared block by (block, page
  offset). Beside each mark it keeps the permissions the kernel last asked for the page: a
  `set_permissions` on a marked page records them and leaves the write bits clear while the mark stands
  (they come back when the page is written); `memory_duplicate` (fork) copies the table as it is, marks
  and all; mapping over, unmapping or zeroing a marked range drops its pieces and marks, as
  `code_changed` does. Writing the bytes back is "the step": drop every piece translated from every
  address that shows the page, in every memory, clear the mark, give every such address the write bits
  the kernel asked for, counted "code written without announcement" with the page.
- **Writes the permission table never sees.** Two other paths change bytes with no fault, and both take
  the step:
  - Writes the kernel makes into guest memory: the copy-out of `read`, `recv`, `pread`,
    `process_vm_writev` and every other call, and the kernel's zeroing. All of them reach memory through
    the CPU side's `Cpu::write`, `zero` or `shared_write`, which by the abi's contract ignore page
    permissions (the kernel checks its own map, crates/wcontainers-kernel/src/user.rs). Those three
    methods look up each page of the range in the marks first (one lookup per page written, against the
    cost of the copy itself; measured at X10) and take the step for a marked page before they write. A
    thread whose own pieces were dropped by its system call carries on from the bottom loop when the call
    returns, as after any drop.
  - Writes through another mapping of the same pages: a memfd or `shm` block mapped twice in one process
    (.NET's W^X double mapping writes code through one address and runs it through the other), a
    `MAP_SHARED` file, or another process writing shared memory. The CPU side knows every address of
    every memory that shows a block's page, since `shared_map` and `shared_unmap` are its own
    operations. Protecting a shared page for code clears the write bits at every address that shows it,
    in every memory (a permission change for each of those processes, under the same rule), and a later
    `shared_map` of a marked page maps it write-protected. A write through any alias then faults, and the
    step drops the pieces of every alias. Writes the kernel makes to the block itself (`write` on a
    memfd, through `shared_write`) take the step as above.
  Threads of other processes whose pieces are dropped (stopped, by the rule above) give up their
  WebAssembly frames when they next run, as in the frames case above.
- **When each part is built.** X6 builds the fault path and the marks in both runners. Until X10 an x86
  program runs only under the runners' stand-in system calls (`write`, `writev`, `exit`, `exit_group`,
  `brk`), which map nothing twice and write guest memory only when `brk` zeroes pages, which takes the
  step; so nothing writes code unseen before the kernel joins. X10, with the kernel's personality, builds
  the step in `Cpu::write`, `zero`, `shared_write` and the shared mappings, with three tests against the
  reference: a program that `read`s new code over code it has run and runs it again, a memfd mapped twice
  (written through one address, run through the other), and two processes sharing a page of code.
- ARM64 keeps `ic ivau`; `CODE_CHANGES` tells the runner which policy to use.
- Expected and unmeasured: a JIT that keeps pages writable and executable while writing code next to
  running code would fault often. The counter shows it; a measurement on such a program comes at X6.

### 5.8 Floating point: SSE and x87

- **SSE through SSE4.2** maps onto WebAssembly's 128-bit vectors: packed and scalar float arithmetic,
  integer SIMD, shuffles, `pmin`/`pmax` (which are exactly `minps` and `maxps` with the operands swapped),
  conversions with x86's "integer indefinite" result (0x8000...) where WebAssembly saturates,
  `pcmpestri`-style string compares as sequences of vector operations, and `crc32` as ARM64 does its CRC
  instructions.
- **The approximate reciprocals** `rcpps`, `rcpss`, `rsqrtps` and `rsqrtss` (baseline SSE, so claimed by
  default) give Intel's own approximations, which no WebAssembly operation gives (`rcpps` of 3.0 is
  0x3EAAA000, not the correctly rounded 0x3EAAAAAB). Decision: matched bit for bit by a closed formula in
  WebAssembly's f64 vector arithmetic, no table, no hook and no slow path. The formula was found and then
  checked on the reference CPU over all 2^32 inputs (section 2, run 3; zero differences, in MXCSR 0x1F80
  and again in 0xFFC0, so the rounding mode, FTZ and DAZ do not touch them, and the packed forms give the
  scalar result in every lane):
  - `rcp`: a normal input `±1.m × 2^(e-127)` gives `q = nearest(8192 / mid)` with `mid = 1 + (m's top 11
    bits + 0.5) / 2048` worked out in f64, then the result `±(q / 8192) × 2^(127-e)`, as the f32 with
    exponent field `253 - e` and mantissa `(q - 4096) << 11`, or a signed zero when `253 - e <= 0`. Zero
    and denormal inputs give a signed infinity, infinities a signed zero, NaNs the input quieted.
  - `rsqrt`: a positive normal input gives `q = nearest(8192 / sqrt(mid))` with `mid = (1 + (m's top 10
    bits + 0.5) / 1024) × 2^b`, `b` the parity of `e - 127`, then exponent field `126 - (e - 127 - b) / 2`
    and mantissa `(q - 4096) << 11`. Zero and denormal inputs give a signed infinity, +infinity gives +0,
    a negative non-zero normal or -infinity gives the default NaN 0xFFC00000, NaNs the input quieted.
  `nearest` is `f64.nearest` (ties to even; no tie occurs for `rcp`, as 2^25 over an odd number is never
  a half-integer). In the meaning each pair of lanes is promoted to f64x2, the special inputs chosen with
  `select` on lane masks, about 30 vector operations for four lanes (expected; cost measured at X5). At
  X5 the same check over all 2^32 inputs becomes a test that runs the meaning's formula against the
  reference CPU on the x86 machine (the scratch check took 73 s there for both instructions), and layer 1
  compares the translated forms like any other.
- **MXCSR**, following the ARM64 pattern: in the default mode (0x1F80: round to nearest, no flush to zero,
  no denormals-are-zero, all exceptions masked) and while the program has not written MXCSR, the fast path
  runs and the sticky exception flags are not kept. Once the program sets another rounding mode, FTZ or
  DAZ, unmasks an exception or writes MXCSR (as `feclearexcept` does), floating point takes the exact path
  through the float hook with MXCSR, counted "exact floating point requested". An unmasked exception stops
  with `FloatException` (SIGFPE with the `si_code` of the exception).
- **Not-a-number results** follow x86's rules (the first source operand's NaN, quieted; the default NaN is
  negative, 0xFFC00000 single), bit for bit, with no setting. The host probe asks whether the engine's NaN
  results already follow x86's rules; where they do not (expected on ARM hosts) the translator adds a
  fix-up, as for ARM64 guests on Intel hosts. The probe answer is in the cache key.
- **x87** (80-bit) runs through software: the x87 hook does every arithmetic, conversion and comparison
  with `rustc_apfloat`'s `X87DoubleExtended` (already a dependency of the ARM64 crate; Apache-2.0 WITH
  LLVM-exception), except the square root and the transcendental instructions (below), counted "x87
  floating point in software". Its state lives only in the block: the eight
  registers R0 to R7 in physical order, the control word, the status word with TOP, the tag word and the
  last-instruction pointers. Instructions that name ST(i) work out R((TOP + i) mod 8) at run time inside
  the hook, so no register numbering depends on run-time state. Loads and stores of memory operands are
  done by the meaning (checked, exact faults), and their values pass through the hook's i64 arguments and
  results. Rounding control and the unmasked-exception rule (the exception is raised at the next x87
  instruction that waits) follow the manual.
- **Precision control** (FCW bits 8 and 9: 24, 53 or 64 bits) affects only fadd, fsub, fsubr, fmul, fdiv,
  fdivr (with their `p` and `i` forms) and fsqrt, as the manual says; the rest always round to 64 bits. At 64
  bits, the default Linux sets (FCW 0x037F), an arithmetic operation is one `X87DoubleExtended` operation. At
  24 or 53 bits the result must be rounded once to that many bits while keeping the 15-bit exponent range,
  which `X87DoubleExtended` does not offer. Decision: two more semantics of rustc_apfloat's public
  `ieee::Semantics` trait, defined in `wcontainers-x86` (15 exponent bits, precision 24 and 53), and a wide
  one (15 exponent bits, precision 66). The operation is done in the wide semantics rounded toward zero, the
  lowest bit is set when apfloat reports the result inexact (rounding to odd), and that is converted once to
  24 or 53 bits in the program's rounding mode, then widened exactly. Rounding to odd at a precision at least
  two bits wider than the target and then rounding once gives the correctly rounded result (Boldo and
  Melquiond, "Emulation of FMA and correctly rounded sums: proved algorithms using rounding to odd", IEEE
  Transactions on Computers, 2008); 66 bits also holds every 64-bit operand exactly. The status flags come
  from the two steps' statuses. Checked in a scratch program
  (`pyro-parked/wcontainers-scratch/x86/apfloat-pc`, rustc_apfloat 0.2.3): the custom semantics build
  (precision 66 and 24, exponent range 16383 to -16382), 1 + 2^-30 + 2^-60 at 24 bits gives 1 + 2^-23 rounding
  up and 1 to nearest, and 2^-16000 stays non-zero at 24 bits. Layer 1 checks it against the reference with
  the precision control field random.
- **The square root** (`fsqrt`, at precision 24, 53 or 64 and in every rounding mode): rustc_apfloat 0.2.3
  has no square root (its src/lib.rs lists `sqrt` under future work), and `ieee-apsqrt` 0.1.1, which the
  ARM64 crate uses, works only on IEEE bit patterns of 32, 64 and 128 bits (its `IeeeBits` is implemented
  for u32, u64 and u128, and only the first two for its exact `sqrt_accurate`), not on 80-bit values or
  x87's reduced precisions. Decision: the root of a finite positive operand
  is worked out with astro-float's `sqrt` (documented as correctly rounded) at 128 bits rounded toward
  zero; it is exact when its square, by astro-float's `mul` at 256 bits (exact for two 128-bit
  mantissas), equals the operand. Its top 66 bits, with the lowest set when any lower bit is set or the
  root is inexact, are the round-to-odd value in the wide semantics above, which is then rounded once to
  the precision control's 24, 53 or 64 bits in the program's rounding mode, as for the other operations
  (the same Boldo and Melquiond result, 66 being at least 64 + 2). The status (precision exception, C1
  for rounded up) comes from the two steps. The root of an 80-bit value is never outside the normal
  range, so no step meets a denormal result. Zero, negative, infinite, NaN, denormal and unsupported
  operands are answered from the manual's table for `fsqrt`. Seen on the reference (section 2, run 4):
  the root of 2 is 0x3fff:b504f333f9de6484 at precision 64 and 0x3fff:b504f30000000000 at precision 24.
  X7 compares it in layer 1 with the precision and rounding control random.
- **The transcendental instructions** (fsin, fcos, fsincos, fptan, fpatan, f2xm1, fyl2x, fyl2xp1)
  are worked out with the `astro-float` crate (0.9.6, MIT; builds for wasm32-unknown-unknown with the
  standard library, which the browser's cdylib has; measured below), at 128 bits, then rounded once to
  64 bits in the program's rounding mode. For fsin, fcos, fsincos and fptan the argument is first reduced
  the way Intel's manual describes for these instructions (a 66-bit approximation of π; expected from the
  manual's description, confirmed against the reference at X7). Intel's own results are documented as
  within one unit in the last place, not as correctly rounded, so layer 1 compares them bit for bit and
  any case where the reference's result is not the correctly rounded one is recorded as a known failure
  with the reason, never masked. Size, measured 2026-11-17 on the Mac (load 17 to 21) in a scratch cdylib
  (`pyro-parked/wcontainers-scratch/x86/x87-crate`, opt-level s, LTO, stripped) using sin, atan, log2,
  exp and add at 128 bits, against the same cdylib without it: 142,688 bytes raw, 40,947 brotli -q 11 more.
  Only the x86 browser build carries it.
- **MMX** (baseline in every x86-64 CPU, so offered): mm0 to mm7 are register bits of type i64, whose
  block slots are the mantissas of R0 to R7. An MMX instruction also sets TOP to 0 and marks every x87
  register valid; `emms` marks them empty. The x87 hook declares the MMX bits in its `registers`, so the
  emitter writes them back before an x87 instruction and loads them after.

### 5.9 The register block

`X86Block`, defined in `wcontainers-x86` until the kernel milestone moves it next to ARM64's in the abi:
16 general registers in hardware order, rip, the flags word (the six arithmetic flags, section 5.1), the
`Rflags` control (DF and ID at their rflags positions), FS base, GS base, a 512-byte area in the FXSAVE layout
(x87 control, status and tag words, last-instruction pointers, MXCSR and its mask, eight 16-byte x87
register slots, sixteen xmm registers) except that the x87 slots are in physical order R0 to R7, then the
upper halves of the sixteen ymm registers. The stop flag follows, padded to a multiple of 16. The kernel
rotates the x87 slots by TOP when it fills an FXSAVE or XSAVE image (a signal frame, `ptrace`, a core
dump).

### 5.10 What the CPU claims (CPUID), and the AVX setting

- **CPUID** is answered by the meaning from constants, chosen by the leaf and sub-leaf in eax and ecx; no
  hook. The claims are what the default set below says, plus designed answers: vendor "GenuineIntel"
  (because the behaviour matched, including undefined flags, is Intel's), family, model and stepping of the
  reference CPU, a brand string of our own ("wcontainers x86-64"), cache and topology leaves of our own
  design, leaf 0xD sizes for the XSAVE area when XSAVE is claimed, and leaf 0x15 stating a 1 GHz time stamp
  counter. Leaves above the maximum answer as Intel CPUs do (the highest basic leaf's values). The table is
  checked by a designed-answers test, not against the reference.
- **Default claims**: only features every instruction of which maps onto WebAssembly without a software
  slow path, plus the x87 unit the architecture requires: FPU (software, counted, section 5.8), TSC, CX8,
  CMOV, MMX, FXSR, SSE, SSE2, SSE3, SSSE3, SSE4.1, SSE4.2, POPCNT, CX16, LAHF-SAHF in 64-bit mode, LZCNT,
  BMI1, ADX, MOVBE, RDTSCP, FSGSBASE, CLFLUSH, and the 64-bit basics (SYSCALL, NX, LM). This covers the
  x86-64-v2 level. `clflush` is a checked one-byte read of its address, which faults as a load there does
  (a page fault where nothing readable is mapped, general protection where the address is not canonical),
  and then nothing, since there is no cache to flush; the Intel manual gives it a byte load's checks and
  the reference agrees (section 2, run 5). The prefetch hints (`prefetchnta`, `prefetcht0` to `t2`) do
  nothing and never fault, as on the reference.
- **FPU is this plan's reading of the brief, not its letter.** The brief's rule admits by default only
  features every instruction of which maps onto WebAssembly without a software slow path; x87 is always
  software, so by the letter FPU would not be claimed. It is claimed anyway, because x86-64 has no CPU
  without it: the System V ABI puts C's `long double` in x87 registers, every x86-64 compiler and C
  library uses it without asking CPUID, so not claiming it would change no program's path, only make
  the answer untrue. The brief itself plans x87 "as a counted slow path", which is the same reading. The
  owner is told (status file). The other baseline instructions that cost more than one WebAssembly
  operation (64-bit `div` and `idiv` of a 128-bit dividend, section 5.2; `rcpps` and `rsqrtps`, section
  5.8) are done entirely in WebAssembly arithmetic and are not slow paths.
- **Not claimed by default**: AVX, AVX2, FMA, F16C, BMI2 (its `pdep` and `pext` have no WebAssembly form),
  XSAVE and OSXSAVE (needed only to announce AVX state). **Never claimed**: AVX-512 and its relatives, AES,
  PCLMULQDQ, SHA, RDRAND, RDSEED, TSX, MPX, SGX, PKU, CET, AMX, XOP and 3DNow. An instruction of a feature
  not claimed raises illegal instruction, as on a CPU without it.
- **The AVX setting**: one container setting turns on the claim of the x86-64-v3 level: AVX, AVX2, FMA,
  F16C, BMI2, XSAVE and OSXSAVE (with `xgetbv` answering x87, SSE and AVX state). AVX and AVX2 are built in
  full, each 256-bit operation as two 128-bit ones; 128-bit VEX forms clear the upper half, legacy SSE
  forms keep it. FMA is fast where the host probe says the host fuses, else software with "software fused
  multiply-add" counted (section 2). `pdep` and `pext` are software loops, counted "software bit deposit or
  extract". The setting is given with each translation as the guest's `Offer` (section 3.2: `translate`
  takes it, meanings read it through `Emitter::offer()`, `offer_key` puts it in the cache key and the
  description section; `()` for ARM64); the kernel answers `/proc/cpuinfo` and AT_HWCAP2 from the same value.
  Deciding the setting covers the whole v3 level rather than AVX alone is this plan's choice: programs
  built for x86-64-v3 check all of it.
- **The two counters**, x86's own reasons, each with a warning: "a program asked whether AVX is available
  and was told no" (CPUID leaf 1, or leaf 7 sub-leaf 0, read while the setting is off) and "an AVX
  instruction was reached while AVX is not offered" (a `Code` whose claim group in the build-time table is
  the AVX setting, reached while the setting is off, which raises illegal instruction). The counter is
  keyed on the claim group, not on the VEX encoding, because the setting would make exactly those
  instructions run: a VEX-encoded instruction of a feature never claimed (AVX-VNNI, VAES, VPCLMULQDQ, the
  AMX tile instructions) raises illegal instruction under the generic "unsupported instruction reached"
  whatever the setting, so it never suggests the setting would help. BMI2, F16C and FMA are in the AVX
  setting's group, so they count here. No program is restarted automatically.
- The first counter is expected to fire in nearly every glibc program: its start-up
  (`init_cpu_features`) reads both leaves to pick string and math routines. That is intended, not noise to
  narrow away: such a program really did take its non-AVX routines because the answer was no, which is
  what someone deciding on the setting wants to know, and nothing narrower can be seen (which bit the
  program then tests is not visible). The counter counts every read; like every warning (phase one), it is
  raised once per address past its threshold, so a program that reads CPUID from one place warns once.
- **rdtsc and rdtscp** return the clock hook's nanoseconds (a 1 GHz counter, as leaf 0x15 says); `rdtscp`
  gives processor number 0 in ecx. `rdpid` is not claimed (the i7-8750H does not have it, so the
  reference could not prove it) and raises illegal instruction, as on that CPU.

### 5.11 How the lookup of pieces works for byte-aligned, variable-length code

- The lookup array has one slot per byte of code (section 3.4), so every byte address can name a piece,
  including an address inside another instruction, which x86 code may jump to.
- The translator follows code only from its starts and from targets it finds, decoding forward; it never
  decodes backwards. Two pieces may hold overlapping instruction streams; each is correct for its own
  start.
- An address with no piece is translated from that address on first use (bounded look-ahead), as on ARM64.
- Switch tables: section 3.4.

## 6. The kernel's side (designed here, built at X10)

The kernel is CPU-free inside; at its edges it needs an **architecture personality**, one per guest CPU,
resolved at compile time like the translator (the kernel generic over a personality type, replacing today's
`&mut dyn Cpu`). It contains:
- **The register block** type and its conversions (the abi's `Cpu` trait becomes generic over it). ARM64's
  `RegisterBlock` stays; `X86Block` moves into the abi. The user-visible rflags (in `sigcontext`, for
  `ptrace`) is put together from the flags word, the `Rflags` control (DF, ID) and bits 1 and 9, and split
  back the same way at sigreturn, with Linux's `FIX_EFLAGS` mask and AC left clear (section 5.1).
- **The address space**: the user address limit, `TASK_SIZE_MAX`, which is 0x7ffffffff000 on x86-64
  (2^47 less one page) against ARM64's 2^48 (today's `user::ADDRESS_LIMIT`, used by `mmap`, `mremap`,
  `brk` and the stack's placement), with no untagging on x86 (LAM is not offered). `mmap` with a hint or
  `MAP_FIXED` at or above the limit is refused as Linux refuses it (`ENOMEM`, or the hint ignored). This
  is what keeps the last page below 2^47 unmapped, on which section 5.4's general protection relies; the
  designed-answers test of X10 maps the highest page Linux allows and checks an access that runs off it
  is general protection.
- **Code coherence**: nothing for the kernel to do on x86. Its writes into guest memory and the aliases of
  shared pages are seen by the CPU side, which owns `Cpu::write`, `zero`, `shared_write` and the shared
  mappings (section 5.7); the kernel keeps announcing mappings through `code_changed` as now.
- **System call numbering and decode**: the number and argument registers, the result register, how a call
  is restarted (pc back by the system call instruction's length: 4 on ARM64, 2 for x86 `syscall`), and the
  dispatch table built from the guest's own generated Linux constants (x86-64's numbers, its 57 legacy
  calls such as `open`, `stat`, `fork`, `pipe`, `select` and `poll` mapped onto the shared typed
  functions).
- **Data layouts** that differ: `stat`, the packed `epoll_event`, `O_*` flags, `semid64_ds` padding,
  `sigaction` with `sa_restorer`, `ucontext`. Generated from the x86-64 headers the way the ARM64 ones are
  from `linux-raw-sys`'s aarch64 files.
- **Signal frames and sigreturn**: x86's `rt_sigframe` with the mandatory restorer, general registers in
  `sigcontext`, the FXSAVE/XSAVE image (x87 slots rotated by TOP, ymm upper halves when AVX is claimed),
  the 128-byte red zone skipped below rsp, and the frame aligned so rsp + 8 is a multiple of 16 at the
  handler's entry. DF is cleared on entry.
- **Clone, exec and the auxiliary vector**: x86 `clone`'s argument order (flags, stack, parent tid, child
  tid, tls); the child's registers (rax 0, the new stack, FS base from tls); exec's start registers (rsp at
  argc, rdx 0); AT_HWCAP (CPUID leaf 1 edx), AT_HWCAP2 (FSGSBASE), AT_PLATFORM "x86_64", AT_MINSIGSTKSZ.
- **`arch_prctl`** (set and get FS and GS; CPUID faulting and AMX permission reported as not supported),
  and the faults and their signals (section 3.2's trap kinds: divide error SIGFPE `FPE_INTDIV`, general
  protection SIGSEGV `SI_KERNEL` with address 0, float exceptions SIGFPE, single step SIGTRAP
  `TRAP_TRACE`, `int3` and `int 3` SIGTRAP `SI_KERNEL`, `int1` SIGTRAP `TRAP_BRKPT`, each breakpoint
  with rip after it). The abi's `Exit` grows these kinds. A memory fault reaches the kernel already
  classified by the runner through `Guest::memory_fault` (a page fault with its address, or general
  protection), so the kernel stays `no_std`, CPU-free and without a CPU crate; hooks never reach it.
- **The machine description**: ELF machine 62, `uname` "x86_64", `/proc/cpuinfo` in x86's format with the
  claimed flags, seccomp's `AUDIT_ARCH_X86_64`, and an x86-64 vDSO (`clock_gettime`, `gettimeofday`,
  `time`, `getcpu`). The legacy vsyscall page is not provided: a call into it faults, as on a kernel booted
  with `vsyscall=none`; recorded as a gap.
- **Its tests**: the kernel test crate's stand-in CPU, reference and guest programs per personality (the
  kernel track's Unicorn stand-in has an x86-64 mode; its reference becomes the x86 machine).

This milestone waits for the kernel track's own milestones, because that half is still being built; until
then this track touches neither the kernel crates nor `wcontainers-abi`.

## 7. Testing

### 7.1 Principles

The same three layers as ARM64, with the real CPU as the reference: results on the i7-8750H decide what
is correct. Every random test is seeded, replayable and has a knob for how many cases run; the cases
actually run are recorded in `docs/status-x86.md`. `proptest` generates and shrinks; Csmith and C-Reduce do
the same for C. Look-ahead none is the reference for every other look-ahead, in both settings of "optimise
for".

### 7.2 The shared harness

`wcontainers-translator-test` becomes generic over a test-side trait, `GuestTest: Guest`, implemented for
each CPU in the test crate (`src/guest/arm64.rs`, `src/guest/x86.rs`), so test-only dependencies (proptest,
dynasm, iced's encoder and assembler) never reach the CPU crates. Every test and the harness are written
once over `G: GuestTest`. What is generic from `Guest` alone needs nothing here: instruction boundaries in
a code range (mixed pieces' random starts, the real binaries' instruction counts) come from decoding
forward with `G::decode`, slot granularity from `G::ALIGN`, traps from `G::trap_kind`, the float and x87
hook bodies from `G::run_hook`, and the code-change policy from `G::CODE_CHANGES`.

```rust
/// What the harness needs of one guest beyond `Guest`.
pub trait GuestTest: Guest {
    // --- names ---
    /// Rust target of the reference runner and native programs: `aarch64-unknown-linux-musl`,
    /// `x86_64-unknown-linux-musl`.
    const RUST_TARGET: &'static str;
    /// zig's and clang's target: `aarch64-linux-musl`, `x86_64-linux-musl`.
    const C_TARGET: &'static str;
    /// Compiler flags for "general registers only": `-mgeneral-regs-only` on both.
    const GENERAL_ONLY: &'static [&'static str];
    /// ELF `e_machine`: 183, 62.
    const ELF_MACHINE: u16;
    /// Docker's platform for images (and for the reference when the host is this CPU): `linux/arm64`,
    /// `linux/amd64`.
    const DOCKER_PLATFORM: &'static str;
    /// musl's loader inside images: `ld-musl-aarch64.so.1`, `ld-musl-x86_64.so.1`.
    const MUSL_LOADER: &'static str;
    /// The environment variable naming the reference's SSH host when the host is not this CPU:
    /// `WC_REFERENCE_AARCH64`, `WC_REFERENCE_X86_64`.
    const REFERENCE_HOST: &'static str;
    /// What a dynamic relocation type of a real binary does (x86: `R_X86_64_RELATIVE` and `IRELATIVE`
    /// relative; `_64`, `GLOB_DAT`, `JUMP_SLOT` absolute).
    fn relocation(kind: u32) -> Relocation;
    /// One instruction as text for failure messages (ARM64 yaxpeax-arm's, x86 iced's formatter).
    fn disassemble(code: &[u8]) -> String;

    // --- layer 1: the forms ---
    type Form: Copy + Eq + core::fmt::Debug + 'static;
    /// Every form in a fixed order: ARM64 the lines of a64-forms.txt; x86 one per iced `Code` (section 7.4).
    fn forms() -> &'static [Self::Form];
    /// The seed key, stable across runs, and the group the summary counts under.
    fn form_name(form: Self::Form) -> &'static str;
    fn form_group(form: Self::Form) -> &'static str;
    /// The random cases of a form, whole: the instruction with its operands shaped (memory bases and
    /// indexes into DATA, or for x86 a share in the last 64 bytes below 2^47; the alignment the presented
    /// CPU needs; biases such as system registers or CPUID leaves) and the starting state from this CPU's
    /// own value strategies (edge values, 80-bit values, NaN-boxing). One strategy per CPU, so ARM64's
    /// strategy is today's `cases(form)` mapped onto `Case` and draws the same values for each seed.
    fn cases(form: Self::Form, offer: Self::Offer) -> BoxedStrategy<Case>;
    /// `Ok` when an instance must run as on the reference with `offer`; `Err(reason)` when it must stop
    /// with illegal instruction (a feature not offered).
    fn presented(code: &[u8], offer: Self::Offer) -> Result<(), &'static str>;
    /// A reason to leave an instance out (a jump to itself, a designed answer, ARM64's unpredictable
    /// encodings, x86 `int 0x80` and selector 0x23), else `None`.
    fn excluded(code: &[u8]) -> Option<&'static str>;
    /// Random bytes that are no instruction of any form (x86: what iced decodes as `INVALID`, among them
    /// prefix runs that take an encoding past 15 bytes, which give general protection, section 5.2).
    fn no_form() -> BoxedStrategy<Encoded>;
    /// The features offered with `offer`, by name, and those the guest is told of (ARM64 the HWCAP bits
    /// decoded; x86 the CPUID answers decoded): a test checks they are the same set.
    fn offered(offer: Self::Offer) -> Vec<&'static str>;
    fn told(offer: Self::Offer) -> Vec<&'static str>;
    /// The designed answers (identity registers, CPUID leaves, rdtsc, xgetbv, AC reading back clear,
    /// `int 0x80`): each an instruction, a starting state and the state and stop it must leave.
    fn designed(offer: Self::Offer) -> Vec<Designed>;

    // --- the comparable state ---
    /// The fields compared, in wire order (x86: 16 general registers, rip, rflags, FS and GS bases, MXCSR,
    /// FCW, FSW, FTW, R0 to R7, xmm0 to xmm15, the ymm upper halves).
    const FIELDS: &'static [Field];
    /// The thread state (`BLOCK_SIZE + PRIVATE_BYTES` bytes) as field values in `FIELDS` order, and back.
    /// x86 puts rflags together from the flags word, `Rflags` and bits 1 and 9 (section 5.1).
    fn to_state(thread: &[u8]) -> State;
    fn from_state(state: &State, thread: &mut [u8]);
    /// Per field, the bits allowed to differ in this case given both outcomes: sticky float
    /// flags and x87 exception bits while not kept, TF in the copies of rflags an instruction makes
    /// (`pushf`'s word, r11 after `syscall`), AC after `popf` (section 5.1), what a faulting access leaves
    /// unknown (ARM64), x87's last-instruction pointers. Never an undefined flag.
    fn may_differ(case: &Case, reference: &Outcome, translated: &Outcome) -> State;
    /// A reference signal as a generic stop, given the instruction's length: section 7.3's list (x86's
    /// held-back trap, general protection as `SI_KERNEL` with address 0, a breakpoint as
    /// `Breakpoint(si_code)`); ARM64 today's `reference_outcome`, with a `SIGTRAP` as
    /// `Breakpoint(si_code)`.
    fn reference_stop(signal: Signal, length: u8) -> Stop;
    /// The `si_code` Linux gives the breakpoint trap whose detail is `detail`, for the translated side's
    /// `Breakpoint`: ARM64 `TRAP_BRKPT` (1) for every `brk` (seen in Docker, section 2, run 5); x86 the
    /// detail itself, which its meanings set to `TRAP_BRKPT` for `int1` and `SI_KERNEL` (128) for `int3`
    /// and `int 3` (section 5.2).
    fn breakpoint_code(detail: i64) -> u32;

    // --- blobs, the stand-in and the programs ---
    /// A flat blob's auxiliary vector entries (AT_HWCAP, AT_HWCAP2, AT_PLATFORM), in Linux's order.
    fn auxiliary(offer: Self::Offer) -> Vec<(u64, u64)>;
    /// The stand-in's system call numbers: ARM64 write 64, writev 66, exit 93, exit_group 94, brk 214;
    /// x86 1, 20, 60, 231, 12.
    const STAND_IN: StandIn;
    /// The folder of per-CPU program sources, `programs/<this>/`: `start.S`, `syscall.h`, `switch.S`, the
    /// assembly tests, `written_code.c` (on x86, writing code with no announcement) and `differences.c`.
    /// The C bodies and `flat.ld` stay shared.
    const PROGRAMS: &'static str;
    /// The targeted corpora of smoke.rs, checks.rs and changes.rs, as named pieces of code with what each
    /// must do; the test bodies stay generic.
    fn smoke() -> Vec<Snippet>;
    fn checks() -> Vec<Snippet>;
    fn changes() -> Vec<Snippet>;

    // --- layer 2 ---
    /// The registers the shared program model may use (ARM64 today's 26, in today's order, so the same
    /// draws pick the same registers).
    const MODEL_REGISTERS: &'static [Self::Reg];
    /// What this CPU adds to the model (x86: read-change-write operands, string instructions, push and
    /// pop, calls through memory, inc and dec before a jcc on CF, red-zone use), drawn only when `Some`:
    /// ARM64 gives `None`, so no extra value is drawn and its programs per seed stay the same.
    type Extra: Clone + core::fmt::Debug;
    fn extra() -> Option<BoxedStrategy<Self::Extra>>;
    /// The program in this CPU's code: ARM64 dynasm (today's `assemble`), x86 iced's `CodeAssembler`.
    fn assemble(program: &Program<Self::Extra>) -> Blob;
}

pub enum Relocation { Relative, Absolute, Other }
/// An instruction's bytes: the first `length` of `bytes`.
pub struct Encoded { pub bytes: [u8; 16], pub length: u8 }
/// A compared field: its name, its width in bits (at most 128), and the bits compared.
pub struct Field { pub name: &'static str, pub bits: u8, pub compared: u128 }
/// One value per field of `G::FIELDS`.
pub struct State(pub Vec<u128>);
pub struct Case { pub code: Encoded, pub state: State, pub tracking: bool, pub seed: u64 }
/// What the reference reported: the signal, its `si_code` and `si_addr`, and the pc in the frame.
pub struct Signal { pub number: u32, pub code: u32, pub address: u64, pub pc: u64 }
/// Where control went or why the thread stopped, in terms both sides share (today's `Stop`, grown).
/// `Breakpoint` holds the `si_code` (so `int1` and `int3` are told apart); `FloatException` likewise.
pub enum Stop { Landed(u64), Illegal, Breakpoint(u32), SingleStep, Syscall, MemoryFault(u64),
    Alignment(u64), GeneralProtection, DivideError, FloatException(u32), Other(u32, u64) }
pub struct Outcome { pub stop: Stop, pub state: State, pub changed: Vec<(u32, u8)> }
pub struct Designed { pub code: Encoded, pub start: State, pub end: State, pub stop: Stop }
pub struct StandIn { pub write: u64, pub writev: u64, pub exit: u64, pub exit_group: u64, pub brk: u64 }
pub struct Snippet { pub name: &'static str, pub code: Vec<u8>, pub expect: Expect }
```

`Expect` is today's expectation of each corpus (the smoke output, the checks' counts, the changes' stops),
unchanged. A translator trap maps to a `Stop` and to an exit status generically, from `TrapKind` and the
signal section 6 gives it (128 + 8 for SIGFPE); a breakpoint's `si_code` comes from `breakpoint_code` and
its detail.

The reference runner (crates/wcontainers-translator-test/reference) is one program built for
`G::RUST_TARGET`; its per-CPU part is a module chosen by `cfg(target_arch)` (`reference/src/aarch64.rs`,
`reference/src/x86_64.rs`) implementing:

```rust
/// The reference runner's per-CPU part; the wire format, the seccomp filter, the forking and the blobs
/// are shared.
trait Machine {
    /// The byte the code area is filled with: ARM64 0 (`udf #0`), x86 0xCC (`int3`).
    const FILLER: u8;
    /// Bytes after CODE that the seccomp filter treats as the case's own: ARM64 4, x86 16.
    const WINDOW: u64;
    /// The field names, sent first on every connection; the harness checks they are `G::FIELDS`' names.
    const FIELDS: &'static [&'static str];
    /// Write a case's state into the signal frame (x86: the general registers, rflags with TF set, the
    /// x87, SSE and AVX components and `xstate_bv` bits 0 to 2), and set the thread pointers (ARM64
    /// tpidr_el0, x86 `arch_prctl` FS and GS).
    fn enter(context: &mut libc::ucontext_t, state: &[u128]);
    /// Read the frame back into a state (a component whose `xstate_bv` bit is clear as its first state),
    /// after giving the runner its own thread pointer back.
    fn leave(context: &libc::ucontext_t, state: &mut [u128]);
    /// Whether the stop is a repeated string instruction that has not finished (x86: `TRAP_TRACE` with
    /// rip still at CODE), which the runner steps on.
    fn unfinished(context: &libc::ucontext_t, signal: &libc::siginfo_t) -> bool;
    /// Make written code visible to the CPU: ARM64 `dc cvau` and `ic ivau`, x86 nothing.
    fn synchronise(code: &[u8]);
    /// A blob's starting registers (x86: rflags 0x202, MXCSR 0x1F80, FCW 0x037F, rsp at argc, rdx 0).
    fn blob_start(context: &mut libc::ucontext_t, entry: u64, stack: u64);
}
```

The wire format is length-prefixed records in both directions. A case is the instruction's length and
bytes, the seed of its data area, and the state as `FIELDS.len()` 16-byte little-endian values; its result
is a status of its own (ran, or runner failed with a message), the signal, `si_code`, `si_addr`, the
state after, and the changed data bytes as (offset, value) pairs. A blob is its bytes, its load address,
its standard input and how many runs to time; its result is its output, its exit status and the best time.

The transport is generic: `Reference::<G>::start()` runs the runner as a child process and talks to it
over its stdin and stdout. When the host's CPU is the guest's (`std::env::consts::ARCH` against
`RUST_TARGET`), the child is `docker run -i --platform G::DOCKER_PLATFORM`, as today; otherwise it is
`ssh` with `ControlMaster` to the host in `G::REFERENCE_HOST`, after uploading the runner to
`~/wcontainers-x86/reference/<content hash>` when it is not there. Both are the same records over the same
pipes, so nothing else differs. `start` fails with a message naming the variable when it is unset; the
test then fails, never skips. `cases(&[Case]) -> Vec<(Signal, State, Vec<(u32, u8)>)>`, `blob(..)` and
`cpu_model()` (read once from `/proc/cpuinfo` on the reference, recorded with every result) are its
methods.

The browser's cdylib is built per guest (a cargo feature naming the guest, `arm64` or `x86_64`, chooses
`type G`). Its `contract()` JSON gives what `runner.js` needs, all from `Guest`: `blockSize`,
`privateBytes`, `threadStateSize`, `pcOffset`, `flagsWordOffset`, `stopFlagOffset`, `align` (one slot per
`G::ALIGN` bytes), `syscall` (the block offsets of the number, the six arguments and the result, from
`SYSCALL` and `BITS`), `codeChanges` (`announced` or `detected`), `traps` (each `TrapKind` with its number
and signal), `hooks` (each of `HOOKS` with its index and body), and the probe module, beside today's
translator numbers. ARM64's `registerFpsr`, `registerFpcr` and `floatTracking` go: the float hook's work is
`cpu_hook` (section 3.4). The stand-in's five numbers come from `GuestTest::STAND_IN` through the page's
settings, which the test crate's browser.rs writes.

Each test file has one test per guest (`layer1_arm64`, `layer1_x86_64`, ...), so `cargo test` runs both.
An x86 test that needs the reference machine and finds it not configured fails with a message that says
so; it is never skipped silently.

### 7.3 The x86 reference runner and how it is reached

- **The runner** is a static Rust program built on the Mac for `x86_64-unknown-linux-musl` with rust-lld
  (the toolchain file gains the target), uploaded to `~/wcontainers-x86/reference/<content hash>` when
  that file is not already there. It needs nothing installed on the machine.
- **Transport**: one SSH connection kept open for a whole run (`ControlMaster`), the host named by
  `WC_REFERENCE_X86_64` (`tom@192.168.2.240` today). Cases and blobs go through stdin as length-prefixed
  binary records; results come back on stdout the same way, each with its own status, so ssh's own exit
  status (255) and signals are never mistaken for a guest's. A new connection costs from about 0.24 s to
  over 1 s with the machines loaded, and a new process on the machine about 1 ms (section 2), so
  connections are never made per case and a runner
  process runs many cases.
- **Layer 1, batched**: cases are a deterministic function of seed and form, so the harness sends a form's
  cases in one batch, collects the results and compares; only a failure goes back to proptest's shrinking,
  which then asks one case at a time. Up to 10 runner processes at once on the x86 machine.
- **Isolation**: the runner forks per batch of cases and the child runs the cases; a case that damages the
  child (a write through a random pointer into the runner's own memory) shows as a child that dies, and that
  batch is rerun one case per child.
- **One instruction** (section 2): the code area is filled with `int3`; the instruction is placed at CODE;
  the handler (on an alternate stack, using no thread-local storage and only raw system calls) writes
  every register into the frame including all vector registers, the whole x87 component (control, status
  and abridged tag words, the eight slots in stack order, rotated from the case's physical R0 to R7 by
  TOP, the last-instruction pointers zero) and the XSAVE header bits 0, 1 and 2 (section 2, run 4), sets
  the FS and GS
  bases with `arch_prctl`, sets TF and returns. The next signal is read back, knowing the instruction's
  length L:
  - `SIGTRAP` with `TRAP_TRACE` (2): landed at rip.
  - `SIGTRAP` with `SI_KERNEL` (128) and rip CODE + L + 1: the instruction ran and fell through with its
    trap held back (`mov ss`), and the `int3` filler at CODE + L stopped it. Reported as landed at CODE + L
    (the filler changes nothing but rip). Any other `SI_KERNEL` stop is a breakpoint of the instruction
    itself (`int3`, `int 3`), rip after it: `Breakpoint(128)`.
  - `SIGTRAP` with `TRAP_BRKPT` (1): `int1`, rip after it: `Breakpoint(1)`.
  - `SIGSEGV`, `SIGBUS`, `SIGILL`, `SIGFPE`: faults with their codes (`SI_KERNEL` with address 0 is general
    protection).
  - `SIGSYS`, from a seccomp filter on the instruction window `[CODE, CODE + 16)`: a system call, for
    `syscall` and for `int 0x80` (seen by seccomp as an i386 call), which never runs.
  Anything else (a stop elsewhere, a signal not listed) is reported as such and fails the case; nothing is
  mapped by guesswork. A repeated string instruction is stepped on while rip stays at CODE. The runner's
  own FS base is set back before anything else.
- **Blobs** (layers 2 and 3 natives): the runner forks per blob, maps it and runs it with the blob start
  state, and reports output and status in-band. Docker is not used on the x86 machine.
- **Recorded results**: every reference result is stored under `target/reference/x86_64/` keyed by the
  reference CPU's model string, the runner's content hash and the case's hash, so a replay or a run while
  the machine is off uses the real CPU's recorded answer. A case with no recorded answer and no machine
  fails.
- **ARM64** keeps its local Docker reference behind the same interface (the transport is chosen by the
  guest: local Docker when the host's CPU is the guest's, SSH otherwise).

### 7.4 The three layers for x86-64

**Layer 1: one instruction at a time.** The forms list comes from iced-x86's own tables: one form per iced
`Code` whose `OpCodeInfo` says it is valid in 64-bit mode (`mode64()`), is an instruction (`is_instruction()`,
not a directive), has an encoding that is not 3DNow, XOP, MVEX or EVEX (`encoding()`), and whose every
`Code::cpuid_features()` entry is in the offered set: one list for the default claims and one for the AVX
setting. Forms that `is_privileged()` marks kernel-only stay in the list as their own group: their case passes
when the translator stops with the fault the reference CPU gives (section 5.2). The forms not offered are a
third list, whose cases must stop with illegal instruction. Its name (the `Code` name) is the seed key; its
group is its mnemonic and encoding. A case is a random `Instruction` for the `Code`, with operands from
`op_kind(i)` (registers, immediates weighted toward edges, memory operands whose base and index point into
DATA), encoded by iced's `Encoder`, which gives the bytes and length. Random registers, flags, vector, x87 and
MXCSR values are weighted toward edge values; MXCSR is the default in half the cases. The reference runs it by
the trap flag; the translator runs it at look-ahead none; every register, every flag (including undefined
ones), the touched memory and the stop are compared. Random byte strings that iced decodes as `INVALID` are
compared with the CPU's stop: SIGILL, or general protection for the ones past 15 bytes (section 5.2), which
`no_form` draws on purpose as runs of prefixes before a body; valid ones also check the length (where the
CPU lands). Exclusions, each
with a reason: a jump to itself; the instructions answered by design (CPUID, rdtsc, rdtscp, xgetbv), which a
designed-answers test checks instead; and `int 0x80`, which the reference kernel runs as a 32-bit system call
and this phase does not provide (section 5.2), also checked by the designed-answers test.

**Layer 2: random small programs**, from the shared program model (loops with counters, calls, bounded
recursion, tail calls, calls and jumps through registers, jump tables in both x86 compiler styles, frames,
groups of accesses through one base, walks, lookups, system calls, a final dump of every register and
the flags) with an x86 emitter written with iced's `CodeAssembler` (test only) that adds what x86 has:
read-change-write memory operands, string instructions with and without rep, push and pop, calls through
memory, partial flag writes (inc and dec before a jcc on CF), red-zone use in leaf functions, and `pushfq`
for the dump. ARM64 keeps its dynasm emitter behind the same model, and its programs for a given seed stay
byte for byte what they are today.

**Layer 3: real compiled programs.** Csmith programs and the hand-picked programs (number printing,
sorting, setjmp and longjmp, C++ exceptions, Rust panics), built for `x86_64-linux-musl` with zig's clang
on the Mac and with gcc from an Alpine x86-64 image run on the Mac (Docker's emulation is used only to
compile, never as a reference), at every optimisation level, as flat blobs at a fixed address and
position-independent, run natively on the x86 machine and translated in wasmtime and Chrome.

Across layers 2 and 3, as for ARM64: every look-ahead against none, mixed pieces, injected rare events
(stack lost, no piece yet, stop flag raised), permission changes at random points, and now also code written
without announcement at random points.

### 7.5 Benchmarks

The benchmark set is built for x86-64 and run on the x86 machine itself: natively, in wasmtime and in
Chrome there, so "times native" compares like with like. This needs wasmtime and Chrome installed on that
machine (each install recorded in the status file). The same translated benchmarks also run in wasmtime
and Chrome on the Mac (an ARM host, so with the NaN fix-ups), reported without a native ratio. The load
probe reads `/proc/loadavg` on Linux and `sysctl -n vm.loadavg` on macOS, and is recorded with every time.

## 8. Memory ordering between parallel guest threads (the one open question)

x86 promises total store order to programs; WebAssembly's plain loads and stores on an ARM host promise
less. Until guest threads run in parallel on several host threads, one guest thread runs at a time and the
ordering holds for free. A measurement of what atomic accesses cost is under way elsewhere. **Question for
the owner, not decided here**: when guest threads run in parallel, how x86's ordering is kept (for example
every guest store and load as a WebAssembly atomic, only some of them, or x86 guests kept to one running
thread). Nothing in this plan depends on the answer: the interface will gain a "memory model" property of
the guest when it is decided.

## 9. Milestones and pass bars

Measured throughout, recorded with machine and load: translation speed, output size against input size
(raw and compressed), engine compile time, run time against native.

- **X0, the plan and the reference proven.** This document; the iced-x86, fused multiply-add and
  single-step questions answered from their sources; a static x86-64 program built on the Mac run on the
  x86 machine over SSH; one arbitrary instruction run from given registers and stopped right after it, with
  the registers read back. Pass bar: an agent can build X1 from this document without asking anything.
- **X1, the interface, with ARM64 on it.** `wcontainers-guest-cpu` with section 3's trait and types;
  `wcontainers-arm64` implements it; `wcontainers-translator` is generic over it and no longer depends on
  `wcontainers-arm64` (`cargo tree -p wcontainers-translator` shows it); the test crate and the browser
  crate name `Arm64` where they call the translator; the float hook's host side moves into
  `Arm64::run_hook`, and the host runner and the browser's cdylib call it instead of `exact::run` (the
  browser's `cpu_hook` export and runner.js's copy of the state come with it, section 3.4 Hooks); the
  effects recorder and `Registers` move into the interface crate (section 3.1); the runners ask
  `G::memory_fault` before they report a memory fault (ARM64 always `Page`, so nothing they report
  changes). Pass bar: the WebAssembly output is byte for byte the
  same as before the move for the benchmark set and all nine real binaries of M5, in both settings
  (`WC_BINARIES_DUMP` before and after, compared with `cmp`; the benchmark set's modules likewise, through
  a knob `WC_BENCH_DUMP` that the X1 builder adds to tests/benchmarks.rs first, as its own commit before
  any interface change, writing each translated module as `<directory>/<benchmark>-<look-ahead>-
  <optimise>-<i>.wasm` the way binaries.rs does, so that the "before" modules exist; the thread module of
  `thread_module()` is compared too); the
  whole test suite of the four crates passes; the benchmarks in wasmtime and Chrome are unchanged within
  noise (best of 3 runs each, before and after, load recorded). The move is done on this branch's copy of
  the translator; fixes the translator track makes later come in by a merge the orchestrator names, and
  are moved onto the interface then.
- **X2, the shared test harness.** `GuestTest` and the ARM64 implementation; the harness, runners, browser
  build (feature-chosen guest, contract JSON with the per-CPU numbers) and reference transport generic; the
  x86 reference runner built, uploaded and proven on the x86 machine through the harness's transport on a
  hand list of cases (as section 2's experiment, now in the repository) and on a blob. Pass bar: every
  ARM64 test passes with the same seeds giving the same cases and programs (layer 1 cases and layer 2
  programs byte for byte the same per seed); the x86 reference answers a hand list of at least 30
  instructions with what the Intel manual says, each checked by hand in the test, among them every stop
  of section 7.3's list (`mov ss` with its held-back trap, `int3`, `int 3`, `int1`, `syscall` and `int
  0x80` as `SIGSYS`, a `ret` to a non-canonical address), x87 instructions whose state is written and read
  back (section 2, run 4) and a `call far` with selector 0x33. The trait of section 7.2 is the one built;
  where building it shows a signature must change, the change is recorded in the status file.
- **X3, x86 at look-ahead none, integers.** `wcontainers-x86` with the decoder, the block, the base integer
  instructions (moves, arithmetic and logic at every width, lea, push and pop, call and ret, jmp and jcc, cmov
  and setcc, shifts and rotates, multiply and divide, string instructions without rep, syscall), lazy flags,
  the build-time claim table from iced's tables (section 2) with its test, the canonical check on branches
  through a register or memory (a call's after its push), `X86_64::memory_fault`, the 128-by-64 division
  (section 5.2), the 15-byte limit with its table of invalid opcodes' lengths and the sweep it comes from
  (section 5.2), the description section's guest and offer bytes (format 4; ARM64's output then
  differs in those bytes and the format byte only). Pass bar: layer 1 for those forms against the real CPU
  with zero mismatches at 1,000 cases a form, including bases in the last 64 bytes below 2^47 and
  dividends whose rdx is not rax's extension, and 10,000 no-form cases, half of them prefix runs near 15
  bytes, with zero mismatches; a C hello world built as a flat x86-64 blob prints and exits in
  wasmtime and Chrome; what the decoder adds to the browser build measured; the long division's cost
  against native measured.
- **X4, x86 at full look-ahead.** Functions, registers in locals, flags liveness, direct conditions, calls
  and returns as WebAssembly calls with the pushed return address, the red zone frames, switch recognition
  for both x86 compiler styles, loop proofs, the stop and permission checks; the x86 layer 2 emitter. Pass
  bar: layer 2 at 10,000 programs with zero mismatches against the CPU and against none; the benchmark set
  gives the same results at none and full and is timed on the x86 machine natively, in wasmtime and in
  Chrome.
- **X5, every default instruction.** All integer instructions of the default claims, SSE through SSE4.2,
  MXCSR fast and exact paths, NaN rules and the probe, CPUID and its designed answers, rep string
  instructions (bulk and one element), FS and GS, the approximate reciprocals by section 5.8's formula. The
  passed-register lists measured and chosen. Pass bar: layer 1 complete for the default forms at 1,000
  cases a form, layer 3 Csmith at 1,000 programs, zero mismatches; the reciprocal formula equal to the
  reference CPU on all 2^32 inputs of `rcpss` and of `rsqrtss`.
- **X6, bounded look-ahead, the rare paths and code written without announcement.** Translating from any
  byte address, the bottom loop, go to address, adding code while running, injection tests, write
  protection of translated pages with the unwind, both runners' stand-ins of it, a test program that writes
  and runs code with no announcement; setjmp, C++ exception and Rust panic programs. Pass bar: those tests
  pass at every look-ahead, in wasmtime and Chrome.
- **X7, x87 and MMX.** The x87 hook with `X87DoubleExtended`, precision control by rounding to odd,
  `fsqrt` by section 5.8's root, the
  transcendental instructions with `astro-float`, MMX registers, `fxsave` and `fxrstor`. Pass
  bar: layer 1 for every x87 and MMX form at 1,000 cases a form, zero mismatches except known failures
  recorded with their reasons; a C program using `long double` gives the native output.
- **X8, the AVX setting.** AVX, AVX2, FMA, F16C, BMI2, XSAVE in the setting, the two counters, the setting
  in the cache key, the instructions of the setting's claim group trapping with the counter when it is off.
  Pass bar: layer 1 for every
  form of the AVX list at 1,000 cases a form, zero mismatches; the benchmark set built for x86-64-v3 runs
  with the setting on, and with it off stops with illegal instruction and the counter.
- **X9, real x86 code at scale.** The amd64 builds of the nine real binaries of M5 (busybox, the C library,
  ripgrep, git, Node, Python, Bun, a Java runtime, a Go program) translated in both settings and loaded in
  both engines. Recorded: sizes, translation time, engine memory, the lookup array's memory, how many
  switch tables were over the limit or had absolute entries.
- **X10, the kernel's x86-64 personality** (after the kernel track's milestones): section 6, and the
  kernel's tests for x86-64 against the x86 machine; the CPU side's part of code written without
  announcement for the kernel's writes and shared pages (section 5.7) with its three tests; the address
  limit and its designed-answers test.

The long runs to the pass bars of section 1 are started by the orchestrator at the end of the track, each
one command recorded in the status file.

## 10. Ideas parked for after everything is built (recorded, not built)

- A look-ahead bounded by call-graph distance (this function plus its callees to a depth) beside none,
  bounded and full.
- Replacing the guest's memory copy, fill, compare and search routines with WebAssembly's bulk operations.
- Recording a run and translating only what ran (the owner's idea, for output size).

## 11. Tuning numbers

Internal constants, part of the cache key, as in phase one: x86's passed registers in and out (starting
values in section 3.4, measured at X5), the bulk string chunk (1 MiB), and phase one's own numbers, which
apply unchanged.

## 12. Not yet measured

- What the x86 translator downloads in the browser once ARM64-only code is left out (the decoder alone
  adds about 47 KB brotli, section 2).
- The lookup array's memory on real x86 programs (four bytes per byte of code, expected).
- How many switch tables exceed the br_table limit at one entry per byte, and how many have absolute
  entries.
- The cost of lazy flags with PF and AF on demand, and of partial flag setters, on real programs.
- The cost of the canonical-address check on every `ret` and every jump or call through a register or
  memory (four operations each, section 3.4).
- The cost of the 128-by-64 long division against the CPU's own `div r64` (section 5.2), and of the
  approximate reciprocals' formula (about 30 vector operations for four lanes, section 5.8).
- The size of the build-time claim table (about 5 KB expected) and of `astro-float` in the real x86
  browser build (40,947 bytes brotli in a scratch cdylib, section 5.8).
- How often programs translate code from a live stack frame, and how often parallel threads must be
  stopped to protect code (section 5.7).
- The cost of looking up the code marks on every kernel write into guest memory (one lookup per page
  written, section 5.7), measured at X10.
- Bulk against element-by-element string instructions on real programs.
- How often code is written without announcement in real programs, and what JITs that write next to
  running code cost.
- The NaN fix-up's cost for x86 guests on ARM hosts.
- x87 in software against native on programs that use `long double`, and how many transcendental cases
  the reference rounds differently from the correctly rounded result.
- The passed-register lists for x86.
- Engine compile time and memory for x86 output, which is expected to differ from ARM64's because x86
  instructions do more each.
- Not seen on the reference: which fault the CPU gives for an encoding past 15 bytes that runs into an
  unmapped page (section 5.2 treats it as running past the end of the code range); layer 1 places no case
  there today.

## 13. Where the evidence is

Experiments and downloads of milestone X0 are in `pyro-parked/wcontainers-scratch/x86`: `iced-size` (the size
builds), `dis` (an iced-based disassembler used to check encodings), `reference-proof` (`hello.c`,
`stepper.c`, `cases.sh`, `cases2.sh` and their outputs), `sources` (the relaxed SIMD overview and sandsifter's
`injector.c`, and in run 3 RISU's source in `sources/risu`), `x87-crate` and `x87-crate-base` (astro-float's
size in a wasm32 cdylib), `x87-nostd` (astro-float without the standard library), `apfloat-pc` (precision
control by rounding to odd with rustc_apfloat); run 3 added to `reference-proof` `stepper3.c`, `approx.c`,
`approx_dump.c` with its output `approx_tables.txt`, `approx_formula.c` and `approx_mxcsr.c`. The second
independent check's seccomp stepper is `check2/exp/stepper_sc.c`. Run 4 added to `reference-proof`
`stepper4.c` (made from `stepper3.c` by `scripts/make_stepper4.py`), `cases4.sh`, `cases5.sh` and their
outputs, and `cases6.out`. Run 5 added `cases7.sh`, `cases8.sh` and `sweep15.sh` with their outputs
(the 15-byte limit, `clflush`, invalid opcodes' lengths; `sweep15.iced` and `sweep15.classes` set the
sweep beside iced's decoding) and `brk.c` (ARM64's breakpoint `si_code`); the fourth independent check's
cases are in `check4`. The x86
machine holds copies in
`~/wcontainers-x86/proof` (with `approx.out`, `approx_formula.out`, `approx_mxcsr.out`) and
`~/wcontainers-x86/check2`.

## Appendix A. Every kind b and c place of the seam map, and where it is covered

Translator area (seam map entry, then section):
- T1 the translator imports the ARM64 crate by name: 3.1, 3.2 (the translator generic over `Guest`, no
  dependency on a CPU crate).
- T2 `Reg { X, Zero, Sp }` matched in generic code: 3.2 `reg_bit`, `SP_BIT`, `bit_reg`; 3.4 Registers.
- T3 the `Registers` bit layout: 3.2 `BITS`; 3.4 Registers.
- T4 4-byte words, alignment checks, decode of a u32: 3.2 `decode`, `ALIGN`, `MAX_LENGTH`; 3.4 Instruction
  unit.
- T5 `Decoded { word }` and the trap detail: 3.2 `trap_detail`; 3.4 Trap detail.
- T6 fixed 4-byte stepping: 3.4 Instruction unit (offset lists, lengths).
- T7 `Flow` without calls, returns or register jumps: 3.2 `Flow`, `FlowKind`; 3.4 Control flow.
- T8 adr and adrp + add: 3.2 `built_address`; 3.4 Addresses the code builds.
- T9 switch window read backwards: 3.2 `jump_table`; 3.4 Switches.
- T10 br_table over target/4: 4 decision 1; 3.4 Switches.
- T11 one lookup slot per 4 bytes: 4 decision 1; 3.4 The lookup array; 5.11.
- T12 misaligned code faults: 3.4 Instruction unit.
- T13 flags wanted in bit 0: 3.2 `FLAGS_WANTED`; 3.4 Flags wanted; 4 decision 2.
- T14 `BOTTOM_RETURN` by Top Byte Ignore: 3.2 `UNREACHABLE_RETURN` and its rule; 3.4 (each CPU's way of
  making the marker unreachable, and x86's canonical check before the compare); 5.3.
- T15 untag: 3.2 `data_address`, `branch_target`; 3.4.
- T16 passed registers x0, x1, sp: 3.2 `PASSED_IN`, `PASSED_OUT`; 3.4 Passed registers; 4 decision 5.
- T17 calls of 4 bytes that keep sp: 3.2 `RETURN_ADDRESS`; 3.3 call and ret; 5.3; 4 decision 2.
- T18 system call registers: 3.2 `SYSCALL`; 3.4 Calls and system calls.
- T19 ARM64 traps: 3.2 `Trap`, `trap_kind`, `TrapKind` growth; 6.
- T20 ARM64 controls and the 32-byte private area: 3.2 controls, `PRIVATE_BYTES`; 3.4.
- T21 `Slot` and `RegisterBlock` offsets: 3.2 `BITS` offsets, `Block`, offsets; 3.4; 5.9.
- T22 four flags at NZCV: 3.2 `FLAGS`; 3.4 Flags.
- T23 ARM64's lazy flag shapes in the translator: 3.2 shapes; 3.4 Flags; 5.1; 4 decision 3.
- T24 ARM64 conditions: 3.2 `Cond`, `condition_direct`, `condition_from_flags`; 5.1.
- T25 "plain" instruction variants, "all four flags set": 3.2 `plain`; 3.4 Flags (the per-flag test).
- T26 FPCR/FPSR slow path: 3.2 `float_slow`, `FLOAT_CONTROLS`; 3.4; 5.8.
- T27 three fixed hooks: 3.2 `HOOKS`, `HookType`; 3.4 Hooks.
- T28 the Arm host probe: 3.2 `Host`, probe; 3.4; 5.8.
- T29 ARM64's ways of moving sp: 3.2 `stack_move`; 3.4 Stack moves.
- T30 nothing below sp is a frame: 3.2 `RED_ZONE`; 3.4; 4 decision 7; 5.3.
- T31 sp alignment checked on access: 3.2 `SP_ALIGNMENT_CHECKED`; 3.4.
- T32 one access, ARM64 addressing: 3.2 `accesses`, `MemoryAccess`; 3.4 Memory accesses; 4 decision 4;
  5.4.
- T33 check() once per instruction: 3.3 `check(.., which)`; 3.4 Memory accesses.
- T34 programs announce code changes: 3.2 `CODE_CHANGES`; 4 decision 6; 5.7.
- T35 thread pointer as a system register: 3.4 Thread pointer; 4 decision 10; 5.5.
- T36 ARM64 loop idioms in the proofs: 3.2 loop proofs, `NARROW_WRITE`; 3.4 Loop proofs.
- T37 no guest identity in description or cache key: 3.2 `ID`; 3.4 Identity; X3.
- T38 vectors exactly 128 bits: 3.2 `Bit::ty`; 4 decision 8; 5.10 (ymm halves).
- T39 every float fits 64 bits, the float hook's signature: 3.5 x86 hooks; 5.8 (x87 hook).
- T40 meanings write registers unconditionally, no loops: 3.3 (select, conditional checks); 4 decision 9;
  5.6.
- T41 one thread, plain accesses, ARM's weak ordering: 8.

Tests area:
- E1 Arm's forms list and its generator: 7.2 (the ARM64 forms stay as they are behind `GuestTest`); 7.4
  (x86's from iced's tables); 4 decision 19.
- E2 `Form` and "covered": 7.2 forms list (name, group, generate, presented).
- E3 system register bias: 7.2 per-form operand shaping (CPUID leaves for x86).
- E4 of a feature not offered: 7.2 offered features; 7.4 (`cpuid_features()` against the offered set).
- E5 exclusions: 7.2 per-instance exclusions; 7.4 (jump to itself, designed answers).
- E6 `Case`: 7.2 `FIELDS`; 4 decision 15.
- E7 branch-target values 4-aligned: 7.2 (from `G::ALIGN`); 4 decision 13.
- E8 vector and float value strategies: 7.2 `cases` (the CPU's value strategies).
- E9 FPCR and FPSR bits: 7.2 `FIELDS` (MXCSR, x87 words); 5.8.
- E10 case shaping (sp, flags, bases into DATA, atomic alignment): 7.2 `cases` (operand shaping); 7.4.
- E11 signal to outcome mapping: 7.2 `reference_stop`, `breakpoint_code`, `Stop`; 7.3; 4 decision 16.
- E12 conversion to `RegisterBlock`: 7.2 `to_state` and `from_state`.
- E13 one 4-byte slot and the case word: 3.4 lookup array (slots per `ALIGN`); 7.2; 7.4 (length from the
  encoder).
- E14 unknown after a faulting access: 7.2 `may_differ` (x86 is exact; rep stops exact, section 5.6).
- E15 compare masks: 7.2 `may_differ`; 4 decision 16.
- E16 report names: 7.2 `FIELDS` (field names).
- E17 BLOB_HWCAP and the auxiliary vector: 7.2 `auxiliary`; 6.
- E18 zero is `udf #0`: 2 and 7.3 (trap flag); 4 decision 14.
- E19 request and result sizes: 7.2 `FIELDS`; 7.3 (length-prefixed records); 4 decision 15.
- E20 the reference over local Docker: 7.3; 4 decision 12.
- E21 the reference's target and link base: 7.3 (x86_64-unknown-linux-musl, rust-lld; the toolchain file).
- E22 frame access in the reference: 2 (experiment), 7.3.
- E23 the seccomp instruction window: 7.3 (`[CODE, CODE + 16)`).
- E24 making written code visible: 3.2 `CODE_CHANGES` (nothing on x86).
- E25 placing the instruction: 7.3.
- E26 blob start state: 3.2 `initial_block`; 7.2.
- E27 the float hook body: 3.2 `run_hook`, `HookBody`, `HookDone`; 3.4 Hooks; 5.8.
- E28 trap kinds in the runner: 3.2 `TrapKind` growth; 7.2 mapping to statuses.
- E29 code changed by `ic ivau`: 5.7; 3.2 `CODE_CHANGES`; 4 decision 17.
- E30 the runner's 4-byte slots: 3.4 The lookup array.
- E31 the stand-in's system call numbers: 7.2.
- E32 set_registers and float tracking: 7.2 `to_state` and `from_state`; 5.8.
- E33 zig, rustc targets and general-registers-only flags: 7.2 names.
- E34 slots and registers for a blob: 3.4; 3.2 `initial_block`.
- E35 statuses and the disassembler in messages: 7.2.
- E36 natives over local Docker and RUNNER_FAILED: 7.3 (in-band statuses, no Docker on the x86 machine).
- E37 flat.ld: 7.2 `PROGRAMS` (flat.ld stays shared; x86 needs nothing new; RISC-V small data, section 3.5
  is paper only).
- E38 inline `svc`, `_start`, context switch, assembly tests: 7.2 `PROGRAMS`; 4 decision 21.
- E39 the written-code program: 7.4 (code written without announcement); X6; 4 decision 17.
- E40 the differences program: 7.2; 5.8 (x86's differences: integer indefinite, NaN rules, minps order,
  MXCSR modes, divide error, x87).
- E41 layer 2 with dynasm aarch64: 7.4 layer 2; 4 decision 18.
- E42 layer 3's gcc image, clang target, native runs: 7.4 layer 3.
- E43 frames from `registers.sp`: 7.2 `FIELDS` (the stack pointer from `SP_BIT`).
- E44 the changes hand programs: 7.2 `changes`.
- E45 random starts at 4 * k: 4 decision 13.
- E46 smoke tests: 7.2 `smoke`.
- E47 checks programs: 7.2 `checks`; X4 and X6 add x86's push, pop, rep and read-change-write cases.
- E48 offered features against HWCAP: 7.2 `offered` and `told`.
- E49 targeted layer 1 cases, words in no form, designed answers: 7.2; 7.4.
- E50 BLOB_HWCAP against the abi: 7.2 `auxiliary`.
- E51 images and loader names: 7.2 names; X9.
- E52 ELF e_machine and alignment of real binaries: 7.2 names; 3.2 `ALIGN`.
- E53 relocation types: 7.2.
- E54 instruction counts and slots of real binaries: 4 decision 13 (linear decode); 3.4.
- E55 contract JSON: 4 decision 22; 7.2.
- E56 runner.js slots: 4 decision 22 (from the contract JSON).
- E57 runner.js registers, stand-in, float and code-change hooks: 4 decision 22; 7.2.
- E58 browser cases of one fixed word: 7.3 and 7.4 (length-prefixed cases).
- E59 native times on the Mac: 7.5; 4 decision 20.
- E60 toolchain targets and the test crate's ARM64 dependency: 7.2, 7.3, X1 and X2.

## Appendix B. Every item of the seam map's interface surface, and where it is covered

Translator (24 items, in the seam map's order):
1. Instruction type, decode with length, alignment and maximum length: 3.2 `Instruction`, `decode`, `ALIGN`,
   `MAX_LENGTH`; 3.4 Instruction unit.
2. `emit` against a generic emitter: 3.2 `emit`; 3.3.
3. The emitter's parameter types: 3.2 `Reg`, `Vector`, `Cond`, `Flag`, `Flags`, `Control`, `Hook`, `Host`,
   `Offer`, `Trap`; 3.3.
4. The register set: 3.2 `BITS`, `Bit`, `reg_bit`, `vector_bit`, `bit_reg`, `SP_BIT`, `NARROW_WRITE`,
   `NarrowWrite`; 3.4 Registers.
5. Effects found by recording the meaning, extended (memory, hooks, controls): 3.1 (the generic
   recorder and `Effects<G>` live in the interface crate); 3.3 (the recorder is an emitter); 3.2 `plain`
   and 3.4 Flags ("plain computation" is the CPU's, not derived from the effects, to keep ARM64's output).
6. Control flow: 3.2 `flow`, `Flow`, `FlowKind`, `Through`, the length from `decode`; 3.4 Control flow.
7. Addresses built into registers: 3.2 `built_address`; 3.4.
8. Switch recognition: 3.2 `jump_table`; 3.4 Switches (indexed by target address, so no case-number
   register is needed).
9. Memory accesses: 3.2 `accesses`, `MemoryAccess`, `Base`; 3.4 Memory accesses; 5.4.
10. Stack moves: 3.2 `stack_move`, `StackMove`; 3.4 Stack moves and frames.
11. Stack facts: 3.2 `SP_ALIGNMENT_CHECKED`, `RED_ZONE`, `RETURN_ADDRESS`; 5.3.
12. Untagging and the address no return reaches: 3.2 `data_address`, `branch_target`, `UNREACHABLE_RETURN`
    and its rule; 3.4 T14; 5.3.
13. The flags-wanted marker: 3.2 `FLAGS_WANTED` (0 without flags); 3.4 Flags wanted.
14. Passed registers: 3.2 `PASSED_IN`, `PASSED_OUT`; 3.4 Passed registers.
15. System call registers and saved pc: 3.2 `SYSCALL`; 3.4 Calls and system calls.
16. Flags, shapes and conditions: 3.2 `FLAGS`, `Flags`, `Shape`, `SHAPES`, `shape`, `shape_locals`,
    `with_locals`, `sets`, `flag_value`, `condition_direct`, `condition_from_flags`; 3.4 Flags; 5.1.
17. Controls and the block: 3.2 `Block`, `PC_OFFSET`, `FLAGS_WORD_OFFSET`, `STOP_FLAG_OFFSET`,
    `BLOCK_SIZE`, `PRIVATE_BYTES`, `Control`, `control_ty`, `control_offset`, `initial_block`; 3.4 Controls
    and the block; 5.9.
18. Floating point slow path: 3.2 `float_slow`, `FLOAT_CONTROLS`; 3.4; 5.8.
19. Hooks: 3.2 `Hook`, `HOOKS`, `HookType`, `hook_index`; 3.4 Hooks.
20. Traps: 3.2 `Trap`, `trap_kind`, `trap_detail`, `TrapKind`, `TrapDetail`; 3.4 Traps; 5.2.
21. Host probe: 3.2 `Host`, `probe_module`, `host_from_probe`, `host_key`; 3.4 Host probe.
22. Loop-proof recognisers: 3.2 `induction`, `branch_comparison`, `loaded_max`, the index's `Extend` in
    `MemoryAccess`, and the types `Value`, `Induction`, `Comparison`, `Relation`; 3.4 Loop proofs.
23. Guest identity: 3.2 `ID`; 3.4 Identity.
24. The software floating point runner on the host side: 3.2 `run_hook`, `HookBody::Cpu`, `HookDone`; 3.4
    Hooks; 5.8.

Tests (25 items, in the seam map's order):
1. Names and target identifiers: 7.2 names.
2. Instruction unit and real boundaries: `G::ALIGN`, `G::MAX_LENGTH`; section 4 decision 13.
3. The forms list: 7.2; 7.4 layer 1.
4. Offered features and the check against what the guest is told: 7.2; 5.10.
5. Random encodings in no form: 7.2 `no_form`; 7.4 (byte strings iced calls `INVALID`);
   5.2 (the 15-byte limit).
6. Operand shaping: 7.2.
7. Exclusions with reasons: 7.2; 7.4 (jump to itself, designed answers, `int 0x80`).
8. Designed answers: 7.2; 5.10.
9. Comparable machine state: 7.2; section 4 decision 15.
10. Value strategies: 7.2.
11. Masks of bits allowed to differ: 7.2; section 4 decision 16.
12. Signal and trap mapping: 7.2; 7.3 One instruction; section 6; 3.2 `memory_fault` and `MemoryFault`
    (general protection for accesses reaching a non-canonical address, 5.4).
13. The reference runner's per-CPU part: 7.3; section 2.
14. Initial state and auxiliary vector entries: 3.2 `initial_block`; 7.2.
15. The stand-in's system call numbers: 7.2.
16. Host facts: 3.2 Host probe (shipped, not test-side).
17. The exact float hook body and its reason: 3.2 `run_hook` and `HookDone::reason` (shipped, so every
    runner shares it; the kernel never runs hooks).
18. Code-change policy: 3.2 `CODE_CHANGES`; 5.7; section 4 decision 17.
19. Trap kinds beyond ARM64's: 3.2 `TrapKind`; 5.2 Breakpoints (the `si_code` in the detail)
    and the 15-byte limit (general protection); 7.2 `Stop::Breakpoint(si_code)`.
20. A disassembler for messages: 7.2.
21. Per-CPU program support: 7.2.
22. The layer 2 generator: 7.4 layer 2; section 4 decision 18.
23. Real-binaries support: 7.2.
24. The browser contract: section 4 decision 22; 3.4 Hooks (`cpu_hook`).
25. Reference location and transport: 7.3; section 4 decision 12.
