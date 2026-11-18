//! A guest CPU made only for this test, shaped like x86-64 where ARM64 differs: byte-aligned instructions of
//! 1 to 3 bytes, the return address written on the stack, a red zone, different registers passed in and
//! out, a flag setter that keeps a flag, a segment base, a trap that leaves pc after it, two accesses in one
//! instruction, a run-time length checked, its own slow path, a hook that works on registers, and a stop
//! raised by an instruction. `Test<false>` is the same CPU without flags, shaped like RISC-V: its
//! conditional jumps compare a register with zero. Their translations at every look-ahead must be valid
//! WebAssembly, and each program run in wasmtime must end with the registers, pc, memory and hook calls
//! its meanings give.

use std::ops::Range;
use std::sync::OnceLock;
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, FunctionSection, InstructionSink, MemArg, MemArgTarget,
    TypeSection, ValType,
};
use wasmparser::{Validator, WasmFeatures};
use wasmtime::{
    AsContextMut, Caller, Config, Engine, Extern, Global, GlobalType, Instance, Linker, Memory,
    MemoryType, Module, Mutability, SharedMemory, Store, Val,
};
use wcontainers_abi::{PAGE_SIZE, Permissions};
use wcontainers_guest_cpu::{
    Access, AccessBytes, AccessKind, Base, Bit, Class, CodeChanges, Comparison, Effects, Emitter,
    Flow, FlowKind, GUEST_REASONS, Guest, GuestId, HookBody, HookDone, HookType, Induction, Locals,
    MemoryAccess, MemoryFault, Personality, Reason as Runtime, Registers, ReturnAddress, StackMove,
    StateBytes, SyscallRegisters, Target, Through, TrapDetail, TrapKind, TrapPc, Ty, Value,
    WriteBack,
};
use wcontainers_translator::contract::{self, permission_byte};
use wcontainers_translator::{LookAhead, Optimise, Sites, Translation, thread_module, translate};

/// The test CPU, with flags or without.
#[derive(Clone, Copy, Debug)]
struct Test<const FLAGGED: bool>;

const SP: u8 = 4;
const FS: u8 = 22;
const MODE: usize = 112;
const PC: usize = 64;
const FLAGS_WORD: usize = 72;
const STOP_FLAG: usize = 128;
const STATE_SIZE: usize = 160;

impl<const F: bool> Personality for Test<F> {
    type Block = [u8; 128];
    const PC_OFFSET: usize = PC;
    const STOP_FLAG_OFFSET: usize = STOP_FLAG;
    const BLOCK_SIZE: usize = 144;
    const PRIVATE_BYTES: usize = 16;

