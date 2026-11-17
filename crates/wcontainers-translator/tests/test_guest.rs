//! A guest CPU made only for this test, shaped like x86-64 where ARM64 differs: byte-aligned instructions of
//! 1 to 4 bytes, the return address written on the stack, a red zone, different registers passed in and
//! out, a flag setter that keeps a flag, a segment base, a trap that leaves pc after it, two accesses in one
//! instruction, a run-time length checked, its own slow path, a hook that works on registers, and a stop
//! raised by an instruction. Its translations at every look-ahead must be valid WebAssembly, so the
//! translator's side of each item exists and fits together before any x86 code is written.

use std::ops::Range;
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, FunctionSection, InstructionSink, MemArg, MemArgTarget,
    Module, TypeSection, ValType,
};
use wasmparser::{Validator, WasmFeatures};
use wcontainers_guest_cpu::{
    Access, AccessBytes, AccessKind, Base, Bit, Class, CodeChanges, Comparison, Effects, Emitter,
    Flow, FlowKind, Guest, GuestId, HookBody, HookDone, HookType, Induction, Locals, MemoryAccess,
    MemoryFault, Personality, Registers, ReturnAddress, StackMove, StateBytes, SyscallRegisters,
    Target, Through, TrapDetail, TrapKind, TrapPc, Ty, Value, WriteBack,
};
use wcontainers_translator::{LookAhead, Optimise, thread_module, translate};

#[derive(Clone, Copy, Debug)]
struct Test;

const SP: u8 = 4;
const FS: u8 = 22;
const MODE: usize = 112;

impl Personality for Test {
    type Block = [u8; 128];
    const PC_OFFSET: usize = 64;
    const STOP_FLAG_OFFSET: usize = 128;
    const BLOCK_SIZE: usize = 144;
    const PRIVATE_BYTES: usize = 16;

    fn initial_block(pc: u64, sp: u64) -> [u8; 128] {
        let mut block = [0; 128];
        block[64..72].copy_from_slice(&pc.to_le_bytes());
        block[32..40].copy_from_slice(&sp.to_le_bytes());
        block
    }
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Nop,
    Add(u8, i8),
    /// Adds 1 and sets Z, keeping C.
    Inc(u8),
    JumpIfZero(i8),
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
    Hook,
    /// r, fs:[r + d].
    LoadFs(u8, i8),
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

const BITS: [Bit; 12] = [
    general(0),
    general(1),
    general(2),
    general(3),
    general(4),
    general(5),
    general(6),
    general(7),
    Bit {
        bit: 16,
        class: Class::Flag,
        ty: Ty::I32,
        offset: 72,
    },
    Bit {
        bit: 17,
        class: Class::Flag,
        ty: Ty::I32,
        offset: 72,
    },
    Bit {
        bit: FS as u32,
        class: Class::Segment,
        ty: Ty::I64,
        offset: 80,
    },
    Bit {
        bit: 64,
        class: Class::Vector,
        ty: Ty::V128,
        offset: 96,
    },
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
        Add(..) | Call(_) | Load(..) | Store(..) => 3,
        LoadFs(..) => 3,
    }
}

/// The access through the stack pointer `bytes` below it.
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

/// Push the guest address `d` bytes from register `r` (from a segment base when `segment`) into a fresh local.
fn address<E: Emitter<Test> + ?Sized>(e: &mut E, r: u8, d: i64, segment: bool) -> u32 {
    let at = e.local(ValType::I64);
    e.get(r);
    if segment {
        e.get(FS);
        e.sink().i64_add();
    }
    e.sink().i64_const(d).i64_add().local_set(at);
    at
}

