# wcontainers, phase two: one guest CPU interface, and x86-64

Status: plan written 2026-11-17 from the owner's brief (decisions of 2026-11-16) and the map of today's
ARM64 assumptions, and revised after each of the nine independent checks of X0 (2026-11-17). The research and the two experiments
of milestone X0 are done; what they found is in section 2. Milestone X1 (the interface of section 3, with
ARM64 on it) is built, and so is X2 (the shared harness of section 7.2, with ARM64's part, apart from what
its "As built" list leaves to later milestones); docs/status-x86.md says where the rest stands.

Words used here (phase one's words still hold: guest, host, engine, look-ahead, piece):
- **guest CPU**: the kind of processor whose machine code is translated: ARM64 today, x86-64 next, RV64GC
  later.
- **CPU crate**: the crate that holds everything true of one guest CPU (`wcontainers-arm64`,
  `wcontainers-x86`, later `wcontainers-riscv`).
- **the interface**: the crate `wcontainers-guest-cpu`, which says what a CPU crate supplies. The
  translator, the test harness and later the kernel are written once against it: the kernel against its
  `no_std` core (section 3.1).
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
running in parallel (section 8), and building RISC-V (it is designed on paper only, section 3.5).

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
- It builds without the standard library. The proof is one that can fail (run 9, scratch
  `run9/iced-strict-nostd` and `run9/iced-strict-std`, the independent check's method and the one used for
  astro-float in `x87-nostd`): a `#![no_std]` cdylib with its own `#[panic_handler]` and
  `#[global_allocator]` that decodes with iced, built for `wasm32-unknown-unknown` (release, `panic =
  "abort"`), links with `default-features = false, features = ["no_std", "decoder", "instr_info",
  "encoder", "op_code_info"]`, and `cargo tree` shows below iced only `lazy_static` (`spin_no_std`) and
  `spin`. The same cdylib with `"std"` in place of `"no_std"` fails with "found duplicate lang item
  `panic_impl`", so a dependency that brought the standard library would show. (Run 1 built only an rlib,
  which would have built either way.)