    fn initial_block(pc: u64, sp: u64) -> [u8; 128] {
        let mut block = [0; 128];
        block[PC..PC + 8].copy_from_slice(&pc.to_le_bytes());
        block[8 * SP as usize..8 * SP as usize + 8].copy_from_slice(&sp.to_le_bytes());
        block
    }
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Nop,
    Add(u8, i8),
    /// Adds 1 and sets Z, keeping C.
    Inc(u8),
    /// With flags on Z; without, when r0 is zero.
    JumpIfZero(i8),
    /// With flags on C; without, when r1 is zero.
    JumpIfCarry(i8),
    Call(i16),
    Ret,
    Load(u8, i8),
    Store(u8, i8),
    Push(u8),
    Syscall,
    Break,
    JumpThrough(u8),
    /// Whether r1 bytes from r2 may be written, into r0.
    Range,
    Slow,
    /// Raises the stop flag when r0 is not zero.
    RaiseStop,
    /// r0 from the hook, which reads r1 and writes r7 in the block.
    Hook,
    /// r, fs:[base + d], with no base when it is `None`.
    LoadFs(u8, Option<u8>, i8),
    /// [r2] from [r1], two accesses.
    Move,
    ZeroVector,
    Undefined,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flag {
    Z,
    C,
}

#[derive(Clone, Copy, Debug)]
enum Flags {
    Add { a: u32, b: u32 },
    Inc { z: u32, c: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Shape {
    Add,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Control {
    Mode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hook {
    Work,
}

#[derive(Clone, Copy, Debug)]
enum Trap {
    Undefined,
    Break,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reason {
    Slow,
}

const fn general(bit: u32) -> Bit {
    Bit {
        bit,
        class: Class::General,
        ty: Ty::I64,
        offset: 8 * bit as u16,
    }
}

const fn flag(bit: u32) -> Bit {
    Bit {
        bit,
        class: Class::Flag,
        ty: Ty::I32,
        offset: FLAGS_WORD as u16,
    }
}

const SEGMENT: Bit = Bit {
    bit: FS as u32,
    class: Class::Segment,
    ty: Ty::I64,
    offset: 80,
};

const VECTOR: Bit = Bit {
    bit: 64,
    class: Class::Vector,
    ty: Ty::V128,
    offset: 96,
};

const BITS: [Bit; 12] = [
    general(0),
    general(1),
    general(2),
    general(3),
    general(4),
    general(5),
    general(6),
    general(7),
    flag(16),
    flag(17),
    SEGMENT,
    VECTOR,
];

const BITS_WITHOUT_FLAGS: [Bit; 10] = [
    general(0),
    general(1),
    general(2),
    general(3),
    general(4),
    general(5),
    general(6),
    general(7),
    SEGMENT,
    VECTOR,
];

fn memory() -> MemArg {
    MemArg {
        offset: 0,
        align: 0,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }
}

fn length(op: Op) -> u8 {
    use Op::*;
    match op {
        Nop | Ret | Syscall | Break | Range | Slow | RaiseStop | Hook | Move | ZeroVector
        | Undefined => 1,
        Inc(_) | JumpIfZero(_) | JumpIfCarry(_) | Push(_) | JumpThrough(_) => 2,
        Add(..) | Call(_) | Load(..) | Store(..) | LoadFs(..) => 3,
    }
}

/// The access through the stack pointer `displacement` bytes from it.
fn stack_access(displacement: i64, kind: AccessKind) -> MemoryAccess<u8> {
    MemoryAccess {
        segment: None,
        base: Some(Base::Register(SP)),
        index: None,
        displacement,
        address_bits: 64,
        write_back: WriteBack::None,
        bytes: AccessBytes::Fixed(8),
        kind,
    }
}

/// Push the guest address `d` bytes from register `base` (if any), from the segment base when `segment`,
/// into a fresh local.
fn address<const F: bool, E: Emitter<Test<F>> + ?Sized>(
    e: &mut E,
    base: Option<u8>,
    d: i64,
    segment: bool,
) -> u32 {
    let at = e.local(ValType::I64);
    e.sink().i64_const(d);
    if let Some(r) = base {
        e.get(r);
        e.sink().i64_add();
    }
    if segment {
        e.get(FS);
        e.sink().i64_add();
    }
    e.sink().local_set(at);
    at
}

fn emit_op<const F: bool, E: Emitter<Test<F>> + ?Sized>(e: &mut E, op: Op) {
    match op {
        Op::Nop => {}
        Op::Add(r, imm) => {
            let (a, b) = (e.local(ValType::I64), e.local(ValType::I64));
            e.get(r);
            e.sink()
                .local_set(a)
                .i64_const(imm.into())
                .local_set(b)
                .local_get(a)
                .local_get(b)
                .i64_add();
            e.set(r);
            if F {
                e.set_flags(Flags::Add { a, b });
            }
        }
        Op::Inc(r) => {
            let value = e.local(ValType::I64);
            let c = F.then(|| {
                let c = e.local(ValType::I32);
                e.flag(Flag::C);
                e.sink().local_set(c);
                c
            });
            e.get(r);
            e.sink().i64_const(1).i64_add().local_tee(value);
            e.set(r);
            if let Some(c) = c {
                let z = e.local(ValType::I32);
                e.sink().local_get(value).i64_eqz().local_set(z);
                e.set_flags(Flags::Inc { z, c });
            }
        }
        Op::JumpIfZero(by) | Op::JumpIfCarry(by) => {
            let zero = matches!(op, Op::JumpIfZero(_));
            if F {
                e.condition(if zero { Flag::Z } else { Flag::C });
            } else {
                e.get(if zero { 0 } else { 1 });
                e.sink().i64_eqz();
            }
            e.jump_if(Target::Relative(by.into()));
        }
        Op::Call(by) => {
            // The CPU's own steps: the push, then the stack pointer, then the call.
            let at = address(e, Some(SP), -8, false);
            e.check(at, 8, Access::Write, 0);
            e.sink().local_get(at);
            e.this_address(length(op).into());
            e.sink().i64_store(memory()).local_get(at);
            e.set(SP);
            e.call(Target::Relative(by.into()));
        }
        Op::Ret => {
            let at = address(e, Some(SP), 0, false);
            e.check(at, 8, Access::Read, 0);
            let target = e.local(ValType::I64);
            e.sink().local_get(at).i64_load(memory()).local_set(target);
            e.ret(target);
            return;
        }
        Op::Load(r, d) => {
            let at = address(e, Some(SP), d.into(), false);
            e.check(at, 8, Access::Read, 0);
            e.sink().local_get(at).i64_load(memory());
            e.set(r);
        }
        Op::LoadFs(r, base, d) => {
            let at = address(e, base, d.into(), true);
            e.check(at, 8, Access::Read, 0);
            e.sink().local_get(at).i64_load(memory());
            e.set(r);
        }
        Op::Store(r, d) => {
            let at = address(e, Some(SP), d.into(), false);
            e.check(at, 8, Access::Write, 0);
            e.sink().local_get(at);
            e.get(r);
            e.sink().i64_store(memory());
        }
        Op::Push(r) => {
            let at = address(e, Some(SP), -8, false);
            e.check(at, 8, Access::Write, 0);
            e.sink().local_get(at);
            e.get(r);
            e.sink().i64_store(memory()).local_get(at);
            e.set(SP);
        }
        Op::Syscall => e.syscall(),
        Op::Break => {
            e.trap(Trap::Break);
            return;
        }
        Op::JumpThrough(r) => {
            let target = e.local(ValType::I64);
            e.get(r);
            e.sink().local_set(target);
            e.jump(Target::Address(target));
            return;
        }
        Op::Range => {
            let (at, bytes) = (e.local(ValType::I64), e.local(ValType::I64));
            e.get(2);
            e.sink().local_set(at);
            e.get(1);
            e.sink().local_set(bytes);
            let allowed = e.check_range(at, bytes, Access::Write);
            e.sink().local_get(allowed).i64_extend_i32_u();
            e.set(0);
        }
        Op::Slow => e.warn(Reason::Slow),
        Op::RaiseStop => {
            e.get(0);
            e.sink().i64_const(0).i64_ne();
            e.raise_stop();
        }
        Op::Hook => {
            e.get(1);
            e.hook(Hook::Work);
            e.set(0);
        }
        Op::Move => {
            let from = address(e, Some(1), 0, false);
            let to = address(e, Some(2), 0, false);
            let value = e.local(ValType::I64);
            e.check(from, 8, Access::Read, 0);
            e.sink().local_get(from).i64_load(memory()).local_set(value);
            e.check(to, 8, Access::Write, 1);
            e.sink().local_get(to).local_get(value).i64_store(memory());
        }
        Op::ZeroVector => {
            if e.float_mode() {
                e.sink().drop();
            }
            e.sink().v128_const(0);
            e.set_vector(0);
        }
        Op::Undefined => {
            e.trap(Trap::Undefined);
            return;
        }
    }
    e.next();
}

impl<const F: bool> Guest for Test<F> {
    const ID: GuestId = GuestId::X86_64;
    const ALIGN: u64 = 1;
    const MAX_LENGTH: u64 = 3;
    type Instruction = Op;

    fn decode(code: &[u8]) -> Option<(Op, u8)> {
        let byte = |i: usize| code.get(i).copied();
        let op = match *code.first()? {
            0x00 => Op::Nop,
            0x01 => Op::Add(byte(1)? & 7, byte(2)? as i8),
            0x02 => Op::Inc(byte(1)? & 7),
            0x03 => Op::JumpIfZero(byte(1)? as i8),
            0x04 => Op::JumpIfCarry(byte(1)? as i8),
            0x05 => Op::Call(i16::from_le_bytes([byte(1)?, byte(2)?])),
            0x06 => Op::Ret,
            0x07 => Op::Load(byte(1)? & 7, byte(2)? as i8),
            0x08 => Op::Store(byte(1)? & 7, byte(2)? as i8),
            0x09 => Op::Push(byte(1)? & 7),
            0x0a => Op::Syscall,
            0x0b => Op::Break,
            0x0c => Op::JumpThrough(byte(1)? & 7),
            0x0d => Op::Range,
            0x0e => Op::Slow,
            0x0f => Op::RaiseStop,
            0x10 => Op::Hook,
            // The second byte: the register in bits 0 to 2, the base in bits 3 to 5, no base with bit 7.
            0x11 => {
                let x = byte(1)?;
                let base = (x & 0x80 == 0).then_some(x >> 3 & 7);
                Op::LoadFs(x & 7, base, byte(2)? as i8)
            }
            0x12 => Op::Move,
            0x13 => Op::ZeroVector,
            _ => Op::Undefined,
        };
        Some((op, length(op)))
    }

    fn trap_detail(code: &[u8]) -> i64 {
        let mut bytes = [0; 8];
        let n = code.len().min(8);
        bytes[..n].copy_from_slice(&code[..n]);
        i64::from_le_bytes(bytes)
    }

    type Reg = u8;
    type Vector = u8;
    const BITS: &'static [Bit] = if F { &BITS } else { &BITS_WITHOUT_FLAGS };

    fn reg_bit(r: u8) -> Option<u32> {
        Some(r.into())
    }

    fn vector_bit(_v: u8) -> u32 {
        64
    }

    fn bit_reg(bit: u32) -> u8 {
        bit as u8
    }

    const SP_BIT: u32 = SP as u32;
    const PASSED_IN: &'static [u32] = &[7, 6, 4];
    const PASSED_OUT: &'static [u32] = &[0, 4];

    type Flag = Flag;
    const FLAGS: &'static [(Flag, u32, u32)] = if F {
        &[(Flag::Z, 6, 16), (Flag::C, 0, 17)]
    } else {
        &[]
    };
    const FLAGS_WORD_OFFSET: usize = FLAGS_WORD;
    type Flags = Flags;
    type Shape = Shape;
    const SHAPES: &'static [Shape] = if F { &[Shape::Add] } else { &[] };

    fn shape(flags: Flags) -> Option<Shape> {
        match flags {
            Flags::Add { .. } => Some(Shape::Add),
            Flags::Inc { .. } => None,
        }
    }

    fn shape_locals(_shape: Shape) -> &'static [ValType] {
        &[ValType::I64, ValType::I64]
    }

    fn with_locals(_shape: Shape, locals: &[u32]) -> Flags {
        Flags::Add {
            a: locals[0],
            b: locals[1],
        }
    }

    fn locals_of(flags: Flags) -> Locals {
        match flags {
            Flags::Add { a, b } => Locals::new(&[a, b]),
            Flags::Inc { z, c } => Locals::new(&[z, c]),
        }
    }

    fn temporaries(_flags: Flags) -> Option<ValType> {
        None
    }

    fn complete(_sink: &mut InstructionSink<'_>, flags: Flags, _temporaries: [u32; 2]) -> Flags {
        flags
    }

    fn sets(flags: Flags) -> Registers {
        match flags {
            Flags::Add { .. } => Registers::of_bits(&[16, 17]),
            Flags::Inc { .. } => Registers::of_bits(&[16]),
        }
    }

    fn flag_value(sink: &mut InstructionSink<'_>, flags: Flags, flag: Flag) {
        match (flags, flag) {
            (Flags::Add { a, b }, Flag::Z) => {
                sink.local_get(a).local_get(b).i64_add().i64_eqz();
            }
            (Flags::Add { a, b }, Flag::C) => {
                sink.local_get(a)
                    .local_get(b)
                    .i64_add()
                    .local_get(a)
                    .i64_lt_u();
            }
            (Flags::Inc { z, .. }, Flag::Z) => {
                sink.local_get(z);
            }
            (Flags::Inc { c, .. }, Flag::C) => {
                sink.local_get(c);
            }
        }
    }

    type Cond = Flag;

    fn condition_direct(sink: &mut InstructionSink<'_>, flags: Flags, cond: Flag) -> bool {
        match flags {
            Flags::Add { .. } => {
                Self::flag_value(sink, flags, cond);
                true
            }
            Flags::Inc { .. } => false,
        }
    }

    fn condition_from_flags<E: Emitter<Self> + ?Sized>(e: &mut E, cond: Flag) {
        e.flag(cond);
    }

    const FLAGS_WANTED: i64 = if F { 1 << 62 } else { 0 };

    type Control = Control;

    fn control_ty(_c: Control) -> ValType {
        ValType::I32
    }

    fn control_offset(_c: Control) -> usize {
        MODE
    }

    fn float_slow<E: Emitter<Self> + ?Sized>(e: &mut E) {
        e.get_control(Control::Mode);
    }

    const FLOAT_CONTROLS: &'static [Control] = &[Control::Mode];

    type Hook = Hook;
    const HOOKS: &'static [HookType] = &[HookType {
        name: "work",
        params: &[ValType::I64],
        results: &[ValType::I64],
        registers: Registers(1 << 1 | 1 << 7),
        state: StateBytes {
            at: 0,
            len: MODE as u16 + 4,
        },
        body: HookBody::Cpu,
    }];

    fn hook_index(_h: Hook) -> usize {
        0
    }

    /// r7 becomes twice r1 as the block holds it, and the result is the argument plus one.
    fn run_hook(_hook: usize, args: &[i64], state: &mut [u8]) -> HookDone {
        let r1 = u64::from_le_bytes(state[8..16].try_into().unwrap());
        state[56..64].copy_from_slice(&(2 * r1).to_le_bytes());
        HookDone {
            results: [args[0] + 1, 0],
            reason: None,
        }
    }

    type Trap = Trap;

    fn trap_kind(t: Trap) -> (TrapKind, TrapDetail, TrapPc) {
        match t {
            Trap::Undefined => (TrapKind::Undefined, TrapDetail::Instruction, TrapPc::At),
            Trap::Break => (TrapKind::Breakpoint, TrapDetail::Const(0), TrapPc::After),
        }
    }

    type Reason = Reason;
    const REASONS: &'static [(Reason, &'static str)] =
        &[(Reason::Slow, "the test guest's slow path")];