fn emit_op<E: Emitter<Test> + ?Sized>(e: &mut E, op: Op) {
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
            e.set_flags(Flags::Add { a, b });
        }
        Op::Inc(r) => {
            let (z, c, value) = (
                e.local(ValType::I32),
                e.local(ValType::I32),
                e.local(ValType::I64),
            );
            e.flag(Flag::C);
            e.sink().local_set(c);
            e.get(r);
            e.sink().i64_const(1).i64_add().local_tee(value);
            e.set(r);
            e.sink().local_get(value).i64_eqz().local_set(z);
            e.set_flags(Flags::Inc { z, c });
        }
        Op::JumpIfZero(by) | Op::JumpIfCarry(by) => {
            let flag = if matches!(op, Op::JumpIfZero(_)) {
                Flag::Z
            } else {
                Flag::C
            };
            e.condition(flag);
            e.jump_if(Target::Relative(by.into()));
        }
        Op::Call(by) => {
            // The CPU's own steps: the push, then the stack pointer, then the call.
            let at = address(e, SP, -8, false);
            e.check(at, 8, Access::Write, 0);
            e.sink().local_get(at);
            e.this_address(length(op).into());
            e.sink().i64_store(memory()).local_get(at);
            e.set(SP);
            e.call(Target::Relative(by.into()));
        }
        Op::Ret => {
            let at = address(e, SP, 0, false);
            e.check(at, 8, Access::Read, 0);
            let target = e.local(ValType::I64);
            e.sink().local_get(at).i64_load(memory()).local_set(target);
            e.ret(target);
            return;
        }
        Op::Load(r, d) | Op::LoadFs(r, d) => {
            let segment = matches!(op, Op::LoadFs(..));
            let base = if segment { r } else { SP };
            let at = address(e, base, d.into(), segment);
            e.check(at, 8, Access::Read, 0);
            e.sink().local_get(at).i64_load(memory());
            e.set(r);
        }
        Op::Store(r, d) => {
            let at = address(e, SP, d.into(), false);
            e.check(at, 8, Access::Write, 0);
            e.sink().local_get(at);
            e.get(r);
            e.sink().i64_store(memory());
        }
        Op::Push(r) => {
            let at = address(e, SP, -8, false);
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
            let from = address(e, 1, 0, false);
            let to = address(e, 2, 0, false);
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

impl Guest for Test {
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
            0x11 => Op::LoadFs(byte(1)? & 7, byte(2)? as i8),
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
    const BITS: &'static [Bit] = &BITS;

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
    const FLAGS: &'static [(Flag, u32, u32)] = &[(Flag::Z, 6, 16), (Flag::C, 0, 17)];
    const FLAGS_WORD_OFFSET: usize = 72;
    type Flags = Flags;
    type Shape = Shape;
    const SHAPES: &'static [Shape] = &[Shape::Add];

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

    const FLAGS_WANTED: i64 = 1 << 62;

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
            at: MODE as u16,
            len: 4,
        },
        body: HookBody::Cpu,
    }];

    fn hook_index(_h: Hook) -> usize {
        0
    }

    fn run_hook(_hook: usize, args: &[i64], _state: &mut [u8]) -> HookDone {
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
        let mut module = Module::new();
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
        let register = |r: u8, kind| MemoryAccess {
            base: Some(Base::Register(r)),
            displacement: 0,
            ..stack_access(0, kind)
        };
        match *i {
            Op::Call(_) | Op::Push(_) => [Some(stack_access(-8, AccessKind::Write)), None],
            Op::Ret => [Some(stack_access(0, AccessKind::Read)), None],
            Op::Load(_, d) => [Some(stack_access(d.into(), AccessKind::Read)), None],
            Op::Store(_, d) => [Some(stack_access(d.into(), AccessKind::Write)), None],
            Op::LoadFs(r, d) => [
                Some(MemoryAccess {
                    segment: Some(FS),
                    displacement: d.into(),
                    ..register(r, AccessKind::Read)
                }),
                None,
            ],
            Op::Move => [
                Some(register(1, AccessKind::Read)),
                Some(register(2, AccessKind::Write)),
            ],
            Op::Range => [
                Some(MemoryAccess {
                    bytes: AccessBytes::Counted { element: 1 },
                    ..register(2, AccessKind::Write)
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

    const UNREACHABLE_RETURN: i64 = i64::MIN;
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
        0x11, 2, 16,   // 17: load r2, fs:[r2 + 16]
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

#[test]
fn a_cpu_shaped_like_x86_64_fills_in_the_interface() {
    let blob = program();
    let starts = [0, FUNCTION as u64];
    let look_aheads = [
        LookAhead::None,
        LookAhead::Bounded(1),
        LookAhead::Bounded(5),
        LookAhead::Bounded(256),
        LookAhead::Full,
    ];
    validate(&thread_module::<Test>());
    for look_ahead in look_aheads {
        for optimise in [Optimise::Speed, Optimise::Size] {
            for offer in [false, true] {
                let translation = translate::<Test>(
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
fn the_test_guest_decodes_its_program() {
    let blob = program();
    let mut at = 0;
    let mut count = 0;
    while let Some((_, n)) = Test::decode(&blob[at..]) {
        at += usize::from(n);
        count += 1;
        if at >= blob.len() {
            break;
        }
    }
    assert!(count > 20);
}