- What it adds to a wasm32 release build, measured on 2026-11-17 (Mac load 15 to 21) with a small cdylib
  that decodes a buffer and uses the decoded fields, after `wasm-tools strip`. The cdylib's total sizes
  with iced:
  - `no_std` + `decoder`: 227,228 bytes raw, 60,682 gzip, 47,058 brotli.
  - `no_std` + `decoder` + `instr_info`: 353,374 raw, 82,697 gzip, 64,947 brotli.
  gzip is the Mac's `/usr/bin/gzip` (Apple gzip 487.40.2) as `gzip -9 -n -c`, so no file name is stored in
  the header; brotli is `brotli -q 11 -c` (Homebrew's brotli 1.2.1). (Run 1 wrote 60,705 and 82,721: the
  same gzip storing the files' names, `size_dec.stripped.wasm` and `size_info.stripped.wasm`, 12 bytes more
  than the names the seventh check used; found in run 8. The Nix shell's GNU gzip 1.14 gives 60,831 and
  82,541 with `-n`.) The same cdylib without iced is 257 bytes raw, 208 gzip and 178 brotli, so what iced
  adds is these totals less that: 226,971 / 60,474 / 46,880 for the decoder alone and 353,117 / 82,489 /
  64,769 with `instr_info` (the fifth and seventh independent checks rebuilt the decoder build and got the
  same bytes; runs 6 and 8 measured the stripped files again).
  - For scale, today's browser translator (`wcontainers-browser`, ARM64, release, stripped) is 2,170,386
    bytes raw, 412,037 gzip and 312,302 brotli (measured the same day, load 21 to 28). So the decoder alone
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
  each of the 4,936 `Code` values writes one byte: its claim group (default, the AVX setting, never, or
  kernel only) from its `cpuid_features()` against the claim lists. The claim lists are
  one source file that the build script and the crate both include, and they name iced's own
  `CpuidFeature` values, not CPUID bits (section 5.10 gives both lists in iced's names, run 10 above why):
  a `Code` is in the default group when every one of its features is in the default list, else in the AVX
  setting's group when every one is in the default or the AVX list, else never. A `Code` that
  `is_privileged()` marks kernel-only (and `rdpmc`) gets a fourth value, "kernel only", when every one of its
  features is in the default list or in a third list, the kernel-only features every x86-64 CPU has (`MSR`,
  `RDPMC`, `SMM`), so that the CPU presented has the instruction (its meaning gives the fault Linux gives a
  program, section 5.2); a kernel-only `Code` of any other feature (VMX, SVM, SMAP, SGX, XSAVES, INVPCID,
  CET and the rest) goes by the lists like any other, so it is never claimed and gives illegal instruction
  as on a CPU without that feature (`xsetbv` falls in the AVX setting's group). Cargo's resolver 3 (the workspace's)
  keeps a build-dependency's features apart from the normal dependency, so the shipped decoder stays
  `decoder` only. The table is 4,936 bytes before compression, one a `Code` (measured at X3). A test checks the
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
  `UD1` instruction (`0F B9 C0+n`: a ModRM with mod 3, the reg field 0 for eax, and the operation number n
  in the r/m field, bits 0 to 2) placed after the test
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
  code) runs the call and gives no `TRAP_TRACE`; the filler stops it the same way. `sysenter` (valid in
  64-bit mode on Intel) also runs a 32-bit call, but Linux returns from it to its vDSO's landing pad, far
  from the case, and the thread then faults there. The runner never lets a case's system call run: a
  seccomp filter turns into `SIGSYS` every call made from the instruction window and every call whose
  `seccomp_data.arch` is `AUDIT_ARCH_I386`, wherever it was made (run 7: `sysenter` with the arch test
  gives `SIGSYS`, without it the 32-bit call ran).

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

   Run 6 (2026-11-17, x86 load 0.00 to 0.12, nothing timed), after the fifth independent check, with
   `stepper4`, `cases9.sh`, `cases10.sh` and their outputs:
   - A non-canonical access through the stack segment is a stack fault (#SS), which Linux sends as
     `SIGBUS` with `SI_KERNEL` (128) and address 0, rip at the instruction; every other non-canonical
     access is general protection (`SIGSEGV`, `SI_KERNEL`, address 0). An access goes through the stack
     segment when its base register is rsp or rbp, or when it is an implicit stack access. Seen as stack
     faults: `push rax` with rsp 0x800000000010 and with 0x800000000004 (the 8 bytes cross 2^47), `ret`,
     `pushfq` and `push fs` with rsp 2^63, `mov rax, [rbp]` and `[rsp]` with 2^63, an 8-byte load from
     `[rbp]` at 0x7ffffffffffc, `[rsp + rbx]` (also with rsp canonical and the sum 2^47) and `[rsp + rbp]`,
     `push [rbp]`, `call [rbp]`, `pop [rbx]` with rsp 2^63, `movaps xmm0, [rsp]` and `cmpxchg16b [rsp]`
     with rsp 2^63; the fifth check also saw `enter` and `leave`. Seen as general protection: base rbx,
     r12 or r13 (so only rsp and rbp count, not the registers sharing their low three encoding bits), base
     rax with index rbp, `[rbp * 2]` with no base, a `moffs` address, `rep stosb` and `movsq`, `pop [rbx]`
     with rbx 2^63 (its write), and an `fs:` or `gs:` prefix on an rbp base. The `cs:`, `ds:`, `es:` and
     `ss:` prefixes change nothing (64-bit mode ignores them): `ss:` and `cs:` on an rbx base are general
     protection, `cs:`, `ds:` and `es:` on an rbp base a stack fault.
   - Alignment comes first: `movaps xmm0, [rsp]` and `cmpxchg16b [rsp]` misaligned and non-canonical, or
     misaligned and crossing 2^47, give general protection, not the stack fault. A canonical unmapped
     `push` or `[rbp]` load is a page fault (`SEGV_MAPERR`, the address) as always.
   - An MMX write sets bits 64 to 79 of the x87 register it writes (sign and exponent) to 0xFFFF, as
     Intel's manual (volume 1, 9.5.1) says: `movq mm0, rax`, `movd mm0, eax`, `pxor mm0, mm0` and `movq
     mm0, [rdi]` each leave R0 as `ffff:` and the 64 bits written, TOP 0 and every register valid. An MMX
     read (`movq rax, mm0`, `movq [rdi], mm0`) leaves R0's top 16 bits as they were.

   Run 7 (2026-11-17, x86 load 0.00, nothing timed), after the sixth independent check, with
   `stepper5.c` (`stepper4.c` that also enters NT and ID, as below, and with `SECCOMP=1` installs the
   filter of section 7.3 with its arch test), `cases11.sh`, `cases12.sh`, `cases13.sh` and their outputs:
   - NT (rflags bit 14) is user state, as Intel's `popf` and `iret` pseudo-code allow at CPL 3: `popf` of
     0x4202 and of 0x204202 keeps NT and ID, `popf` of 0x202 clears them, `pushf` stores them, a `nop`
     keeps them, the `syscall` instruction keeps them (and r11 holds them), and a fault's signal frame
     carries them (`ud2` from 0x204202 gives 0x214202 in the frame, RF added as for every fault). The
     sixth check saw `popf` of 0x3FFFFF give 0x244ED7: NT, AC and ID kept, IOPL, RF, VIF and VIP not.
   - `iretq` with NT set gives general protection, rip at the instruction, before it reads anything (its
     frame unmapped, or rsp not canonical: still general protection). With NT clear it lands.
   - `iretq` reads its 40-byte frame out of order: with nothing mapped the page fault is at rsp + 16
     (rflags); rflags then CS are read before rip; SS is read before rsp (both unmapped: the fault is at
     SS's slot); with rip's slot unmapped and the rest valid, the fault is at rip's slot; a null CS gives
     general protection before SS is read, while CS 0x2b (data) with rip's slot unmapped gives the page
     fault.
   - Entering NT and ID: Linux's `rt_sigreturn` takes only the `FIX_EFLAGS` bits (the six flags, TF, DF,
     RF, AC) from the frame and keeps the others from the rflags the caller had when it made the call
     (the sixth check: a frame with ID or NT, IOPL or VIF entered without them; AC entered). So the
     handler sets NT and ID in its own rflags (`pushfq; or; popfq`) just before it returns through
     `rt_sigreturn`, and they reach the case: `pushf` from 0x200202, 0x4202 and 0x204AD7 stores those
     values with TF. The handler that reads the case back clears them in its own rflags the same way.
   - `sysenter` with the filter's arch test gives `SIGSYS` with rip and si_addr in the vDSO; `int 0x80`
     and `syscall` give `SIGSYS` at CODE + 2. Without the arch test (the sixth check's `stepper_sc`)
     `sysenter` ran i386 getpid and then faulted far from the case.
   - A legacy prefix that makes a VEX or EVEX instruction invalid (66, F2, F3, REX, and 66 with REX) does
     not change what the CPU counts: segment prefixes, then one of those, then `vaddps xmm0, xmm0, [rsp +
     disp32]` (9 bytes as `C5`, 10 as `C4`) give `SIGILL` at 15 bytes in all and general protection at 16;
     the EVEX `vaddps zmm0, zmm0, [rsp + disp32]` (11 bytes, this CPU has no AVX-512), alone or after a
     66, gives `SIGILL` at 15 and general protection at 16. So the CPU counts the whole VEX or EVEX
     instruction after such a prefix, with its ModRM, SIB and displacement. iced decodes each of them as
     `INVALID` (66 then the 9 bytes: length 10; the 16-byte ones: length 15).
   - On the reference, Ubuntu's glibc 2.31 loader `/lib64/ld-linux-x86-64.so.2` has `fxsave` and
     `fxrstor` at 0x18aa5 and 0x18aba and `xsavec` and `xrstor` at 0x18c2c and 0x18c48: its lazy binding
     trampolines, which save and restore the whole vector state, MXCSR included, at every lazy binding
     (section 5.8).

   Run 8 (2026-11-17, x86 load 0.00 to 0.16, nothing timed), after the seventh independent check, with
   `stepper6.c` (`stepper5.c` that can also run without the trap flag, `NOTF=1`, step on while a
   `TRAP_TRACE` stop leaves rip at CODE, `STEPS=n`, and print any 32 bytes of the data area, `DUMP=`),
   `cases14.sh`, `cases15.sh`, `smc.c` and `copyuser.c`, all in scratch `run8/` with their outputs. DATA
   is 0x20000000 to 0x20010000; the pages just below and above it are unmapped:
   - Code written on its own page: `smc.c` loops 200 times over `mov [rip + 1], cl` (writing the immediate
     of the next instruction, `mov eax, imm32`), `add edx, eax`, `dec ecx`, `jnz`, all on one page: it
     prints `eax 1 sum 20100`, so every pass's write is seen by the very next instruction (section 5.7).
   - `bt`, `bts` with a register bit offset address whole operands far from the operand: `bt [rax], rcx`
     with rcx 0x80000 faults at rax + 0x10000 (64-bit), as does the 32-bit form with ecx 0x80000; a
     32-bit offset of 0x80000000 is read signed (no fault where the unsigned reading would reach an
     unmapped page); a 16-bit offset of 0x8000 likewise, and rcx 0xffffffff00007fff reads only cx.
     Reaching past 2^47: through rbp a stack fault (`SIGBUS`, `SI_KERNEL`), through rax general
     protection. `xlat` with al 0xFF reads rbx + 0xFF.
   - Gathers (`vgatherdps xmm0, [rax + xmm1 * 4], xmm2`): in lane order, each element gathered clears its
     mask lane; when an element faults, the ones before it are gathered and their mask lanes clear and the
     rest untouched, and without the trap flag the page fault names the faulting element's first byte not
     allowed. With the trap flag, the CPU first stops with `TRAP_TRACE` at the gather (rflags with RF)
     after the elements before the faulting one, and the next step gives the page fault, state unchanged.
     With elements 1 and 3 unmapped, only element 0 is gathered and the fault names element 1. An element
     across the page end faults at the page's end, nothing gathered. A masked-off unmapped element does
     not fault and the mask ends all zero.
   - Masked moves: `vmaskmovps` and `vpmaskmovd` (128 and 256 bits) never fault on masked-off elements and
     store or load nothing when a selected element faults. A load reports the first byte not allowed; a
     store reports the first byte of the first selected element when it is not allowed, else the last
     byte of the last selected element (nine cases, section 5.4). `maskmovdqu` faults whatever the mask
     (all-zero included), stores nothing on a fault, and reports as if it checked rdi + 8 to rdi + 15
     first and then rdi to rdi + 7, each at its first byte not allowed (thirteen cases, section 5.4).
   - Copies into user memory that run into a page that may not be written (`copyuser.c`): a pipe's
     `read`, `/dev/zero`'s `read` and `uname`, with the buffer ending 1 to 389 bytes before the page,
     write every byte before it (section 6).

   Run 9 (2026-11-17, x86 load 0.04, nothing timed), after the eighth independent check, with `stepper6`
   and `cases16.sh` in scratch `run9/` (39 cases) and the scratch decoder `run9/dis` (iced 1.22.0 with
   `DecoderOptions::NONE` and with `FORCE_RESERVED_NOP`):
   - The reference has none of CET (no `ibt`, `shstk`), CLDEMOTE, TSX (no `rtm`, `hle`), WAITPKG, CLWB,
     PTWRITE, SERIALIZE, MOVDIRI, RDPID, PREFETCHWT1, PREFETCHITI; it has MPX (unused: Linux no longer
     enables it), 3DNowPrefetch (`prefetchw`), CLFLUSHOPT, XSAVEOPT, XSAVEC, AES, PCLMULQDQ, RDRAND and
     RDSEED (its `/proc/cpuinfo` flags, `run9/cpuflags.txt`).
   - The reserved-NOP space runs as NOPs: `nop [rax]`, `0F 19`, `0F 1E` and `0F 18 /4` with rax
     non-canonical or unmapped, `cldemote [rdi]` likewise, every reg value of `0F 0D` (`prefetchw`,
     `prefetchwt1`) and `prefetchnta` with a non-canonical address, the register forms of `0F 0D` and
     `0F 18`, `prefetchit0` rip-relative near and 2 GiB away, MPX's `0F 1A` and `F3 0F 1B` (non-canonical
     and rip-relative), `bndmov`, `bndcu`, `endbr64` with a `66` before it, and `rdsspd eax` with eax
     0x77: each stops by the trap flag after the instruction with nothing else changed. `lock` before
     `nop [rax]`, `0F 19` or `endbr64` gives SIGILL (iced: `INVALID`). iced with `FORCE_RESERVED_NOP`
     decodes every one of them (and `rdsspq`, `prefetchit1`) as a `Reservednop_*` Code of the same length
     with the feature `MULTIBYTENOP`; with `NONE` it gives `Endbr64`, `Rdsspq_r64`, `Cldemote_m8`,
     `Prefetchw_m8` and the like with their own features.
   - Outside that space, instructions of features the reference lacks give SIGILL at the instruction:
     `tpause`, `umwait`, `umonitor`, `clwb`, `ptwrite`, `xbegin`, `xtest`, `serialize`, `movdiri` (and
     run 8's check: `incsspq`). `clflushopt`, which it has, runs.
   - Prefixes a CPU without their feature ignores are ignored: `xacquire lock add [rdi], eax` leaves
     1 + 5 = 6 in memory, as `lock add` does (`run9/xacquire.out`); `bnd jmp` and `notrack jmp rax` jump.

   Run 10 (2026-11-17, x86 load 0.00 to 0.24, nothing timed), after the ninth independent check, with
   scratch `run10/tf.c` (a program that sets TF itself, with a SIGTRAP handler that records every stop
   and keeps TF in the frame for a chosen number of stops; output `run10/tf.out`), `run10/rf.c` (which
   signal frames carry RF; `run10/rf.out`) and `run10/feat` (iced 1.22.0's `cpuid_features()` over every
   `Code` valid in 64-bit mode; `run10/feat.out`):
   - A trap flag a program sets stops it after every instruction that **starts** with TF set, with
     SIGTRAP `TRAP_TRACE` and rip at the next instruction (at the target for a `jmp`):
     - `popf` that sets TF does not stop itself; the instruction after it runs and then stops. `iretq`
       that sets TF likewise: the first instruction at its target runs, then stops.
     - `popf` that clears TF while TF is set does stop after itself; the frame shows TF clear.
     - `syscall` with TF set does not stop after itself: the instruction after it runs and then stops (r11
       holds rflags with TF). Linux returns from a system call with TF set through `iret`, which is a
       return into the program with TF set, so the first instruction runs before the stop.
     - `int3` with TF set gives its own SIGTRAP `SI_KERNEL` and no `TRAP_TRACE`; a load from address 0
       gives SIGSEGV and no `TRAP_TRACE`. In both the frame keeps TF, and when the handler returns with TF
       still in the frame, the first instruction runs and then stops (as after `popf`).
     - A handler runs with TF clear (its own `pushf`: 0x246), and `pushfq` with TF set stores it (0x346).
   - A branch to an unmapped address stops by the trap flag at the target, before any fetch there: `jmp
     rel32` 256 MiB ahead, and `jmp rax` to 0x700000000000 and to 0x7ffffffff000 each give SIGTRAP
     `TRAP_TRACE` with rip at the target (`stepper6`, `run10/branch-unmapped.out`; the ninth check saw
     the same for `ret` to 0). `sldt eax`, `str eax` and `smsw eax` run in user mode (the reference has
     no UMIP; `run10/umip.out`).
   - RF (bit 16) is set in the frame of every signal from a fault: `ud2` (SIGILL), a divide by zero
     (SIGFPE), a page fault, general protection and the stack fault. It is clear for traps (`int3`, `int1`,
     the trap flag) and for a signal sent by `kill`.
   - iced's feature names are not CPUID's: of the 4,690 Codes valid in 64-bit mode, the baseline ones carry
     `INTEL8086` (217), `INTEL186` (26), `INTEL286` (15, among them `ud2`, `ud0`, `ud1` and the user-mode
     `sldt`, `str`, `smsw`, `lar`, `lsl`, `verr`, `verw`), `INTEL386` (204), `INTEL486` (11), `X64` (215),
     `CPUID` (1), `PAUSE` (1) and `MULTIBYTENOP` (30); x87 carries `FPU` (145), `FPU287` (4: `fstsw ax`,
     `fnstsw ax`, `fsetpm`, `fnsetpm`) and `FPU387` (11: `fsin`, `fcos`, `fsincos`, `fprem1`, `fucompp` and
     the 108- and 28-byte `fsave`, `fnsave`, `frstor`, `fstenv`, `fnstenv`, `fldenv`); CLFLUSH is `CLFSH`,
     CX16 is `CMPXCHG16B`, SSE4.1 and 4.2 are `SSE4_1` and `SSE4_2`; `syscall` carries `SYSCALL` and
     `sysenter` `SEP`. Fifteen x87 Codes also carry a non-x87 feature: `fcmovcc` (8), `fcomi`, `fcomip`,
     `fucomi` and `fucomip` carry `CMOV`, and the three `fisttp` carry `SSE3`. By encoding: 1,627 legacy,
     996 VEX, 1,733 EVEX, 103 XOP, 24 3DNow (`femms` is `0F 0E`, the rest `0F 0F`), 207 MVEX. The MVEX
     Codes and the 48 VEX Codes of feature `KNC` (Knights Corner) are decoded by iced only with its `KNC`
     decoder option (`DecoderOptions::KNC`, decoder.rs), which the translator does not set.

   What this teaches the reference runner (section 7.3): write every vector register and the whole x87
   component (FCW, FSW, the abridged tag byte, FOP and the last-instruction pointers as zero, the eight
   slots in stack order) (else the runner's own values leak in), set the x87, SSE and AVX bits of the
   frame's XSAVE header (`xstate_bv` bits 0, 1 and 2) or the kernel loads their first state, read a
   component as its first state when its `xstate_bv` bit is clear, find the
   upper ymm halves at the offset CPUID leaf 0xD sub-leaf 2 gives, mask `RF` (bit 16) and `IF` out of the
   comparison of rflags and report `TF` as the instruction left it, not as the runner's own trap flag left
   it (section 7.3, as built at X2), mask `TF` out of every copy of rflags an instruction makes (the word
   `pushf` stores, r11 after `syscall`), enter NT and ID through its own rflags (run 7), stop every
   32-bit call by its arch, and step on after `TRAP_TRACE` at CODE only for a rep string instruction or a
   gather (run 8).
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

`wcontainers-guest-cpu` holds two traits and the plain types they use, in two layers:
- **The core**, always built and `no_std`: the trait `Personality` (what the kernel and the runners need of
  a CPU without translating anything: the register block type, where pc and the stop flag are, the sizes of
  the block and of the CPU's private words; at X10 also the rest of the kernel's architecture personality of
  section 6: system call numbering and decode, signal frames, data layouts, the machine description), and
  the plain types beside it (`Registers`, `Bit`, `Class`, `Ty`, `TrapKind`, `MemoryFault`).
- **The translation half**, behind the cargo feature `translate`, on by default: the trait
  `Guest: Personality`, the `Emitter` trait meanings write to, the plain types both use (accesses, flow,
  stack moves, the loop-proof facts, hooks), the runtime's `Reason` list, `Effects` and the effects recorder.
  It needs the standard library and `wasm-encoder`.

A CPU crate is layered the same way: its core (a zero-sized type and its `Personality`) is `no_std` and
always built; everything else (decoder, meanings, software floating point, `Guest`) is behind its own
`translate` feature, on by default, which turns on the interface's. So the kernel depends on the interface
and on the CPU crate it is built for, both with `translate` off, and is written once against `Personality`;
the translator, the runners and the tests use them with it on. Each CPU crate defines one zero-sized type:
`wcontainers_arm64::Arm64`, `wcontainers_x86::X86_64`, later `wcontainers_riscv::Rv64`. The translator's
public functions take the guest as a type parameter:

```rust
pub fn translate<G: Guest>(blob: &[u8], code: Range<u64>, starts: &[u64], look_ahead: LookAhead,
    optimise: Optimise, host: G::Host, offer: G::Offer) -> Result<Translation, Error>;
pub fn thread_module<G: Guest>() -> Vec<u8>;
```

`translate_omitting` and `translate_in_modules` take the same type parameter and the same `host` and `offer`
arguments. Every use is monomorphised; nothing of a guest is reached through `dyn`, so nothing is looked
up at run time. ARM64's small per-instruction queries are marked `#[inline]` so they inline across the
crate boundary (release builds have no LTO), and the translator walks only the flag states a set holds, not
every bit of the `u128`. Measured at X1 (old and new alternating, one thread, python and busybox at three
look-aheads, on a loaded Mac): ARM64 then translates in about 2.4 to 3.5 percent fewer instructions than
before the move but about 0 to 5 percent more cycles a pair (about 2 to 3 percent on average); where the rest
goes was looked for and not found. The output is the same bytes, so the guest's run time is untouched.
(The loop-proof
module passes a few of its own closures as `&dyn Fn` while it plans, as before the move: translate time only,
nothing of the guest, nothing in the output.) The translator crate
depends on `wcontainers-guest-cpu` only, never on a CPU crate (tests in the translator crate fail if its
manifest or sources name one, or if its resolved dependencies, as `cargo tree` gives them for every target
with dev-dependencies included, hold one). The
program that ships (the browser's cdylib, the host runner) names the one
guest it is built for.

The interface crate depends on `zerocopy` (the `Block` bound) and, in the translation half, `wasm-encoder`
(meanings write WebAssembly through its `InstructionSink`), both already in the workspace. Besides the traits
and the plain types it holds what is generic over any guest and used by both the translator and the tests:
`Registers`, the `Effects` type and the recorder that finds an instruction's effects by running its meaning
against a recording emitter (`wcontainers_guest_cpu::effects::<G>(&i, host, offer) -> Effects<G>`, with
`Effects::flags: Option<G::Flags>`). The recorder runs with the translation's own host answer and offer,
because what a meaning does may depend on both (an x86 instruction of the AVX setting traps and writes
nothing when the offer is off, so its uses and defs differ), and both are part of the cache key; for ARM64
the effects are the same for every host, so its own switch-table habits record with a fixed one.

### 3.2 The trait

Comments say what each item is for; section 3.4 gives the reason for each choice.

```rust
/// The core, `no_std`: what the kernel and the runners need of a CPU without translating anything. At X10
/// it gains the rest of the kernel's personality (section 6).
pub trait Personality: Sized + Send + Sync + 'static {
    /// The saved register block the kernel sees (`wcontainers_abi::RegisterBlock` on ARM64).
    type Block: zerocopy::FromBytes + zerocopy::IntoBytes + zerocopy::Immutable + Copy;
    /// Where pc sits in the block, and the i32 stop flag after it.
    const PC_OFFSET: usize;
    const STOP_FLAG_OFFSET: usize;
    /// Bytes of the block and its stop flag, a multiple of 16.
    const BLOCK_SIZE: usize;
    /// Bytes after those that only the CPU side uses (ARM64: FPSR tracking and the exclusive monitor).
    const PRIVATE_BYTES: usize;
    /// The registers a thread starts with at `pc` with stack pointer `sp` (x86: MXCSR 0x1F80, FCW 0x037F,
    /// rflags 0x202).
    fn initial_block(pc: u64, sp: u64) -> Self::Block;
}

/// Everything the translator needs that is true of one guest CPU only (behind `translate`).
pub trait Guest: Personality {
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
    /// (x86's TF, DF, NT and ID and the bits rflags always has are not in it, section 5.1).
    const FLAGS: &'static [(Self::Flag, u32, u32)];
    /// How flags were last set: a shape with the locals holding its operands.
    type Flags: Copy + core::fmt::Debug;
    /// A shape without its locals: the key of the out-of-line flags-word functions.
    type Shape: Copy + Eq + core::hash::Hash + core::fmt::Debug;
    /// The shapes worth an out-of-line flags-word function, in function order.
    const SHAPES: &'static [Self::Shape];
    /// The shape of a flags value, `None` for one with no flags-word function (ARM64's and x86's
    /// `Constant` and `Packed`): today's `Shape::of`, whose `None` keeps a block's flags in locals.
    fn shape(flags: Self::Flags) -> Option<Self::Shape>;
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
    /// The type of the two temporaries `complete` needs for flags set this way, `None` when it needs none
    /// (ARM64: an addition's width; logic keeps everything, and today's code takes no temporaries for it).
    fn temporaries(flags: Self::Flags) -> Option<ValType>;
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

    // --- the flags word and controls ---
    /// Where the i32 flags word sits in the block.
    const FLAGS_WORD_OFFSET: usize;
    /// A register kept only in the thread's state, never in a local.
    type Control: Copy + Eq + core::fmt::Debug;
    fn control_ty(c: Self::Control) -> ValType;
    /// Byte offset from the start of the block (into the block or the private bytes).
    fn control_offset(c: Self::Control) -> usize;

    // --- floating point modes ---
    /// Push an i32 not zero when floating point must take the exact path now.
    fn float_slow<E: Emitter<Self>>(e: &mut E);
    /// Controls whose write makes an earlier `float_slow` answer stale.
    const FLOAT_CONTROLS: &'static [Self::Control];

    // --- hooks the meanings call ---
    type Hook: Copy + Eq + core::fmt::Debug;
    /// Name, parameter and result types, the register bits the hook reads or writes in the thread block
    /// (written back before the call and loaded after), the bytes of the thread's state a `Cpu` hook's
    /// work reads and writes, and who does the work. Function indices follow the translator's own hooks,
    /// in this order.
    const HOOKS: &'static [HookType];
    fn hook_index(h: Self::Hook) -> usize;

    // --- traps ---
    type Trap: Copy + core::fmt::Debug;
    /// The generic stop a trap is, where its detail comes from, and where it leaves pc: `TrapPc::At` the
    /// instruction (a fault; every ARM64 trap) or `TrapPc::After` it (x86 `int3`, `int 3`, `int1`, `int 4`).
    fn trap_kind(t: Self::Trap) -> (TrapKind, TrapDetail, TrapPc);
    /// Slow paths of this CPU's own, numbered from `GUEST_REASONS` (64) in this order, with fixed names.
    type Reason: Copy + Eq + core::fmt::Debug;
    const REASONS: &'static [(Self::Reason, &'static str)];

    // --- what the host's engine does where WebAssembly leaves it open ---
    type Host: Copy + Eq + core::hash::Hash + core::fmt::Debug;
    /// A module whose `probe` export, an i32 function, gives the answer `host_from_probe` reads.
    fn probe_module() -> Vec<u8>;
    fn host_from_probe(answer: i32) -> Self::Host;
    /// The host answer as a number, for the cache key.
    fn host_key(host: Self::Host) -> u32;

    // --- what the container offers beyond the default claims ---
    /// A container setting that changes what the CPU claims, given with every translation: x86 the AVX
    /// setting (`Offer { avx: bool }`), `()` for ARM64 and RV64GC. Its default is what the CPU claims when
    /// the container asks for nothing more (AVX not claimed), which the tests and the browser glue use.
    type Offer: Copy + Eq + core::hash::Hash + core::fmt::Debug + Default;
    /// Bytes of the offer for the cache key and the description section (0 for `()`).
    fn offer_key(offer: Self::Offer) -> u32;

    // --- the host side of this CPU's own hooks ---
    /// What the runner does when a piece calls a hook whose `HookType::body` is `Cpu`: the work, on the
    /// bytes of the thread's state its `HookType::state` names (`state[0]` is the byte at `state.at`), with
    /// the hook's arguments. The runner hands over only those bytes (in place where it can), so the hook
    /// costs no more than the work.
    /// ARM64's float hook is `exact::run` behind it; x86's float and x87 hooks likewise. Every runner (the
    /// host runner, the browser's cdylib) calls this and nothing CPU-named. Hooks are a runner's job,
    /// never the kernel's: the runner owns the WebAssembly instance whose imports they are.
    /// `hook` is the hook's index in `HOOKS`, which is what a runner knows from the import it serves.
    fn run_hook(hook: usize, args: &[i64], state: &mut [u8]) -> HookDone;

    // --- what a memory fault is on this CPU ---
    /// The memory fault hook reported `address` (the first byte not allowed) for the instruction at the
    /// start of `code`, with the thread's registers exact in `block`: the fault the CPU gives for it.
    /// ARM64 and RV64GC always `Page`; x86, when the access that faulted reaches a non-canonical
    /// address, `Stack` if that access goes through the stack segment and `GeneralProtection` otherwise
    /// (section 5.4). The runner calls this before it tells the kernel about the fault, so the kernel is
    /// told the fault itself.
    fn memory_fault(address: u64, code: &[u8], block: &Self::Block) -> MemoryFault;

    // --- single stepping by a trap flag the program sets ---
    /// Whether a program can turn single stepping on (x86's TF); when false the runners never call `step`
    /// and copy nothing for it.
    const STEPS: bool = false;
    /// What the stop hook does about single stepping (x86's TF, section 5.1), worked out on the thread's
    /// state (`BLOCK_SIZE + PRIVATE_BYTES` bytes from the block). The runner asks it first at every stop
    /// hook call, with `StepAt::Stop(pc)`, and with `StepAt::Kernel` whenever the kernel has had the thread
    /// (after the system call hook returns, and before each entry into `run`). The default, kept by ARM64
    /// and RV64GC, which have no single stepping a user program can turn on, answers `Step::Off`, so the
    /// runner's call compiles to nothing for them.
    fn step(state: &mut [u8], at: StepAt) -> Step {
        let _ = (state, at);
        Step::Off
    }

    // --- meanings ---
    fn emit<E: Emitter<Self>>(e: &mut E, i: &Self::Instruction);

    // --- facts about one instruction, for the analyses ---
    fn flow(i: &Self::Instruction) -> Flow<Self::Reg>;
    /// Up to two memory accesses, in the order the instruction makes them, each described exactly (an
    /// access that is not base plus scaled index says so through its `Index` or `AccessBytes`).
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
    /// `before(o)` is the function's instruction that falls through into offset `o` (with its offset, a
    /// conditional branch among them, so a switch's guard is reached),
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
    /// Whether a real CPU of this architecture may give a different result from the meaning's for this
    /// instruction (the brief's "mark what depends on the host CPU", from X3): the meaning fixes what the
    /// architecture leaves open to what the reference CPU does (undefined flags), answers by design (cpuid,
    /// the time stamp counter, Linux's 32-bit system call gates), estimates, or stops where a CPU with more
    /// features runs on (an invalid encoding, an instruction outside the default claims or with no meaning
    /// yet). A native output may copy an instruction through only where this is false. ARM64 says true for
    /// every instruction until its meanings are audited; x86 as section 5.1 lists; a test checks x86's
    /// answers for each kind.
    fn may_vary(i: &Self::Instruction) -> bool;

    // --- calls, returns and the stack ---
    /// Where a call leaves the return address.
    const RETURN_ADDRESS: ReturnAddress;
    /// Bytes below the stack pointer a function may use without moving it: x86 128, else 0.
    const RED_ZONE: u64;
    /// The CPU checks the stack pointer's alignment when it is the base of an access (ARM64 under Linux).
    const SP_ALIGNMENT_CHECKED: bool;
    /// Push the address in this i64 local as data accesses use it (ARM64 untags it; others push it).
    fn data_address(sink: &mut InstructionSink<'_>, local: u32);
    /// On the translator's own slow paths for a jump or call through a register (a switch whose case was
    /// not taken, the lookup's miss): push the target the CPU would use from the address in this local
    /// (ARM64 untags it), or stop with the fault the CPU gives (x86: general protection when it is not
    /// canonical). Pushed rather than made in place so ARM64's switch keeps today's bytes, which untag the
    /// guest register into the target's local.
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
    /// Each instruction of `path` comes with its effects, so the CPU can find the flag setter the branch
    /// reads; `branch` comes with its offset (`Comparison::at` for ARM64 `cbz`).
    fn branch_comparison(path: &[(u64, Self::Instruction, Effects<Self>)],
        branch: (u64, &Self::Instruction)) -> Option<Comparison<Self::Reg>>;
    /// When this instruction is a load that zero-extends what it loads (at most 4 bytes): the register it
    /// loads and the largest value it leaves there (the analyses work with register bits, so they compare
    /// its bit with their own).
    fn loaded_max(i: &Self::Instruction) -> Option<(Self::Reg, i64)>;
}
```

The plain types, also in the interface crate:

```rust
/// A set of register bits (what `Registers` in wcontainers-arm64 is today).
pub struct Registers(pub u128);

/// One register bit: its number in `Registers` (0 to 127), what it is, and its offset in the thread block.
pub struct Bit { pub bit: u32, pub class: Class, pub ty: Ty, pub offset: u16 }
pub enum Class { General, Flag, Vector, Segment }
/// I32 for a flag (ARM64's N, Z, C, V), I64, V128.
pub enum Ty { I32, I64, V128 }
/// Which guest CPU, in the description section and the cache key.
pub enum GuestId { Arm64 = 1, X86_64 = 2, Rv64 = 3 }
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
    /// A segment base added to the address: x86 `fs:` or `gs:` (a register of class `Segment`), whatever
    /// the base and index are (`fs:[rdx + rax * 4]`); `None` on ARM64 and RISC-V.
    pub segment: Option<R>,
    pub base: Option<Base<R>>,
    pub index: Option<Index<R>>,
    pub displacement: i64,
    /// The address is worked out modulo 2^this: 64, except 32 for x86's 0x67 prefix. The loop proofs and
    /// check grouping leave a 32-bit one alone (it is checked by itself).
    pub address_bits: u8,
    pub write_back: WriteBack,     // None, Before(by), After(by)
    pub bytes: AccessBytes,
    pub kind: AccessKind,          // Read, Write, ReadWrite
}
pub enum Base<R> { Register(R), Pc }
/// What the index adds to the address.
pub enum Index<R> {
    /// A general register, read as `extend` says, shifted left by `shift` (ARM64 `uxtw`, `sxtw`, `lsl`;
    /// x86 `[b + i * s]` all 64 bits, and `xlat`'s al, `Extend { from_bits: 8, signed: false }`).
    Register { reg: R, shift: u8, extend: Extend },
    /// x86 `bt`, `bts`, `btr`, `btc` with a register bit offset: whole operands of `width` bits from the
    /// operand's address, the register read signed at `width` (16, 32 or 64) and shifted right
    /// arithmetically by log2(width), then left by log2(width / 8). Far from the operand: `bt [rax], rcx`
    /// with rcx 0x80000 faults 64 KiB past rax on the reference (section 2, run 8).
    /// The operand's own index, when it has one (`bt [rbx + rsi * 4], rcx`), is `operand`: a register
    /// shifted left (found at X3).
    BitOffset { reg: R, width: u8, operand: Option<(R, u8)> },
    /// x86 VSIB (the gathers): each element's address takes one lane of this vector register, `lane_bits`
    /// (32 or 64) read signed, shifted left by `shift`.
    Lanes { reg: R, lane_bits: u8, shift: u8 },
}
/// How a register is read as an index: its low `from_bits` (8, 16, 32 or 64), zero- or sign-extended
/// (today's `wcontainers_arm64::Extend`, moved; ARM64 uses 32 and 64 only).
pub struct Extend { pub from_bits: u8, pub signed: bool }
pub enum AccessBytes {
    Fixed(u16),
    /// A rep string instruction: rcx elements of this size.
    Counted { element: u8 },
    /// Elements of `element` bytes, `count` of them from the address (or one per lane of a `Lanes`
    /// index), of which only those a mask register selects at run time are accessed: x86 `vmaskmov`,
    /// `vpmaskmov` and the gathers. Masked-off elements never fault (section 5.4).
    Masked { element: u8, count: u8 },
}

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
/// The fault a memory fault is: a page fault at the address, general protection (address 0), or a
/// stack fault (x86 #SS, address 0: a non-canonical access through the stack segment).
pub enum MemoryFault { Page, GeneralProtection, Stack }
/// When `Guest::step` is asked: at a stop hook call with pc, or after the kernel has had the thread.
pub enum StepAt { Stop(u64), Kernel }
/// What the runner then does. `Off`: carry on, with the stop flag lowered unless another reason holds it.
/// `Run`: carry on with the stop flag kept raised. `Exact`: run the instruction at pc alone (the stop hook
/// returns `UNWIND_GO_TO_EXACT`), the flag kept raised. `Trap`: stop the thread with a single step at pc
/// (SIGTRAP, `TRAP_TRACE`), before any other reason the flag was raised for.
pub enum Step { Off, Run, Exact, Trap }
pub enum StackMove { None, By { delta: i64, before_access: bool }, Unknown }
pub enum ReturnAddress { Register, Stack { bytes: u8 } }
pub struct SyscallRegisters<R> { pub number: R, pub args: [R; 6], pub result: R }
pub enum CodeChanges { Announced, Detected }
pub enum TrapDetail { Instruction, Local(u32), Const(i64) }
/// Where a trap leaves pc: at the instruction, or after it (`Guest::trap_kind`).
pub enum TrapPc { At, After }
pub struct HookType { pub name: &'static str, pub params: &'static [ValType],
    pub results: &'static [ValType], pub registers: Registers, pub state: StateBytes, pub body: HookBody }
/// At most `HookType::MAX_PARAMS` (4) parameters and `MAX_RESULTS` (2) results, which both runners check
/// at compile time (the browser's `cpu_hook` takes four i64 arguments after the hook's index; eight made
/// ARM64's float hook about 15 percent slower in Chrome, measured at X2, so a CPU whose hook needs more
/// raises it then and measures it again).
/// `len` bytes of a thread's state from `at`, an offset from the start of the block (`NONE` for a hook
/// whose work is not the CPU's).
pub struct StateBytes { pub at: u16, pub len: u16 }
/// Who does a hook's work on the host side.
pub enum HookBody {
    Clock,        // the runner's clock, in nanoseconds
    // the runner drops the pieces of the `line` bytes (a power of two) holding the hook's one i64
    // parameter, an address (ARM64 `ic ivau`, line 64)
    CodeChanged { line: u32 },
    Cpu,          // `Guest::run_hook`
}
/// What `run_hook` did: its results (as many as `HookType::results` says), and the slow path it was, by
/// reason number (the translator's or the CPU's), for the runner to count and warn.
pub struct HookDone { pub results: [i64; 2], pub reason: Option<u32> }
```

Built whole at X1, before any x86 code: every item above, and the translator's side of each, so that what
x86-64 and RV64GC need of the translator lands while ARM64's byte-for-byte proof guards it. The translator
passes `PASSED_IN` as parameters and gives `PASSED_OUT` back as results (a register passed out but not in
is read from, and also kept in, the thread block; a register passed in but not out, which a callee may
change, is stored into the thread block by every return and reloaded by the caller after the call, as
built at X4); keys check plans by (offset, which access) and groups
accesses by (segment, base, write), forgetting a segment's groups when its base register is written;
checks frames down to `RED_ZONE` below the stack pointer; moves the stack pointer up past a `Stack` return
address in `ret`, and after a `call` expects it `bytes` above its value at the call (the frame analysis
likewise); writes back and reloads a hook's `registers` around it; emits `check_range`, `warn` (counted
by the CPU's reason number) and `raise_stop` (with a stop check before the next instruction in the full
emitter); gives `built_address` and `jump_table` the walk back over the function's instructions found so
far; and the effects recorder counts a hook's `registers` and a `Stack` return's stack pointer. The
runners count a CPU's own reasons by number and name them from `REASONS`, give a CPU's hook only its
`state` bytes, ask `memory_fault` (the browser through a `cpu_memory_fault` export over the `cpu_state`
buffer, with the instruction's bytes after the state), and stop with the faults the abi does not name yet
(divide error, general protection, float exception, overflow, stack fault) as the test crate's own
`Stopped::Fault`. ARM64 fills in each item so that its output does not change. Three uses of these items
change ARM64's output and so come with the milestone whose x86 code needs them, each as a commit of its
own compared byte for byte on both sides with the difference shown to be only that: the description
section's guest and offer bytes (X3), and the stop and memory fault hooks returning the unwind payload,
which is what the runners' `step` and code-change handling need (X6). One refinement waits for x86 to be
measured: the first frame check of a function also covering a `Stack` return address slot, so `ret`'s read
needs no check of its own (X4; it is gated on `ReturnAddress::Stack`, so ARM64's output cannot change).

No item says what a write narrower than 64 bits does to the rest of the register (an earlier draft had
`NARROW_WRITE`): the proofs never need it. `induction` reports only 32- and 64-bit steps that write all
64 bits, a 32-bit step is used only through a 32-bit read of the register (`Domain::U32` or `S32`), where
zero and sign extension of the write agree, and a 32-bit comparison bounds all 64 bits only after a load
`loaded_max` says zero-extends. x86's 8- and 16-bit writes, which merge, are never inductions.

`TrapKind` moves from the translator's contract into the interface (the contract re-exports it) and gains
the kinds x86 needs, numbered after ARM64's four so ARM64's numbers do not change: `DivideError` (4),
`GeneralProtection` (5), `FloatException` (6, detail the `si_code`) and `Overflow` (7, x86 `int 4`: a trap
the kernel must tell from general protection, section 5.2). Each comes with the signal Linux sends (section
6). A single step is not a `TrapKind`: no meaning raises one; the runner stops the thread
with it when `Guest::step` answers `Step::Trap` (section 5.1), and the abi's `Exit` and the tests' `Stop`
carry it. Nor is a stack fault a `TrapKind`, for the same reason. It comes only from the memory fault hook, classified by `memory_fault` as `MemoryFault::Stack`, because every check a
meaning makes before its access (alignment, a canonical branch target) gives general protection on the
reference even where the access is through the stack segment (section 2, run 6: a misaligned
non-canonical `movaps xmm0, [rsp]` is general protection). The runner turns `MemoryFault::Stack` into the
stack fault exit (section 6) and the tests into `Stop::StackFault` (section 7.2).

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
  make two). ARM64 always passes 0. A meaning may check one access more than once (a gather's check per
  element, a masked store's two one-byte checks, section 5.4); such accesses are the ones the analyses
  leave alone (section 3.4), so they have no plan and every check is made as written.
- `check_range(address, length, access) -> i32 local`: whether a run-time length of bytes may be accessed,
  without stopping. Used by the x86 string instructions to choose the bulk path (section 5.6).
- `raise_stop()` (new): pop an i32; when it is not zero, raise the thread's stop flag (a store of 1 at
  `G::STOP_FLAG_OFFSET`), so that the stop hook is called before the next instruction runs. The emitter
  makes sure of that: an instruction that goes on to the next one ends with a stop check there (pc at the
  next instruction, as after a system call today); one that leaves through the bottom loop meets the stop
  check at its target piece's entry. Only x86's `popf` and `iretq` call it, when they set TF (section
  5.1); ARM64 and RV64GC never do, so their output does not change.
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
simply two sets of offsets. Where they meet again (both fall into one offset, as after x86's `je 1f; lock;
1: cmpxchg`), that offset leads a block of its own, and the walk back (`before`) stops there as at a jump
target; ARM64's instructions never overlap, so its blocks are cut as before. The bottom loop gives an alignment fault for an address not a multiple of
`ALIGN` that has no piece; no other code tests it. ARM64 faults so (bit 0 or 1 set); x86 never reaches the
test (`ALIGN` 1); RV64GC never gives it either, because no address it goes to can be odd: `jalr` clears bit
0 in its meaning, and branch and jump offsets are even. So the CPU's own meanings, not an interface item,
decide whether a misaligned target can arise; an item saying "misaligned targets fault" would have no CPU
that needs it today.

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
out-of-line flags-word functions, the liveness that leaves out flag work nothing reads, and the choice "direct
condition if the shape is known, else from the flags". What it knew about ARM64 moves into the CPU: the
shapes, their locals and types, completing them, each flag's value and the conditions. Two things are new,
both for x86:
- **Partial setters.** `sets(flags)` says which flags a shape sets. Setting a shape that does not set every
  flag first completes the other flags of the old shape into their locals, only where they are live. x86
  `inc` and `dec` keep CF; `mul` sets CF and OF and leaves the rest as the reference CPU leaves them
  (section 5.1).
- **Flags kept only sometimes** (x86 shift by a run-time count of 0 changes no flag). The meaning reads the
  old flags it may keep through `flag()` into locals and names them in the shape; `flag_value` selects.
  This is rare (shifts by cl) and needs nothing new in the emitter.
The flags-word functions keep today's layout: first, at `F_FLAGS_WORD`, the one that builds the word from
flags held in locals (generic: one i32 parameter per entry of `FLAGS`, each shifted to its bit in the
word and or-ed in `FLAGS` order, as `push_word` does today over N, Z, C, V), then one per entry of
`SHAPES` in its order, at `F_FLAGS_WORD + 1 + i`. `FLAGS_WORDS` (11 today) becomes `1 + SHAPES.len()`, and
`shapes()` goes. ARM64's `SHAPES` is today's ten shapes in today's order (section 3.5), so the function
indices and bodies stay byte for byte the same. `shape` returns an `Option`, as `Shape::of` does today:
`Constant` and `Packed` have no flags-word function, and where today's code reads
`d.effects.flags.and_then(Shape::of)` (full.rs:1057 and 1066, which number the shapes a function uses and
pick `FLAGS_IN_LOCALS` when a flags write has none) the generic code reads `.and_then(G::shape)`, so the
numbering and the flag states, and with them the output, do not change.
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
made (`FLAGS_WORDS` is 0), and a return compares its target with the expected return address without
masking. The test crate's test guest has a variant without flags whose translations are run (section 9,
X1). The flag locals are a fixed array of at most 8 (x86 has 6). Where the flags are is a set of states, bits
of a `u128`: in locals, in the block, then one for each shape a function uses, so `SHAPES` may hold at most
126 shapes, checked at compile time (x86's shapes at four widths are about 30, section 5.1).

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
`BranchRegister::Br`. A RISC-V `auipc ra, hi; jalr ra, lo(ra)` pair (a far call) is decoded by the RV64GC
crate as one 8-byte instruction whose `Flow` is `FlowKind::Call` with its target, so the translator treats it
as any direct call and needs nothing more; the pair cannot fault or stop between its halves, and its last
write of ra is the return address, so the result is exact. A jump to the `jalr` alone decodes it alone, by the
overlap rule of the instruction unit above. Any other `jalr` that links is `CallThrough` and goes through the
lookup.

Addresses the code builds (T8). `built_address(at, i, blob, code, before)` replaces "adr, or adrp plus
add at the 4-byte slot before". ARM64 keeps reading that slot from the blob, not the instruction before
on the walk's path: the two differ when the add is a jump target the walk first reached by the jump (the
walk records each offset once), and reading the slot is what today's translator does, so the X1 output
stays the same. It finds x86 `lea r, [rip + d]` alone and RISC-V `auipc` plus `addi` (2 or 4
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
`wcontainers_abi::RegisterBlock`, unchanged at X1; x86's is defined in the `no_std` core of `wcontainers-x86`
(section 6).

Floating point slow path (T26). `float_slow` replaces "FPCR or FPSR tracking not zero";
`FLOAT_CONTROLS` replaces "a write to `Fpcr` or `FloatTracking`". Calls and system calls forget the answer,
as today. x86's `float_slow` is "MXCSR's mode bits (`Mxcsr & 0xFFC0`) are not 0x1F80, or `MxcsrTracking`
is not zero", and its `FLOAT_CONTROLS` are `Mxcsr` and `MxcsrTracking` (section 5.8).

Hooks (T27). The CPU lists its hooks; the translator gives them function indices after its own six (system
call, stop, memory fault, load code, warn, trap), so ARM64's float, clock and code-changed hooks keep indices
6, 7 and 8 and the output does not change. `HookType::registers` is new: the bits a hook reads or writes in
the block (x86's x87 hook and the MMX registers, section 5.8). ARM64's hooks list none. `HookType::body` says
who does the work on the host side. The clock and the code-changed hook are the runner's own. The software
floating point (ARM64's `exact::run`, x86's SSE exact path and x87) is the CPU's, through `Guest::run_hook` on
the thread's state bytes, so no runner names a CPU function. Today the host runner (runner.rs:903) and the
browser's cdylib (browser/src/lib.rs:126) call `wcontainers_arm64::exact::run` by name and work out FPSR and
the reason themselves; at X1 that code moves into ARM64's `run_hook`, unchanged in what it computes. The
browser's cdylib then exports one function, `cpu_hook(hook, a, b, c, d)`, over a buffer of the cdylib's own
into which the JavaScript runner copies the hook's `state` bytes of the thread's state before the call and
from which it copies them back after, with `cpu_hook_result(i)` and `cpu_hook_reason()` for the results and
the reason. The hook is a slow path, but a program that sets a rounding mode takes it on every floating point
operation, so it must cost no more than its work: `run_hook` gets only the bytes `HookType::state` names
(ARM64's float hook: FPSR and FPCR through FPSR tracking, 36 bytes), which the browser copies and the host
runner hands over in place, as a slice of the thread's state memory (the guest thread is in the hook, so
nothing else touches them); the host runner registers each hook with wasmtime's raw-value form
(`func_new_unchecked`) through a closure made for the hook's index at compile time, so the hook's types and
state range are constants there and its arguments go into a fixed array with no lookup, and nothing is
allocated per call. Measured at X1: the float hook program retires no more instructions than before the
move. The buffer is found as `input` and `output` are today: an export `cpu_state() -> *mut
u8` returns its address, a buffer of the contract's `threadStateSize` bytes and `maxLength` more that the
cdylib allocates once on the first call and never frees or moves, so runner.js reads the address once after
instantiating the cdylib (and, as for `output`, makes its byte view of the cdylib's memory afresh after the
memory grows). Per hook call runner.js copies the hook's `stateLength` bytes from the thread's state memory
at its block plus `stateAt` to that address, calls `cpu_hook` with the hook's index (from the contract's `hooks`) and its
arguments (unused ones 0), reads `cpu_hook_result(i)` for each of the hook's results and `cpu_hook_reason()`
(a `Reason` number, or -1 for none), and copies the bytes back. The contract lists each guest hook's result
types, so an i32 result goes back as a number and two results (x86's X87 hook) as an array. The copies are
two `set`s between views of the hook's bytes made once per hook (the state memory is shared and never
detaches; the view at `cpu_state` is made again only after the cdylib's memory has grown), so a call
allocates nothing but the BigInts of its i32 arguments (and the array of a two-result hook). `Guest::step` reaches the browser the same
way from X6: an export `cpu_step(kind, pc) -> i32` (kind 0 for `StepAt::Stop`, 1 for `StepAt::Kernel`; the
answer is `Step`'s index) over the same buffer, which runner.js calls first in its stop hook and at its
returns from the kernel only while the guest's contract says the CPU steps (`steps: true` for x86), so an
ARM64 page never copies anything for it. The kernel never runs a hook (the runner owns
the instance whose imports they are), so hooks are part of a CPU crate's translation half, which may use the
standard library (x86's `astro-float`, section 5.8), and the kernel, which uses only the `no_std` cores
(section 3.1), never builds them. Should a `no_std` user ever need them, astro-float 0.9.6 does build without the standard library once `lazy_static`'s
`spin_no_std` feature is on (checked in scratch `x87-nostd`: a `no_std` cdylib with its own panic handler
links with it, and without it fails on a duplicate `panic_impl`, because astro-float-num's `lazy_static` then
brings std).

Memory faults (from section 5.4). The memory fault hook reports an address and a direction; whether that
is a page fault or something else is the CPU's (the browser asks through an export `cpu_memory_fault(address,
length) -> i32`, the answer's place in the contract's `memoryFaults`, after copying the thread's state and
the faulting instruction's bytes, `length` of them, to `cpu_state`) (x86: for an access reaching a non-canonical address, a
stack fault when it goes through the stack segment and general protection otherwise; two answers the
reference gives with different signals, `SIGBUS` and `SIGSEGV`). `Guest::memory_fault(address, code,
block)` answers it from the faulting
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
is below `sp - RED_ZONE` is no frame. A call whose return address is on the stack moves the frame down by its
push and, once the callee has returned, back up by `bytes`, where the emitter checks the stack pointer is.
From X4 (section 3.2, "Built whole at X1"), with `Stack` return addresses, the first frame check of a function
also covers the return address slot `[entry sp, entry sp + bytes)`, so the `ret` that reads it needs no
check of its own when the stack pointer is back at its entry value. (Built at X4 as: a frame check made where
nothing is known about the frame covers `[sp, sp + bytes)` too.) A move down makes no check when the frame
already covers its red zone, and the check a move down makes reaches as far down as the moves down that follow
it in the block before anything that calls, makes a system call, or moves the stack pointer up or by an unknown
amount, so x86's run of pushes in a prologue makes one check, not one a push; a failing check leaves to exact
code at the first move, so the later moves fault where the CPU does. A call that pushes its return address
checks no frame for the callee (the callee checks its own); its push is checked by itself only when the frame
does not cover it. `SP_ALIGNMENT_CHECKED` is false on x86
and RISC-V, where the "aligned after" analysis is never run.

Memory accesses (T32, T33). `accesses(i)` replaces "the one memory access": up to two, each with an
optional segment base (x86 `fs:` and `gs:`), base (register or pc), index (a register with shift and
extension, x86's bit offset of `bt` and its kin, or a vector register's lanes for the gathers),
displacement, write-back, size ("counted" for rep, "masked" for the masked moves and gathers), and read,
write or both. Check plans are keyed by
(offset, which). An access that reads and writes the same place (x86 `add [m], r`) is one access of kind
`ReadWrite`, checked once for writing before anything changes. The base key of the check analysis becomes
the pair (segment, base), so all `fs:[...]` accesses of a function with the same base group under the FS
base, which no instruction in the function changes unless it writes the FS base's register bit (`wrfsbase`,
and the loads of FS, section 5.5), and so do all `fs:[d]` accesses with no base or index (the commonest
thread-local form), keyed by the segment alone. ARM64's segment is
always `None`, so its keys, and its output, stay as today. The check analysis uses an
access only when its index is `None` or `Index::Register` with `from_bits` 32 or 64 and its bytes are
`Fixed` (`MemoryAccess::plain`), with or without a segment; the loop proofs only such accesses without a
segment, exactly the accesses ARM64 has today (a proof bounds the base register's values, not the segment
base's); every other access (a `BitOffset` or `Lanes` index, an
8- or 16-bit index, `Counted` or `Masked` bytes, a 32-bit address) is checked by its own meaning every
time it runs and is covered by no group and no proof, so nothing can skip a check those accesses need.
They are rare in hot code, so this costs little; the cost is measured at X5 and X8.

Code changes (T34). `CodeChanges::Announced` (ARM64 `ic ivau`, RISC-V `fence.i` and the flush system call)
or `Detected` (x86): section 5.7.

Thread pointer (T35). Nothing generic: it is whatever the CPU says. ARM64 a control (`tpidr_el0`), x86 two
register bits of class `Segment` (FS and GS bases, i64, in the block) that accesses add through
`MemoryAccess::segment`, RISC-V general register x4.

Loop proofs (T36). The recognisers become CPU functions returning plain descriptions (section 3.2's types):
`induction` ("adds step s to r at width w", today's match on `AddSub` and write-back addresses);
`branch_comparison` ("on the taken edge, r relation v holds at width w, signed or not, read at offset a"),
from which the translator makes both the counting exit (negating the relation, `Eq` and `Ne` swapped and
`Below { i }` and `Above { !i }` swapped, when the loop goes on along the fall-through edge, as today's
`cond ^ 1`) and the bound a switch's guard puts on an index (an unsigned `Below` on the edge into the
block); and `loaded_max` ("a load into r zero-extends to at most m"). How an index is read comes with the
access (the `Extend` of `Index::Register`), so it needs no function of its own. Comparisons at widths other
than 32 and 64 bits give no proof (the proof module's `Domain` has only those two), which costs only
speed. What a narrow write does to the rest of the register needs no item (section 3.2, after the
plain types): RISC-V's sign-extending `*W` steps and x86's zero-extending 32-bit ones are read only at
32 bits, and x86's merging 8- and 16-bit writes are never inductions. The generic parts (`Domain`, `Region`, `Counter`, the
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
- Flags: N Z C V at bits 31 to 28 of the flags word; `Flags` is today's `Flags` (Add and Logic at 32
  and 64 bits, Constant, Packed); `Shape` is today's `Shape`, and `SHAPES` is exactly today's ten in the
  order `shapes()` gives them: for W32 and then X64, `Add` with carry 0, 1 and 2, then `Add` with carry 1
  subtracting, then `Logic`. `sets` is always all four. FLAGS_WANTED 1.
- Block: `wcontainers_abi::RegisterBlock`; controls `tpidr_el0`, FPCR, FPSR, FloatTracking, Exclusive*;
  private bytes 32.
- Hooks: Float `(i32, i64, i64, i64) -> i64` (body `Cpu`: `run_hook` reads FPCR and FPSR from the state,
  calls `exact::run`, writes FPSR back and names the reason, today's code in runner.rs and the browser's
  lib.rs moved into the CPU crate), Clock `() -> i64`, CodeChanged `(i64)` with line 64 (`ic ivau`'s
  line); the runners step over the line by `ALIGN`.
- RETURN_ADDRESS `Register` (x30, written by the meaning of `bl` and `blr`); RED_ZONE 0; SP_ALIGNMENT_CHECKED true; data_address and branch_target
  untag (bit 55 rule), as the branch meanings already do; UNREACHABLE_RETURN i64::MIN; Offer `()`; SYSCALL
  x8, x0 to x5, x0; CODE_CHANGES Announced; memory_fault always `Page`.

x86-64 (`wcontainers_x86::X86_64`, section 5 gives the reasons):
- ALIGN 1, MAX_LENGTH 15; `decode` is `iced_x86::Decoder` in 64-bit mode over the bytes up to the end of the
  code range (an instruction that would run past it is `None`); trap detail the first 8 bytes.
- Bits: rax, rcx, rdx, rbx, rsp, rbp, rsi, rdi, r8 to r15 at 0 to 15 (hardware order); CF PF AF ZF SF OF
  at 16 to 21; FS base 22, GS base 23 (class Segment); mm0 to mm7 at 32 to 39 (i64, the mantissas of x87
  registers R0 to R7; their sign and exponent fields stay in the block's x87 area, section 5.8); xmm0
  to xmm15 at 64 to 79 (v128); the upper halves of ymm0 to ymm15 at 80 to 95 (v128). There is no zero
  register. SP_BIT 4.
- Flags: the six arithmetic flags at their rflags positions (CF 0, PF 2, AF 4, ZF 6, SF 7, OF 11); shapes
  in section 5.1. FLAGS_WANTED 1 << 62.
- Block: `X86Block` (section 5.9). Controls: `Rflags` (TF, DF, NT and ID, section 5.1), MXCSR, MXCSR
  tracking, the x87 control, status (with TOP) and tag words, the x87 last-instruction pointers, and
  `X87High(0..8)`, the sign and exponent of R0 to R7 (section 5.8, MMX); and, in the private bytes, `Step`
  (i32), `StepAt` (i64) and `StepHeld` (i32) for single stepping (section 5.1).
- step: the single-stepping machine of section 5.1. ARM64 and RV64GC keep the default (`Off`).
- Hooks: Float (SSE exact path, with MXCSR), X87 `(op: i32, low: i64, high: i64) -> (i64, i64)` reading and
  writing the x87 area and the MMX bits (both body `Cpu`, run by `run_hook` on the state), Clock.
- RETURN_ADDRESS `Stack { bytes: 8 }`; RED_ZONE 128; SP_ALIGNMENT_CHECKED false; data_address nothing (a
  non-canonical data address is past the permission table and faults); memory_fault, when the faulting access
  reaches a non-canonical address, `Stack` if its base is rsp or rbp (also every implicit stack access) with
  no `fs:` or `gs:`, else `GeneralProtection`, and `Page` otherwise (section 5.4); branch_target, and the
  meanings of every branch through a register or memory, stop with a general protection fault when the target
  is not canonical (a call after its push); UNREACHABLE_RETURN i64::MIN; Offer `{ avx: bool }`; SYSCALL rax,
  (rdi, rsi, rdx, r10, r8, r9), rax; CODE_CHANGES Detected.

RV64GC (`wcontainers_riscv::Rv64`, paper only, the check that nothing assumes flags, a link register that
is special, 4-byte instructions or a special stack pointer):
- ALIGN 2, MAX_LENGTH 8; decode reads a 16-bit unit and, when its low two bits are 11, a second; trap detail
  that unit or word. An `auipc ra, hi` followed by `jalr ra, lo(ra)` decodes as one 8-byte call with its
  target (3.4 Control flow).
- Bits: x1 to x31 at 1 to 31 (x0 is the hard zero, no bit), f0 to f31 at 64 to 95 as i64 (single precision
  values are NaN-boxed). SP_BIT 2 (sp is x2 by the ABI only; nothing in the CPU checks it). tp (x4) is an
  ordinary general register, so the thread pointer needs no control. Passed in and out: a0, a1, sp.
- Flags: none. FLAGS, SHAPES empty, `Cond` empty, FLAGS_WANTED 0. Branches compare two registers directly.
- Controls: fcsr (frm and fflags), the LR/SC reservation (the Exclusive* shape again).
- Hooks: Float (fcsr; RISC-V instructions may also carry a static rounding mode, so `float_slow` may be
  answered per instruction), Clock (rdtime), and for `fence.i` a hook with no parameters whose runner drops every piece: a
  `HookBody` variant of its own (`AllCodeChanged`), added with the RV64GC crate, since no CPU built
  before it needs it.
- RETURN_ADDRESS `Register` (ra by convention, but `jalr` may link to any register: the meaning writes
  it, and a call is recognised by `FlowKind`, so the interface names no register); RED_ZONE 0; SP_ALIGNMENT_CHECKED false; no untag;
  UNREACHABLE_RETURN 1 (no `jalr` target has bit 0 set); Offer `()`; SYSCALL a7, a0 to a5, a0;
  CODE_CHANGES Announced; memory_fault always `Page`. Its branches compare two registers, so `branch_comparison` reads them at the branch.

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
   GS segment bases, plans per access; the forms that are not base plus scaled index (x86's bit offsets,
   gathers, masked moves) are described exactly and checked only by their own meanings, never grouped or
   proven (sections 3.4 and 5.4).
5. **Passed registers.** CPU-chosen, different in and out (section 3.4).
6. **Unannounced code changes.** Write protection through the permission table; a write faults, the page's
   pieces are dropped, the page becomes writable, and the faulting instruction runs once by phase one's exact
   path (`UNWIND_GO_TO_EXACT`, `EXACT_SLOT`), its piece translated without protecting its page, asked for by
   the memory fault hook returning the unwind payload, so writes into the running piece, even into the next
   instruction on the writer's own page, take effect at once and always make progress (section 5.7).
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
    result, never an emulator's. (Batching and recorded results are not built at X2, which serves cases one
    at a time; they are proposed for X3, section 7.3.)
13. **Instruction unit and boundaries.** From `G::ALIGN`; real boundaries for random starts come from a
    linear decode (with `G::decode` and `G::flow`) from each piece start the translator found.
14. **Stopping the reference after one instruction.** The trap flag (section 2), proven.
15. **Register state and the wire format.** A CPU-described comparable state of named fields with widths
    and the bits that are compared; instructions are length-prefixed.
16. **Bits allowed to differ, and outcomes.** Per instance, from the CPU's test support. x86's undefined
    flags are not masked: they must match the reference CPU, as the brief says. New outcomes: divide error
    (SIGFPE), general protection (SIGSEGV, SI_KERNEL, address 0), stack fault (SIGBUS, SI_KERNEL,
    address 0), float exception, single step.
17. **Unannounced code writes in the runners.** The runners hold a stand-in of the write protection of
    section 5.7, and a program writes code with no announcement and runs it.
18. **Layer 2 generator.** A shared program model with a per-CPU emitter.
19. **Forms list for x86.** From iced-x86's own tables, split by the features of the `Code` the
    translator's decoder gives; the forms not offered are compared with the reference too, except for
    the features the reference has (section 7.4).
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
- **As built at X4**: the shapes are `Add`, `Sub` (each plain, with a carry or borrow in, and with CF kept),
  `Logic`, `Multiply` and `Shift` (shl, sal, shr, sar), at each width, all in `SHAPES`; `condition_direct`
  answers for a plain difference and the logic instructions. `Shift` keeps the result, CF and OF (CF and OF
  already chosen as they were when the masked count is zero), and, only when the count may be zero (by cl, or
  an immediate whose masked value is zero), the count and PF, AF, ZF and SF as they were: three locals or
  eight, not nine, since folding the count into CF and OF at the shift saves two. Rotates, bt and its kin,
  clc, stc and cmc change only CF (and OF), so they keep no shape: `Flags::Each` holds the six flags in
  locals of their own (the kept ones read as they were), which the translator copies into its flag locals.
  `Bits` for bsf, bsr, popcnt, lzcnt and tzcnt comes with those instructions at X5. `Packed` stays for
  popf, the system call's return and the bottom loop. With look-ahead, an instruction whose flags nothing
  reads before every one is set again (`flags_needed_after`) emits nothing for them, on both guests:
  `set_flags` writes nothing (no shape copies, no flag state), and a meaning asks `Emitter::flags_live`
  first so that what it works out only for the flags is left out too (a shift's or rotate's CF and OF, a bit
  test's CF, the flags `Each` keeps, `inc` and `dec`'s kept CF, the high half of a two- or three-operand
  `imul`). Look-ahead none answers that every flag is live. What the reference CPU leaves where Intel says undefined, found
  by layer 1 and a scratch sweep of every shift and rotate at every width and count 0 to 31 (scratch
  `x3/run1/shifts.txt`): OF after a shift or rotate is what a count of one gives, from the value before it,
  for every count, except `rol` and `ror` of a register by an immediate above one, which keep OF; AF is clear
  after the shifts and the logic instructions; after `mul` and `imul` ZF and AF are clear and SF and PF come
  from the low half of the result; `bt` and its kin change only CF; `div` and `idiv` change no flag.
- **DF is a control, not one of the lazily tracked flags.** Only `std`, `cld`, `popf` and `iretq` write
  it and only the string instructions read it, so it gains nothing from lazy tracking, and keeping it out
  of `FLAGS` keeps the liveness test per flag exact (section 3.4): no arithmetic instruction sets DF, so
  with DF among the flags no flag update could ever be found dead, and every string instruction would
  count as a flag reader. It lives in `Control::Rflags`, an i64 in the block holding the bits of rflags
  outside the flags word that user code changes and keeps, at their rflags positions: TF (bit 8, the trap
  flag, below), DF (bit 10), NT (bit 14, nested task) and ID (bit 21, which programs toggle to see that
  CPUID exists). At CPL 3 `popf` and `iretq` write exactly these four of the bits outside the six flags,
  besides AC (below), and the reference keeps them (section 2, run 7: `popf` of 0x3FFFFF leaves NT, AC
  and ID set and IOPL, RF, VIF and VIP not written; run 10 for TF). A string instruction reads DF with one
  load and a shift per pass.
- **NT** means nothing to a 64-bit user program except one rule: `iretq` while NT is set gives general
  protection (Intel's `iret` pseudo-code for IA-32e mode; seen on the reference, section 2, run 7), before
  it reads its frame. So `iretq`'s meaning tests NT in `Control::Rflags` first. Nothing else reads it;
  it is kept, stored by `pushf` and carried through signal frames like any user bit.
- **The flags word** in the block holds the six arithmetic flags at their rflags positions and nothing
  else. The user-visible rflags is that word, or `Control::Rflags`, or bits 1 and 9 (IF), which user code
  always sees set: `pushf` puts it together in its meaning; `popf` and `iretq` split it (the six flags as a
  `Packed` shape, TF, DF, NT and ID into the control, IOPL, IF, RF, VIF and VIP ignored as user mode does);
  the kernel's personality does the same at signal frames and `ptrace` (section 6). So the interface needs
  no item for bits a CPU always has.
- **TF, the trap flag**, is held exactly, in `Control::Rflags` bit 8, and stepped exactly as the reference
  steps (section 2, run 10). The rule the CPU keeps: an instruction that **starts** with TF set and
  finishes stops the thread right after it with a single step (SIGTRAP, `TRAP_TRACE`, rip at the next
  instruction or the branch's target). So the instruction that sets TF (`popf`, `iretq`) does not stop,
  the one after it does, and `popf` that clears TF still stops after itself. An instruction that faults or
  traps (`int3`, a page fault) gives only its own signal. A system call does not stop after itself: Linux
  returns from it with TF set through `iret`, so the instruction after it runs first; the same holds after
  every return from the kernel with TF set (`rt_sigreturn` of a frame with TF, a signal handler's return
  after a fault). `pushf`, the r11 of `syscall` and signal frames show TF, as they are put together from
  `Control::Rflags`; a signal handler starts with TF clear (section 6).
  How it is done, with no cost to any instruction while TF is clear:
  - The stop flag carries it. While TF is set, the thread's stop flag stays raised, and every stop hook
    call goes first to `Guest::step` (section 3.2), which x86 fills in as a small machine on three words of
    the CPU side's private bytes (controls `Step`, `StepAt` and `StepHeld`, which start zero): idle, one
    instruction about to run alone at an address, or that instruction running.
  - Entering stepping. `popf` and `iretq` call `Emitter::raise_stop` with the new TF bit, and count it
    ("trap flag set by the program", x86's own reason, warned); the kernel's personality, when it returns
    to a thread with TF set, makes the runner ask `step` with `StepAt::Kernel`, which answers `Run` while
    TF is set and the runner raises the flag. Either way the next instruction's stop check calls the stop
    hook.
  - Each step. At a stop hook call while idle with TF set, `step` notes the pc and answers `Exact`; the
    stop hook returns `UNWIND_GO_TO_EXACT`, and the bottom loop runs that one instruction from the piece
    of look-ahead none in `EXACT_SLOT` (phase one's exact path, as in section 5.7). The next stop hook call
    is that piece's entry stop check, at the noted pc (the bottom loop calls only `warn` and `load_code`
    in between; if `load_code` says leave, the thread leaves and `StepAt::Kernel` makes the machine idle):
    `step` marks the instruction running and answers `Run`.
    The instruction runs; the next stop check is the next piece's entry (any look-ahead), or the stop check
    after a rep string instruction's pass (section 5.6), and there `step` answers `Trap`: the runner stops
    the thread with the single step at that pc, ahead of any other reason the flag was raised for, as
    Linux delivers the synchronous SIGTRAP first. Every step also counts phase one's "left to address".
  - Back to idle. `StepAt::Kernel` (after the system call hook, before each entry into `run`) sets the
    machine idle, so after a system call, a fault, a trap or a signal the first instruction runs alone
    before the next stop, as on the reference. A stop with TF clear and nothing running answers `Off`, and
    the flag is lowered unless another reason holds it. When `step` answers `Run` or `Exact` and another
    reason holds the flag too (a signal to deliver, the thread asked to stop), that reason is dealt with
    first, as today, and the thread leaves; the return makes the machine idle.
  - `mov ss` holds a trap back one instruction (section 2): its meaning writes 1 to `StepHeld`; when the
    instruction running was such a one, `step` clears `StepHeld` and answers `Exact` for the next
    instruction instead of `Trap`, so the trap comes after that one. `StepHeld` is cleared whenever an
    instruction starts to run alone, so a stale 1 from a `mov ss` run without TF does nothing.
  - What the stop hook returns: at X6 it returns the unwind payload, as the memory fault hook does
    (section 5.7): 0 to carry on, `UNWIND_LEAVE` to leave, `UNWIND_GO_TO_EXACT` to run the instruction at pc
    alone; its call sites unwind with what it returns. ARM64's runners return only 0 and `UNWIND_LEAVE`, so
    ARM64 behaves as today; its output changes only inside each stop check's branch taken when the flag is
    raised (the returned payload is unwound with instead of the constant), and its whole suite runs again
    at X6.
  - One known difference: a gather that faults part way while TF is set. The reference first stops with
    `TRAP_TRACE` after the elements gathered (rflags with RF, section 2, run 8) and faults only when run
    again; the translated gather faults at once with the same elements gathered. A program sees the extra
    SIGTRAP only if it sets TF around a faulting gather, which only the AVX setting offers. Recorded as a
    known difference with that reason and checked by a designed-answers case at X8.
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
- iced decodes with `DecoderOptions::FORCE_RESERVED_NOP` and otherwise its defaults, which follow Intel
  where Intel and AMD decode differently. That one option makes iced decode the whole reserved-NOP space
  (`0F 0D` and `0F 18` to `0F 1F` with a ModRM) as its `Reservednop_*` Codes, whose feature is the
  baseline multi-byte NOP, instead of as the instructions of later features placed there (why: "A feature
  not offered" below). It changes no length and no `INVALID` (a lock prefix there stays `INVALID`). An
  encoding iced calls `INVALID` traps `Undefined` (SIGILL), except one longer than 15 bytes, which traps
  `GeneralProtection` (SIGSEGV, `SI_KERNEL`, address 0, rip at the instruction): the CPU's length limit wins
  over every other rule, invalid opcodes and invalid lock prefixes included (section 2, run 5). iced reports
  both as `INVALID` (it stops reading at 15 bytes, and at the field that makes an encoding invalid, so a
  14-byte `INVALID` can be 18 bytes to the CPU), so `decode` counts every encoding iced calls `INVALID`, or
  cannot finish, or decodes as a `Code` of the never-claimed group, as the CPU's length decoder counts it,
  by a table: the CPU counts bytes by its own tables even for opcodes that do not exist in 64-bit mode
  (section 2, run 5: `0F 38 FF` and `9A` count bytes after the opcode; `06` and `0F 24` count nothing after
  it, where iced counts one), and counts other makers' instructions by Intel's map (3DNow's `0F 0F` counts
  nothing after it, XOP's `8F` a ModRM, SSE4a's `66 0F 78` a ModRM and no immediates; seen at X3). The table
  was made at X3 from a sweep on the reference (`x86-lengths` in the test crate, its output
  `crates/wcontainers-x86/tests/lengths.txt`): every opcode of the one-byte, 0F, 0F38 and 0F3A maps, alone
  and after 66, REX.W, 67, F2 and F3; every opcode after a two-byte VEX prefix, a three-byte one with each
  of the 32 values of its map field (W and pp varied for the first four), an EVEX one with each of the 8
  values of its map bits (W, pp, the bits that must be 1 and 0, and R and X varied for the first four), XOP's
  `8F` with every next byte, and every 3DNow suffix; each with three fillers after the opcode (a register
  ModRM with reg 0 and with reg 7, and a ModRM asking for a SIB and a 32-bit displacement); each length found
  by how many segment prefixes before it first give general protection (76,100 lengths in 300 lines; run twice,
  the same both times). What it
  shows, and the table holds: the one-byte and 0F maps count by opcode (nothing, a ModRM with what it asks
  for, immediates of 8, 16, 16 or 32 by 66, 32 whatever the operand size for `E8`, `E9` and `0F 8x`, 16,
  32 or 64 by 66 and REX.W for `B8` to `BF`, a far pointer for `9A` and `EA`, an 8- or 4-byte address by
  67 for `A0` to `A3`, the `F6` and `F7` immediates only for reg 0 and 1, and one byte after `0F 20` to `0F
  23`, whose mode the CPU ignores); the 0F38 map counts a ModRM after every opcode and the 0F3A map a ModRM
  and an 8-bit immediate; F2 and F3 change nothing; in the legacy 0F map, `38` to `3F` lead a third opcode
  byte, counted as the 0F38 map when bit 1 is clear and as 0F3A when set. A VEX or EVEX prefix takes its map
  from the low two bits of its map field and counts its opcode by the same tables, except that `38` to `3F`
  of its 0F map count nothing after them; W, pp, L and the EVEX bits that must be fixed change nothing; with
  the low two map bits 0 the CPU counts only the byte after `C4` or `62` as a ModRM. 66 and REX.W before a VEX
  or EVEX prefix only make it invalid and change no length; 67 does. Each table entry the sweep cannot
  measure (an instruction that faults however short it is: `in`, `out`, `ret` to the filler's
  non-canonical address, and the like) is Intel's opcode map, and a second test checks every entry against
  iced's length of the valid instruction (a lock prefix makes nearly every instruction invalid, so its
  length comes from the table). The longest legal instruction is 15 bytes: the four kept prefixes and a
  one-byte opcode with ModRM, SIB, a 32-bit displacement and a 32-bit immediate (`F3 66 67 48 C7 84 24`,
  then 4 and 4 bytes; REX.W wins over `66`, so the immediate stays 4 bytes); with a segment prefix before
  it, 16 bytes, it gives general protection (seen by the fifth independent check). Over 15, the instruction
  stays iced's `INVALID` with that length set in it (`set_len`, a byte) and its meaning traps
  `GeneralProtection`; otherwise its meaning traps `Undefined`, or `Unsupported` for a never-claimed
  `Code`. It costs nothing on any valid instruction of a claimed feature; layer 1's no-form cases check it
  against the reference. A too-long encoding whose rest runs past the end of the code range is `None` like
  any instruction that runs past it (what the CPU does there is not seen, section 12).
- **A feature not offered: its bytes do what a CPU without that feature does with them.** The reference is
  such a CPU for most of them, and decides. For nearly every such instruction that is illegal
  instruction, so it traps `Unsupported` (SIGILL, counted, warned), with x86's own reason when its claim
  group is the AVX setting (section 5.10). Which claim group a `Code` belongs to comes from the table the
  build script makes from iced's own `instr_info` (section 2), since the shipped decoder has no
  `encoding()` or `cpuid_features()`. A `Code` with no meaning written is of a feature never claimed and
  traps `Unsupported`. Two kinds of encoding run on a CPU without their feature, and run the same way here:
  - The reserved-NOP space. Intel reserves `0F 0D` and `0F 18` to `0F 1F` as NOPs with a ModRM, and later
    features put instructions there so that older CPUs run them as NOPs: CET's `endbr64`, `endbr32`, `rdsspq`
    and `rdsspd`, `cldemote`, MPX's bound instructions, `prefetchw`, `prefetchwt1`, `prefetchit0` and
    `prefetchit1`. Seen on the reference, which has no CET and no CLDEMOTE (section 2, run 9; the independent
    check's run 8 first): every one of them, every reg value of `0F 0D` and `0F 18`, register and memory
    forms, with the address mapped, unmapped, non-canonical or rip-relative, ends with rip after the
    instruction and nothing else changed (`rdsspq rax` leaves rax as it was), while a lock prefix on any of
    them gives SIGILL. So with `FORCE_RESERVED_NOP` they are all `Reservednop_*` Codes, whose one meaning does
    nothing: no access, no check, no fault. That is what a program built with CET or `-fcf-protection` needs:
    Ubuntu 20.04's libc on the reference holds 2,949 `endbr64` and 12 `rdssp` (`objdump -d`, libc6
    2.31-0ubuntu9.18; the `endbr64` begin nearly every function, and most `rdsspq` sit behind glibc's own test
    of its thread's CET bits, `testl $0x2, %fs:0x48`, while some run unguarded, as NOPs on the reference too).
    The claimed prefetches and `nop` with a ModRM are in that space too and do nothing and never fault either
    (section 5.10). The CET instructions outside it (`incssp`, `rstorssp`, `wrss`, `setssbsy`, `saveprevssp`,
    `clrssbsy`) give SIGILL, as `incsspq` does on the reference.
  - Prefixes a CPU without the feature ignores: `xacquire` and `xrelease` (HLE) on a locked instruction,
    `bnd` (MPX) on a branch, `notrack` (CET, a `3E`) on an indirect branch. iced keeps them as prefixes of
    the instruction, and its meaning ignores them, as the reference does (section 2, run 9).
- Kernel-only instructions (hlt, in and out, cli and sti, lgdt and the rest, mov to control registers,
  rdmsr, wrmsr, rdpmc while Linux does not allow it, `int n` other than 3, 4, 0x80 and the `int1` and
  `int3` forms; every `Code` in the claim table's "kernel only" group, section 2, whatever its feature): the
  fault Linux gives a user program, which for most is general protection (SIGSEGV, SI_KERNEL, address 0)
  and for some #UD (SIGILL); layer 1 decides each against the reference. As found at X3 (layer 1, the 49
  kernel-only forms at 1,000 cases each): every one is general protection but `rsm`, which is #UD outside
  system management mode; `in` and `out` and the string forms included. A kernel-only `Code` of a feature
  not presented is in the never-claimed group (section 2) and gives illegal instruction. `rdpmc` is put in "kernel only"
  by the build script although iced's `is_privileged()` is false for it and its feature is `RDPMC`: with
  CR4.PCE clear, as Linux leaves it, the reference gives general protection (check 10, `0f33`).
  `int 4` (`CD 04`) is a trap, not a fault: vector 4 is a user-callable gate, so the reference gives SIGSEGV
  `SI_KERNEL` address 0 with rip after the instruction and no RF in the frame (check 10); its meaning
  traps with `TrapKind::Overflow` (vector 4, #OF) and `TrapPc::After`, a kind of its own so the kernel can
  tell it from general protection: its frame has trap number 4 and no RF, general protection's 13 and RF. `int 5` and `int 0x81` fault at the instruction.
- VMX instructions (`vmcall`, `vmlaunch`, `vmread`, `vmwrite`, `vmptrst`, `vmclear`, `vmxon`, `invept` and
  the rest; feature VMX, never claimed) give illegal instruction, as on a CPU without VMX, which is what we
  present. The reference's own answer depends on its KVM setting, not on the CPU: with `kvm_intel` loaded
  and `enable_virt_at_load=Y` the CPU stays in VMX root operation and they give general protection
  (check 10), without it #UD. So layer 1 does not compare them with the reference; the designed-answers
  test checks SIGILL, and the reason is recorded with the exclusion.
- Breakpoints: `int3` (`CC`) and `int 3` (`CD 03`) stop with `Breakpoint`, SIGTRAP with `SI_KERNEL`, rip
  after the instruction; `int1` (`F1`) stops with `Breakpoint`, SIGTRAP with `TRAP_BRKPT`, rip after it
  (section 2). Their traps answer `TrapPc::After` from `Guest::trap_kind`, so the emitters save pc as the
  next instruction's (a SIGTRAP handler that returns carries on after the breakpoint, as on the CPU). The trap's detail carries the `si_code`, and layer 1 compares it (`Stop::Breakpoint` holds
  it, section 7.2).
- `int 0x80` from 64-bit code is Linux's 32-bit system call gate on the reference (section 2: it ran i386
  getpid). 32-bit code and its system call numbers and layouts are deferred with 32-bit mode, so this
  phase behaves as a Linux kernel built without 32-bit emulation (`CONFIG_IA32_EMULATION` off), where
  vector 0x80 is a kernel-only gate like `int 0x81`: general protection (SIGSEGV, SI_KERNEL), rip at the
  instruction. That is reporting the missing feature the way Linux does, not a pretence. It is counted and
  warned, x86's own reason "32-bit system call from 64-bit code, not provided". Expected from Linux's
  source and not seen on a running kernel without 32-bit emulation; layer 1 excludes `int 0x80` with that
  reason and the designed-answers test checks the fault.
- `sysenter` (`0F 34`, iced `Code::Sysenter`, valid in 64-bit mode on Intel CPUs) is the other way into
  Linux's 32-bit system calls: on the reference it runs one and returns to the vDSO (section 2, run 7).
  A Linux kernel without 32-bit emulation loads `IA32_SYSENTER_CS` with the null selector
  (`GDT_ENTRY_INVALID_SEG`, in `arch/x86/kernel/cpu/common.c`), and `sysenter` with that selector is
  general protection by Intel's pseudo-code. So its meaning is the same as `int 0x80`'s: general
  protection, rip at the instruction, counted and warned under the same reason. Expected from Linux's
  source and the manual, not seen on such a kernel. Its feature, SEP, is claimed by default (section
  5.10), as Linux without 32-bit emulation still reports it: the instruction exists; the kernel has no
  32-bit system calls behind it. `sysexit` is kernel-only (general protection, as above). Layer 1
  excludes `sysenter` with the reason of `int 0x80`, and the designed-answers test checks the fault.
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
    work; the CPU's own 64-bit `div` is a long instruction too. Measured at X3 (`measure_long_division_x86_64`,
    `programs/x86_64/divide.c`: 50 million `div r64` in a loop, with a high half and with none, best of 5):
    the long path adds 13.4 ns a division over the fast path at look-ahead none in wasmtime on the Apple M5
    (load about 38), and the CPU's own `div` with a high half takes 14.7 ns more than with none on the
    i7-8750H (load 0.2 to 1.3): the long path costs about what the CPU's own does.
- `mov ss` and `pop ss`: `pop ss` does not exist in 64-bit mode (iced decodes it as `INVALID`); `mov ss`
  with a selector Linux allows changes nothing a user program sees, except that a trap flag the program
  set stops after the instruction after it, as on the CPU: its meaning writes 1 to the `StepHeld` control
  (section 5.1).

### 5.3 Calls and returns

- `call` (direct, through a register, through memory) becomes a WebAssembly call to the target's piece,
  as on ARM64. Its meaning does what the reference CPU does, in the reference's order (section 2, run 3):
  1. `call [m]` reads its target (access 0); a fault there stops first, nothing changed.
  2. It writes the real return address (this instruction's address plus its length) at rsp - 8 (the push:
     access 1 for `call [m]`, else 0); a fault there stops next, nothing changed: a page fault, or the
     stack fault when rsp - 8 is not canonical or the 8 bytes cross 2^47 (section 5.4). The write is
     inside the frame checked when rsp last moved down, so it usually needs no check of its own (below).
  3. It checks the target is canonical; if not, it stops with general protection at the instruction, with
     the return address already written at rsp - 8 and rsp unchanged. That is what the i7-8750H does for
     `call rax` with 0x8000000000001000 or 0x800000000000, `call r8`, `call [rdi]`, and a direct `call
     rel32` to 0x800000000000 from code at 0x7fffffff8000; `jmp rax` and `jmp rel32` to the same targets
     write nothing. With rsp 0x1008 (unmapped below) the push's page fault comes instead of the general
     protection; `call [rdi]` with rdi 0x3000 and rsp 0x1008 faults at 0x3000, so the read comes first.
  4. It sets rsp to rsp - 8 and calls the emitter's `call`, which makes the WebAssembly call.
  Intel's manual's pseudo-code checks the target before the push; the CPU does not, and the CPU decides.
- `ret` reads [rsp] (a stack fault when rsp is not canonical, section 5.4) and checks the value is canonical,
  else it stops with general protection, rsp not moved, as on the CPU (section 3.4, T14: this check comes
  before the compare, so a popped value can never match the bottom loop's marker). If the value is the address
  the caller expects, rsp moves up by 8 and the piece returns directly. If not, rsp moves and the thread
  leaves to the bottom loop through "go to address". `ret imm16` also adds its immediate. `jmp` through a
  register or memory makes the same canonical check first, before anything changes; `call` makes it after its
  push (above).
- **Far transfers** (`call far m16:64`, `jmp far m16:64`, `retf` and `retf imm16` in their 16, 32 and 64-bit
  operand sizes, `iretq`; valid in 64-bit user mode, so in layer 1's forms list). Their meanings read the
  selector with the offset and act on it as the CPU does in user mode on Linux. Seen on the reference (section
  2, run 4): with selector 0x33 (Linux's 64-bit user code segment, the one running) `call far [rdi]` reads its
  10 bytes, pushes 16 (the return address at rsp - 16, the old CS 0x33 zero-extended above it) and lands at
  the offset; `jmp far` lands with nothing written; `retfq` pops the offset and the selector, rsp up by 16;
  `iretq` pops rip, CS, rflags, rsp and SS (40 bytes) and lands with rsp and rflags loaded (IF kept set). A
  selector that names no code segment (0, 0x2b, which is data, and SS 0x33 for `iretq`) gives general
  protection with nothing written and rsp unchanged, so for far transfers the selector is checked before the
  push; a non-canonical offset with selector 0x33 gives general protection after the 16-byte push, as a near
  `call` does. So the meanings: read (access 0, checked first), test the selector, push (access 1, for `call
  far`), test the offset is canonical, then move rsp and transfer. `iretq` differs in two ways (section 2, run
  7). It first tests NT and gives general protection when it is set, before any read (section 5.1). And it
  reads its frame as five accesses of the operand size in the CPU's order, not as one, so a fault names the
  slot the CPU names, each slot canonical or not by itself (X5, scratch `x5/run5/iret.c` and `iret.out`, partly
  mapped frames on the reference): rflags first, then cs, then rip, then the cs test (null, not code, or a
  privilege level other than 3: general protection), then ss, then rsp, then the ss test (0x2b alone), and
  the rip's canonical test last (its general protection comes after ss's read). Whether rsp's read comes
  before the ss test cannot be seen (no frame maps ss's slot but not rsp's). `retf` reads cs, then rip,
  then tests cs (0x33 alone; 0x30 to 0x32 are general protection). Of the cs and ss slots a return reads the
  selector's 16 bits alone (layer 1 on the reference: a `retfq` whose cs slot starts 5 bytes below 2^47 page
  faults there, and a `retfd` whose cs slot ends on an unmapped byte past the selector gives general
  protection; `iret` is given the same reads, which no case has yet told apart on the reference). A far
  pointer (`call`, `jmp`, `lfs`,
  `lgs`, `lss`) is read as the offset, then the selector, each faulting by itself; `pop` of a segment
  register reads the selector's 16 bits alone; a far `call` writes cs's slot before the return
  address's. Seen too: the CPU makes a null selector with a privilege level (1 to 3) 0 on every `iret` to
  user mode, so a program sees what it loaded only until the next interrupt or signal (layer 1 reconciles
  it, and the meanings that load selectors are marked as varying); and an `iret` that loads AC from an
  unaligned frame takes an alignment check on its later reads, which Linux takes for a split lock and
  retries for ever (layer 1 leaves such cases out; AC is never held here, plan 5.1). A far transfer with selector 0x33 is, for the translator, a jump or
  call through memory (`FlowKind::JumpThrough` or `CallThrough(Memory)`, the return address 16 bytes up for
  `call far`'s matching `retf`) and `retf` and `iretq` are returns that leave to the bottom loop when the
  target is not the one expected; they are rare, so no more than that. Selector 0x23 (Linux's 32-bit user code
  segment) switches the reference to 32-bit mode, which this phase does not have: it stops with general
  protection at the instruction, nothing changed, the fault a CPU gives for a selector it cannot load; counted
  and warned, x86's own reason "far transfer to 32-bit code, not provided", like `int 0x80` (section 5.2).
  Layer 1 excludes selector 0x23 with that reason; the designed-answers test checks the fault.
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
  the manual says. `cmps` reads [rdi] before [rsi]: with both unmapped the reference faults at rdi's (seen at
  X3); `movs` reads [rsi] before it writes [rdi].
- **Accesses that are not one operand** (section 2, run 8, all on the reference). Each is described
  exactly by `accesses`, checked by its own meaning only (section 3.4), and its meaning makes the checks
  and changes in the CPU's order, so a fault leaves the CPU's state and the CPU's address:
  - **`bt`, `bts`, `btr`, `btc` with a register bit offset and a memory operand**: one access of the
    operand's size (`Fixed`, read for `bt`, `ReadWrite` for the others) at the operand's address plus
    `Index::BitOffset`: the register read signed at the operand's width, shifted right by log2(width) and
    left by log2(width / 8). `bt [rax], rcx` with rcx 0x80000 faults at rax + 0x10000; a 32-bit offset of
    0x80000000 reaches 256 MiB below the operand and a 16-bit one of 0x8000 4 KiB below; the register's
    bits above the width are ignored. With a base of rbp the access goes through the stack segment
    (`bt [rbp], rcx` reaching past 2^47 is a stack fault, through rax general protection). The immediate
    forms address the operand itself.
  - **`xlat`**: one byte read at rbx + al, `Index::Register` with `Extend { from_bits: 8, signed: false }`
    (al 0xFF reads rbx + 0xFF).
  - **The gathers** (`vgatherdps`, `vgatherqps`, `vpgatherdd` and the rest, AVX2): one access with an
    `Index::Lanes` index and `Masked` bytes. The meaning goes through the elements in lane order; for each
    element whose mask lane is set it checks the element's range (reporting its first byte not allowed),
    loads it, writes the destination lane and clears the mask lane. A fault therefore leaves what the CPU
    leaves: the elements before it gathered and their mask lanes clear, the rest untouched, rip at the
    gather, and the fault at the first byte not allowed of the faulting element (seen: elements 0 and 1
    gathered and their mask lanes cleared when element 2 faults; with elements 1 and 3 unmapped only
    element 0 is gathered and the fault names element 1; an element across the page end faults at the
    page's end with nothing gathered). Masked-off elements never fault. When all are done the whole mask
    register is zero (seen). A gather is one instruction for the stop check; the program's
    signal handler may map the page and return, and the gather goes on from the mask it left.
  - **`vmaskmovps`, `vmaskmovpd`, `vpmaskmovd`, `vpmaskmovq`** (AVX setting): one access with `Masked`
    bytes. Masked-off elements never fault, and nothing is loaded or stored when a selected one faults
    (seen: lane 0 not stored when lane 3 faults; the load leaves its destination unchanged). With S the
    first byte of the first selected element and E the last byte of the last one: a load checks [S, E] for
    reading and reports its first byte not allowed; a store reports S when S is not allowed and E otherwise
    (seen in nine store cases, 128-bit and 256-bit, `vmaskmovps` and `vpmaskmovd`, including elements across
    the page end: lanes 0 and 3 from 0x2000fff4 fault at 0x20010003, lane 3 alone at 0x20010000). Since [S,
    E] is at most 32 bytes it crosses at most one page boundary, so the store's meaning makes two one-byte
    write checks, at S and then at E, which give exactly that address, and then stores the selected
    elements.
  - **`maskmovdqu`** (SSE2, offered by default): the CPU ignores the mask for faults. Its accesses are two
    8-byte writes in this order: rdi + 8, then rdi. Both are checked (each reporting its first byte not
    allowed) before any byte is stored, then the bytes the mask selects are stored (seen: an all-zero mask
    faults; rdi 0x2000fffc faults at rdi + 8 whatever the mask, with byte 0 not stored; rdi 0x2000fff4
    faults at 0x20010000; rdi 0x1ffffffc and 0x1ffffff8, low half below a mapped page, fault at rdi).
  Which fault a non-canonical gather element, masked span or `maskmovdqu` half gives is decided by the
  rule below, on the range `memory_fault` works out; layer 1 draws those addresses near 2^47 too.
- **Non-canonical data addresses** stop the thread at the permission table (they are above every page it
  covers). The CPU faults on any access that reaches a non-canonical address, also one that starts at a
  canonical one: on the reference an 8-byte load at 0x7ffffffffffc and a 4-byte load at 0x7ffffffffffe
  through rdi fault so, while an 8-byte load at 0x7ffffffff000 and a 1-byte load at 0x7fffffffffff give a
  page fault (`SEGV_MAPERR`) at their address (section 2, run 3). So the CPU's check of canonical
  addresses comes before the page check. Which fault it is depends on the segment the access goes
  through (section 2, run 6):
  - **the stack segment** gives a stack fault (#SS), which Linux sends as `SIGBUS` with `SI_KERNEL` and
    address 0. An access goes through it when its base register is rsp or rbp (r12 and r13 do not
    count) and it has no `fs:` or `gs:` prefix, and for every implicit stack access: the pushes and pops
    of `push`, `pop` (its read), `call`, `ret`, `enter`, `leave`, `pushf`, `popf`, the far transfers and
    `iretq`. The `ds:`, `es:`, `cs:` and `ss:` prefixes change nothing in 64-bit mode. x86's `accesses`
    names rsp as the base of every implicit stack access (rbp for the read of `leave`), so the rule is
    one test on the access: segment `None` and base `Register(rsp)` or `Register(rbp)`.
  - **every other access** (another base, no base, an index only, rip-relative, `moffs`, the string
    instructions' rsi and rdi, anything with `fs:` or `gs:`) gives general protection (`SIGSEGV`,
    `SI_KERNEL`, address 0).
  One instruction can have one of each: `pop [rbx]` gives the stack fault when rsp is not canonical and
  general protection when rbx is not. The meanings' own checks (the alignment of `movaps`, `cmpxchg16b` and
  the like, a branch's canonical target) give general protection whatever the segment. An alignment check
  comes before the access's own fault: a misaligned non-canonical `movaps xmm0, [rsp]` is general protection
  on the reference. A call's canonical check comes after its push (section 5.3). The meanings add nothing
  for this (no cost on any access): the permission table already stops every such access, since the last
  page below 2^47 is never mapped: Linux's x86-64 user space ends at 0x7ffffffff000 (`TASK_SIZE_MAX`), and
  the x86 personality gives our kernel that same limit (section 6), so no mapping reaches the last page and
  an access reaching 2^47 always faults. The registers are exact at the fault, so
  `X86_64::memory_fault(address, code, block)` works out the instruction's accesses (`accesses` of the
  instruction at `code`) and the range each really touches, from the block's registers and segment bases:
  for `Fixed` bytes the address and size; for a `BitOffset` index the operand the bit offset selects; for
  `Counted` bytes the current element (rsi or rdi); for `Masked` bytes the span from the first to the last
  element the mask selects, which for a gather is the first element whose mask bit is still set (the
  elements before it are done and their bits clear at the fault, next item). The two accesses of one
  instruction may differ in size (`call far [m]` reads 10 bytes and pushes 16), so it takes the access that
  faulted: the first, in the instruction's order (the order the CPU and the meaning check them), whose range
  holds the reported address. When that range is not wholly canonical (its first or last byte is not canonical, bits
  63 to 47 not all equal; an access in the kernel's half, 0xffff800000000000 and up, is canonical and gives
  a page fault at its address on the reference, as does one that wraps past 2^64, seen at X3), `Stack` if the access goes through the stack segment by the rule
  above, else `GeneralProtection`; otherwise a page fault at the address. No access is longer than a page
  (the largest offered, `xsave` with x87, SSE and AVX state, is 832 bytes in the manual's layout), so no
  access reaching 2^47 starts below the last page. Layer 1 places a share of its memory operands' bases (rsp
  and rbp among them), of its stack pointers, and of the other base registers, in the last 64 bytes below
  2^47 and at non-canonical values, so that all three answers are compared with the reference.
- **Addresses through a segment** (`fs:`, `gs:`): `MemoryAccess::segment` names the segment base added,
  whatever base and index the operand has (`fs:[rdx + rax * 4]`, a thread-local array).
- **Alignment**: x86 allows unaligned access except for a few instructions (`movaps`, `movdqa` and the
  other aligned SSE forms, `cmpxchg16b`, `fxsave`): those meanings test the address and stop with general
  protection, as the reference does (section 2).
- **lock**: one guest thread runs at a time (section 8), so a locked instruction is its plain form.

### 5.5 Segment bases and the thread pointer

FS and GS bases are thread state: two register bits (i64, in the block), so a function that reads
`fs:[0x28]` (the stack protector) many times keeps the base in a local. They are written by `wrfsbase` and
`wrgsbase` (FSGSBASE, offered: Linux enables it and says so in AT_HWCAP2) and by the kernel's `arch_prctl`
through the block at a system call. `rdfsbase` and `rdgsbase` read them. Loading FS or GS also writes its
base, as on the reference (Intel): `mov fs, r16` and `pop fs` with selector 0 or the user data selector
0x2b set the FS base to 0 (check 10: `8ee0` with fs base 0x40000000 left it 0), and the same for GS. Their
meanings write the base's register bit, so the check analysis, which keys `fs:` accesses on the FS base
and treats any instruction that writes the bit as changing it (its effects say so), sees them as
writers like `wrfsbase`. The selectors of ds, es, fs and gs are thread state too (`X86Block::selectors`,
the control `Selectors`); cs and ss always read as Linux's 64-bit user segments, 0x33 and 0x2b. A load
(`mov`, `pop`, `lfs`, `lgs`, `lss`) takes a null selector or a readable segment of Linux's (0x20 to 0x23, 0x28
to 0x2b, 0x30 to 0x33, 0x78 to 0x7b) for ds, es, fs and gs, 0x2b alone for ss, and is general protection
otherwise; `mov` to cs is illegal. All of this was decided by layer 1 against the reference (X5).

### 5.6 String instructions

- **One element** (no rep prefix): an ordinary instruction with its accesses.
- **rep movs and rep stos, bulk path**: when DF and TF are clear (one test of `Control::Rflags` against
  both bits, from the load the meaning makes for DF anyway; with TF set each pass moves one element and
  then stops, as on the reference, section 5.1) and the ranges do not overlap in a way that changes
  the result (a forward copy onto a destination that starts inside the source), the meaning checks the
  whole source range for reading and the whole destination for writing with `check_range`, and then does
  WebAssembly's `memory.copy` (for movs) or `memory.fill` (for stos with one repeated byte, which includes
  zero; other values store 8 or 16 bytes at a time in a loop over the checked range). At most 1 MiB moves
  per pass; rcx, rsi and rdi are then updated as the CPU would after that many elements, and the
  instruction jumps to itself, which is a backward jump, so the stop check runs between passes.
- **Otherwise, one element per pass**: the meaning moves one element, updates rcx, rsi and rdi with a
  `select` (unchanged when rcx is 0), and jumps to itself while rcx is not 0. Faults and stops are then
  exact to the element. This is the slow twin, counted "string instruction one element at a time" with the
  reason (direction flag set, trap flag set, overlap, range not all allowed).
- **repe and repne cmps and scas** stop on data, so they are always one element per pass.
- **Stops between any two elements**: every pass of either path ends on an element boundary with exact rcx,
  rsi and rdi and goes through the one stop check, and a bulk pass's element count is the one value a
  stop limit would bound (the brief: the stop flag becomes a limit on a progress count), so a stop can land
  after any element and elements can be counted without a second path.
- What the reference does (section 2): the trap flag stops it after each element, so layer 1 lets the
  reference step on while rip stays on a rep string instruction, up to the case's rcx (kept small by the
  generator), and compares the end state (`Machine::unfinished`, section 7.2, which steps on only for rep
  string instructions and gathers, never for a branch whose target is CODE).

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
  only write-protected for code and takes the step (below): it drops every piece translated from the page
  and sets the write bits back. Counter and warning: "code written without announcement", with the page.
- **Making progress: the faulting instruction runs once unprotected.** The thread goes on at the faulting
  instruction, which writes again. It must not be translated the ordinary way, since translating its bytes
  protects its page anew: when the instruction sits on the page it writes (a loop that writes the immediate
  of its own next instruction, section 2, run 8), the retried write would fault again forever. So it runs
  once by phase one's exact path, which already does what is needed: the unwind payload
  `UNWIND_GO_TO_EXACT` makes the bottom loop call `load_code(pc, exact = 1)`, which puts the piece of the one
  instruction at pc, at look-ahead none, in `EXACT_SLOT` (never entered in the lookup, replaced by the next
  such piece), runs it, and carries on from the lookup after it (contract.rs, `EXACT_SLOT`,
  `UNWIND_GO_TO_EXACT`, `HOOK_LOAD_CODE`; thread.rs's bottom loop). Two additions, made at X6, and no second
  path:
  - How it is asked for: the hook `HOOK_MEMORY_FAULT` (not `Guest::memory_fault`, which classifies) returns
    the unwind payload, `(address: i64, access: i32) -> i32`, and its call sites unwind with what it returns
    instead of the constant `UNWIND_LEAVE` (code.rs's `check` and full.rs's checks: `call F_MEMORY_FAULT; call
    F_UNWIND`). It returns `UNWIND_LEAVE` for every fault, as today, except after the step, when it returns
    `UNWIND_GO_TO_EXACT`. ARM64's runners always return `UNWIND_LEAVE`, so ARM64 behaves as today; its output
    loses one `i32.const` per fault site, and its whole suite runs again at X6. A kernel write, which has no
    faulting instruction, takes the step in the kernel and needs neither.
  - Where the exact mark lives: in the CPU side (the runner, at X10 the `Cpu` implementation), per thread, not
    in the thread's state: "translate A without protecting", A the faulting instruction's address, set by the
    memory fault hook that took the step (the page marks of "Who keeps the marks" below are another thing).
    That thread's next `load_code` reads and clears it; when that call is `load_code(A, 1)` (the next hook
    call of the thread but for `warn`'s "left to address", which the bottom loop raises first) the piece in
    `EXACT_SLOT` is translated without protecting its page. Any other hook call that stops the thread clears
    it too. Like every piece at none, the exact piece passes control to the next address through the lookup,
    so the next instruction is translated afresh from the bytes as written, which protects the page again. The
    instruction itself runs as it was read before its write, as the CPU runs an instruction it has already
    fetched (expected from the manual, checked at X6 by a program that writes into its own instruction); every
    later instruction sees the new bytes, as on the reference. Each such write costs one fault, one step and
    two translations, counted with the step (and with `warn`'s "left to address", which every unwind to an
    address raises). It always makes progress: the next `load_code` or stop of the thread clears the exact
    mark, so it applies to that one entry only; and when the exact piece's write faults on another page
    protected for code (an access across two pages of code, a `rep movs` pass), that step clears that page's
    mark and sets the exact mark again, so each repeat removes one page's mark and the repeats end. A rep
    string instruction that writes its own page runs one pass in the exact piece and then jumps to itself
    through the lookup, so each fault moves it on by at least one element and it ends with rcx. With threads
    in parallel (section 8) the other threads stay stopped from the step until the exact piece has run, so
    none protects the page again in between.
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
- **MXCSR**, following the ARM64 pattern, where FPCR is the mode, FPSR the sticky flags, and a write of
  FPSR turns on `FloatTracking`. MXCSR holds both: its mode is bits 6 to 15 (DAZ, the six exception
  masks, the rounding control, FTZ), its sticky exception flags bits 0 to 5. It is one control, `Mxcsr`,
  beside a control `MxcsrTracking`. Floating point takes the exact path through the float hook, counted
  "exact floating point requested", when the mode is not the default's (0x1F80: round to nearest, no
  flush to zero, no denormals-are-zero, all exceptions masked) or tracking is on; otherwise the fast path
  runs and the sticky flags are not kept. What changes them:
  - `ldmxcsr` and `vldmxcsr`, the program's own write of MXCSR (what `feclearexcept`, `fesetenv` and
    `fesetround` do), write the control and turn tracking on, as ARM64's write of FPSR does.
  - `fxrstor`, `xrstor` and the kernel's `rt_sigreturn` restore a saved state: they write the control and
    never turn tracking on, so only the mode they restore decides the path. This matters because glibc's
    dynamic loader runs `fxsave` and `fxrstor` (or `xsavec` and `xrstor` with XSAVE claimed) in its lazy
    binding trampoline at every lazy binding of a program not linked with `BIND_NOW` (section 2, run 7;
    glibc chooses the `fxsave` one when XSAVE is not claimed): restoring 0x1F80 there must leave the
    program on the fast path.
  - Nothing turns tracking off within a process; `execve` starts it off again with MXCSR 0x1F80.
  An unmasked exception stops with `FloatException` (SIGFPE with the `si_code` of the exception).
- **Reading the sticky flags while they are not kept** is a counted slow path, x86's form of ARM64's
  `ReadStatus` (an FPSR read while `FloatTracking` is 0 calls the float hook, which only counts): `stmxcsr`
  and `vstmxcsr`, and `fxsave`, `xsave`, `xsavec` and `xsaveopt` when they store MXCSR, test
  `MxcsrTracking` and, when it is off, call the float hook with `ReadStatus`, counted "MXCSR's exception
  flags read while not kept", with its warning. Then they store the control as it is: the mode exactly,
  the flags as last written, which may be fewer than the CPU's. A save and its restore therefore round-trip
  exactly. This counter fires at every lazy binding in a program without `BIND_NOW`, from one address in
  the loader, so it warns once there; that is intended, not noise: the loader really did read flags that
  may differ from the CPU's, and the warning says only that. A signal frame's MXCSR is written by the
  kernel's personality, which, as ARM64's kernel does with FPSR today, writes the control without counting
  (section 12 lists this as a known gap). The x87 status word is always kept (below), so x87 has no such
  path.
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
- **Why not Berkeley SoftFloat.** SoftFloat 3 (BSD-3) does both jobs directly: `extF80` with
  `extF80_roundingPrecision` 32, 64 or 80, and a correctly rounded `extF80_sqrt`. It is C, and no crate
  that wraps it builds for this project's targets (checked in run 9, scratch `run9/sf` and `run9/sfprobe`):
  softfloat-sys 0.1.4 (BSD-3-Clause; softfloat-wrapper 0.3.4, MIT OR Apache-2.0, is built on it) and
  softfloat-rs 0.1.0 (BSD-3-Clause) choose their build rules with `cfg!` inside the build script, which is
  the machine building, not the target, and have rules only for an x86-64 one: for wasm32-unknown-unknown
  on the Mac the build stops with "build rules are not implemented for the current target_arch and
  target_os"; before that, softfloat-sys's dependency `c99` 0.1.1 runs CMake, which fails with the CMake
  of the Nix shell ("Compatibility with CMake < 3.5 has been removed"). The pure-Rust soft-float crates
  do not do it: softfloat 1.0.0 has only 32 and 64 bits, era2m_softfloat 0.2.1 no 80-bit format, and
  machina-softfloat 0.1.2 (MIT) has an 80-bit type with a square root but no precision control, and names
  no repository, so where its code comes from cannot be checked. Using SoftFloat itself would mean
  vendoring its C sources and building them with a build script of our own for wasm32 (zig's clang
  compiles `extF80_sqrt.c` for `wasm32-freestanding`, seen) in every build of the x86 crate, the browser's
  cdylib included, with its rounding mode, precision and flags in C globals. The project is Rust, and
  both crates the plan uses are already dependencies of it (rustc_apfloat in the ARM64 crate, astro-float
  for the transcendental instructions below), so the plan keeps them and writes only the glue: two
  `Semantics` definitions and the rounding to odd. Both are checked against the reference in layer 1, as
  SoftFloat would have to be. The owner is told (status file).
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
  register valid; `emms` marks them empty. An MMX instruction that writes mmN also sets the sign and
  exponent of RN (bits 64 to 79) to 0xFFFF, as the reference does (section 2, run 6); one that only
  reads leaves them. Those fields are eight i32 controls, `X87High(0)` to `X87High(7)`, each over bytes 8
  to 11 of its register's 16-byte slot (the sign and exponent, then two bytes that are always zero, the
  FXSAVE layout's reserved bytes), so the meaning writes 0xFFFF with one `set_control`, a single store
  into the block, and the x87 hook, the kernel's signal frame and layer 1 read the full 80 bits. The x87
  hook declares the MMX bits in its `registers`, so the emitter writes them back before an x87
  instruction and loads them after.

### 5.9 The register block

`X86Block`, defined in the `no_std` core of `wcontainers-x86`, where the kernel's personality finds it: 16
general registers in hardware order, rip, the flags word (the six arithmetic flags, section 5.1), the `Rflags`
control (TF, DF, NT and ID at their rflags positions), FS base, GS base, a 512-byte area in the FXSAVE layout (x87
control, status and tag words, last-instruction pointers, MXCSR and its mask, eight 16-byte x87 register
slots, sixteen xmm registers) except that the x87 slots are in physical order R0 to R7, then the upper halves
of the sixteen ymm registers. The stop flag follows, padded to a multiple of 16. The kernel rotates the x87
slots by TOP when it fills an FXSAVE or XSAVE image (a signal frame, `ptrace`, a core dump).

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
  BMI1, ADX, MOVBE, RDTSCP, FSGSBASE, CLFLUSH, SEP (whose `sysenter` gives general protection as on a
  kernel without 32-bit emulation, section 5.2), and the 64-bit basics (SYSCALL, NX, LM). This covers the
  x86-64-v2 level.
- **The claim lists in iced's names** (the source file of section 2, which names `iced_x86::CpuidFeature`
  values; run 10 shows the names). The default list: `INTEL8086`, `INTEL186`, `INTEL286`, `INTEL386`,
  `INTEL486`, `X64`, `CPUID`, `PAUSE`, `MULTIBYTENOP` (the instructions every x86-64 CPU has, which iced
  files by the CPU that brought them), `FPU`, `FPU287`, `FPU387` (all of x87, under the reading below),
  `TSC`, `CX8`, `CMOV`, `MMX`, `FXSR`, `SSE`, `SSE2`, `SSE3`, `SSSE3`, `SSE4_1`, `SSE4_2`, `POPCNT`,
  `CMPXCHG16B`, `LZCNT`, `BMI1`, `ADX`, `MOVBE`, `RDTSCP`, `FSGSBASE`, `CLFSH`, `SEP` and `SYSCALL`. NX,
  LM and LAHF-SAHF in 64-bit mode are CPUID bits with no iced feature of their own (`lahf` and `sahf` are
  `INTEL8086`). The AVX setting's list: `AVX`, `AVX2`, `FMA`, `F16C`, `BMI2` and `XSAVE` (`xgetbv`,
  `xsave`, `xrstor`, and `xsetbv`, which is general protection in user mode when XSAVE is presented; its
  meaning is X5's with the setting's). Every other name is never claimed; section 2 says which kernel-only
  `Code`s are "kernel only". The 15 baseline Codes of
  `INTEL286` include `sldt`, `str`, `smsw`, `lar`, `lsl`, `verr` and `verw`, which run in user mode on the
  reference (it has no UMIP); their meanings give what layer 1 finds there, except where the answer depends
  on which processor ran it. `sgdt` and `sidt` (baseline, not privileged) store kernel addresses there, and
  the GDT's base differs per processor (check 10: three CPUs, three bases); `lsl` of the per-CPU segment
  0x7b returns the processor's number. Their meanings give fixed designed answers: `sgdt` and `sidt` the
  dummy values Linux's UMIP emulation gives (limit 0 and fixed bases, taken from Linux's
  `arch/x86/kernel/umip.c` at X5, expected and not yet seen), and `lsl` on 0x7b processor 0, node 0 (a
  guest thread does not know where the host runs it). Layer 1 excludes these three from comparison with
  that reason and the designed-answers test checks them. `clflush` is a checked one-byte read of its address, which faults as a load there does
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
  owner is told (status file). The same reading covers the x87 instructions iced files under other
  features (section 2, run 10): SSE3 holds the three `fisttp` and CMOV holds `fcmovcc`, `fcomi`, `fcomip`,
  `fucomi` and `fucomip`. They run through the x87 hook like every x87 instruction, counted with it, and
  SSE3 and CMOV are claimed: every other instruction of both maps onto WebAssembly directly, no CPU has
  SSE3 or CMOV without these x87 forms, and a program that tests either bit wants the SSE3 or `cmov`
  instructions, not the x87 ones (which a compiler emits only for `long double`). The other baseline
  instructions that cost more than one WebAssembly
  operation (64-bit `div` and `idiv` of a 128-bit dividend, section 5.2; `rcpps` and `rsqrtps`, section
  5.8) are done entirely in WebAssembly arithmetic and are not slow paths.
- **Not claimed by default**: AVX, AVX2, FMA, F16C, BMI2 (its `pdep` and `pext` have no WebAssembly form),
  XSAVE and OSXSAVE (needed only to announce AVX state). **Never claimed**: AVX-512 and its relatives, AES,
  PCLMULQDQ, SHA, RDRAND, RDSEED, TSX, MPX, SGX, PKU, CET, AMX, XOP and 3DNow. A feature named in neither
  the default claims nor the AVX setting is not claimed either (among those the reference has:
  CLFLUSHOPT, XSAVEOPT, XSAVEC, PREFETCHW). The bytes of an instruction of a feature not claimed do what
  they do on a CPU without it (section 5.2): illegal instruction for nearly all, a NOP for those in the
  reserved-NOP space (CET's `endbr` and `rdssp`, `cldemote`, MPX, `prefetchw` and the rest), and prefixes
  such a CPU ignores (`xacquire`, `xrelease`, `bnd`, `notrack`) are ignored. The multi-byte NOP (iced's
  `MULTIBYTENOP`, which the `Reservednop_*` Codes carry) is part of the default claims.
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
`&mut dyn Cpu`). The personality is the interface's `no_std` core trait `Personality` (section 3.1), which
X1 builds with the register block and its offsets and which X10 fills in with the rest below; each CPU crate
implements it in its own `no_std` core, so the kernel depends on `wcontainers-guest-cpu` and on the one CPU
crate it is built for, both with the `translate` feature off. It contains:
- **The register block** type and its conversions (the abi's `Cpu` trait becomes generic over it). ARM64's
  `RegisterBlock` stays where it is (it may move into ARM64's core); `X86Block` is in x86's core. The
  user-visible rflags (in `sigcontext`, for
  `ptrace`) is put together from the flags word, the `Rflags` control (TF, DF, NT, ID) and bits 1 and 9,
  with RF (bit 16) added in the frame of a signal for a fault (page fault, general protection, stack
  fault, divide error, illegal instruction, float exception) and not for a trap (single step, `int3`,
  `int1`) or a signal sent by another (section 2, run 10). It is split back at sigreturn as Linux does:
  only the `FIX_EFLAGS` bits come from the frame (the six flags, TF, DF, RF and AC). TF goes into the
  `Rflags` control, and the return into the thread asks `Guest::step` with `StepAt::Kernel`, so a frame
  with TF set runs one instruction and then stops (section 5.1; seen in run 10). AC is left clear (section
  5.1). RF is not kept: it only holds off an instruction breakpoint for one instruction, no instruction
  breakpoint is offered, `pushf` never stores it and the CPU clears it after the next instruction, so no
  program can see it. NT and ID keep the values the thread had when it called `rt_sigreturn` (section 2,
  run 7). The trap flag a frame sets is counted as "trap flag set by the program". The frame's MXCSR is the `Mxcsr` control, and sigreturn
  writes it back as `fxrstor` does, without turning tracking on (section 5.8).
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
- **The copy routines' partial faults**: each CPU gets its own measured model, since a program can see
  what a failed copy left (the bytes before the fault, and `read`'s count). ARM64 keeps today's model in
  user.rs exactly (`stored_before_fault`, `clear_partial`, the measured kernel-buffer offsets). The x86-64
  model is simpler and was measured on the reference (Linux 7.3.4 on the i7-8750H, which has ERMS and not
  FSRM; section 2, run 8): `copy_to_user` and `clear_user` write every byte up to the first byte not
  allowed, whatever the start's alignment and the length. Seen for a pipe's `read` (`copy_to_user`; the
  call returns `EFAULT`, as `pipe_read` does for a short copy), `/dev/zero`'s `read` (`clear_user`;
  returns the bytes cleared) and `uname` (a struct copied out; `EFAULT`), each with the buffer ending
  1, 3, 7, 8, 9, 13, 15, 16, 17, 31, 33, 64, 100 and 389 bytes before a page it may not write: every byte
  before that page written, none after. So the personality's copy model is a function the kernel calls
  where user.rs calls `stored_before_fault` and `clear_partial` today: x86's answers "all bytes before
  the first one not allowed". What each call returns stays the kernel's own, shared code. The kernel
  test of X10 repeats these three cases against the reference.
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
  handler's entry. DF (with TF and RF) is cleared on entry and NT and ID are kept, as Linux's
  `handle_signal` does (from its source; TF's clearing seen in run 10, the rest checked against the
  reference at X10).
- **Clone, exec and the auxiliary vector**: x86 `clone`'s argument order (flags, stack, parent tid, child
  tid, tls); the child's registers (rax 0, the new stack, FS base from tls); exec's start registers (rsp at
  argc, rdx 0); AT_HWCAP (CPUID leaf 1 edx), AT_HWCAP2 (FSGSBASE), AT_PLATFORM "x86_64", AT_MINSIGSTKSZ.
- **`arch_prctl`** (set and get FS and GS; CPUID faulting and AMX permission reported as not supported),
  and the faults and their signals (section 3.2's trap kinds: divide error SIGFPE `FPE_INTDIV`, general
  protection SIGSEGV `SI_KERNEL` with address 0, float exceptions SIGFPE, single step SIGTRAP
  `TRAP_TRACE`, `int3` and `int 3` SIGTRAP `SI_KERNEL`, `int1` SIGTRAP `TRAP_BRKPT`, each breakpoint
  with rip after it; general protection also for `int 0x80` and `sysenter`; `int 4` is `TrapKind::Overflow`,
  SIGSEGV `SI_KERNEL` like general protection but a trap, with trap number 4, rip after it and no RF,
  section 5.2; where a trap leaves pc is already in the block's pc when the trap hook is called), and the stack
  fault from a memory fault, SIGBUS `SI_KERNEL` with address 0 (Linux's
  #SS handler; section 2, run 6). The abi's `Exit` grows these kinds. A memory fault reaches the kernel
  already classified by the runner through `Guest::memory_fault` (a page fault with its address, general
  protection, or a stack fault), so the kernel, written against `Personality` alone, never decodes an
  instruction; hooks never reach it.
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
    /// `Ok` when an instance's bytes, decoded as the translator decodes them, are offered with `offer`;
    /// `Err(feature)` when they are of a feature not offered (x86: by the decoded `Code`, so `endbr64`,
    /// decoded as a `Reservednop_*` Code, is offered, section 5.2). Both run on the reference and are
    /// compared with it; an `Err` case whose reference run does not stop with illegal instruction passes
    /// only when `feature` is in `REFERENCE_HAS` and the translated side stops with illegal instruction.
    fn presented(code: &[u8], offer: Self::Offer) -> Result<(), &'static str>;
    /// The features this CPU does not offer by default that the reference CPU has, so their instructions
    /// run there: x86 AES, PCLMULQDQ, RDRAND, RDSEED, CLFLUSHOPT, XSAVEOPT, XSAVEC, and with the AVX setting
    /// off its group (AVX, AVX2, FMA, F16C, BMI2, XSAVE), from the i7-8750H's `/proc/cpuinfo` (section 2,
    /// run 9). A run fails when an entry's cases all stop with illegal instruction on the reference, so
    /// the list never holds a feature the reference lacks. ARM64: empty, and its `presented` is always
    /// `Ok`: today's words of forms not presented and of features not offered (`of_feature_not_offered`)
    /// stay among its `excluded` reasons, so it draws and compares exactly today's cases (the M5 has
    /// those features, so they could only be checked as designed answers, which today's tests do).
    const REFERENCE_HAS: &'static [&'static str];
    /// A reason to leave an instance out (a jump to itself, a designed answer, ARM64's unpredictable
    /// encodings and its words not presented or of a feature not offered, x86 `int 0x80`, `sysenter` and
    /// selector 0x23), else `None`.
    fn excluded(code: &[u8]) -> Option<&'static str>;
    /// Random bytes that are no instruction of any form (x86: what iced decodes as `INVALID`, among them
    /// prefix runs that take an encoding past 15 bytes, which give general protection, section 5.2).
    fn no_form() -> BoxedStrategy<Encoded>;
    /// The features offered with `offer`, by name, and those the guest is told of (ARM64 the HWCAP bits
    /// decoded; x86 the CPUID answers decoded): a test checks they are the same set.
    fn offered(offer: Self::Offer) -> Vec<&'static str>;
    fn told(offer: Self::Offer) -> Vec<&'static str>;
    /// The designed answers (identity registers, CPUID leaves, rdtsc, xgetbv, AC reading back clear,
    /// `int 0x80` and `sysenter`, and x86's MXCSR rules: `ldmxcsr` turns tracking on, `fxrstor` of
    /// 0x1F80 keeps the fast path, `stmxcsr` and `fxsave` count while not tracking, section 5.8): each
    /// instructions, a starting state and the state, stop and counters they must leave.
    fn designed(offer: Self::Offer) -> Vec<Designed>;

    // --- the comparable state ---
    /// The fields compared, in wire order (x86: 16 general registers, rip, rflags, FS and GS bases, MXCSR,
    /// FCW, FSW, FTW, R0 to R7, xmm0 to xmm15, the ymm upper halves).
    const FIELDS: &'static [Field];
    /// The thread state (`BLOCK_SIZE + PRIVATE_BYTES` bytes) as field values in `FIELDS` order, and back.
    /// x86 puts rflags together from the flags word, `Rflags` (TF, DF, NT, ID) and bits 1 and 9 (section
    /// 5.1). Tracking is not a field: the case's `tracking`, drawn at random, is written beside the
    /// fields (ARM64 `FloatTracking` as today, x86 `MxcsrTracking`).
    fn to_state(thread: &[u8]) -> State;
    fn from_state(state: &State, thread: &mut [u8]);
    /// Per field, the bits allowed to differ in this case given both outcomes: the sticky float flags
    /// while not kept, which must instead equal the case's starting ones (ARM64 FPSR's, today's rule;
    /// x86 MXCSR bits 0 to 5 when the case does not track, starts with MXCSR's mode at the default and
    /// writes no MXCSR, section 5.8; x87's status is always kept and always compared), TF in the
    /// copies of rflags an instruction makes
    /// (`pushf`'s word, r11 after `syscall`; TF in rflags itself is compared, as the runner reports the TF
    /// the instruction left, section 7.3), AC after `popf` (section 5.1), what a faulting access leaves
    /// unknown (ARM64), x87's last-instruction pointers. Never an undefined flag.
    fn may_differ(case: &Case, reference: &Outcome, translated: &Outcome) -> State;
    /// A reference signal as a generic stop, given the instruction's length: section 7.3's list (x86's
    /// held-back trap, general protection as `SIGSEGV` `SI_KERNEL` with address 0, the stack fault as
    /// `SIGBUS` `SI_KERNEL` with address 0, a breakpoint as `Breakpoint(si_code)`); ARM64 today's
    /// `reference_outcome`, with a `SIGTRAP` as
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
    /// The C bodies and `flat.ld` stay shared. (As built: `programs/<NAME>`, `switch.h`, no `start.S`.)
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
/// `StackFault` is x86's #SS (`MemoryFault::Stack` on the translated side). `Alignment` holds `si_addr`:
/// ARM64 the faulting address; x86 0, as Linux reports the alignment check (#AC, only with AC set, which
/// the translator never holds, section 5.1), seen on the i7-8750H.
pub enum Stop { Landed(u64), Illegal, Breakpoint(u32), SingleStep, Syscall, MemoryFault(u64),
    Alignment(u64), GeneralProtection, StackFault, DivideError, FloatException(u32), Other(u32, u64) }
pub struct Outcome { pub stop: Stop, pub state: State, pub changed: Vec<(u32, u8)> }
pub struct Designed { pub code: Encoded, pub start: State, pub end: State, pub stop: Stop }
pub struct StandIn { pub write: u64, pub writev: u64, pub exit: u64, pub exit_group: u64, pub brk: u64 }
pub struct Snippet { pub name: &'static str, pub code: Vec<u8>, pub expect: Expect }
```

As built at X2 (`src/guest/mod.rs`), where building it showed a signature must change:
- The names group is `REFERENCE: Target` (the runner's Rust target, Docker's platform, the SSH host's
  variable and the field names, which are `FIELDS`' names), with `C_TARGET`, `GENERAL_ONLY` and `NAME` (the
  suffix of the per-guest tests' names) beside it.
- `Form` is `Clone`, not `Copy` (ARM64's forms hold their names), and `forms()` gives only the forms layer 1
  draws from (ARM64's covered ones). `cases(form)` takes no offer: it uses the offer's default
  (`G::Offer::default()`, the interface's `Offer: Default`) until the AVX setting (X8) needs a second one.
- `no_form(seed)` gives whole cases (the bytes and a plain starting state), since the harness cannot make a
  state; `hand_cases()` stands for `designed(offer)` and also holds the hand-made cases that are compared
  with the reference (`Designed { name, case, stop, fields, compared }`).
- `may_differ` is `reconcile(case, &mut expected, &actual)`: the CPU makes the reference's outcome what the
  translated one must equal. A mask could not say "equal to the case's starting value" (ARM64's FPSR rule)
  or "the reference's stored bytes do not count" (a store that faults).
- `track(thread)` sets the tracking beside the fields; `PC_FIELD`, `SP_FIELD` and `GENERAL_FIELDS` name
  the fields the harness reads (the changes test's pointers and frames; its frames start `RED_ZONE` below
  the stack pointer, as the translator's frame checks do).
- Layer 1's no-form cases are compared with the reference and nothing more: what stop they give is the
  CPU's (ARM64 illegal instruction; x86 also general protection past 15 bytes).
- `breakpoint_code` takes the abi's 16-bit breakpoint detail.
- Layer 2's model is shared (`src/layer2.rs`) and its operations and ranges are those ARM64 encodes in one
  instruction; another CPU's `assemble` gives each its nearest form. In place of `MODEL_REGISTERS` the guest
  gives `MODEL: Model` (the registers the model draws from, in today's order; those a system call sets; the
  one every function keeps, the base of a group across a call), since the model draws all three. `assemble`
  takes the plain `Program`: `Extra` and `extra()` come with x86's layer 2, where what they hold is known
  (an associated type and a strategy ARM64 would only answer `None` to have no use before then). The output
  size is worked out from `MODEL` (`output::<G>()`).
- `excluded(code)` is used by layer 2 to show every instruction of a program is one the architecture defines.
- The corpora are not `Snippet` and `Expect` but one type each, holding what the test checks: `smoke() ->
  Smoke` (named programs with the field each test reads), `checks() -> Vec<Check>` (a blob, a counter and
  what it must read, whether it fails at a function's entry and so runs in Chrome), `changes() ->
  Vec<Change>` (a blob, the point the page is changed at as pc and a field's value, the page). The hand-made
  checks and changes each run in one test over the guest's list, every program on a thread of its own.
- `auxiliary(offer)` is the constant `AUXILIARY` (no second offer until X8), and the blob record carries
  the entries to the runner, so a flat blob is told the same on both sides.
- No `PROGRAMS` constant: the guest's program folder is `programs/<NAME>`. A program is looked for there
  first, then among the shared ones, and the folder is on the include path. It holds `syscall.h` (the
  system call numbers and `syscall3`), `switch.h` (not `switch.S`: the context switch as file-level
  assembly, with `start_context` and `CALL_ON_STACK`, so each program stays one source file), the assembly
  tests, `written_code.c` and `differences.c`. No `start.S`: the programs without a C library start from a
  `_start` written in C. The benchmark set's translations are the same bytes after the move.
- `PROBE_ANSWERS`: every answer the guest's probe can give (ARM64 0 to 3), so the byte for byte comparison
  translates for every host, and `differences` is run for each.
- Tests of one CPU's own types stay that CPU's (`decoding.rs`, `valid.rs`, the check that a blob is told the
  abi's HWCAP, the counters a host without fused multiply-add must show), each named with its suffix.
- `form_group(form)` takes the form by reference (ARM64: its group in Arm's list); layer 1's summary counts
  the failing forms under their groups. `offered()` and `told()` take no offer, as `cases` does (ARM64:
  `OFFERED_FEATURES`, and the abi's HWCAP bits by name), and one test written once checks they are the same
  set (it replaced ARM64's own check of `OFFERED_FEATURES` against HWCAP).
- Not built at X2, each with the first CPU that gives it something to do: `presented(code, offer)`,
  `REFERENCE_HAS` and layer 1's rule for an `Err` case come with x86's never-claimed list in layer 1 (X5):
  x86's layer 1 at X3 draws only forms whose meaning is written, so nothing reaches them before. ARM64 would answer `Ok`
  to every instance and give an empty list, so the rule would be code no case reaches; its words of features
  not offered stay among its `excluded` reasons, as above. `Stop::SingleStep` comes with X6, where the
  translated side first stops by the guest's own trap flag; until then no stop of either side maps to it
  (the reference's own trap-flag stop is `Landed`).

`Expect` is today's expectation of each corpus (the smoke output, the checks' counts, the changes' stops),
unchanged. A translator trap maps to a `Stop` and to an exit status generically, from `TrapKind` and the
signal section 6 gives it (128 + 8 for SIGFPE); a breakpoint's `si_code` comes from `breakpoint_code` and
its detail. A memory fault maps through `G::memory_fault`: `Page` to `MemoryFault(address)`,
`GeneralProtection` to `GeneralProtection` (128 + 11) and `Stack` to `StackFault` (128 + 7, SIGBUS).

The reference runner (crates/wcontainers-translator-test/reference) is one program built for
`G::RUST_TARGET`; its per-CPU part is a module chosen by `cfg(target_arch)` (`reference/src/aarch64.rs`,
`reference/src/x86_64.rs`) implementing:

```rust
/// The reference runner's per-CPU part; the wire format, the seccomp filter, the forking and the blobs
/// are shared.
trait Machine {
    /// The byte the code area is filled with: ARM64 0 (`udf #0`), x86 0xCC (`int3`).
    const FILLER: u8;
    /// How far the code area reaches either side of CODE: ARM64 128 MiB (today's `CODE_SPAN`, a branch's
    /// reach), x86 32 KiB (the steppers' area, section 2). On x86 it matters only for falling through
    /// and for rip-relative operands: a branch to any address beyond it, mapped or not, stops with the
    /// trap flag's `TRAP_TRACE` at the target before anything is fetched there (section 2, run 10),
    /// which is `Landed` on both sides.
    const CODE_SPAN: u64;
    /// The end of the instruction window from CODE, as seccomp reports a call's instruction pointer (the
    /// address after the call instruction): ARM64 8 (the instruction and the `svc` after it), x86 18 (the
    /// longest instruction and a `syscall` after it). The filter also stops every call whose arch is not
    /// `AUDIT_ARCH`, the guest's, made anywhere (x86: `AUDIT_ARCH_I386`).
    const WINDOW: u64;
    const AUDIT_ARCH: u32;
    /// The field names, sent first on every connection; the harness checks they are `G::FIELDS`' names.
    const FIELDS: &'static [&'static str];
    /// Write a case's state into the signal frame (x86: the general registers, rflags with TF set, the
    /// x87, SSE and AVX components and `xstate_bv` bits 0 to 2), and set the thread pointers (ARM64
    /// tpidr_el0, x86 `arch_prctl` FS and GS). x86 also sets the case's NT and ID in the handler's own
    /// rflags, since `rt_sigreturn` takes those from the caller, not the frame (section 2, run 7).
    /// It first keeps the runner's own frame (and x86 its XSAVE area and thread pointers). `Err` names
    /// what Linux refused to set up (x86: `arch_prctl` refuses a base at or above 2^47 - 4096), with the
    /// frame and thread pointers left the runner's; the case is then answered as refused.
    unsafe fn enter(context: *mut libc::ucontext_t, state: &[u128]) -> Result<(), &'static str>;
    /// Read the frame back into a state (a component whose `xstate_bv` bit is clear as its first state),
    /// after giving the runner its own thread pointer back, and put the runner's frame back. Given the
    /// instruction, the state it started from and the stop's `siginfo`: x86 reports TF as the instruction
    /// left it (section 7.3).
    unsafe fn leave(code: &[u8], start: &[u128], context: *mut libc::ucontext_t,
        signal: *const libc::siginfo_t, state: &mut [u128]);
    /// Whether the stop leaves the case's instruction `code` unfinished, so the runner steps on (at most
    /// `limit(code, state)` times). x86: `TRAP_TRACE` with rip still at CODE. Only the instructions the
    /// trap flag stops part way get steps from `limit` (section 2, run 8), so a `ret`, `call` or `jmp`
    /// whose target is CODE stops there finished and is never stepped on. ARM64 never.
    unsafe fn unfinished(code: &[u8], context: *const libc::ucontext_t,
        signal: *const libc::siginfo_t) -> bool;
    /// How many steps an unfinished instruction can take, worked out before the case runs (outside the
    /// handlers, where the runner's thread pointer is its own): x86, with `code` decoded by iced, the
    /// case's rcx (ecx with an address-size prefix) for a rep string instruction, 8 for a gather (a VSIB
    /// form), else 0; ARM64 0.
    fn limit(code: &[u8], state: &[u128]) -> u64;
    /// Make written code visible to the CPU: ARM64 `dc cvau` and `ic ivau`, x86 nothing.
    fn synchronise(address: u64, length: u64);
    /// A blob's starting registers (x86: rflags 0x202, MXCSR 0x1F80, FCW 0x037F, rsp at argc, every
    /// other register 0, no FS or GS base; ARM64 every register 0 but sp and pc, as before).
    unsafe fn blob_start(context: *mut libc::ucontext_t, entry: u64, stack: u64);
    /// A blob's stop record: pc and the general registers (ARM64 x0 to x30, sp, pc; x86 the 16 in field
    /// order, rip, rflags), which its stop handler stores into a page shared with the runner: the blob's
    /// child holds no descriptor but its standard input, output and error (output through pipes, since a
    /// write from a buffer that faults part way fails on a pipe and is cut short on a file).
    unsafe fn blob_stop(context: *const libc::ucontext_t, out: &mut [u64; 40]) -> usize;
}
```

Both sides of a comparison hold the same code area. `FILLER` and `CODE_SPAN` are defined once per guest in
`layout.rs`, which the runner includes as it does today (`include!("../../src/layout.rs")`), and both
`Machine` and the harness read them there. The runner maps `CODE - CODE_SPAN .. CODE + CODE_SPAN` readable
and executable, filled with `FILLER` (the parent fills it once; the children it forks share it). The
translated side gives the same range the same permissions (today's `regions()`) and, before loading the
case's instruction, writes `FILLER` over it. In wasmtime it does so for each case when `FILLER` is not 0:
fresh memory is already 0, so ARM64 writes nothing and runs as today, and x86 writes 64 KiB per case. In
Chrome the page does the fill itself, once, from the range and the byte given with the page's settings
(`fill`), when the byte is not 0, so nothing is sent per case (the code area is not writable, so no case
changes it). On both, the lookup covers the instruction's address alone (one slot) and the translator is given the
instruction's own bytes (in Chrome each case carries its length, then the instruction padded to
`MAX_LENGTH` with `FILLER`), so control that lands anywhere else, CODE + L after an x86 instruction of
length L among them, stops there as landed, as on the CPU. So an operand that reads the code area (an x86 rip-relative operand
that reaches past the instruction, a base that points there) reads `int3` bytes on both sides, and layer
1 draws such operands on purpose (section 7.4).

The x86 fields are the general registers in encoding order (rax, rcx, rdx, rbx, rsp, rbp, rsi, rdi, r8 to
r15), rip, rflags, the FS and GS bases, MXCSR, FCW, FSW, FTW as the abridged tag byte of `fxsave` (one bit a
physical register), R0 to R7 as physical registers of 80 bits (the runner rotates them by TOP into the
frame's stack order), xmm0 to xmm15 and the ymm upper halves: 64 fields. ARM64's are x0 to x30, sp, pc,
NZCV at its `pstate` bits, tpidr_el0, v0 to v31, FPSR and FPCR: 69 fields.

The wire format is length-prefixed records in both directions. The runner's first record names its fields
and the reference CPU's model (`/proc/cpuinfo`'s `model name`, or ARM64's implementer and part). A case is the instruction's length and
bytes, the seed of its data area, and the state as `FIELDS.len()` 16-byte little-endian values; its result
is a status of its own (ran; runner failed, with a message; or refused, with what the CPU's Linux would not
set up of the starting state, such as a thread pointer, which no program can start from: `CaseRan::Refused`,
never a stop), then for a case that ran the signal, `si_code`, `si_addr`, the
state after, and the changed data bytes as (offset, value) pairs. A blob is its bytes, its load address,
stack and program headers, the guest's auxiliary vector entries, its standard input and how many runs to time; its result is its output, its exit
status, the best time and, when a signal ended it, its stop record. Cases run in one child process of the
runner that serves them until a case ends it; that case is reported as the runner's failure and the next
case gets a new child (section 7.3's isolation, without a fork per batch). Each blob run is a child of its
own. ARM64's blobs go through the same records (one long-lived container per runner, shared by the run),
not a container per blob. A program is an ordinary static executable's bytes; the runner runs it as Linux
starts a program (from a memory file with `fexecve`, under a blob's name with no other arguments, no
environment, empty input, no signal blocked or ignored, the blob's time limit by `alarm`), in a child of its
own, and its result is a blob's with no stop record. Layer 3 uses it to tell a blob that Linux ends by a
signal from a broken blob (the program built as an ordinary executable must end the same way), so that
judgement too comes from the guest's real CPU through the transport, never from Docker's emulation.

The transport is generic: `Reference::start(&G::REFERENCE)`, where `REFERENCE` is a `Target` value (the
runner's Rust target, Docker's platform, the SSH host's variable, the runner's directory there and the field names; it stands for the
names group above, and lets X2 prove the x86 transport before X3 makes an x86 `Guest`), runs the runner as a child process and talks to it
over its stdin and stdout. When the host's CPU is the guest's (`std::env::consts::ARCH` against
`RUST_TARGET`), the child is `docker run -i --platform G::DOCKER_PLATFORM`, as today; otherwise it is
`ssh` with `ControlMaster` to the host in `G::REFERENCE_HOST`, after uploading the runner to the target's
own directory (`~/wcontainers-x86/reference/<SHA-256>` for x86-64) when no file with that content is there (`sha256sum` checks the kept
file, and each upload before it is renamed into place), tried five times ten seconds apart before
the run fails. A connection lost later (a request written or its result read when the child has ended) is
the transport's failure, not the guest's: the runner is started again the same way and the request sent
again, at most five times for one request (each request stands alone). Both runners are static executables
that are not position independent (Rust's musl target for x86-64 builds a static PIE by default, which
Linux loads at a random high address, so x86's is built with `-C relocation-model=static`): x86's is linked
at 1.5 GiB (its static start code calls undefined weak functions at 0, which must be within 2 GiB) and
ARM64's at 2 GiB as before, with the heap Linux places up to 1 GiB after each; all above every guest
address of the tests (below 0x5800_0000). Seen on the i7-8750H: image at 0x6000_0000, heap at 0x8417_2000.
The runner's own memory is still reachable by a case's registers (a 32-bit address, a bit offset), and the
CPU would then read or write the runner where a clean process faults, so each runner's place is one range in
`layout.rs` (`RUNNER`: the image base and 1.25 GiB after it), which the runner checks at start, and x86's
case generator drops every case whose accesses (iced's `used_memory` from the case's registers, with a bit
offset's move) reach it (found by the independent check of X3).
Both are the same records over the same pipes, so nothing else differs. `start` fails with a message naming the variable when it is unset; the
test then fails, never skips. `cases(&[Case]) -> Vec<(Signal, State, Vec<(u32, u8)>)>`, `blob(..)` and
the model (read once from `/proc/cpuinfo` on the reference, from the runner's first record) are what it
gives. As built at X2 it serves one case a request (`case`) and records no results; the model is to be
recorded with every result when recorded results are built (proposed for X3, section 7.3).

The browser's cdylib is built per guest (a cargo feature naming the guest, `arm64` or `x86_64`, chooses
`type G`). Its `contract()` JSON gives what `runner.js` needs, beside today's translator numbers, all from
`Guest` and the contract: `threadBlockSize`, `threadStateSize`, `pcOffset`, `flagsWordOffset`, `stopFlag`,
`align` (one 4-byte slot per `G::ALIGN` bytes, `contract::slot_entry`), `maxLength`, `trapKinds` (the name
of each `TrapKind`), `reasons`, `guestHooks` (each of `HOOKS` with its body, its line for `codeChanged`, its
state words and its results), `hookParams` and the probe export. The page reads nothing else of the block:
the system call's number and arguments come to it through the syscall hook's own parameters, and a trap's
kind through the trap hook's, so the block offsets of the call and a table of trap numbers and signals are
not part of it. ARM64's `registerFpsr`, `registerFpcr` and `floatTracking` go: the float hook's work is
`cpu_hook` (section 3.4). The stand-in's five numbers come from `GuestTest::STAND_IN` through the page's
settings, which the test crate's browser.rs writes.

Each test file has one test per guest (`layer1_arm64`, `layer1_x86_64`, ...), so `cargo test` runs both.
An x86 test that needs the reference machine and finds it not configured fails with a message that says
so; it is never skipped silently.

### 7.3 The x86 reference runner and how it is reached

- **The runner** is a static Rust program built on the Mac for `x86_64-unknown-linux-musl` with rust-lld
  (the toolchain file gains the target), uploaded to `~/wcontainers-x86/reference/<SHA-256>` when no file
  with that content is there. It needs nothing installed on the machine beyond coreutils' `sha256sum`.
- **Transport**: one SSH connection kept open for a whole run (`ControlMaster`), the host named by
  `WC_REFERENCE_X86_64` (`tom@192.168.2.240` today). Cases and blobs go through stdin as length-prefixed
  binary records; results come back on stdout the same way, each with its own status, so ssh's own exit
  status (255) and signals are never mistaken for a guest's. The statuses are: ran, could not run (the
  case's child died; the runner goes on), refused (Linux refused the starting state) and the runner itself
  failed (a panic of the serving process, sent as the request's result before it ends with status 101), which
  fails the run with the runner's message. A stream that ends with no result is a lost connection: the
  runner is started again and the request sent again, up to five times. A local reference runs in Alpine
  3.24.2, pinned by digest, as layer 3's gcc image does. A new connection costs from about 0.24 s to
  over 1 s with the machines loaded, and a new process on the machine about 1 ms (section 2), so
  connections are never made per case and a runner
  process runs many cases.
- **Layer 1, batched** (not built at X2, proposed for X3 with x86's layer 1, the first run whose reference
  is remote and has many cases a form; X2 sends one case a request): cases are a deterministic function of seed and form, so the harness sends a form's
  cases in one batch, collects the results and compares; only a failure goes back to proptest's shrinking,
  which then asks one case at a time. Up to 10 runner processes at once on the x86 machine.
- **Isolation**: as built at X2, cases run in one long-lived child of the runner; a case that damages the
  child (a write through a random pointer into the runner's own memory) shows as a child that dies, that
  case is reported as the runner's failure, and the next case gets a new child. With batching (X3), a batch
  whose child dies is rerun one case per child.
- **One instruction** (section 2): the code area is filled with `int3`; the instruction is placed at CODE; the
  handler (on an alternate stack, using no thread-local storage and only raw system calls) writes every
  register into the frame including all vector registers, the whole x87 component (control, status and
  abridged tag words, the eight slots in stack order, rotated from the case's physical R0 to R7 by TOP, the
  last-instruction pointers zero) and the XSAVE header bits 0, 1 and 2 (section 2, run 4), sets the FS and GS
  bases with `arch_prctl` (first, before anything else changes: a base Linux refuses, at or above 2^47 -
  4096, makes the case refused, answered so with the runner's own state untouched), sets TF in the frame,
  sets the case's NT and ID in its own rflags with `pushfq; or;
  popfq` (Linux's `rt_sigreturn` keeps those bits from its caller and takes only the `FIX_EFLAGS` bits from
  the frame; section 2, run 7) and returns. The handler that reads the case back first clears NT, ID and AC
  in its own rflags the same way before the runner goes on (a handler starts with the case's AC, and its own
  unaligned accesses then fault: seen at X2 as a `popf` setting AC that ended the runner's child by
  `SIGBUS`). TF in the frame is the runner's own, so the reported rflags hold the frame's TF only when the
  stop is `TRAP_TRACE` and the instruction loads rflags (`popf`, `iret`, by iced's decode: no other user
  instruction writes TF), and otherwise the case's starting TF, which is what the instruction left; a
  `popf` that faults leaves TF as it was. The next signal is read back, knowing the instruction's
  length L:
  - `SIGTRAP` with `TRAP_TRACE` (2): landed at rip.
  - `SIGTRAP` with `SI_KERNEL` (128) and rip CODE + L + 1: the instruction ran and fell through with its
    trap held back (`mov ss`), and the `int3` filler at CODE + L stopped it. Reported as landed at CODE + L
    (the filler changes nothing but rip). Any other `SI_KERNEL` stop is a breakpoint of the instruction
    itself (`int3`, `int 3`), rip after it: `Breakpoint(128)`.
  - `SIGTRAP` with `TRAP_BRKPT` (1): `int1`, rip after it: `Breakpoint(1)`.
  - `SIGSEGV`, `SIGBUS`, `SIGILL`, `SIGFPE`: faults with their codes. `SIGSEGV` with `SI_KERNEL` and
    address 0 is `GeneralProtection`; `SIGBUS` with `SI_KERNEL` and address 0 is `StackFault`; `SIGBUS`
    with `BUS_ADRALN` is `Alignment(si_addr)`, which on x86 is always 0 (Linux gives #AC no address; seen
    on the i7-8750H with AC set and an 8-byte load at an odd address).
  - `SIGSYS`, from the seccomp filter, which stops every call made from the instruction window `[CODE,
    CODE + 18)` (the end as seccomp reports a call's instruction pointer, after the `syscall`: the longest
    instruction and a `syscall`, `Machine::WINDOW`) and every call whose `seccomp_data.arch` is `AUDIT_ARCH_I386` wherever it was made: a
    system call, for `syscall`, `int 0x80` and `sysenter` (whose `SIGSYS` arrives with rip in the vDSO,
    section 2, run 7), which never runs.
  Anything else (a stop elsewhere, a signal not listed) is reported as such and fails the case; nothing is
  mapped by guesswork. A `TRAP_TRACE` stop with rip still at CODE is stepped on only when the instruction
  is a rep string instruction (one element per step) or a gather (stopped after the elements before a
  faulting one, rflags with RF, then the fault on the next step: section 2, run 8), up to `Machine::limit`
  steps, through `Machine::unfinished`, which is given the instruction's bytes; any other instruction
  landing at CODE (a `ret`, `call` or `jmp` to CODE) is finished there. The runner's own FS base is set
  back before anything else.
- **Blobs** (layers 2 and 3 natives): the runner forks per blob, maps it and runs it with the blob start
  state, and reports output and status in-band. When a signal ends the blob, the record also holds the
  signal, its `si_code`, rip and the general registers from the frame: before it enters the blob, the child
  installs one handler for SIGTRAP, SIGILL, SIGSEGV, SIGBUS and SIGFPE that writes them out and ends the
  child with that signal's status. Flat blobs make no system calls but the stand-in's, so they never
  install handlers of their own. The translated side reports the same from its stop (X6's trap-flag
  blobs compare these, section 9). Ordinary executables (layer 3's check of a blob's signal ending) run the
  same way, a child each, through `fexecve`. Docker is not used on the x86 machine.
- **Recorded results** (not built at X2, proposed for X3 with batching): every reference result is stored under `target/reference/x86_64/` keyed by the
  reference CPU's model string, the runner's content hash and the case's hash, so a replay or a run while
  the machine is off uses the real CPU's recorded answer. A case with no recorded answer and no machine
  fails.
- **ARM64** keeps its local Docker reference behind the same interface (the transport is chosen by the
  guest: local Docker when the host's CPU is the guest's, SSH otherwise).

### 7.4 The three layers for x86-64

**Layer 1: one instruction at a time.** The forms list comes from iced-x86's own tables: one form per iced
`Code` whose `OpCodeInfo` says it is valid in 64-bit mode (`mode64()`) and is an instruction
(`is_instruction()`, not a directive), of every encoding (legacy, VEX, EVEX, XOP, 3DNow) but MVEX: the MVEX
Codes and the `KNC` VEX Codes exist only on Knights Corner, iced decodes them only with its `KNC` option,
which the translator does not set, and their bytes decode as other Codes or `INVALID`, which the forms and
`no_form` cover (section 2, run 10). Each form goes into one of four lists by the claim group (section 2's
table, with section 5.10's lists in iced's names) of the `Code` the translator's own decoder gives for the
form's encoding (`FORCE_RESERVED_NOP`, section 5.2), not of the form's own `Code`: the default list, the AVX
setting's list, the never-claimed list, and the kernel-only list. So `endbr64`, `rdsspq`, `cldemote`, MPX's
forms and the prefetches, which decode as `Reservednop_*`, are in the default list and compared with the
reference like any other form; the `Reservednop_*` forms' cases are drawn with random `66`, `F2`, `F3` and
REX.W prefixes and every ModRM, so they reach every encoding of the space, those of features not yet known
included. The kernel-only list's cases pass when the translator stops with the fault the reference CPU
gives (section 5.2). The never-claimed list (every EVEX form, AVX-512 and all, every XOP and 3DNow form,
`femms` among them, and every legacy or VEX form of a feature in neither claim list, such as AES, SHA, TSX,
AVX-VNNI and VAES) is also run on the reference and compared, at 100 cases a form (the knob of every list),
since an instruction the CPU lacks stops before it touches its operands. Where the reference lacks the
feature too (AVX-512 and the other EVEX forms, seen in run 7; TSX, WAITPKG, CLWB, PTWRITE, SERIALIZE,
MOVDIRI and RDPID, run 9; 3DNow and XOP, which Intel CPUs never had, expected to give SIGILL there too),
both sides must stop with illegal instruction. Where the reference has it (`GuestTest::REFERENCE_HAS`: AES,
PCLMULQDQ, RDRAND, RDSEED, CLFLUSHOPT, XSAVEOPT, XSAVEC, and the AVX setting's list while the setting is
off), the translated side must stop with illegal instruction and the reference's run is recorded, not
compared. A form's
name (the `Code` name) is the seed key; its group is its mnemonic and encoding. A case is a random
`Instruction` for the `Code`, with operands from `op_kind(i)` (registers, immediates weighted toward edges,
memory operands whose base and index point into DATA), encoded by iced's `Encoder`, which gives the bytes and
length. A share of the cases instead puts a memory operand's base, or rsp for the implicit stack accesses, in
the last 64 bytes below 2^47 or at a non-canonical value (section 5.4), with every base register drawn, rsp
and rbp and r12 and r13 among them and `fs:` and `gs:` prefixes among the operands, so that the page fault,
general protection and the stack fault are each compared with the reference. The FS and GS bases themselves
are drawn below 2^47 - 4096, what `arch_prctl` sets; a case the reference answers as refused fails with that
reason, never compared (a canonical base above it, which `wrfsbase` can set, is not drawn while the runner
sets bases by `arch_prctl`). Another share gives rip-relative
operands displacements that reach the code area around the instruction (its own bytes and the `int3` filler,
which both sides hold, section 7.2) or just past the code area's ends (a page fault on both sides). The
accesses that are not one operand (section 5.4) get draws of their own: bit offsets of `bt` and its kin of
every sign and size, three quarters of them aimed with the operand (the base or index set so that the
address after the offset's move is the drawn target: the data area, its edges, past 2^47, non-canonical); gather indexes and masks with a share of
the elements on an unmapped page, across the page end or masked off there; masks of the masked moves and
`maskmovdqu` (all-zero among them) with the operand placed across the end of DATA, below its start or near
2^47. A gather that faults part way is stepped on by the reference runner (section 7.3) and compared at its
fault. Random registers, flags, vector, x87 and MXCSR values are weighted toward edge values; MXCSR is the
default in half the cases, and tracking (section 5.8) is on in half, as ARM64's is today. The starting rflags
holds what user code can hold: the six flags, DF, NT and ID drawn, bits 1 and 9 set, and AC, TF, IOPL, RF, VIF
and VIP clear (AC because the translator never holds it, section 5.1; TF because the reference runner's
own trap flag is what stops the case, so a case cannot start with the program's TF; NT and ID are entered as
section 7.3 says). Single stepping is tested by the trap-flag programs of X6 and X10 instead (section 9). The reference runs it by the trap flag; the translator runs it at look-ahead none; every register,
every flag (including undefined ones), the touched memory and the stop are compared. Random byte strings that
iced decodes as `INVALID` are compared with the CPU's stop: SIGILL, or general protection for the ones past 15
bytes (section 5.2), which `no_form` draws on purpose as runs of prefixes before a body (a legacy one, or a
VEX or EVEX one after a 66, F2, F3 or REX that makes it invalid); valid ones also check the length (where the
CPU lands). Exclusions, each with a reason: a jump to itself; the instructions answered by design (CPUID,
rdtsc, rdtscp, xgetbv), which a designed-answers test checks instead; `int 0x80` and `sysenter`, which the
reference kernel runs as 32-bit system calls and this phase does not provide (section 5.2), also checked by
the designed-answers test; the VMX instructions, whose answer on the reference depends on its KVM setting
(section 5.2); and `sgdt`, `sidt` and `lsl` of segment 0x7b, whose answer depends on which processor ran
them (section 5.10). The never-claimed list's rule (both sides illegal instruction where the
reference lacks the feature) therefore holds for every form left in it: `rdpmc` is kernel only and the VMX
forms are excluded.

**Layer 2: random small programs**, from the shared program model (loops with counters, calls, bounded
recursion, tail calls, calls and jumps through registers, jump tables in both x86 compiler styles, frames,
groups of accesses through one base, walks, lookups, system calls, a final dump of every register and
the flags) with an x86 emitter written with iced's `CodeAssembler` (test only) that adds what x86 has:
read-change-write memory operands, string instructions without rep (with rep at X5, which builds them), push and pop, calls through
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
- **X1, the interface, with ARM64 on it.** `wcontainers-guest-cpu` with all of section 3's traits and
  types, and the translator's and runners' side of each (section 3.2, "Built whole at X1"), its core
  `no_std` and the rest behind `translate`; `wcontainers-arm64` implements
  them, layered the same way (section 3.1); `wcontainers-translator` is generic over `Guest` and depends on
  no CPU crate: `cargo tree -p wcontainers-translator` shows none, also among its dev-dependencies, since
  its validity test (`tests/valid.rs`, which draws ARM64's forms) moves to the test crate, and tests in
  the translator crate fail if its manifest or sources name a CPU crate again or its resolved normal, build
  and dev dependencies (`cargo tree`, all features, every target) hold one, under whatever name; the test
  crate and the
  browser crate name `Arm64` where they call the translator; the float hook's host side moves into
  `Arm64::run_hook`, and the host runner and the browser's cdylib call it instead of `exact::run` (the
  browser's `cpu_hook` export and runner.js's copy of the state come with it, section 3.4 Hooks); the
  effects recorder and `Registers` move into the interface crate (section 3.1); both runners ask
  `G::memory_fault` before they report a memory fault (ARM64 always `Page`, so nothing they report
  changes). Pass bar: the WebAssembly output is byte for byte the same as before the move for the
  benchmark set (the whole program and each workload) and all nine real binaries of M5, in both settings,
  at every look-ahead (none, bounded 1, 17, 64, 256 and 4096, full; a binary also from each kind of start
  alone at full and bounded 256), and the thread module too, by one command kept in the repository:
  `crates/wcontainers-translator-test/same-output.sh <commit>`, which builds the earlier commit from its own
  copy and has `tests/output.rs` on both sides write every module's length and BLAKE3 hash, then compares
  the lists with `cmp` (added as its own commit before any interface change, so the "before" exists; both
  sides run the current tree's `tests/output.rs`); the whole test suite of the interface, ARM64, translator
  and test crates passes, and the browser crate builds and lints; the cores of the interface and of
  `wcontainers-arm64` build for `wasm32v1-none`, a target without the standard library, with `translate`
  off, checked by a test; a test guest in the test crate's tests fills in the interface the way x86-64 does
  (byte-aligned variable-length instructions, a return address on the stack, a red zone, different
  registers passed in and out, partial flag setters, a segment base, a trap that leaves pc after it, its
  own slow path and hook registers), and a variant of it without flags the way RV64GC does; their
  translations at every look-ahead validate, and programs of each run in wasmtime at every look-ahead (the
  comparison's list, and bounded 5, which cuts these small programs part way) in both settings and end with
  the registers, pc, memory and hook calls their meanings give (among them two accesses of one segment group
  that fault exactly, the groups' exact checks gone at full look-ahead, and two instruction streams through
  one instruction that meet again); the benchmarks in wasmtime are
  unchanged within noise (old and new alternating, best of 5 each, load recorded), and so is a program that
  takes the float hook on every operation. The move is done on this branch's copy of the
  translator; fixes the translator track makes later come in by a merge the orchestrator names, and are
  moved onto the interface then, with the proof run again.
- **X2, the shared test harness.** `GuestTest` and the ARM64 implementation; the harness, runners, browser
  build (feature-chosen guest, contract JSON with the per-CPU numbers) and reference transport generic; the
  x86 reference runner built, uploaded and proven on the x86 machine through the harness's transport on a hand
  list of cases (as section 2's experiment, now in the repository) and on a blob. Pass bar: every ARM64 test
  passes with the same seeds giving the same cases and programs (layer 1 cases and layer 2 programs byte for
  byte the same per seed); the x86 reference answers a hand list of at least 30 instructions with what the
  Intel manual says, each checked by hand in the test, among them every stop of section 7.3's list (`mov ss`
  with its held-back trap, `int3`, `int 3`, `int1`, `syscall`, `int 0x80` and `sysenter` as `SIGSYS`, `pushf`
  from a state with NT and ID set storing them, a `ret` to a non-canonical address, general protection from
  `mov rax, [rbx]` and the stack fault from `mov rax, [rbp]` and from `push rax`, each with a non-canonical
  address), x87 instructions whose state is written and read back (section 2, run 4) and a `call far` with
  selector 0x33. The trait of section 7.2 is the one built; where building it shows a signature must change,
  the change is recorded in the status file.
- **X3, x86 at look-ahead none, integers.** `wcontainers-x86` with the decoder, the block, the base integer
  instructions (moves, arithmetic and logic at every width, lea, push and pop, call and ret, jmp and jcc
  (`jrcxz` and `jecxz` with them, on the manual's Jcc page; `loop` and its kin are X5's), cmov
  and setcc, shifts and rotates, multiply and divide, string instructions without rep, syscall), lazy flags,
  the build-time claim table from iced's tables (section 2) with its test, the canonical check on branches
  through a register or memory (a call's after its push), `X86_64::memory_fault`, the 128-by-64 division
  (section 5.2), the 15-byte limit with its table of invalid opcodes' lengths and the sweep it comes from
  (section 5.2), the description section's guest and offer bytes (format 4; ARM64's output then differs in
  those bytes and the format byte only). Pass bar: layer 1 for those forms against the real CPU with zero
  mismatches at 1,000 cases a form, including bases (rsp and rbp among them) and stack pointers in the last 64
  bytes below 2^47 and at non-canonical values, so that page faults, general protection and stack faults are
  all compared, and dividends whose rdx is not rax's extension, and 10,000 no-form cases, half of them prefix
  runs near 15 bytes, with zero mismatches; a C hello world built as a flat x86-64 blob prints and exits in
  wasmtime and Chrome; what the decoder adds to the browser build measured; the long division's cost against
  native measured.
- **X4, x86 at full look-ahead.** Functions, registers in locals, flags liveness, direct conditions, calls
  and returns as WebAssembly calls with the pushed return address, the red zone frames, switch recognition
  for both x86 compiler styles, loop proofs, the stop and permission checks; the x86 layer 2 emitter. Pass
  bar: layer 2 at 10,000 programs with zero mismatches against the CPU and against none; the benchmark set
  gives the same results at none and full and is timed on the x86 machine natively, in wasmtime and in
  Chrome.
- **X5, every default instruction.** All integer instructions of the default claims, SSE through SSE4.2,
  MXCSR fast and exact paths with tracking and the counted read of its flags (section 5.8) and their
  designed answers, NaN rules and the probe, CPUID and its designed answers, rep string
  instructions (bulk and one element), FS and GS, the approximate reciprocals by section 5.8's formula. The
  passed-register lists measured and chosen. Pass bar: layer 1 complete for the default forms at 1,000
  cases a form and for the never-claimed list (section 7.4) at 100 cases a form, layer 3 Csmith at 1,000
  programs, zero mismatches; the reciprocal formula equal to the
  reference CPU on all 2^32 inputs of `rcpss` and of `rsqrtss`.
- **X6, bounded look-ahead, the rare paths and code written without announcement.** Translating from any
  byte address, the bottom loop, go to address, adding code while running, injection tests, write
  protection of translated pages with the unwind and the unprotected exact piece (the memory fault hook
  returning the unwind payload, section 5.7), both runners'
  stand-ins of it, test programs that write and run code with no announcement (one that writes code on
  another page and runs it; one whose loop writes the immediate of its own next instruction on the same
  page, scratch `run8/smc.c`, which prints `eax 1 sum 20100` on the reference; one whose instruction writes
  into its own bytes); setjmp, C++ exception and Rust panic programs; single stepping by the trap flag
  (section 5.1: TF in `Rflags`, `raise_stop`, `Guest::step` and its private words, the stop hook returning
  the unwind payload, in both runners), with trap-flag blobs run on the reference and translated and their
  stops and registers compared: `popf` setting TF before a `nop`, a `jmp`, a `call`, a `ret`, a `syscall`
  (getpid; the stop comes after the instruction after it), a `popf` that clears TF, a `rep movsb` (one
  element, rip on the instruction), `mov ss` (held back one instruction) and an `iretq` setting TF; each
  stops by SIGTRAP `TRAP_TRACE`, and the blob runner's record of the stop (rip and the general registers,
  section 7.3) must equal the translated side's. Pass bar: those tests pass at every
  look-ahead, in wasmtime and Chrome, with the native output (the trap-flag blobs with the reference's
  stop).
- **X7, x87 and MMX.** The x87 hook with `X87DoubleExtended`, precision control by rounding to odd,
  `fsqrt` by section 5.8's root, the
  transcendental instructions with `astro-float`, MMX registers, `fxsave` and `fxrstor`. Pass
  bar: layer 1 for every x87 and MMX form at 1,000 cases a form, zero mismatches except known failures
  recorded with their reasons; a C program using `long double` gives the native output.
- **X8, the AVX setting.** AVX, AVX2, FMA, F16C, BMI2, XSAVE in the setting, the two counters, the setting
  in the cache key, the instructions of the setting's claim group trapping with the counter when it is off,
  and the designed-answers case of a gather that faults part way with TF set (section 5.1's known
  difference).
  Pass bar: layer 1 for every
  form of the AVX list at 1,000 cases a form, zero mismatches; the benchmark set built for x86-64-v3 runs
  with the setting on, and with it off stops with illegal instruction and the counter.
- **X9, real x86 code at scale.** The amd64 builds of the nine real binaries of M5 (busybox, the C library,
  ripgrep, git, Node, Python, Bun, a Java runtime, a Go program) translated in both settings and loaded in
  both engines. Recorded: sizes, translation time, engine memory, the lookup array's memory, how many
  switch tables were over the limit or had absolute entries.
- **X10, the kernel's x86-64 personality** (after the kernel track's milestones): section 6, and the
  kernel's tests for x86-64 against the x86 machine, among them scratch `run10/tf.c` (single stepping with
  a SIGTRAP handler, faults and `int3` under TF, TF kept across `rt_sigreturn`) and `run10/rf.c` (RF in
  fault frames only) as programs whose output must equal the reference's (`run10/tf.out`, `run10/rf.out`
  apart from addresses); the CPU side's part of code written without
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

- What the x86 translator downloads in the browser once its meanings are complete (X5). At X3, measured
  from the link map of the release browser build for x86 (`--Map`, every custom section stripped): 872,638
  bytes, 203,533 brotli (`-q 11`); iced-x86's own functions and statics in it are 156,416 bytes (115,635 of
  code, 40,781 of data), 27,565 brotli compressed alone (a scratch cdylib of the decoder alone gave 46,880,
  section 2). A lower bound: iced code inlined into the x86 crate's own functions is not counted.
- The lookup array's memory on real x86 programs (four bytes per byte of code, expected).
- What a CPU without MPX hardware does with `0F 1A` and `0F 1B`: a NOP, expected from the manual's
  reserved-NOP space; the reference has MPX, which Linux leaves off, and runs them as NOPs (section 2,
  run 9).
- How many switch tables exceed the br_table limit at one entry per byte, and how many have absolute
  entries.
- The cost of lazy flags with PF and AF on demand, and of partial flag setters, on real programs.
- The cost of the canonical-address check on every `ret` and every jump or call through a register or
  memory (four operations each, section 3.4): measured at X4 as user-mode instructions of a wasmtime run on
  the i7-8750H with and without it, the benchmark set at full: 1.4% of fib's, 2.0% of sort's, 1.4% of
  interpret's, 4.4% of search's; within the ±2% that code layout moves the workloads without calls.
- The cost of the approximate reciprocals' formula (about 30 vector operations for four lanes, section 5.8).
- The size of `astro-float` in the real x86 browser build (40,947 bytes brotli in a scratch cdylib, section 5.8).
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
- Engine memory for x86 output. (Compile time and output size against input size, raw and gzipped, at
  none and full: `measure_x86_64`, first at X4, in the status file.)
- Not seen on the reference: which fault the CPU gives for an encoding past 15 bytes that runs into an
  unmapped page (section 5.2 treats it as running past the end of the code range); layer 1 places no case
  there today.
- The cost of the counted MXCSR read in glibc's lazy binding trampoline (one float hook call per lazy
  binding, expected small beside the binding's own work), and how many programs fire that counter.
- Not seen, expected from sources: `sysenter` giving general protection on a kernel without 32-bit
  emulation (section 5.2), and Linux keeping NT and ID across signal delivery (section 6). Where rip's
  read falls in `iretq`'s order: settled at X5 (section 5.3).
- The cost of checking the accesses that no group or proof covers (bit offsets, gathers, masked moves,
  8- and 16-bit indexes, section 3.4) on real programs, measured at X5 and X8; and the cost of code
  written on the page that writes it (one fault, one step and two translations a write, section 5.7),
  measured at X6.
- Not seen, expected: which fault a gather element, a masked span or a `maskmovdqu` half that reaches a
  non-canonical address gives (section 5.4 applies the stack-segment rule to it); that an instruction
  writing into its own bytes runs as it was read (section 5.7). Layer 1 and X6's programs check both.
- A known gap shared with ARM64 today: a signal handler that reads the sticky float flags from its
  frame (MXCSR on x86, FPSR on ARM64) while they are not kept sees the flags last written, and nothing
  counts it (section 5.8).
- The cost of a single step (a trip to the bottom loop, a translation of one instruction and two stop hook
  calls, section 5.1), measured at X6; only programs that set TF pay it. Not seen, expected from Intel's
  manual (a float exception is a fault): RF in the frame of a SIMD or x87 float exception's signal, checked
  at X10 beside `rf.c`'s cases.

## 13. Where the evidence is

Experiments and downloads of milestone X0 are in `pyro-parked/wcontainers-scratch/x86`: `iced-size` (the
size builds), `dis` (an iced-based disassembler used to check encodings), `reference-proof` (`hello.c`,
`stepper.c`, `cases.sh`, `cases2.sh` and their outputs), `sources` (the relaxed SIMD overview and
sandsifter's `injector.c`, and in run 3 RISU's source in `sources/risu`), `x87-crate` and `x87-crate-base`
(astro-float's size in a wasm32 cdylib), `x87-nostd` (astro-float without the standard library),
`apfloat-pc` (precision control by rounding to odd with rustc_apfloat); run 3 added to `reference-proof`
`stepper3.c`, `approx.c`, `approx_dump.c` with its output `approx_tables.txt`, `approx_formula.c` and
`approx_mxcsr.c`. The second independent check's seccomp stepper is `check2/exp/stepper_sc.c`. Run 4 added
to `reference-proof` `stepper4.c` (made from `stepper3.c` by `scripts/make_stepper4.py`), `cases4.sh`,
`cases5.sh` and their outputs, and `cases6.out`. Run 5 added `cases7.sh`, `cases8.sh` and `sweep15.sh` with
their outputs (the 15-byte limit, `clflush`, invalid opcodes' lengths; `sweep15.iced` and `sweep15.classes`
set the sweep beside iced's decoding) and `brk.c` (ARM64's breakpoint `si_code`); the fourth independent
check's cases are in `check4`. Run 6 added `cases9.sh` and `cases10.sh` with their outputs (the stack fault
against general protection, alignment first, MMX writes and the x87 registers); the fifth independent
check's cases are in `check5`. Run 7 added `stepper5.c` (NT and ID entered, the seccomp filter with its arch
test), `cases11.sh`, `cases12.sh` and `cases13.sh` with their outputs (NT and ID, `sysenter`, a legacy
prefix before VEX and EVEX, `iretq`'s order); the sixth independent check's cases are in `check6`. Run 8's
are in `run8`: `stepper6.c`, `cases14.sh` and `cases15.sh` (bit offsets, `xlat`, gathers, masked moves),
`smc.c` (code written on its own page) and `copyuser.c` (partial copies to user memory), with their outputs;
the seventh independent check's cases are in `check7`. Run 9's are in `run9`: `cases16.sh` with its output
and `xacquire.out` (the reserved-NOP space and other features not offered), `cpuflags.txt` (the reference's
features), `dis` (iced's decoding with and without `FORCE_RESERVED_NOP`), `iced-strict-nostd` and
`iced-strict-std` (iced without the standard library, and the control that fails), and `sf`, `sfprobe`,
`sfc` (Berkeley SoftFloat's crates and its C for wasm32); the eighth independent check's cases are in
`check8`. Run 10's are in `run10`: `tf.c` and `tf.out` (the trap flag a program sets), `rf.c` and `rf.out`
(RF in signal frames), `feat` and `feat.out` (iced's feature names per `Code`), `umip.out` and
`branch-unmapped.out`; the ninth independent check's are in `check9`. The x86 machine holds copies of the
programs and scripts in `~/wcontainers-x86/proof` (with `approx.out`, `approx_formula.out`,
`approx_mxcsr.out`; the outputs of the cases scripts up to run 7 are kept only on the Mac), for run 8 in
`~/wcontainers-x86/proof8`, for run 9 in `~/wcontainers-x86/proof9` and for run 10 in
`~/wcontainers-x86/proof10` (`tf`, `rf`, their sources and outputs), and the independent checks' own files
in `~/wcontainers-x86/check`, `check2` to `check9`.

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
- T36 ARM64 loop idioms in the proofs: 3.2 loop proofs and the paragraph on narrow writes after the plain
  types; 3.4 Loop proofs.
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
- E4 of a feature not offered: 7.2 offered features; 7.4 (the claim group of the decoded `Code`, from
  section 5.10's lists in iced's names).
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
4. The register set: 3.2 `BITS`, `Bit`, `reg_bit`, `vector_bit`, `bit_reg`, `SP_BIT` (narrow writes:
   the paragraph after 3.2's plain types); 3.4 Registers.
5. Effects found by recording the meaning, extended (memory, hooks, controls): 3.1 (the generic
   recorder and `Effects<G>` live in the interface crate); 3.3 (the recorder is an emitter); 3.2 `plain`
   and 3.4 Flags ("plain computation" is the CPU's, not derived from the effects, to keep ARM64's output).
6. Control flow: 3.2 `flow`, `Flow`, `FlowKind`, `Through`, the length from `decode`; 3.4 Control flow.
7. Addresses built into registers: 3.2 `built_address`; 3.4.
8. Switch recognition: 3.2 `jump_table`; 3.4 Switches (indexed by target address, so no case-number
   register is needed).
9. Memory accesses: 3.2 `accesses`, `MemoryAccess`, `Base`, `Index`, `AccessBytes`; 3.4 Memory accesses; 5.4.
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
    (general protection or, through the stack segment, the stack fault for accesses reaching a
    non-canonical address, 5.4); 7.2 `Stop::StackFault`.
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