    type Host = ();

    fn probe_module() -> Vec<u8> {
        let mut module = wasm_encoder::Module::new();
        let mut types = TypeSection::new();
        types.ty().function([], [ValType::I32]);
        let mut functions = FunctionSection::new();
        functions.function(0);
        let mut exports = ExportSection::new();
        exports.export("probe", ExportKind::Func, 0);
        let mut code = CodeSection::new();
        let mut f = wasm_encoder::Function::new([]);
        f.instructions().i32_const(0).end();
        code.function(&f);
        module
            .section(&types)
            .section(&functions)
            .section(&exports)
            .section(&code);
        module.finish()
    }

    fn host_from_probe(_answer: i32) {}

    fn host_key(_host: ()) -> u32 {
        0
    }

    type Offer = bool;

    fn offer_key(offer: bool) -> u32 {
        offer.into()
    }

    fn memory_fault(_address: u64, _code: &[u8], _block: &[u8; 128]) -> MemoryFault {
        MemoryFault::Page
    }

    fn emit<E: Emitter<Self> + ?Sized>(e: &mut E, i: &Op) {
        emit_op(e, *i);
    }

    fn flow(i: &Op) -> Flow<u8> {
        let (falls_through, target, kind) = match *i {
            Op::JumpIfZero(by) | Op::JumpIfCarry(by) => (true, Some(by.into()), FlowKind::Plain),
            Op::Call(by) => (true, Some(by.into()), FlowKind::Call),
            Op::Ret => (false, None, FlowKind::Return),
            Op::JumpThrough(r) => (false, None, FlowKind::JumpThrough(Through::Register(r))),
            Op::Break | Op::Undefined => (false, None, FlowKind::Plain),
            _ => (true, None, FlowKind::Plain),
        };
        Flow {
            falls_through,
            target,
            kind,
        }
    }

    fn accesses(i: &Op) -> [Option<MemoryAccess<u8>>; 2] {
        let register = |r: Option<u8>, kind| MemoryAccess {
            base: r.map(Base::Register),
            displacement: 0,
            ..stack_access(0, kind)
        };
        match *i {
            Op::Call(_) | Op::Push(_) => [Some(stack_access(-8, AccessKind::Write)), None],
            Op::Ret => [Some(stack_access(0, AccessKind::Read)), None],
            Op::Load(_, d) => [Some(stack_access(d.into(), AccessKind::Read)), None],
            Op::Store(_, d) => [Some(stack_access(d.into(), AccessKind::Write)), None],
            Op::LoadFs(_, base, d) => [
                Some(MemoryAccess {
                    segment: Some(FS),
                    displacement: d.into(),
                    ..register(base, AccessKind::Read)
                }),
                None,
            ],
            Op::Move => [
                Some(register(Some(1), AccessKind::Read)),
                Some(register(Some(2), AccessKind::Write)),
            ],
            Op::Range => [
                Some(MemoryAccess {
                    bytes: AccessBytes::Counted { element: 1 },
                    ..register(Some(2), AccessKind::Write)
                }),
                None,
            ],
            _ => [None, None],
        }
    }

