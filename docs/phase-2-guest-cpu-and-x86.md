# wcontainers, phase two: one guest CPU interface, and x86-64

Status: plan written 2026-11-17 from the owner's brief (decisions of 2026-11-16) and the map of today's
ARM64 assumptions. Nothing of it is built yet. The research and the two experiments of milestone X0 are
done; what they found is in section 2.

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
  translator and tests areas and where this plan covers it.

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
- **RISU** (QEMU's random instruction tester, x86 patches of 2019): puts a `UD1` "operation" instruction
  after each test instruction, the way our ARM64 harness relies on zeros being `udf #0`.
- **Debuggers** (`ptrace(PTRACE_SINGLESTEP)`) use the same trap flag.

Zero bytes are a real instruction on x86 (`add [rax], al`), so the ARM64 trick does not carry over, and a
trap filler after the instruction does not catch jumps. The method chosen and proven (experiment below):
**the trap flag set in the signal frame the reference runner returns through.** The runner enters the guest
state from a signal handler (as on ARM64): it writes the guest's registers into the frame, sets `TF` in the
saved rflags and returns. The CPU runs exactly one instruction and raises a debug trap, which arrives as
`SIGTRAP` with `si_code` `TRAP_TRACE` (2) and rip at the next instruction, or at the target of a jump. The
handler reads the registers back and returns to the runner's own saved state.

### The two experiments on the x86 machine (scratch, not in the repository)

Machine: Intel i7-8750H, 12 threads, Ubuntu 20.04.3 with a mainline Linux 7.3.4 kernel, idle (load 0.03).
Its CPU reports `avx2`, `fma` and `fsgsbase`, and no AVX-512. Nothing was installed on it; the
experiments are static binaries copied to `~/wcontainers-x86/proof`.

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
   - `syscall` (getpid): the trap came only after the instruction after it, so system calls must be
     stopped by seccomp before they run, as the ARM64 reference already does.

   What this teaches the reference runner (section 7.3): write every vector register (else the runner's own
   values leak in), set the SSE and AVX bits of the frame's XSAVE header (`xstate_bv` bits 1 and 2) or the
   kernel loads their first state, read a component as zero when its `xstate_bv` bit is clear, find the
   upper ymm halves at the offset CPUID leaf 0xD sub-leaf 2 gives, and mask `TF`, `RF` (bit 16) and `IF`
   out of the comparison of rflags.
3. **Round trips**, measured 2026-11-17 (Mac load 57): a new SSH connection took about 344 ms; 1,000 runs
   of the stepper on the x86 machine, each a new process, took 410 ms in all. So the reference must keep one
   connection open and stream cases through it (section 7.3).

## 3. The guest CPU interface

### 3.1 Shape

`wcontainers-guest-cpu` holds one trait, `Guest`, the `Emitter` trait meanings write to, and the plain
types both use (register sets, accesses, flow, stack moves). A CPU crate defines one zero-sized type and
implements `Guest` for it: `wcontainers_arm64::Arm64`, `wcontainers_x86::X86_64`, later
`wcontainers_riscv::Rv64`. The translator's public functions take the guest as a type parameter:

```rust
pub fn translate<G: Guest>(blob: &[u8], code: Range<u64>, starts: &[u64], look_ahead: LookAhead,
    optimise: Optimise, host: G::Host) -> Result<Translation, Error>;
pub fn thread_module<G: Guest>() -> Vec<u8>;
```

Every use is monomorphised; there is no `dyn` anywhere in the translator, so nothing is looked up at run
time and nothing is paid at translate time either beyond what ARM64 pays today. The translator crate
depends on `wcontainers-guest-cpu` only, never on a CPU crate. The program that ships (the browser's
cdylib, a host runner, later the kernel's CPU side) names the one guest it is built for.

The interface depends on `wasm-encoder` (meanings write WebAssembly through its `InstructionSink`) and
`wcontainers-abi` (only for `PAGE_SIZE`).

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
    /// One entry per register bit used, in bit order; see `Bit`.
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
    /// Each flag with its bit in the saved flags word and its register bit.
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

    // --- meanings ---
    fn emit<E: Emitter<Self>>(e: &mut E, i: &Self::Instruction);

    // --- facts about one instruction, for the analyses ---
    fn flow(i: &Self::Instruction) -> Flow<Self::Reg>;
    /// Up to two memory accesses, in the order the instruction makes them.
    fn accesses(i: &Self::Instruction) -> [Option<MemoryAccess<Self::Reg>>; 2];
    fn stack_move(i: &Self::Instruction) -> StackMove;
    /// The blob offset of an address this instruction builds into a register, given the instruction
    /// before it on the path (adrp + add, lea [rip + d], auipc + addi).
    fn built_address(previous: Option<(u64, &Self::Instruction)>, at: u64, i: &Self::Instruction)
        -> Option<u64>;
    /// The targets of a jump through `reg`, given the instructions on the path into it (oldest first) and
    /// the blob.
    fn jump_table(path: &[(u64, Self::Instruction)], reg: Self::Reg, blob: &[u8], code: Range<u64>)
        -> Option<SwitchTable>;

    // --- calls, returns and the stack ---
    /// Where a call leaves the return address.
    const RETURN_ADDRESS: ReturnAddress;
    /// Bytes below the stack pointer a function may use without moving it: x86 128, else 0.
    const RED_ZONE: u64;
    /// The CPU checks the stack pointer's alignment when it is the base of an access (ARM64 under Linux).
    const SP_ALIGNMENT_CHECKED: bool;
    /// Push the address in this i64 local as data accesses use it (ARM64 untags it; others nothing).
    fn data_address(sink: &mut InstructionSink<'_>, local: u32);
    /// On the path where a jump, call or return leaves for a target that is not the expected one:
    /// untag it in place (ARM64), or stop with the fault the CPU gives (x86: not canonical).
    fn branch_target<E: Emitter<Self>>(e: &mut E, local: u32);
    /// The return address the bottom loop passes, which no guest return can reach.
    const UNREACHABLE_RETURN: i64;

    // --- system calls ---
    const SYSCALL: SyscallRegisters<Self::Reg>;

    // --- code changes ---
    const CODE_CHANGES: CodeChanges;

    // --- loop proofs ---
    fn induction(i: &Self::Instruction) -> Option<Induction<Self::Reg>>;
    fn counting_exit(path: &[(u64, Self::Instruction)], branch: &Self::Instruction)
        -> Option<CountingExit<Self::Reg>>;
    fn narrow_load(i: &Self::Instruction) -> Option<NarrowLoad<Self::Reg>>;
    fn unsigned_bound(path: &[(u64, Self::Instruction)], branch: &Self::Instruction, taken: bool)
        -> Option<Bound<Self::Reg>>;
    fn index_extension(i: &Self::Instruction) -> Option<IndexExtension<Self::Reg>>;
    /// How a write narrower than 64 bits leaves the rest of the register, per width in bytes (1, 2, 4).
    const NARROW_WRITE: [Extend; 3];
}
```

The plain types, also in the interface crate:

```rust
/// A set of register bits (what `Registers` in wcontainers-arm64 is today).
pub struct Registers(pub u128);

pub struct Bit { pub class: Class, pub ty: Ty, pub offset: u16 }   // offset in the thread block
pub enum Class { General, Flag, Vector, Segment }
pub enum Ty { I64, V128 }

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
    pub index: Option<(R, u8 /* scale shift */, Extend)>,
    pub displacement: i64,
    pub write_back: WriteBack,     // None, Before(by), After(by)
    pub bytes: AccessBytes,        // Fixed(n) or Counted (a rep string instruction: rcx elements)
    pub kind: AccessKind,          // Read, Write, ReadWrite
}
pub enum Base<R> { Register(R), Pc, Segment(Segment) }  // Segment::Fs, Segment::Gs: base plus a segment base
pub enum StackMove { None, By { delta: i64, before_access: bool }, Unknown }
pub enum ReturnAddress { Register(u32 /* bit */), Stack { bytes: u8 } }
pub struct SyscallRegisters<R> { pub number: R, pub args: [R; 6], pub result: R }
pub enum CodeChanges { Announced, Detected }
pub enum TrapDetail { Instruction, Local(u32), Const(i64) }
pub struct HookType { pub name: &'static str, pub params: &'static [ValType],
    pub results: &'static [ValType], pub registers: Registers }
```

`TrapKind` moves from the translator's contract into the interface (the contract re-exports it) and gains
the kinds x86 needs, numbered after ARM64's four so ARM64's numbers do not change: `DivideError` (4),
`GeneralProtection` (5), `FloatException` (6, detail the `si_code`), `SingleStep` (7). Each comes with the
signal Linux sends (section 6).

### 3.3 The Emitter

Today's `Emitter` (crates/wcontainers-arm64/src/meaning.rs) becomes `Emitter<G: Guest>`, with the same
methods over the guest's types: `get(G::Reg)`, `set(G::Reg)`, `get_vector(G::Vector)`, `set_vector`,
`set_flags(G::Flags)`, `flag(G::Flag)`, `get_control(G::Control)`, `set_control`, `hook(G::Hook)`,
`host() -> G::Host`, `trap(G::Trap)`, `condition(G::Cond)` (default: `G::condition_from_flags`, the full
emitter tries `G::condition_direct` first, as today), `float_mode` (default from `G::float_slow`),
`stack_aligned`, `this_address`, `jump`, `jump_if`, `call`, `ret`, `syscall`, `next`, `sink`, `local`.
The x86 type of a vector register is per register (`Bit::ty`), so `get_vector` pushes a v128 for xmm and
an i64 for MMX; RISC-V's f registers are i64.

Changes to the methods' contract, each needed by a CPU named here:
- `check(address, bytes, access)` becomes `check(address, bytes, access, which: u8)`: which of the
  instruction's accesses (0 or 1) this is, so plans are kept per access (x86 `movs`, `push [m]`, `call [m]`
  make two). ARM64 always passes 0.
- `check_range(address, length, access) -> i32 local`: whether a run-time length of bytes may be accessed,
  without stopping. Used by the x86 string instructions to choose the bulk path (section 5.6).
- `call(target)` and `ret(target)` follow `G::RETURN_ADDRESS`: with `Stack { bytes }` the emitter itself
  stores the return address at sp - bytes and moves sp, after the target is known to be valid, and `ret`
  reads the address the meaning loaded, compares it with the expected one, and moves sp up by `bytes` only
  then (section 5.3). With `Register` nothing changes from today.
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
`flags_needed_after` (T25) stops listing ARM64 variants: "plain computation" comes from the recorded
effects (an instruction that touches no memory, calls no hook, writes no control and does not leave), and
"all flags set again" is `sets(..) == FLAGS mask`, which x86 `inc` does not meet. A CPU with no flags
(RISC-V) has empty `FLAGS` and `SHAPES`, its meanings never call `set_flags`, `flag` or `condition`, its
`Cond` is an empty enum, and `FLAGS_WANTED` is 0, so the flag locals, the flag state and the flags-word
functions are never made. The flag locals are a fixed array of at most 8 (x86 has 7).

Flags wanted (T13). Bit 0 of the expected return address is free on ARM64 because returns are 4-aligned;
on x86 returns can be odd. The CPU names the bit: ARM64 1 (unchanged), x86 bit 62 (no canonical user
address has it), RISC-V none. Every test is `target == L_RETURN & !G::FLAGS_WANTED`.

Address no return reaches (T14) and untagging (T15). `UNREACHABLE_RETURN` stays `i64::MIN` for all three:
ARM64 because untagged return targets never have it, x86 because it is not canonical and every x86 branch
to a non-canonical address stops with a general protection fault before it lands (`branch_target`), RV64
because it is above every user address and a jump there finds no piece and faults at that address, as the
CPU does. `untag` is called today at code.rs:1305, full.rs:2561 and 2857; those calls become
`G::branch_target` (branches) and `G::data_address` (data), which on ARM64 emit exactly today's untag.

Control flow (T7). `Flow` gains `FlowKind`, so the translator no longer matches `Branch { link: true }` or
`BranchRegister::Br`. A RISC-V `auipc ra, hi; jalr ra, lo(ra)` pair is a call through a register whose
target the translator knows from `built_address` on the pair; it is treated as a direct call.

Addresses the code builds (T8). `built_address(previous, at, i)` replaces "adr, or adrp plus add at the
4-byte slot before". It finds x86 `lea r, [rip + d]` alone and RISC-V `auipc` plus `addi` or `jalr` (2 or
4 bytes apart).

Switches (T9, T10). `jump_table` gets the decoded instructions on the path into the jump, oldest first
(the function's own instructions, never a window read backwards from the jump, which x86 cannot do), and
the blob. The patterns stay in the CPU crate's `habits` module. The jump table the translator emits is
indexed by the target address, as today, scaled by the alignment: `(target - lo) >> log2(ALIGN)`. Decision
and reason: indexing by the address the guest computed is exact even when the guest changed its table in
memory at run time (an unknown target leaves through the lookup), and it needs no second kind of table. On
x86 a table then has one entry per byte of the span its targets cover, so spans reach the 65,520-entry
limit sooner; a table over the limit falls back to the lookup with the existing counter "jump through a
register not recognised". Tables whose entries are absolute addresses (non-PIE x86 code: `jmp
[T + rax*8]`) need the load address the translator never sees, so they also go through the lookup, counted
the same way. How often either happens is measured on the x86 real binaries at X9; only if it is common is
indexing by case number brought back for a decision.

The lookup array (T11). One i32 slot per `ALIGN` bytes of guest code: ARM64 one per 4 bytes (as today), x86
one per byte, RISC-V one per 2 bytes. The slot's byte index is `(pc - table_base) << (2 - log2(ALIGN))`,
which is `pc - table_base` on ARM64 (nothing emitted, so the output is unchanged), and the "is it aligned"
test is emitted only when `ALIGN > 1`. `TABLE_BASE` and `SLOTS_COVER` in the contract say "one entry per
`G::ALIGN` bytes". Decision and reason: one memory read per lookup is the fastest lookup there is, and the
cost is state memory, four bytes per byte of x86 code (untouched pages cost nothing, so only code ranges
that hold pieces cost memory). For scale, expected and not measured: 38 MB of code, the size of the largest
ARM64 sample, would need 152 MB of slots. If the x86 binaries at X9 show the 4 GiB state memory is at risk,
a two-level table is measured then.

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

Host probe (T28). An associated type with its own probe module. It is part of the cache key.

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

Loop proofs (T36). The recognisers become CPU functions returning plain descriptions: "adds step s to r at
width w", "the loop leaves when counter relation bound, signed or not, at width w", "a narrow load into r
zero-extends to at most m", "on this edge r is below b unsigned", "an index is extended this way".
`NARROW_WRITE` says what a narrow write does to the rest of the register: ARM64 32-bit zero-extends; RISC-V
`*W` sign-extends; x86 32-bit zero-extends and 8- and 16-bit writes keep the upper bits (merge). The proof
module's `Domain` reasoning takes these instead of ARM64's "a 32-bit result clears the top half".

Identity (T37). `ID` names the guest. To keep X1's proof exact, the description section's format and the
cache key are not changed when ARM64 moves; at X3 the format becomes 4 with a guest byte after the format
byte, and ARM64's output changes in that byte only, checked then.

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
- Hooks: Float `(i32, i64, i64, i64) -> i64`, Clock `() -> i64`, CodeChanged `(i64)`.
- RETURN_ADDRESS `Register(30)`; RED_ZONE 0; SP_ALIGNMENT_CHECKED true; data_address and branch_target
  untag (bit 55 rule); SYSCALL x8, x0 to x5, x0; CODE_CHANGES Announced; NARROW_WRITE [-, -, Zero] (only
  32-bit writes exist).

x86-64 (`wcontainers_x86::X86_64`, section 5 gives the reasons):
- ALIGN 1, MAX_LENGTH 15; `decode` is `iced_x86::Decoder` in 64-bit mode over the bytes up to the end of the
  code range (an instruction that would run past it is `None`); trap detail the first 8 bytes.
- Bits: rax, rcx, rdx, rbx, rsp, rbp, rsi, rdi, r8 to r15 at 0 to 15 (hardware order); CF PF AF ZF SF OF DF
  at 16 to 22; FS base 23, GS base 24 (class Segment); mm0 to mm7 at 32 to 39 (i64, the mantissas of x87
  registers R0 to R7); xmm0 to xmm15 at 64 to 79 (v128); the upper halves of ymm0 to ymm15 at 80 to 95
  (v128). There is no zero register. SP_BIT 4.
- Flags: the seven at their rflags positions (CF 0, PF 2, AF 4, ZF 6, SF 7, OF 11, DF 10); shapes in
  section 5.1. FLAGS_WANTED 1 << 62.
- Block: `X86Block` (section 5.9). Controls: MXCSR, MXCSR tracking, the x87 control, status (with TOP) and
  tag words, the x87 last-instruction pointers.
- Hooks: Float (SSE exact path, with MXCSR), X87 `(op: i32, low: i64, high: i64) -> (i64, i64)` reading and
  writing the x87 area and the MMX bits, Clock.
- RETURN_ADDRESS `Stack { bytes: 8 }`; RED_ZONE 128; SP_ALIGNMENT_CHECKED false; data_address nothing (a
  non-canonical data address is past the permission table and faults; section 5.4 says how that fault is
  reported); branch_target stops with a general protection fault when the target is not canonical; SYSCALL
  rax, (rdi, rsi, rdx, r10, r8, r9), rax; CODE_CHANGES Detected; NARROW_WRITE [Merge, Merge, Zero].

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
  SYSCALL a7, a0 to a5, a0; CODE_CHANGES Announced; NARROW_WRITE [-, -, Sign] (the `*W` instructions).

## 4. Design decisions for the translator and the tests

Each decision the seam map lists, decided, with the reason. The brief's own decisions are kept as written.

Translator:
1. **Variable-length, byte-aligned code: lookup and switch tables.** One lookup slot per `ALIGN` bytes, and
   switch tables indexed by target address over the alignment (section 3.4, "The lookup array" and
   "Switches").
2. **Return addresses pushed on the stack.** `RETURN_ADDRESS = Stack { bytes: 8 }`: the emitter pushes in
   `call` and pops in `ret` after the target is known, a return moves sp by +8 relative to the call, the
   push is covered by the red-zone-extended frame and the `ret`'s read by the first frame check (section
   5.3). "Flags wanted" lives in bit 62.
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
  kept for a count of 0), `Multiply { high_nonzero, result }` (CF and OF), `Bits { .. }` for bt, bsf, bsr,
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
- **DF** is a flag only `std`, `cld` and `popf` set, which string instructions read.
- **The flags word** in the block is rflags with bits 1 and 9 (IF) always set, as user code sees them.
  `pushf` pushes it; `popf` writes the seven flags, ignores IOPL and IF as user mode does, and when it sets
  TF the instruction after it runs and then stops with `SingleStep` (SIGTRAP, `TRAP_TRACE`), as on the CPU;
  that path is a counted slow path ("trap flag set by the program").

### 5.2 What an instruction is

- `iced_x86::Instruction` is the instruction type, with its length. The meaning of each `Code` the CPU
  offers is written once, against the emitter, in `wcontainers-x86`, from the Intel manual and the
  reference CPU. Nothing of gex (AGPL) is copied or paraphrased.
- iced decodes with `DecoderOptions::NONE`, which follows Intel where Intel and AMD decode differently. An
  encoding iced calls `INVALID` traps `Undefined` (SIGILL). An instruction of a feature not offered traps
  `Unsupported` (SIGILL, counted, warned), with x86's own reason for AVX (section 5.10).
- Kernel-only instructions (hlt, in and out, cli and sti, lgdt and the rest, mov to control registers,
  rdmsr, wrmsr, rdpmc while Linux does not allow it, `int n` other than 3): the fault Linux gives a user
  program, which for most is general protection (SIGSEGV, SI_KERNEL, address 0) and for some #UD
  (SIGILL); layer 1 decides each against the reference.

### 5.3 Calls and returns

- `call` (direct, through a register, through memory) becomes a WebAssembly call to the target's piece,
  as on ARM64. The emitter writes the real return address (this instruction's address plus its length) at
  rsp - 8 and moves rsp, after the target is known and valid: a target that is not canonical stops with
  general protection before anything changes, as the reference does (section 2). `call [m]` reads its
  target first (access 0) and pushes second (access 1); both are checked before anything changes.
- `ret` reads [rsp]. If the value is the address the caller expects, rsp moves up by 8 and the piece
  returns directly. If not, `branch_target` checks it is canonical (else general protection with rsp not
  moved, as on the CPU), rsp moves, and the thread leaves to the bottom loop through "go to address".
  `ret imm16` also adds its immediate.
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
  anything changes. `pop [m]` computes m with rsp already moved, as the manual says.
- **Non-canonical data addresses** stop the thread at the permission table (they are above every page it
  covers). The fault reports the address; the CPU side reports a fault at a non-canonical address as
  general protection (SIGSEGV, SI_KERNEL, address 0), which is what the reference CPU gives (section 2).
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
- When a page's bytes are translated, the CPU side clears the page's write bits in the permission table
  (`PAGE_WRITE` of the page and `NEXT_PAGE_WRITE` of the page before) and remembers that it did. Translated
  code is unchanged: it already checks writes, so this costs nothing on the normal path.
- A write to such a page reaches the memory fault hook with exact registers. The CPU side sees the page is
  only write-protected for code, drops every piece translated from the page, sets the write bits back, and
  continues the thread at the faulting instruction, which writes again. Counter and warning: "code written
  without announcement", with the page.
- Pieces in progress on the WebAssembly stack may have come from that page, including the running one. So
  dropping pieces unwinds to the bottom loop (as after a lost stack); functions in progress continue from
  their return addresses, translated afresh ("continued after stack loss").
- The permission table changes only while the thread is stopped, which the fault guarantees, and with
  several threads the existing rule (stop the others first) applies.
- ARM64 keeps `ic ivau`; `CODE_CHANGES` tells the runner which policy to use.
- Expected and unmeasured: a JIT that keeps pages writable and executable while writing code next to
  running code would fault often. The counter shows it; a measurement on such a program comes at X6.

### 5.8 Floating point: SSE and x87

- **SSE through SSE4.2** maps onto WebAssembly's 128-bit vectors: packed and scalar float arithmetic,
  integer SIMD, shuffles, `pmin`/`pmax` (which are exactly `minps` and `maxps` with the operands swapped),
  conversions with x86's "integer indefinite" result (0x8000...) where WebAssembly saturates,
  `pcmpestri`-style string compares as sequences of vector operations, and `crc32` as ARM64 does its CRC
  instructions.
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
  LLVM-exception), counted "x87 floating point in software". Its state lives only in the block: the eight
  registers R0 to R7 in physical order, the control word, the status word with TOP, the tag word and the
  last-instruction pointers. Instructions that name ST(i) work out R((TOP + i) mod 8) at run time inside
  the hook, so no register numbering depends on run-time state. Loads and stores of memory operands are
  done by the meaning (checked, exact faults), and their values pass through the hook's i64 arguments and
  results. Precision control, rounding control and the unmasked-exception rule (the exception is raised at
  the next x87 instruction that waits) follow the manual. The transcendental instructions (fsin, fcos,
  fsincos, fptan, fpatan, f2xm1, fyl2x, fyl2xp1) are computed in extended precision in software; layer 1
  compares them bit for bit with the reference, and any form whose results Intel's undocumented algorithm
  makes impossible to match is recorded as a known failure with the reason.
- **MMX** (baseline in every x86-64 CPU, so offered): mm0 to mm7 are register bits of type i64, whose
  block slots are the mantissas of R0 to R7. An MMX instruction also sets TOP to 0 and marks every x87
  register valid; `emms` marks them empty. The x87 hook declares the MMX bits in its `registers`, so the
  emitter writes them back before an x87 instruction and loads them after.

### 5.9 The register block

`X86Block`, defined in `wcontainers-x86` until the kernel milestone moves it next to ARM64's in the abi:
16 general registers in hardware order, rip, rflags, FS base, GS base, a 512-byte area in the FXSAVE layout
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
  BMI1, ADX, MOVBE, RDTSCP, FSGSBASE, CLFLUSH (does nothing), and the 64-bit basics (SYSCALL, NX, LM). This
  covers the x86-64-v2 level.
- **Not claimed by default**: AVX, AVX2, FMA, F16C, BMI2 (its `pdep` and `pext` have no WebAssembly form),
  XSAVE and OSXSAVE (needed only to announce AVX state). **Never claimed**: AVX-512 and its relatives, AES,
  PCLMULQDQ, SHA, RDRAND, RDSEED, TSX, MPX, SGX, PKU, CET, AMX, XOP and 3DNow. An instruction of a feature
  not claimed raises illegal instruction, as on a CPU without it.
- **The AVX setting**: one container setting turns on the claim of the x86-64-v3 level: AVX, AVX2, FMA,
  F16C, BMI2, XSAVE and OSXSAVE (with `xgetbv` answering x87, SSE and AVX state). AVX and AVX2 are built in
  full, each 256-bit operation as two 128-bit ones; 128-bit VEX forms clear the upper half, legacy SSE
  forms keep it. FMA is fast where the host probe says the host fuses, else software with "software fused
  multiply-add" counted (section 2). `pdep` and `pext` are software loops, counted "software bit deposit or
  extract". The setting is given with each translation (an associated `Offer` type of the guest, `()` for
  ARM64) and is part of the cache key; the kernel answers `/proc/cpuinfo` and AT_HWCAP2 from the same value.
  Deciding the setting covers the whole v3 level rather than AVX alone is this plan's choice: programs
  built for x86-64-v3 check all of it.
- **The two counters**, x86's own reasons, each with a warning: "a program asked whether AVX is available
  and was told no" (CPUID leaf 1, or leaf 7 sub-leaf 0, read while the setting is off) and "an AVX
  instruction was reached while AVX is not offered" (a VEX-encoded instruction while the setting is off,
  which raises illegal instruction). No program is restarted automatically.
- **rdtsc and rdtscp** return the clock hook's nanoseconds (a 1 GHz counter, as leaf 0x15 says); `rdtscp`
  and `rdpid` give processor number 0.

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
  `RegisterBlock` stays; `X86Block` moves into the abi.
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
  `TRAP_TRACE`, int3 SIGTRAP `SI_KERNEL`). The abi's `Exit` grows these kinds.
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
dynasm, iced's encoder and assembler) never reach the CPU crates. It supplies:
- names: Rust target of the reference (`aarch64-unknown-linux-musl`, `x86_64-unknown-linux-musl`), zig and
  clang triple, ELF machine, Docker platform, musl loader name, the flags for "general registers only"
  (x86: `-mgeneral-regs-only`);
- the forms list (section 7.4), the offered features and the check they match what the guest is told
  (ARM64 HWCAP; x86 the CPUID answers), random encodings in no form, per-form operand shaping (which
  registers are memory bases or indexes, alignment the presented CPU needs, biases such as CPUID leaves),
  per-instance exclusions with reasons, and the designed-answers table;
- the comparable state: fields with names, widths and compared bits (x86: 16 general registers, rip,
  rflags arithmetic bits and DF, FS and GS bases, MXCSR, x87 words, R0 to R7, xmm and ymm upper halves),
  conversion to and from the block, value strategies (80-bit values, NaN-boxing for RISC-V);
- per-case masks of bits allowed to differ (x86: MXCSR sticky flags and x87 status exception bits while not
  kept; `TF` in the word `pushf` stores, which the reference's own stepping sets; never undefined flags);
- the mapping of reference signals (signal, `si_code`, address, rip) and of translator traps to generic
  stops and exit statuses (128 + 8 for SIGFPE);
- the reference runner's per-CPU part, the initial blob state, the auxiliary vector entries, the
  stand-in's five system call numbers (x86 write 1, writev 20, exit 60, exit_group 231, brk 12), the exact
  float hook body, the code-change policy, a disassembler for messages (iced's formatter for x86);
- per-CPU program support (system call stub, `_start`, context switch, assembly tests, the code-writing
  program, the differences program, smoke and checks programs), the layer 2 emitter, relocation types of
  real binaries (x86: `R_X86_64_RELATIVE`, `_64`, `GLOB_DAT`, `JUMP_SLOT`, `IRELATIVE`) and the images
  to take binaries from (`--platform linux/amd64`);
- where the reference is and how to reach it (section 7.3), and the reference CPU's vendor and model,
  recorded with every result.

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
  status (255) and signals are never mistaken for a guest's. A new connection costs about 344 ms and a
  process on the machine 0.4 ms (section 2), so connections are never made per case.
- **Layer 1, batched**: cases are a deterministic function of seed and form, so the harness sends a form's
  cases in one batch, collects the results and compares; only a failure goes back to proptest's shrinking,
  which then asks one case at a time. Up to 10 runner processes at once on the x86 machine.
- **Isolation**: the runner forks per batch of cases and the child runs the cases; a case that damages the
  child (a write through a random pointer into the runner's own memory) shows as a child that dies, and that
  batch is rerun one case per child.
- **One instruction** (section 2): the code area is filled with `int3`; the instruction is placed at CODE;
  the handler (on an alternate stack, using no thread-local storage and only raw system calls) writes
  every register into the frame including all vector registers and the XSAVE header bits, sets the FS and GS
  bases with `arch_prctl`, sets TF and returns. The next signal is read back: `SIGTRAP` with `TRAP_TRACE` is
  "landed at rip"; with `SI_KERNEL` it is `int3`; `SIGSEGV`, `SIGBUS`, `SIGILL`, `SIGFPE` are faults with
  their codes; `SIGSYS` (from a seccomp filter on the instruction window `[CODE, CODE + 16)`) is a system
  call. A repeated string instruction is stepped on while rip stays at CODE. The runner's own FS base is set
  back before anything else.
- **Blobs** (layers 2 and 3 natives): the runner forks per blob, maps it and runs it with the blob start
  state, and reports output and status in-band. Docker is not used on the x86 machine.
- **Recorded results**: every reference result is stored under `target/reference/x86_64/` keyed by the
  reference CPU's model string, the runner's content hash and the case's hash, so a replay or a run while
  the machine is off uses the real CPU's recorded answer. A case with no recorded answer and no machine
  fails.
- **ARM64** keeps its local Docker reference behind the same interface (the transport is chosen by the
  guest: local Docker when the host's CPU is the guest's, SSH otherwise).

### 7.4 The three layers for x86-64

**Layer 1: one instruction at a time.** The forms list comes from iced-x86's own tables: one form per
iced `Code` whose `OpCodeInfo` says it is valid in 64-bit mode (`mode64()`), is an instruction
(`is_instruction()`, not a directive), has an encoding that is not 3DNow, XOP, MVEX or EVEX
(`encoding()`), and whose every `Code::cpuid_features()` entry is in the offered set: one list for the
default claims and one for the AVX setting. Forms that `is_privileged()` marks kernel-only stay in the list
as their own group: their case passes when the translator stops with the fault the reference CPU gives
(section 5.2). The forms not offered are a third list, whose cases must stop with illegal instruction. Its name (the
`Code` name) is the seed key; its group is its mnemonic and encoding. A case is a random `Instruction` for
the `Code`, with operands from `op_kind(i)` (registers, immediates weighted toward edges, memory operands
whose base and index point into DATA), encoded by iced's `Encoder`, which gives the bytes and length. Random
registers, flags, vector, x87 and MXCSR values are weighted toward edge values; MXCSR is the default in
half the cases. The reference runs it by the trap flag; the translator runs it at look-ahead none; every
register, every flag (including undefined ones), the touched memory and the stop are compared. Random byte
strings that iced decodes as `INVALID` are compared with the CPU's SIGILL, and valid ones also check the
length (where the CPU lands). Exclusions, each with a reason: a jump to itself, and the instructions
answered by design (CPUID, rdtsc, rdtscp, rdpid, xgetbv), which a designed-answers test checks instead.

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
  crate name `Arm64` where they call the translator. Pass bar: the WebAssembly output is byte for byte the
  same as before the move for the benchmark set and all nine real binaries of M5, in both settings
  (`WC_BINARIES_DUMP` before and after, compared with `cmp`; the benchmark set's modules likewise); the
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
  instructions with what the Intel manual says, each checked by hand in the test.
- **X3, x86 at look-ahead none, integers.** `wcontainers-x86` with the decoder, the block, the base integer
  instructions (moves, arithmetic and logic at every width, lea, push and pop, call and ret, jmp and jcc,
  cmov and setcc, shifts and rotates, multiply and divide, string instructions without rep, syscall), lazy
  flags, the description section's guest byte (format 4; ARM64's output then differs in that byte only).
  Pass bar: layer 1 for those forms against the real CPU with zero mismatches at 1,000 cases a form; a
  C hello world built as a flat x86-64 blob prints and exits in wasmtime and Chrome; what the decoder adds
  to the browser build measured.
- **X4, x86 at full look-ahead.** Functions, registers in locals, flags liveness, direct conditions, calls
  and returns as WebAssembly calls with the pushed return address, the red zone frames, switch recognition
  for both x86 compiler styles, loop proofs, the stop and permission checks; the x86 layer 2 emitter. Pass
  bar: layer 2 at 10,000 programs with zero mismatches against the CPU and against none; the benchmark set
  gives the same results at none and full and is timed on the x86 machine natively, in wasmtime and in
  Chrome.
- **X5, every default instruction.** All integer instructions of the default claims, SSE through SSE4.2,
  MXCSR fast and exact paths, NaN rules and the probe, CPUID and its designed answers, rep string
  instructions (bulk and one element), FS and GS. The passed-register lists measured and chosen. Pass bar:
  layer 1 complete for the default forms at 1,000 cases a form, layer 3 Csmith at 1,000 programs, zero
  mismatches.
- **X6, bounded look-ahead, the rare paths and code written without announcement.** Translating from any
  byte address, the bottom loop, go to address, adding code while running, injection tests, write
  protection of translated pages with the unwind, both runners' stand-ins of it, a test program that writes
  and runs code with no announcement; setjmp, C++ exception and Rust panic programs. Pass bar: those tests
  pass at every look-ahead, in wasmtime and Chrome.
- **X7, x87 and MMX.** The x87 hook with `X87DoubleExtended`, MMX registers, `fxsave` and `fxrstor`. Pass
  bar: layer 1 for every x87 and MMX form at 1,000 cases a form, zero mismatches except known failures
  recorded with their reasons; a C program using `long double` gives the native output.
- **X8, the AVX setting.** AVX, AVX2, FMA, F16C, BMI2, XSAVE in the setting, the two counters, the setting
  in the cache key, VEX instructions trapping with the counter when it is off. Pass bar: layer 1 for every
  form of the AVX list at 1,000 cases a form, zero mismatches; the benchmark set built for x86-64-v3 runs
  with the setting on, and with it off stops with illegal instruction and the counter.
- **X9, real x86 code at scale.** The amd64 builds of the nine real binaries of M5 (busybox, the C library,
  ripgrep, git, Node, Python, Bun, a Java runtime, a Go program) translated in both settings and loaded in
  both engines. Recorded: sizes, translation time, engine memory, the lookup array's memory, how many
  switch tables were over the limit or had absolute entries.
- **X10, the kernel's x86-64 personality** (after the kernel track's milestones): section 6, and the
  kernel's tests for x86-64 against the x86 machine.

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
- The cost of the canonical-address check on mismatched returns and indirect branches.
- Bulk against element-by-element string instructions on real programs.
- How often code is written without announcement in real programs, and what JITs that write next to
  running code cost.
- The NaN fix-up's cost for x86 guests on ARM hosts.
- x87 in software against native on programs that use `long double`.
- The passed-register lists for x86.
- Engine compile time and memory for x86 output, which is expected to differ from ARM64's because x86
  instructions do more each.

## 13. Where the evidence is

Experiments and downloads of milestone X0 are in `pyro-parked/wcontainers-scratch/x86`: `iced-size` (the
size builds), `dis` (an iced-based disassembler used to check encodings), `reference-proof` (`hello.c`,
`stepper.c`, `cases.sh`, `cases2.sh` and their outputs), `sources` (the relaxed SIMD overview and
sandsifter's `injector.c`). The x86 machine holds copies in `~/wcontainers-x86/proof`.

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
- T14 `BOTTOM_RETURN` by Top Byte Ignore: 3.2 `UNREACHABLE_RETURN`; 3.4.
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
- T25 "plain" instruction variants, "all four flags set": 3.4 Flags.
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
- E6 `Case`: 7.2 comparable state; 4 decision 15.
- E7 branch-target values 4-aligned: 7.2 (from `G::ALIGN`); 4 decision 13.
- E8 vector and float value strategies: 7.2 value strategies.
- E9 FPCR and FPSR bits: 7.2 comparable state (MXCSR, x87 words); 5.8.
- E10 case shaping (sp, flags, bases into DATA, atomic alignment): 7.2 operand shaping; 7.4.
- E11 signal to outcome mapping: 7.2 mapping of signals; 7.3; 4 decision 16.
- E12 conversion to `RegisterBlock`: 7.2 conversion to and from the block.
- E13 one 4-byte slot and the case word: 3.4 lookup array (slots per `ALIGN`); 7.2; 7.4 (length from the
  encoder).
- E14 unknown after a faulting access: 7.2 per-case masks (x86 is exact; rep stops exact, section 5.6).
- E15 compare masks: 7.2 per-case masks; 4 decision 16.
- E16 report names: 7.2 comparable state (field names).
- E17 BLOB_HWCAP and the auxiliary vector: 7.2 auxiliary vector entries; 6.
- E18 zero is `udf #0`: 2 and 7.3 (trap flag); 4 decision 14.
- E19 request and result sizes: 7.2 comparable state; 7.3 (length-prefixed records); 4 decision 15.
- E20 the reference over local Docker: 7.3; 4 decision 12.
- E21 the reference's target and link base: 7.3 (x86_64-unknown-linux-musl, rust-lld; the toolchain file).
- E22 frame access in the reference: 2 (experiment), 7.3.
- E23 the seccomp instruction window: 7.3 (`[CODE, CODE + 16)`).
- E24 making written code visible: 7.2 code-change policy (nothing on x86).
- E25 placing the instruction: 7.3.
- E26 blob start state: 3.2 `initial_block`; 7.2.
- E27 the float hook body: 7.2 exact float hook; 5.8.
- E28 trap kinds in the runner: 3.2 `TrapKind` growth; 7.2 mapping to statuses.
- E29 code changed by `ic ivau`: 5.7; 7.2 code-change policy; 4 decision 17.
- E30 the runner's 4-byte slots: 3.4 The lookup array.
- E31 the stand-in's system call numbers: 7.2.
- E32 set_registers and float tracking: 7.2 conversion; 5.8.
- E33 zig, rustc targets and general-registers-only flags: 7.2 names.
- E34 slots and registers for a blob: 3.4; 3.2 `initial_block`.
- E35 statuses and the disassembler in messages: 7.2.
- E36 natives over local Docker and RUNNER_FAILED: 7.3 (in-band statuses, no Docker on the x86 machine).
- E37 flat.ld: 7.2 per-CPU program support (x86 needs nothing new; RISC-V small data, section 3.5 is
  paper only).
- E38 inline `svc`, `_start`, context switch, assembly tests: 7.2 per-CPU program support; 4 decision 21.
- E39 the written-code program: 7.4 (code written without announcement); X6; 4 decision 17.
- E40 the differences program: 7.2; 5.8 (x86's differences: integer indefinite, NaN rules, minps order,
  MXCSR modes, divide error, x87).
- E41 layer 2 with dynasm aarch64: 7.4 layer 2; 4 decision 18.
- E42 layer 3's gcc image, clang target, native runs: 7.4 layer 3.
- E43 frames from `registers.sp`: 7.2 comparable state (the stack pointer from `SP_BIT`).
- E44 the changes hand programs: 7.2 per-CPU program support (smoke, checks and changes programs).
- E45 random starts at 4 * k: 4 decision 13.
- E46 smoke tests: 7.2 per-CPU program support.
- E47 checks programs: 7.2 per-CPU program support; X4 and X6 add x86's push, pop, rep and read-change-write
  cases.
- E48 offered features against HWCAP: 7.2 offered features and the check against CPUID answers.
- E49 targeted layer 1 cases, words in no form, designed answers: 7.2; 7.4.
- E50 BLOB_HWCAP against the abi: 7.2 auxiliary vector entries.
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