    fn stack_move(i: &Op) -> StackMove {
        match i {
            Op::Call(_) | Op::Push(_) => StackMove::By {
                delta: -8,
                before_access: true,
            },
            _ => StackMove::None,
        }
    }

    fn built_address(
        _at: u64,
        _i: &Op,
        _blob: &[u8],
        _code: Range<u64>,
        _before: impl Fn(u64) -> Option<(u64, Op)>,
    ) -> Option<u64> {
        None
    }

    /// A jump through r after `add r, k`, as a table of the next 4 instructions' offsets: only to walk back.
    fn jump_table(
        at: u64,
        reg: u8,
        _blob: &[u8],
        code: Range<u64>,
        before: impl Fn(u64) -> Option<(u64, Op)>,
    ) -> Option<Vec<u64>> {
        match before(at)? {
            (_, Op::Add(r, _)) if r == reg => {
                let targets: Vec<u64> = (1..=4).map(|k| at + 2 * k).collect();
                targets.iter().all(|t| code.contains(t)).then_some(targets)
            }
            _ => None,
        }
    }

    fn plain(i: &Op) -> bool {
        matches!(i, Op::Nop | Op::Add(..) | Op::Inc(_))
    }

    const RETURN_ADDRESS: ReturnAddress = ReturnAddress::Stack { bytes: 8 };
    const RED_ZONE: u64 = 128;
    const SP_ALIGNMENT_CHECKED: bool = false;

    fn data_address(sink: &mut InstructionSink<'_>, local: u32) {
        sink.local_get(local);
    }

    fn branch_target<E: Emitter<Self> + ?Sized>(e: &mut E, local: u32) {
        e.sink().local_get(local);
    }

    const UNREACHABLE_RETURN: i64 = if F { i64::MIN } else { 1 };
    const SYSCALL: SyscallRegisters<u8> = SyscallRegisters {
        number: 0,
        args: [7, 6, 2, 3, 5, 1],
        result: 0,
    };
    const CODE_CHANGES: CodeChanges = CodeChanges::Detected;

    fn induction(i: &Op) -> Option<Induction<u8>> {
        let (reg, step) = match *i {
            Op::Add(r, k) => (r, k.into()),
            Op::Inc(r) => (r, 1),
            _ => return None,
        };
        Some(Induction {
            reg,
            bits: 64,
            step: Value::Imm(step),
            negate: false,
        })
    }

    fn branch_comparison(
        _path: &[(u64, Op, Effects<Self>)],
        _branch: (u64, &Op),
    ) -> Option<Comparison<u8>> {
        None
    }

    fn loaded_max(_i: &Op) -> Option<(u8, i64)> {
        None
    }
}

fn validate(wasm: &[u8]) {
    let features = WasmFeatures::default()
        | WasmFeatures::TAIL_CALL
        | WasmFeatures::FUNCTION_REFERENCES
        | WasmFeatures::EXCEPTIONS
        | WasmFeatures::THREADS
        | WasmFeatures::MEMORY64
        | WasmFeatures::MULTI_MEMORY
        | WasmFeatures::EXTENDED_CONST
        | WasmFeatures::RELAXED_SIMD;
    if let Err(e) = Validator::new_with_features(features).validate_all(wasm) {
        panic!("invalid: {e}");
    }
}

/// The comparison's look-aheads, and bounded 5, which cuts these small programs part way.
const LOOK_AHEADS: [LookAhead; 8] = [
    LookAhead::None,
    LookAhead::Bounded(1),
    LookAhead::Bounded(5),
    LookAhead::Bounded(17),
    LookAhead::Bounded(64),
    LookAhead::Bounded(256),
    LookAhead::Bounded(4096),
    LookAhead::Full,
];

/// Where the called function starts.
const FUNCTION: usize = 64;

/// A program of every instruction, with a function it calls, a loop, a switch and a jump into the middle of
/// an instruction.
fn program() -> Vec<u8> {
    let call_at = 26;
    let by = (FUNCTION - call_at) as u8;
    let mut code = vec![
        0x01, 0, 1, // 0: add r0, 1
        0x02, 3, // 3: inc r3
        0x03, 2,    // 5: jz +2
        0x13, // 7: zero the vector
        0x00, // 8: nop
        0x09, 5, // 9: push r5
        0x08, 5, 0xf8, // 11: store [sp - 8], r5
        0x07, 6, 0, // 14: load r6, [sp]
        0x11, 0o20, 16,   // 17: load r0, fs:[r2 + 16]
        0x12, // 20: move [r2] from [r1]
        0x0d, // 21: range
        0x0e, // 22: slow
        0x0f, // 23: raise stop
        0x10, // 24: hook
        0x0a, // 25: syscall
        0x05, by, 0, // 26: call the function
        0x04, 0xfe, // 29: jc into the call's displacement
        0x02, 1, // 31: inc r1
        0x01, 1, 0xff, // 33: add r1, -1
        0x03, 0xfb, // 36: jz back to the inc, a loop
        0x01, 4, 2, // 38: add r4, 2
        0x0c, 4, // 41: jump through r4, a switch with cases at 43, 45, 47, 49
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 43
        0x0b, // 50: break
    ];
    assert_eq!(code[call_at], 0x05);
    code.resize(FUNCTION, 0x00);
    // The function: through the red zone and back.
    code.extend([0x08, 3, 0xf0, 0x07, 3, 0xf0, 0x06, 0xff]);
    code
}

fn every_translation_valid<const F: bool>() {
    let blob = program();
    let starts = [0, FUNCTION as u64];
    validate(&thread_module::<Test<F>>());
    for look_ahead in LOOK_AHEADS {
        for optimise in [Optimise::Speed, Optimise::Size] {
            for offer in [false, true] {
                let translation = translate::<Test<F>>(
                    &blob,
                    0..blob.len() as u64,
                    &starts,
                    look_ahead,
                    optimise,
                    (),
                    offer,
                )
                .unwrap();
                assert!(!translation.modules.is_empty());
                for module in &translation.modules {
                    validate(&module.wasm);
                }
            }
        }
    }
}

#[test]
fn a_cpu_shaped_like_x86_64_fills_in_the_interface() {
    every_translation_valid::<true>();
}

#[test]
fn a_cpu_without_flags_fills_in_the_interface() {
    every_translation_valid::<false>();
}

#[test]
fn the_test_guest_decodes_its_program() {
    let blob = program();
    let mut at = 0;
    let mut count = 0;
    while let Some((_, n)) = Test::<true>::decode(&blob[at..]) {
        at += usize::from(n);
        count += 1;
        if at >= blob.len() {
            break;
        }
    }
    assert!(count > 20);
}

// A small runner for the test guest: one thread, the program's pieces placed before it runs, code reached
// later translated at look-ahead none, and every hook call recorded.

/// Where the program is placed.
const LOAD: u64 = 0x10000;
/// Readable and writable data, with nothing allowed in the page after it.
const DATA: u64 = 0x40000;
/// The stack pointer at the start, the top of readable and writable pages from 0x70000.
const STACK: u64 = 0x80000;
const MEMORY_BYTES: u64 = 0x100000;
const BLOCK_AT: usize = 0;
const PERMISSIONS_AT: usize = 4096;
const SLOTS_AT: usize = 8192;

/// A hook call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Syscall { args: [i64; 7], pc: u64 },
    Stop { pc: u64, r0: u64 },
    Fault { address: u64, write: bool, pc: u64 },
    Trap { kind: i32, detail: i64, pc: u64 },
    Warn { reason: i32, detail: i64 },
    NoCode(u64),
}

struct Host {
    state: SharedMemory,
    memory: Option<Memory>,
    thread: Option<Instance>,
    blob: Vec<u8>,
    optimise: Optimise,
    next_slot: u32,
    events: Vec<Event>,
}

/// How a run ended.
struct Ran {
    block: Vec<u8>,
    memory: Vec<u8>,
    events: Vec<Event>,
    sites: Sites,
}

impl Ran {
    fn reg(&self, r: u8) -> u64 {
        let at = 8 * usize::from(r);
        u64::from_le_bytes(self.block[at..at + 8].try_into().unwrap())
    }

    fn pc(&self) -> u64 {
        u64::from_le_bytes(self.block[PC..PC + 8].try_into().unwrap())
    }

    fn flags(&self) -> u32 {
        u32::from_le_bytes(self.block[FLAGS_WORD..FLAGS_WORD + 4].try_into().unwrap())
    }

    fn word(&self, address: u64) -> u64 {
        let at = address as usize;
        u64::from_le_bytes(self.memory[at..at + 8].try_into().unwrap())
    }

    /// The last hook call, which ends the run.
    fn last(&self) -> Event {
        *self.events.last().expect("a hook call ends the run")
    }

    /// The guest's own slow paths warned.
    fn slow(&self) -> Vec<Event> {
        self.events
            .iter()
            .copied()
            .filter(|e| matches!(e, Event::Warn { reason, .. } if *reason as u32 >= GUEST_REASONS))
            .collect()
    }
}

/// What a run starts from: registers (the stack pointer is `STACK` unless given), the FS base and words of
/// memory.
#[derive(Clone, Default)]
struct Start {
    registers: Vec<(u8, u64)>,
    fs: u64,
    memory: Vec<(u64, u64)>,
}

fn engine() -> Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            let mut config = Config::new();
            config
                .wasm_tail_call(true)
                .wasm_function_references(true)
                .wasm_gc(true)
                .wasm_exceptions(true)
                .wasm_threads(true)
                .wasm_memory64(true)
                .wasm_multi_memory(true)
                .shared_memory(true)
                .memory_may_move(false);
            Engine::new(&config).expect("engine")
        })
        .clone()
}

fn write_shared(memory: &SharedMemory, at: usize, bytes: &[u8]) {
    for (cell, &byte) in memory.data()[at..at + bytes.len()].iter().zip(bytes) {
        // SAFETY: the guest thread is not running, so nothing else touches the byte.
        unsafe { *cell.get() = byte };
    }
}

fn read_shared(memory: &SharedMemory, at: usize, into: &mut [u8]) {
    for (byte, cell) in into.iter_mut().zip(&memory.data()[at..]) {
        // SAFETY: as for `write_shared`.
        *byte = unsafe { *cell.get() };
    }
}

fn block(caller: &Caller<'_, Host>) -> Vec<u8> {
    let mut bytes = vec![0; STATE_SIZE];
    read_shared(&caller.data().state, BLOCK_AT, &mut bytes);
    bytes
}

fn pc_of(block: &[u8]) -> u64 {
    u64::from_le_bytes(block[PC..PC + 8].try_into().unwrap())
}

fn page_permissions(page: u64) -> Permissions {
    let address = page * PAGE_SIZE;
    if (LOAD..LOAD + PAGE_SIZE).contains(&address) {
        Permissions::READ | Permissions::EXECUTE
    } else if (DATA..DATA + PAGE_SIZE).contains(&address) || (0x70000..STACK).contains(&address) {
        Permissions::READ | Permissions::WRITE
    } else {
        Permissions::empty()
    }
}

fn record(caller: &mut Caller<'_, Host>, event: Event) {
    caller.data_mut().events.push(event);
}

fn constant(store: &mut impl AsContextMut, value: Val) -> Global {
    let ty = GlobalType::new(value.ty(&*store).unwrap(), Mutability::Const);
    Global::new(store, ty, value).unwrap()
}

/// Every hook a module may import, recording each call.
fn hooks<const F: bool>(linker: &mut Linker<Host>) {
    use contract::*;
    linker
        .func_wrap(
            HOOKS,
            HOOK_SYSCALL,
            move |mut caller: Caller<'_, Host>,
                  n: i64,
                  a0: i64,
                  a1: i64,
                  a2: i64,
                  a3: i64,
                  a4: i64,
                  a5: i64|
                  -> i64 {
                let pc = pc_of(&block(&caller));
                let args = [n, a0, a1, a2, a3, a4, a5];
                record(&mut caller, Event::Syscall { args, pc });
                42
            },
        )
        .unwrap();
    linker
        .func_wrap(
            HOOKS,
            HOOK_STOP,
            move |mut caller: Caller<'_, Host>| -> i32 {
                let block = block(&caller);
                write_shared(&caller.data().state, BLOCK_AT + STOP_FLAG, &[0; 4]);
                let r0 = u64::from_le_bytes(block[..8].try_into().unwrap());
                let pc = pc_of(&block);
                record(&mut caller, Event::Stop { pc, r0 });
                0
            },
        )
        .unwrap();
    linker
        .func_wrap(
            HOOKS,
            HOOK_MEMORY_FAULT,
            move |mut caller: Caller<'_, Host>, address: i64, access: i32| {
                let pc = pc_of(&block(&caller));
                let (address, write) = (address as u64, access == 1);
                record(&mut caller, Event::Fault { address, write, pc });
            },
        )
        .unwrap();
    linker
        .func_wrap(
            HOOKS,
            HOOK_LOAD_CODE,
            move |mut caller: Caller<'_, Host>, address: i64, exact: i32| -> i32 {
                let address = address as u64;
                let (blob, optimise) = (caller.data().blob.clone(), caller.data().optimise);
                if !(LOAD..LOAD + blob.len() as u64).contains(&address) {
                    record(&mut caller, Event::NoCode(address));
                    return 0;
                }
                // The exact piece is the one instruction alone, as the host runner makes it.
                let look_ahead = if exact == 1 {
                    LookAhead::Bounded(1)
                } else {
                    LookAhead::None
                };
                let translation = translate::<Test<F>>(
                    &blob,
                    0..blob.len() as u64,
                    &[address - LOAD],
                    look_ahead,
                    optimise,
                    (),
                    false,
                )
                .expect("translate");
                instantiate::<F>(&mut caller, &translation, exact == 1);
                1
            },
        )
        .unwrap();
    linker
        .func_wrap(
            HOOKS,
            HOOK_WARN,
            move |mut caller: Caller<'_, Host>, reason: i32, detail: i64| {
                record(&mut caller, Event::Warn { reason, detail });
            },
        )
        .unwrap();
    linker
        .func_wrap(
            HOOKS,
            HOOK_TRAP,
            move |mut caller: Caller<'_, Host>, kind: i32, detail: i64| {
                let pc = pc_of(&block(&caller));
                record(&mut caller, Event::Trap { kind, detail, pc });
            },
        )
        .unwrap();
    let hook = Test::<F>::HOOKS[0];
    linker
        .func_wrap(
            HOOKS,
            hook.name,
            move |caller: Caller<'_, Host>, argument: i64| -> i64 {
                let mut bytes = block(&caller);
                let range = hook.state.range();
                let done = Test::<F>::run_hook(0, &[argument], &mut bytes[range.clone()]);
                write_shared(&caller.data().state, BLOCK_AT + range.start, &bytes[range]);
                done.results[0]
            },
        )
        .unwrap();
}

/// Instantiate a translation's code modules: their pieces in new slots, each given to its address, or with
/// `exact` the one piece in the exact slot.
fn instantiate<const F: bool>(
    store: &mut impl AsContextMut<Data = Host>,
    translation: &Translation,
    exact: bool,
) {
    let mut store = store.as_context_mut();
    let engine = store.engine().clone();
    let thread = store.data().thread.unwrap();
    let Some(Extern::Table(table)) = thread.get_export(&mut store, contract::LOOKUP) else {
        panic!("the thread module exports its lookup table");
    };
    let first = if exact {
        contract::EXACT_SLOT as u32
    } else {
        let first = store.data().next_slot;
        let pieces = translation.pieces().count() as u32;
        let more = (first + pieces).saturating_sub(table.size(&store) as u32);
        table
            .grow(&mut store, more.into(), wasmtime::Ref::Func(None))
            .unwrap();
        store.data_mut().next_slot = first + pieces;
        first
    };
    let mut linker = Linker::new(&engine);
    linker.allow_shadowing(true);
    hooks::<F>(&mut linker);
    let (memory, state) = (store.data().memory.unwrap(), store.data().state.clone());
    linker
        .define(&store, contract::ENV, contract::MEMORY, memory)
        .unwrap();
    linker
        .define(&store, contract::ENV, contract::STATE, state.clone())
        .unwrap();
    let cover = store.data().blob.len() as i64 * Test::<F>::ALIGN as i64;
    for (name, value) in [
        (contract::LOAD_ADDRESS, Val::I64(LOAD as i64)),
        (contract::TABLE_BASE, Val::I64(LOAD as i64)),
        (contract::THREAD_BLOCK, Val::I32(BLOCK_AT as i32)),
        (contract::PERMISSIONS, Val::I32(PERMISSIONS_AT as i32)),
        (
            contract::PERMISSION_PAGES,
            Val::I64((MEMORY_BYTES / PAGE_SIZE) as i64),
        ),
        (contract::SLOTS, Val::I32(SLOTS_AT as i32)),
        (contract::SLOTS_COVER, Val::I64(cover)),
    ] {
        let global = constant(&mut store, value);
        linker.define(&store, contract::ENV, name, global).unwrap();
    }
    let exports: Vec<(String, Extern)> = thread
        .exports(&mut store)
        .map(|export| (export.name().to_string(), export.into_extern()))
        .collect();
    for (name, item) in exports {
        linker
            .define(&store, contract::THREAD, &name, item)
            .unwrap();
    }
    let mut slot = first;
    for module in &translation.modules {
        let base = constant(&mut store, Val::I32(slot as i32));
        linker
            .define(&store, contract::ENV, contract::SLOT_BASE, base)
            .unwrap();
        let compiled = Module::new(&engine, &module.wasm).expect("a valid module");
        linker
            .instantiate(&mut store, &compiled)
            .expect("code instance");
        for &offset in &module.pieces {
            if !exact {
                let at = SLOTS_AT + contract::slot_entry::<Test<F>>(offset) as usize;
                write_shared(&state, at, &slot.to_le_bytes());
            }
            slot += 1;
        }
    }
}

/// Translate `code` from `starts` and run it from the first, until a hook makes the thread leave.
fn run<const F: bool>(
    code: &[u8],
    starts: &[u64],
    start: &Start,
    look_ahead: LookAhead,
    optimise: Optimise,
) -> Ran {
    let engine = engine();
    let state = SharedMemory::new(&engine, MemoryType::shared(1, 1)).unwrap();
    let mut store = Store::new(
        &engine,
        Host {
            state: state.clone(),
            memory: None,
            thread: None,
            blob: code.to_vec(),
            optimise,
            next_slot: contract::EXACT_SLOT as u32 + 1,
            events: Vec::new(),
        },
    );
    let pages = MEMORY_BYTES / 65536;
    let memory = Memory::new(&mut store, MemoryType::new64(pages, Some(pages))).unwrap();
    store.data_mut().memory = Some(memory);
    for &(address, value) in &start.memory {
        memory
            .write(&mut store, address as usize, &value.to_le_bytes())
            .unwrap();
    }
    let page_count = MEMORY_BYTES / PAGE_SIZE;
    let table: Vec<u8> = (0..=page_count)
        .map(|page| {
            if page == page_count {
                0
            } else {
                permission_byte(page_permissions(page), page_permissions(page + 1))
            }
        })
        .collect();
    write_shared(&state, PERMISSIONS_AT, &table);
    let mut block = vec![0; STATE_SIZE];
    block[..128].copy_from_slice(&Test::<F>::initial_block(LOAD + starts[0], STACK));
    for &(r, value) in &start.registers {
        let at = 8 * usize::from(r);
        block[at..at + 8].copy_from_slice(&value.to_le_bytes());
    }
    block[80..88].copy_from_slice(&start.fs.to_le_bytes());
    write_shared(&state, BLOCK_AT, &block);

    let mut linker = Linker::new(&engine);
    hooks::<F>(&mut linker);
    linker
        .define(&store, contract::ENV, contract::STATE, state.clone())
        .unwrap();
    let cover = code.len() as i64 * Test::<F>::ALIGN as i64;
    for (name, value) in [
        (contract::TABLE_BASE, Val::I64(LOAD as i64)),
        (contract::THREAD_BLOCK, Val::I32(BLOCK_AT as i32)),
        (contract::SLOTS, Val::I32(SLOTS_AT as i32)),
        (contract::SLOTS_COVER, Val::I64(cover)),
    ] {
        let global = constant(&mut store, value);
        linker.define(&store, contract::ENV, name, global).unwrap();
    }
    let thread_module = Module::new(&engine, thread_module::<Test<F>>()).unwrap();
    let thread = linker.instantiate(&mut store, &thread_module).unwrap();
    store.data_mut().thread = Some(thread);

    let translation = translate::<Test<F>>(
        code,
        0..code.len() as u64,
        starts,
        look_ahead,
        optimise,
        (),
        false,
    )
    .expect("translate");
    let mut sites: Sites = std::array::from_fn(|_| 0);
    for module in &translation.modules {
        validate(&module.wasm);
        for (all, one) in sites.iter_mut().zip(module.sites) {
            *all += one;
        }
    }
    instantiate::<F>(&mut store, &translation, false);
    let run = thread
        .get_typed_func::<(), ()>(&mut store, contract::RUN)
        .unwrap();
    run.call(&mut store, ()).expect("the thread runs");

    let mut block = vec![0; STATE_SIZE];
    read_shared(&state, BLOCK_AT, &mut block);
    Ran {
        block,
        memory: memory.data(&store).to_vec(),
        events: std::mem::take(&mut store.data_mut().events),
        sites,
    }
}

/// Run a program at every look-ahead in both settings, and check each run.
fn each<const F: bool>(
    code: &[u8],
    starts: &[u64],
    start: &Start,
    check: impl Fn(&Ran, LookAhead),
) {
    for look_ahead in LOOK_AHEADS {
        for optimise in [Optimise::Speed, Optimise::Size] {
            eprintln!("flags {F}, {look_ahead:?}, {optimise:?}");
            let ran = run::<F>(code, starts, start, look_ahead, optimise);
            check(&ran, look_ahead);
        }
    }
}

fn both(code: &[u8], starts: &[u64], start: &Start, check: impl Fn(&Ran, LookAhead, bool)) {
    each::<true>(code, starts, start, |ran, look_ahead| {
        check(ran, look_ahead, true)
    });
    each::<false>(code, starts, start, |ran, look_ahead| {
        check(ran, look_ahead, false)
    });
}

fn breakpoint(pc: u64) -> Event {
    Event::Trap {
        kind: TrapKind::Breakpoint as i32,
        detail: 0,
        pc,
    }
}

const Z: u32 = 1 << 6;
const C: u32 = 1;

#[test]
fn a_trap_leaves_pc_after_it() {
    both(&[0x00, 0x0b], &[0], &Start::default(), |ran, _, _| {
        assert_eq!(ran.last(), breakpoint(LOAD + 2));
        assert_eq!(ran.pc(), LOAD + 2);
    });
}

#[test]
fn a_call_pushes_its_return_address_and_a_register_comes_back_out_only() {
    let mut code = vec![
        0x05, 16, 0, // 0: call 16
        0x01, 0, 0xfb, // 3: add r0, -5
        0x0b, // 6: break
    ];
    code.resize(16, 0);
    code.extend([
        0x08, 3, 0xf0, // 16: store [sp - 16], r3, in the red zone
        0x07, 5, 0xf0, // 19: load r5, [sp - 16]
        0x01, 0, 5,    // 22: add r0, 5
        0x06, // 25: ret
    ]);
    let start = Start {
        registers: vec![(3, 0x1234)],
        ..Start::default()
    };
    both(&code, &[0, 16], &start, |ran, _, flagged| {
        assert_eq!(ran.last(), breakpoint(LOAD + 7));
        assert_eq!(ran.reg(0), 0);
        assert_eq!(ran.reg(5), 0x1234);
        assert_eq!(ran.reg(SP), STACK);
        assert_eq!(ran.word(STACK - 8), LOAD + 3);
        assert_eq!(ran.flags(), if flagged { Z | C } else { 0 });
    });
}

#[test]
fn a_flag_kept_by_an_instruction_reaches_the_block() {
    let code = [
        0x01, 1, 0xff, // 0: add r1, -1, which sets Z and C
        0x02, 3,    // 3: inc r3, which sets Z and keeps C
        0x0b, // 5: break
    ];
    let start = Start {
        registers: vec![(1, 1), (3, 5)],
        ..Start::default()
    };
    each::<true>(&code, &[0], &start, |ran, _| {
        assert_eq!(ran.last(), breakpoint(LOAD + 6));
        assert_eq!((ran.reg(1), ran.reg(3)), (0, 6));
        assert_eq!(ran.flags(), C);
    });
}

#[test]
fn a_jump_into_the_middle_of_an_instruction_runs_what_it_finds() {
    let code = [
        0x01, 1, 0xff, // 0: add r1, -1
        0x02, 3, // 3: inc r3, keeping C
        0x04, 1,    // 5: jc to 6, the jump's own displacement: add r0, 11
        0x00, // 7: nop, which is the add's register when jumped into
        0x0b, // 8: break, or the add's 11
        0x0b, // 9: break
    ];
    // r1 = 1: C set (flags) and r1 zero (no flags), so the jump is taken.
    let taken = Start {
        registers: vec![(1, 1)],
        ..Start::default()
    };
    both(&code, &[0], &taken, |ran, _, _| {
        assert_eq!(ran.last(), breakpoint(LOAD + 10));
        assert_eq!((ran.reg(0), ran.reg(3)), (11, 1));
        assert_eq!(ran.flags(), 0);
    });
    let not_taken = Start::default();
    both(&code, &[0], &not_taken, |ran, _, _| {
        assert_eq!(ran.last(), breakpoint(LOAD + 9));
        assert_eq!(ran.reg(0), 0);
    });
}

/// Two streams that meet again: a jump into `inc r0`'s register byte, a nop, ends where the `inc` ends.
#[test]
fn two_streams_through_one_instruction_meet_again() {
    let code = [
        0x01, 1, 0xff, // 0: add r1, -1
        0x03, 3, // 3: jz to 6
        0x02, 0,    // 5: inc r0; at 6 a nop
        0x0b, // 7: break
    ];
    // r1 = 1: Z set (flags) and r0 zero (no flags), so the jump is taken.
    let taken = Start {
        registers: vec![(1, 1)],
        ..Start::default()
    };
    both(&code, &[0], &taken, |ran, _, _| {
        assert_eq!(ran.last(), breakpoint(LOAD + 8));
        assert_eq!(ran.reg(0), 0);
    });
    let not_taken = Start {
        registers: vec![(1, 5)],
        ..Start::default()
    };
    each::<true>(&code, &[0], &not_taken, |ran, _| {
        assert_eq!(ran.last(), breakpoint(LOAD + 8));
        assert_eq!(ran.reg(0), 1);
    });
}

/// A jump past an instruction's first byte into the rest of it, as x86's `je 1f; lock; 1: cmpxchg`: both
/// streams go on from the same offset.
#[test]
fn a_jump_past_an_instructions_first_byte_meets_it_at_its_end() {
    let code = [
        0x01, 1, 0xff, // 0: add r1, -1
        0x03, 3, // 3: jz to 6
        0x01, 0, 0x00, // 5: add r0, 0; at 6 and 7 nops
        0x01, 0, 7,    // 8: add r0, 7
        0x0b, // 11: break
    ];
    for r1 in [1, 5] {
        let start = Start {
            registers: vec![(1, r1)],
            ..Start::default()
        };
        both(&code, &[0], &start, |ran, _, _| {
            assert_eq!(ran.last(), breakpoint(LOAD + 12));
            assert_eq!(ran.reg(0), 7);
        });
    }
}

/// Loads through the FS base: two from one base register, two from the segment base alone.
fn segment_loads() -> Vec<u8> {
    vec![
        0x11, 0o20, 0, // 0: load r0, fs:[r2]
        0x11, 0o21, 8, // 3: load r1, fs:[r2 + 8]
        0x11, 0x83, 16, // 6: load r3, fs:[16]
        0x11, 0x85, 24,   // 9: load r5, fs:[24]
        0x0b, // 12: break
    ]
}

#[test]
fn accesses_from_a_segment_base_are_grouped_and_fault_exactly() {
    let code = segment_loads();
    let start = Start {
        registers: vec![(2, 0x100)],
        fs: DATA,
        memory: vec![
            (DATA + 0x100, 0x1111),
            (DATA + 0x108, 0x2222),
            (DATA + 16, 0x3333),
            (DATA + 24, 0x4444),
        ],
    };
    both(&code, &[0], &start, |ran, look_ahead, _| {
        assert_eq!(ran.last(), breakpoint(LOAD + 13));
        let loaded = [0, 1, 3, 5].map(|r| ran.reg(r));
        assert_eq!(loaded, [0x1111, 0x2222, 0x3333, 0x4444]);
        let exact = ran.sites[Runtime::MemoryFault as usize];
        match look_ahead {
            // Each pair is checked once, by a group, with no exact check.
            LookAhead::Full => assert_eq!(exact, 0),
            LookAhead::None => assert_eq!(exact, 4),
            _ => {}
        }
    });
    // The second access of a group reaches a page nothing may read: the group's check fails, the first runs
    // exactly, and the second faults with the registers exact.
    let through_base = Start {
        registers: vec![(1, 0x77), (2, 0xff8)],
        fs: DATA,
        memory: vec![(DATA + 0xff8, 0x5555)],
    };
    both(&code, &[0], &through_base, |ran, _, _| {
        let fault = Event::Fault {
            address: DATA + PAGE_SIZE,
            write: false,
            pc: LOAD + 3,
        };
        assert_eq!(ran.last(), fault);
        assert_eq!((ran.reg(0), ran.reg(1)), (0x5555, 0x77));
    });
    let alone = [
        0x11, 0x83, 8, // 0: load r3, fs:[8]
        0x11, 0x85, 16,   // 3: load r5, fs:[16]
        0x0b, // 6: break
    ];
    let segment_alone = Start {
        registers: vec![(5, 0x77)],
        fs: DATA + PAGE_SIZE - 16,
        memory: vec![(DATA + PAGE_SIZE - 8, 0x6666)],
    };
    both(&alone, &[0], &segment_alone, |ran, _, _| {
        let fault = Event::Fault {
            address: DATA + PAGE_SIZE,
            write: false,
            pc: LOAD + 3,
        };
        assert_eq!(ran.last(), fault);
        assert_eq!((ran.reg(3), ran.reg(5)), (0x6666, 0x77));
    });
}

#[test]
fn a_hook_sees_and_changes_its_registers_in_the_block() {
    let code = [
        0x01, 1, 3,    // 0: add r1, 3
        0x10, // 3: hook: r0 = r1 + 1, and r7 = 2 * r1 in the block
        0x01, 7, 1,    // 4: add r7, 1
        0x0b, // 7: break
    ];
    let start = Start {
        registers: vec![(1, 4)],
        ..Start::default()
    };
    both(&code, &[0], &start, |ran, _, _| {
        assert_eq!(ran.last(), breakpoint(LOAD + 8));
        assert_eq!([0, 1, 7].map(|r| ran.reg(r)), [8, 7, 15]);
    });
}

#[test]
fn a_raised_stop_is_seen_before_the_next_instruction() {
    let code = [
        0x01, 0, 1,    // 0: add r0, 1
        0x0f, // 3: raise the stop flag, r0 not being zero
        0x01, 0, 1,    // 4: add r0, 1
        0x0b, // 7: break
    ];
    both(&code, &[0], &Start::default(), |ran, _, _| {
        let stops: Vec<Event> = ran
            .events
            .iter()
            .copied()
            .filter(|e| matches!(e, Event::Stop { .. }))
            .collect();
        assert_eq!(
            stops,
            [Event::Stop {
                pc: LOAD + 4,
                r0: 1
            }]
        );
        assert_eq!(ran.last(), breakpoint(LOAD + 8));
        assert_eq!(ran.reg(0), 2);
    });
}

#[test]
fn a_system_call_and_a_slow_path_reach_their_hooks() {
    let code = [
        0x0e, // 0: slow
        0x0a, // 1: syscall
        0x0b, // 2: break
    ];
    let start = Start {
        registers: vec![(0, 60), (7, 1), (6, 2), (2, 3), (3, 4), (5, 5), (1, 6)],
        ..Start::default()
    };
    both(&code, &[0], &start, |ran, _, _| {
        let slow = Event::Warn {
            reason: GUEST_REASONS as i32,
            detail: LOAD as i64,
        };
        assert_eq!(ran.slow(), [slow]);
        let call = Event::Syscall {
            args: [60, 1, 2, 3, 4, 5, 6],
            pc: LOAD + 2,
        };
        assert!(ran.events.contains(&call), "{:?}", ran.events);
        assert_eq!(ran.reg(0), 42);
        assert_eq!(ran.last(), breakpoint(LOAD + 3));
    });
}

#[test]
fn a_range_is_checked_at_run_time() {
    let code = [0x0d, 0x0b];
    for (at, allowed) in [(DATA, 1), (DATA + 0x800, 0)] {
        let start = Start {
            registers: vec![(1, PAGE_SIZE), (2, at)],
            ..Start::default()
        };
        both(&code, &[0], &start, |ran, _, _| {
            assert_eq!(ran.last(), breakpoint(LOAD + 2));
            assert_eq!(ran.reg(0), allowed);
        });
    }
}

#[test]
fn a_switch_goes_to_its_case_or_through_the_lookup() {
    let code = [
        0x01, 5, 2, // 0: add r5, 2
        0x0c, 5, // 3: jump through r5: cases at 5, 7, 9, 11
        0x02, 0, // 5: inc r0
        0x02, 0, // 7: inc r0
        0x02, 0, // 9: inc r0
        0x02, 0,    // 11: inc r0
        0x0b, // 13: break
    ];
    for (to, count) in [(7, 3), (13, 0)] {
        let start = Start {
            registers: vec![(5, LOAD + to - 2)],
            ..Start::default()
        };
        both(&code, &[0], &start, |ran, _, _| {
            assert_eq!(ran.last(), breakpoint(LOAD + 14));
            assert_eq!(ran.reg(0), count);
        });
    }
}

#[test]
fn two_accesses_of_one_instruction_fault_exactly() {
    let code = [0x12, 0x0b];
    let start = Start {
        registers: vec![(1, DATA), (2, DATA + 0x100)],
        memory: vec![(DATA, 0x9999)],
        ..Start::default()
    };
    both(&code, &[0], &start, |ran, _, _| {
        assert_eq!(ran.last(), breakpoint(LOAD + 2));
        assert_eq!(ran.word(DATA + 0x100), 0x9999);
    });
    let unwritable = Start {
        registers: vec![(1, DATA), (2, DATA + PAGE_SIZE)],
        memory: vec![(DATA, 0x9999)],
        ..Start::default()
    };
    both(&code, &[0], &unwritable, |ran, _, _| {
        let fault = Event::Fault {
            address: DATA + PAGE_SIZE,
            write: true,
            pc: LOAD,
        };
        assert_eq!(ran.last(), fault);
    });
}
