//! The meaning of each x87 instruction (plan 5.8): the check for a pending exception, the memory operand
//! read or checked by the meaning, the last instruction pointer kept, and the rest done by the x87 hook
//! (`crate::x87`) on the block.

use crate::decode::Instruction;
use crate::flags::{Condition, Flag, Flags};
use crate::guest::{Control, Hook, Trap};
use crate::meaning::{checked, each_with};
use crate::operand::*;
use crate::x87::{
    Arithmetic, Code, ENVIRONMENT_WORDS, EXCEPTIONS, FOP_SHIFT, Format, NOT_STORED, OPCODE_BITS,
    Operation, REGISTER_MASK, REGISTERS, TOP_SHIFT,
};
use crate::{CF, PF, X86_64, ZF};
use iced_x86::{MemorySize, Mnemonic, OpKind, Register};
use wasm_encoder::{BlockType, MemArg, ValType};
use wcontainers_guest_cpu::{Access, Emitter};

/// Whether the instruction is one of these.
pub fn written(m: Mnemonic) -> bool {
    use Mnemonic::*;
    matches!(
        m,
        Fld | Fild
            | Fbld
            | Fld1
            | Fldl2t
            | Fldl2e
            | Fldpi
            | Fldlg2
            | Fldln2
            | Fldz
            | Fst
            | Fstp
            | Fstpnce
            | Fist
            | Fistp
            | Fisttp
            | Fbstp
            | Fadd
            | Faddp
            | Fiadd
            | Fsub
            | Fsubp
            | Fisub
            | Fsubr
            | Fsubrp
            | Fisubr
            | Fmul
            | Fmulp
            | Fimul
            | Fdiv
            | Fdivp
            | Fidiv
            | Fdivr
            | Fdivrp
            | Fidivr
            | Fcom
            | Fcomp
            | Fcompp
            | Fucom
            | Fucomp
            | Fucompp
            | Ficom
            | Ficomp
            | Fcomi
            | Fcomip
            | Fucomi
            | Fucomip
            | Ftst
            | Fxam
            | Fchs
            | Fabs
            | Fxch
            | Fcmovb
            | Fcmove
            | Fcmovbe
            | Fcmovu
            | Fcmovnb
            | Fcmovne
            | Fcmovnbe
            | Fcmovnu
            | Ffree
            | Ffreep
            | Fincstp
            | Fdecstp
            | Fninit
            | Fnclex
            | Fldcw
            | Fnstcw
            | Fnstsw
            | Frndint
            | Fscale
            | Fxtract
            | Fsqrt
            | Fnop
            | Wait
            | Fneni
            | Fndisi
            | Fnsetpm
            | Fnstenv
            | Fldenv
            | Fnsave
            | Frstor
            | Fprem
            | Fprem1
            | Fsin
            | Fcos
            | Fsincos
            | Fptan
            | Fpatan
            | F2xm1
            | Fyl2x
            | Fyl2xp1
            | Fxsave
            | Fxsave64
            | Fxrstor
            | Fxrstor64
    )
}

/// The instructions that never wait for a pending exception.
fn no_wait(m: Mnemonic) -> bool {
    use Mnemonic::*;
    matches!(
        m,
        Fninit | Fnclex | Fnstcw | Fnstsw | Fnstenv | Fnsave | Fneni | Fndisi | Fnsetpm
    )
}

/// The control instructions, which leave the last instruction pointer as it is (seen on the reference).
fn control(m: Mnemonic) -> bool {
    use Mnemonic::*;
    no_wait(m) || matches!(m, Fldcw | Fldenv | Frstor | Wait)
}

/// Stop with the floating point exception when an exception flag is raised and unmasked, as every x87
/// instruction that waits and every MMX instruction does before anything else.
pub(crate) fn pending<E: Emitter<X86_64> + ?Sized>(e: &mut E) {
    let words = e.local(ValType::I32);
    let raised = e.local(ValType::I64);
    e.get_control(Control::X87Words);
    e.sink()
        .local_tee(words)
        .i32_const(u16::BITS as i32)
        .i32_shr_u()
        .local_get(words)
        .i32_const(-1)
        .i32_xor()
        .i32_and()
        .i32_const(EXCEPTIONS.into())
        .i32_and()
        .i64_extend_i32_u()
        .local_tee(raised)
        .i64_const(0)
        .i64_ne()
        .if_(BlockType::Empty);
    e.trap(Trap::FloatException(raised));
    e.sink().end();
}

/// ST(i)'s number in an operand.
fn st(r: Register) -> Option<u8> {
    let n = r as u32;
    (Register::ST0 as u32..=Register::ST7 as u32)
        .contains(&n)
        .then(|| (n - Register::ST0 as u32) as u8)
}

/// The memory format of the instruction's operand.
fn format(i: &Instruction) -> Format {
    match i.iced.memory_size() {
        MemorySize::Float32 => Format::Single,
        MemorySize::Float64 => Format::Double,
        MemorySize::Float80 => Format::Extended,
        MemorySize::Int16 => Format::Word,
        MemorySize::Int32 => Format::Integer,
        MemorySize::Int64 => Format::Long,
        MemorySize::Bcd => Format::Decimal,
        _ => Format::Register,
    }
}

use crate::simd::{BYTE, DWORD as DOUBLEWORD, WORD};

/// Load `bytes` bytes at `at` plus `offset` as an i64.
pub(crate) fn load_at<E: Emitter<X86_64> + ?Sized>(e: &mut E, at: u32, offset: u64, bytes: u32) {
    let m = MemArg { offset, ..memory() };
    let mut s = e.sink();
    s.local_get(at);
    match bytes {
        BYTE => s.i64_load8_u(m),
        WORD => s.i64_load16_u(m),
        DOUBLEWORD => s.i64_load32_u(m),
        _ => s.i64_load(m),
    };
}

/// Store the low `bytes` bytes of the i64 local `value` at `at` plus `offset`.
pub(crate) fn store_at<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    at: u32,
    offset: u64,
    bytes: u32,
    value: u32,
) {
    let m = MemArg { offset, ..memory() };
    let mut s = e.sink();
    s.local_get(at).local_get(value);
    match bytes {
        BYTE => s.i64_store8(m),
        WORD => s.i64_store16(m),
        DOUBLEWORD => s.i64_store32(m),
        _ => s.i64_store(m),
    };
}

/// The bytes of a memory value the hook's first argument holds; the rest, up to an 80-bit value, go in the
/// second.
pub(crate) const VALUE_LOW: u32 = size_of::<u64>() as u32;
pub(crate) const VALUE_HIGH: u32 = size_of::<u16>() as u32;

/// Call the x87 hook with `code`, the values in these locals (or zeros) and the data address; its two
/// results into fresh locals.
pub(crate) fn call<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    code: Code,
    a: Option<u32>,
    b: Option<u32>,
    data: Option<u32>,
) -> (u32, u32) {
    let (low, high) = (e.local(ValType::I64), e.local(ValType::I64));
    let mut s = e.sink();
    s.i32_const(code.encode());
    for v in [a, b, data] {
        match v {
            Some(v) => s.local_get(v),
            None => s.i64_const(0),
        };
    }
    e.hook(Hook::X87);
    e.sink().local_set(high).local_set(low);
    (low, high)
}

/// The memory value at `at`, `bytes` long, as the hook's two arguments.
fn read<E: Emitter<X86_64> + ?Sized>(e: &mut E, at: u32, bytes: u32) -> (u32, Option<u32>) {
    let a = e.local(ValType::I64);
    load_at(e, at, 0, bytes.min(VALUE_LOW));
    e.sink().local_set(a);
    if bytes <= VALUE_LOW {
        return (a, None);
    }
    let b = e.local(ValType::I64);
    load_at(e, at, VALUE_LOW.into(), VALUE_HIGH);
    e.sink().local_set(b);
    (a, Some(b))
}

/// Store the hook's results at `at` unless it says an unmasked exception kept the value from memory, then
/// leave the status and tag words the store gave: memory is checked first, so a fault leaves the unit as it
/// was. An 80-bit value goes in two parts, its first eight bytes written before the last two are checked, as
/// on the reference.
fn write<E: Emitter<X86_64> + ?Sized>(e: &mut E, at: u32, bytes: u32, low: u32, high: u32) {
    e.sink()
        .local_get(high)
        .i64_const(NOT_STORED as i64)
        .i64_and()
        .i64_eqz()
        .if_(BlockType::Empty);
    e.check(at, bytes.min(VALUE_LOW), Access::Write, 0);
    store_at(e, at, 0, bytes.min(VALUE_LOW), low);
    if bytes > VALUE_LOW {
        let rest = e.local(ValType::I64);
        e.sink()
            .local_get(at)
            .i64_const(VALUE_LOW.into())
            .i64_add()
            .local_set(rest);
        e.check(rest, VALUE_HIGH, Access::Write, 0);
        store_at(e, rest, 0, VALUE_HIGH, high);
    }
    e.sink()
        .local_get(high)
        .i64_const(u16::BITS.into())
        .i64_shr_u()
        .i32_wrap_i64();
    e.set_control(Control::X87StatusTag);
    e.sink().end();
}

/// Whether the instruction's memory is an 80-bit value, which it reads or writes in two parts.
pub(crate) fn split(i: &Instruction) -> bool {
    use Mnemonic::*;
    matches!(i.iced.mnemonic(), Fld | Fstp | Fbld | Fbstp)
        && memory_bytes(i) == VALUE_LOW + VALUE_HIGH
}

/// Check an 80-bit value's memory in its two parts, as the CPU reads and writes it.
fn check_split<E: Emitter<X86_64> + ?Sized>(e: &mut E, at: u32, access: Access) {
    let rest = e.local(ValType::I64);
    e.check(at, VALUE_LOW, access, 0);
    e.sink()
        .local_get(at)
        .i64_const(VALUE_LOW.into())
        .i64_add()
        .local_set(rest);
    e.check(rest, VALUE_HIGH, access, 0);
}

/// For the instructions that save and load the unit's state, the first bytes `check_ends` checks alone (none
/// for `fxsave` and `fxrstor`); none for any other.
pub(crate) fn state_probe(i: &Instruction) -> Option<Option<u32>> {
    use Mnemonic::*;
    match i.iced.mnemonic() {
        Fnstenv | Fnsave | Fldenv | Frstor => Some(Some(Layout::of(i).word)),
        Fxsave | Fxsave64 | Fxrstor | Fxrstor64 => Some(None),
        _ => None,
    }
}

/// Check the `bytes` at `at` (at most two pages) for the access as the instructions that save and load the
/// unit's state do on the reference (scratch `x7/probes/faults.c`): the first `first` bytes when given, then
/// the last byte, then all, so that a fault names the byte where that one met it.
fn check_ends<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    at: u32,
    bytes: u32,
    access: Access,
    first: Option<u32>,
) {
    if let Some(first) = first {
        e.check(at, first, access, 0);
    }
    // With 32-bit addresses the offset wraps within 32 bits, before the segment base is added.
    let last = e.local(ValType::I64);
    let base = segment(i).filter(|_| i.address32);
    if let Some(base) = base {
        e.sink().local_get(at);
        e.get(base);
        e.sink().i64_sub();
    } else {
        e.sink().local_get(at);
    }
    e.sink().i64_const((bytes - 1).into()).i64_add();
    if i.address32 {
        e.sink().i64_const(u32::MAX.into()).i64_and();
    }
    if let Some(base) = base {
        e.get(base);
        e.sink().i64_add();
    }
    e.sink().local_set(last);
    e.check(last, 1, access, 0);
    e.check(at, bytes, access, 0);
}

pub fn emit<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    use Mnemonic::*;
    let ins = &i.iced;
    if matches!(m, Fxsave | Fxsave64 | Fxrstor | Fxrstor64) {
        image(e, i, m);
        return;
    }
    if !no_wait(m) {
        pending(e);
    }
    let bytes = memory_bytes(i);
    let memory = memory_use(i);
    // A store's memory is checked once its value is known: an unmasked exception keeps it from memory.
    let at = match m {
        Fst | Fstp | Fist | Fistp | Fisttp | Fbstp => memory.map(|_| address(e, i)),
        Fnstenv | Fnsave | Fldenv | Frstor => {
            let at = address(e, i);
            let access = check_kind(memory.expect("a memory operand"));
            check_ends(e, i, at, bytes, access, Some(Layout::of(i).word));
            Some(at)
        }
        Fld | Fbld if split(i) => {
            let at = address(e, i);
            check_split(e, at, Access::Read);
            Some(at)
        }
        _ => checked(e, i),
    };
    // A memory operand that is read is read before anything changes.
    let reads = matches!(
        m,
        Fld | Fild
            | Fbld
            | Fadd
            | Fiadd
            | Fsub
            | Fisub
            | Fsubr
            | Fisubr
            | Fmul
            | Fimul
            | Fdiv
            | Fidiv
            | Fdivr
            | Fidivr
            | Fcom
            | Fcomp
            | Ficom
            | Ficomp
            | Fldcw
    );
    let (a, b) = match at {
        Some(at) if reads => {
            let (a, b) = read(e, at, bytes);
            (Some(a), b)
        }
        _ => (None, None),
    };
    let mut c = Code::new(Operation::Load);
    c.opcode = i.x87_opcode & ((1 << OPCODE_BITS) - 1);
    c.format = format(i);
    let registers: Vec<u8> = (0..ins.op_count())
        .filter(|&n| ins.op_kind(n) == OpKind::Register)
        .filter_map(|n| st(ins.op_register(n)))
        .collect();
    // The register that is not the first ST(0), and whether the destination is it.
    match registers.as_slice() {
        [0, other] => c.register = *other,
        [first, _] => {
            c.register = *first;
            c.to_register = true;
        }
        [only] => c.register = *only,
        _ => {}
    }
    let data = at.filter(|_| c.format != Format::Register);
    match m {
        Fld | Fild | Fbld => {
            call(e, c, a, b, data);
        }
        Fld1 | Fldl2t | Fldl2e | Fldpi | Fldlg2 | Fldln2 | Fldz => {
            c.operation = Operation::LoadConstant;
            c.register = [Fld1, Fldl2t, Fldl2e, Fldpi, Fldlg2, Fldln2, Fldz]
                .iter()
                .position(|x| *x == m)
                .expect("a constant") as u8;
            call(e, c, None, None, None);
        }
        Fst | Fstp | Fstpnce | Fist | Fistp | Fisttp | Fbstp => {
            c.operation = Operation::Store;
            c.pops = u8::from(!matches!(m, Fst | Fist));
            c.truncate = m == Fisttp;
            c.quiet = m == Fstpnce;
            let (low, high) = call(e, c, None, None, data);
            if let Some(at) = at {
                write(e, at, bytes, low, high);
            }
        }
        Fadd | Faddp | Fiadd | Fsub | Fsubp | Fisub | Fsubr | Fsubrp | Fisubr | Fmul | Fmulp
        | Fimul | Fdiv | Fdivp | Fidiv | Fdivr | Fdivrp | Fidivr => {
            c.operation = Operation::Arithmetic;
            c.arithmetic = match m {
                Fadd | Faddp | Fiadd => Arithmetic::Add,
                Fsub | Fsubp | Fisub => Arithmetic::Subtract,
                Fsubr | Fsubrp | Fisubr => Arithmetic::SubtractReversed,
                Fmul | Fmulp | Fimul => Arithmetic::Multiply,
                Fdiv | Fdivp | Fidiv => Arithmetic::Divide,
                _ => Arithmetic::DivideReversed,
            };
            c.pops = u8::from(matches!(m, Faddp | Fsubp | Fsubrp | Fmulp | Fdivp | Fdivrp));
            call(e, c, a, b, data);
        }
        Fcom | Fcomp | Fcompp | Fucom | Fucomp | Fucompp | Ficom | Ficomp | Ftst => {
            c.operation = if m == Ftst {
                Operation::Test
            } else {
                Operation::Compare
            };
            c.quiet = matches!(m, Fucom | Fucomp | Fucompp);
            c.pops = match m {
                Fcompp | Fucompp => 2,
                Fcomp | Fucomp | Ficomp => 1,
                _ => 0,
            };
            if matches!(m, Fcompp | Fucompp) {
                c.register = 1;
            }
            call(e, c, a, b, data);
        }
        Fcomi | Fcomip | Fucomi | Fucomip => {
            c.operation = Operation::Compare;
            c.quiet = matches!(m, Fucomi | Fucomip);
            c.flags = true;
            c.pops = u8::from(matches!(m, Fcomip | Fucomip));
            let (low, _) = call(e, c, None, None, None);
            compare_flags(e, low);
        }
        Fxam | Fchs | Fabs | Fxch | Fincstp | Fdecstp | Fninit | Fnclex | Frndint | Fscale
        | Fxtract | Fsqrt => {
            c.operation = match m {
                Fxam => Operation::Examine,
                Fchs => Operation::ChangeSign,
                Fabs => Operation::Absolute,
                Fxch => Operation::Exchange,
                Fincstp => Operation::Increment,
                Fdecstp => Operation::Decrement,
                Fninit => Operation::Initialize,
                Fnclex => Operation::ClearExceptions,
                Frndint => Operation::RoundToInteger,
                Fscale => Operation::Scale,
                Fxtract => Operation::Extract,
                _ => Operation::SquareRoot,
            };
            call(e, c, None, None, None);
        }
        Fsin | Fcos | Fsincos | Fptan | F2xm1 | Fpatan | Fyl2x | Fyl2xp1 => {
            c.operation = match m {
                Fsin => Operation::Sine,
                Fcos => Operation::Cosine,
                Fsincos => Operation::SineCosine,
                Fptan => Operation::Tangent,
                F2xm1 => Operation::ExponentMinusOne,
                Fpatan => Operation::ArcTangent,
                Fyl2x => Operation::Logarithm,
                _ => Operation::LogarithmPlusOne,
            };
            c.pops = u8::from(matches!(m, Fpatan | Fyl2x | Fyl2xp1));
            call(e, c, None, None, None);
        }
        Fprem | Fprem1 => {
            c.operation = Operation::PartialRemainder;
            c.truncate = m == Fprem;
            call(e, c, None, None, None);
        }
        Fcmovb | Fcmove | Fcmovbe | Fcmovu | Fcmovnb | Fcmovne | Fcmovnbe | Fcmovnu => {
            c.operation = Operation::ConditionalMove;
            let condition = match m {
                Fcmovb => Condition::Below,
                Fcmove => Condition::Equal,
                Fcmovbe => Condition::BelowOrEqual,
                Fcmovu => Condition::Parity,
                Fcmovnb => Condition::AboveOrEqual,
                Fcmovne => Condition::NotEqual,
                Fcmovnbe => Condition::Above,
                _ => Condition::NotParity,
            };
            let holds = e.local(ValType::I64);
            e.condition(condition);
            e.sink().i64_extend_i32_u().local_set(holds);
            call(e, c, Some(holds), None, None);
        }
        Ffree | Ffreep => {
            c.operation = Operation::Free;
            c.pops = u8::from(m == Ffreep);
            call(e, c, None, None, None);
        }
        Fldcw => {
            c.operation = Operation::LoadControl;
            call(e, c, a, None, None);
        }
        Fnstcw | Fnstsw => {
            let value = e.local(ValType::I64);
            e.get_control(Control::X87Words);
            let mut s = e.sink();
            if m == Fnstsw {
                s.i32_const(u16::BITS as i32).i32_shr_u();
            }
            s.i32_const(u16::MAX.into())
                .i32_and()
                .i64_extend_i32_u()
                .local_set(value);
            match at {
                Some(at) => store_at(e, at, 0, bytes, value),
                None => set_general(e, general(ins.op_register(0)).expect("ax"), value),
            }
        }
        // The 8087's and 287's controls do nothing on later x87s.
        Fnop | Wait | Fneni | Fndisi | Fnsetpm => {}
        Fnstenv | Fnsave => {
            environment_store(e, i, m, at.expect("a memory operand"));
        }
        Fldenv | Frstor => {
            environment_load(e, i, m, at.expect("a memory operand"));
        }
        _ => unreachable!("{m:?} is written"),
    }
    if !control(m) {
        e.this_address(0);
        e.set_control(Control::X87InstructionPointer);
    }
}

/// rflags from `fcomi`'s result: ZF, PF and CF as it gives them, OF, SF and AF clear.
fn compare_flags<E: Emitter<X86_64> + ?Sized>(e: &mut E, result: u32) {
    let each = Flag::ALL.map(|_| e.local(ValType::I32));
    for (flag, &local) in Flag::ALL.iter().zip(&each) {
        let bit = match flag {
            Flag::Z => ZF,
            Flag::P => PF,
            Flag::C => CF,
            _ => 0,
        };
        e.sink()
            .local_get(result)
            .i64_const(bit as i64)
            .i64_and()
            .i64_const(0)
            .i64_ne()
            .local_set(local);
    }
    let each = each_with(e, &Flag::ALL.iter().copied().zip(each).collect::<Vec<_>>());
    e.set_flags(Flags::Each(each));
}

/// The protected-mode environment's layout (Intel SDM Vol. 1 8.1.10, figures 8-9 and 8-10): the control,
/// status and tag words, each in a doubleword whose reserved half is all ones in the 32-bit form, then the
/// instruction pointer, its selector (FOP above it in the 32-bit form), the data pointer and its selector.
struct Layout {
    word: u32,
    ip: u64,
    selector: u64,
    data: u64,
    data_selector: u64,
    size: u64,
}

const ENVIRONMENT_32: Layout = Layout {
    word: size_of::<u32>() as u32,
    ip: 12,
    selector: 16,
    data: 20,
    data_selector: 24,
    size: crate::x87::ENVIRONMENT_32_BYTES as u64,
};
const ENVIRONMENT_16: Layout = Layout {
    word: size_of::<u16>() as u32,
    ip: 6,
    selector: 8,
    data: 10,
    data_selector: 12,
    size: crate::x87::ENVIRONMENT_16_BYTES as u64,
};
/// The bytes of each register in the saved state, ST(0) first.
const REGISTER_BYTES: u64 = (crate::X87_REGISTER_BITS / u8::BITS) as u64;
/// What a reserved half of the 32-bit form holds.
const RESERVED_HALF: i64 = (u16::MAX as i64) << u16::BITS;

impl Layout {
    fn of(i: &Instruction) -> &'static Layout {
        match i.iced.memory_size() {
            MemorySize::FpuEnv14 | MemorySize::FpuState94 => &ENVIRONMENT_16,
            _ => &ENVIRONMENT_32,
        }
    }

    fn wide(&self) -> bool {
        self.word > u16::BITS / u8::BITS
    }
}

/// For each physical register R(n), `each` with its number and the address in a saved state at `at` of
/// the slot of ST((n - TOP) mod 8), `stride` bytes a slot, by the TOP the status word holds now: the
/// registers move in place, without the x87 hook.
fn by_stack_position<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    at: u32,
    stride: u64,
    mut each: impl FnMut(&mut E, u8, u32),
) {
    let (top, slot) = (e.local(ValType::I64), e.local(ValType::I64));
    e.get_control(Control::X87StatusTag);
    e.sink()
        .i32_const(TOP_SHIFT.into())
        .i32_shr_u()
        .i64_extend_i32_u()
        .local_set(top);
    for n in 0..REGISTERS as u8 {
        e.sink()
            .i64_const(n.into())
            .local_get(top)
            .i64_sub()
            .i64_const(REGISTER_MASK as i64)
            .i64_and()
            .i64_const(stride as i64)
            .i64_mul()
            .local_get(at)
            .i64_add()
            .local_set(slot);
        each(e, n, slot);
    }
}

/// Store R(n)'s 80 bits at `slot` plus `offset`, its sign and exponent `high` bytes long.
fn store_register<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    n: u8,
    slot: u32,
    offset: u64,
    high: u32,
) {
    let value = e.local(ValType::I64);
    e.get(mmx_register(n));
    e.sink().local_set(value);
    store_at(e, slot, offset, VALUE_LOW, value);
    e.get_control(Control::X87High(n));
    e.sink().i64_extend_i32_u().local_set(value);
    store_at(e, slot, offset + u64::from(VALUE_LOW), high, value);
}

/// Load R(n)'s 80 bits from `slot` plus `offset`.
fn load_register<E: Emitter<X86_64> + ?Sized>(e: &mut E, n: u8, slot: u32, offset: u64) {
    load_at(e, slot, offset, VALUE_LOW);
    e.set(mmx_register(n));
    load_at(e, slot, offset + u64::from(VALUE_LOW), VALUE_HIGH);
    e.sink().i32_wrap_i64();
    e.set_control(Control::X87High(n));
}

/// `fnstenv` and `fnsave`: the registers (for `fnsave`) and the environment as they were before the
/// instruction masks every exception or initialises the unit.
fn environment_store<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    m: Mnemonic,
    at: u32,
) {
    let layout = Layout::of(i);
    let save = m == Mnemonic::Fnsave;
    if save {
        by_stack_position(e, at, REGISTER_BYTES, |e, n, slot| {
            store_register(e, n, slot, layout.size, VALUE_HIGH)
        });
    }
    let fip = e.local(ValType::I64);
    e.get_control(Control::X87InstructionPointer);
    e.sink().local_set(fip);
    let c = Code::new(if save {
        Operation::Save
    } else {
        Operation::StoreEnvironment
    });
    let (words, fdp) = call(e, c, None, None, None);
    let value = e.local(ValType::I64);
    let reserved = if layout.wide() { RESERVED_HALF } else { 0 };
    for k in 0..ENVIRONMENT_WORDS {
        e.sink()
            .local_get(words)
            .i64_const((k * u16::BITS).into())
            .i64_shr_u()
            .i64_const(u16::MAX.into())
            .i64_and()
            .i64_const(reserved)
            .i64_or()
            .local_set(value);
        store_at(e, at, u64::from(k * layout.word), layout.word, value);
    }
    let (ip, selector, data, data_selector) = (
        layout.ip,
        layout.selector,
        layout.data,
        layout.data_selector,
    );
    store_at(e, at, ip, layout.word, fip);
    // The selectors are zero: the reference deprecates FCS and FDS.
    if layout.wide() {
        e.sink()
            .local_get(words)
            .i64_const(FOP_SHIFT.into())
            .i64_shr_u()
            .i64_const(u16::BITS.into())
            .i64_shl()
            .local_set(value);
    } else {
        e.sink().i64_const(0).local_set(value);
    }
    store_at(e, at, selector, layout.word, value);
    store_at(e, at, data, layout.word, fdp);
    e.sink().i64_const(reserved).local_set(value);
    store_at(e, at, data_selector, layout.word, value);
}

/// `fldenv` and `frstor`: the environment, then (for `frstor`) the registers by the TOP just loaded.
fn environment_load<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic, at: u32) {
    let layout = Layout::of(i);
    let words = e.local(ValType::I64);
    e.sink().i64_const(0).local_set(words);
    for k in 0..ENVIRONMENT_WORDS {
        load_at(e, at, u64::from(k * layout.word), VALUE_HIGH);
        e.sink()
            .i64_const((k * u16::BITS).into())
            .i64_shl()
            .local_get(words)
            .i64_or()
            .local_set(words);
    }
    let (ip, selector, data) = (layout.ip, layout.selector, layout.data);
    if layout.wide() {
        load_at(e, at, selector, layout.word);
        e.sink()
            .i64_const(u16::BITS.into())
            .i64_shr_u()
            .i64_const((1 << OPCODE_BITS) - 1)
            .i64_and()
            .i64_const(FOP_SHIFT.into())
            .i64_shl()
            .local_get(words)
            .i64_or()
            .local_set(words);
    }
    let (fip, fdp) = (e.local(ValType::I64), e.local(ValType::I64));
    load_at(e, at, ip, layout.word);
    e.sink().local_set(fip);
    load_at(e, at, data, layout.word);
    e.sink().local_set(fdp);
    call(
        e,
        Code::new(Operation::LoadEnvironment),
        Some(words),
        Some(fip),
        Some(fdp),
    );
    if m == Mnemonic::Frstor {
        by_stack_position(e, at, REGISTER_BYTES, |e, n, slot| {
            load_register(e, n, slot, layout.size)
        });
    }
}

/// The FXSAVE image's offsets (Intel SDM Vol. 1 10.5.1, table 10-2): the x87 words and FOP, the pointers,
/// MXCSR and its mask, the registers ST(0) first, the xmm registers.
const IMAGE_POINTER: u64 = core::mem::offset_of!(crate::Fxsave, fip) as u64;
const IMAGE_DATA: u64 = core::mem::offset_of!(crate::Fxsave, fdp) as u64;
const IMAGE_MXCSR: u64 = core::mem::offset_of!(crate::Fxsave, mxcsr) as u64;
const IMAGE_MXCSR_MASK: u64 = core::mem::offset_of!(crate::Fxsave, mxcsr_mask) as u64;
const IMAGE_REGISTERS: u64 = core::mem::offset_of!(crate::Fxsave, st) as u64;
const IMAGE_XMM: u64 = core::mem::offset_of!(crate::Fxsave, xmm) as u64;
const IMAGE_SLOT: u64 = size_of::<[u8; 16]>() as u64;
/// The reserved byte after the abridged tag word, which the image holds as zero.
const HEADER_KEPT: i64 =
    !((u8::MAX as i64) << (core::mem::offset_of!(crate::Fxsave, reserved0) as u32 * u8::BITS));

/// `fxsave` and `fxrstor`: the x87, MXCSR and xmm state to and from a 16-byte aligned image; without REX.W
/// the pointers are 32 bits and their selectors zero. Neither waits for a pending exception.
fn image<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    // A non-canonical first byte faults first, then a misaligned address, then a non-canonical last byte,
    // then the pages (scratch `x7/probes/fxsave_order.c`).
    let at = address(e, i);
    let kind = memory_use(i).expect("a memory operand");
    let bytes = memory_bytes(i);
    let last = e.local(ValType::I64);
    e.sink()
        .local_get(at)
        .i64_const((bytes - 1).into())
        .i64_add()
        .local_set(last);
    let not_canonical = |e: &mut E, end: u32| {
        e.sink()
            .local_get(end)
            .i64_const(crate::CANONICAL_SHIFT.into())
            .i64_shl()
            .i64_const(crate::CANONICAL_SHIFT.into())
            .i64_shr_s()
            .local_get(end)
            .i64_ne()
            .if_(BlockType::Empty);
        e.check(at, bytes, check_kind(kind), 0);
        e.sink().end();
    };
    not_canonical(e, at);
    crate::simd::aligned16(e, at);
    not_canonical(e, last);
    check_ends(e, i, at, memory_bytes(i), check_kind(kind), None);
    let wide = matches!(m, Mnemonic::Fxsave64 | Mnemonic::Fxrstor64);
    let pointer_bytes = if wide {
        VALUE_LOW
    } else {
        size_of::<u32>() as u32
    };
    let value = e.local(ValType::I64);
    if matches!(m, Mnemonic::Fxsave | Mnemonic::Fxsave64) {
        e.get_control(Control::X87Header);
        e.sink().local_set(value);
        store_at(e, at, 0, VALUE_LOW, value);
        for (control, offset) in [
            (Control::X87InstructionPointer, IMAGE_POINTER),
            (Control::X87DataPointer, IMAGE_DATA),
        ] {
            e.get_control(control);
            e.sink().local_set(value);
            if !wide {
                e.sink()
                    .local_get(value)
                    .i64_const(u32::MAX.into())
                    .i64_and()
                    .local_set(value);
            }
            store_at(e, at, offset, VALUE_LOW, value);
        }
        crate::float::read_mxcsr(e, value);
        store_at(e, at, IMAGE_MXCSR, size_of::<u32>() as u32, value);
        e.sink()
            .i64_const(crate::MXCSR_WRITABLE.into())
            .local_set(value);
        store_at(e, at, IMAGE_MXCSR_MASK, size_of::<u32>() as u32, value);
        // The slot's bytes past the 80 bits are zero.
        by_stack_position(e, at, IMAGE_SLOT, |e, n, slot| {
            store_register(e, n, slot, IMAGE_REGISTERS, VALUE_LOW)
        });
        for x in 0..crate::guest::XMM_COUNT as u8 {
            e.get_vector(x);
            let v = e.local(ValType::V128);
            e.sink().local_set(v);
            let offset = IMAGE_XMM + u64::from(x) * IMAGE_SLOT;
            e.sink()
                .local_get(at)
                .local_get(v)
                .v128_store(MemArg { offset, ..memory() });
        }
        return;
    }
    // A reserved bit in MXCSR is general protection, before anything changes.
    let mxcsr = e.local(ValType::I64);
    load_at(e, at, IMAGE_MXCSR, size_of::<u32>() as u32);
    e.sink()
        .local_tee(mxcsr)
        .i64_const(!i64::from(crate::MXCSR_WRITABLE))
        .i64_and()
        .i64_const(0)
        .i64_ne()
        .if_(BlockType::Empty);
    e.trap(Trap::GeneralProtection);
    e.sink().end();
    load_at(e, at, 0, VALUE_LOW);
    e.sink().i64_const(HEADER_KEPT).i64_and();
    e.set_control(Control::X87Header);
    for (control, offset) in [
        (Control::X87InstructionPointer, IMAGE_POINTER),
        (Control::X87DataPointer, IMAGE_DATA),
    ] {
        load_at(e, at, offset, pointer_bytes);
        e.set_control(control);
    }
    // MXCSR as restored decides the path; tracking stays as it was (plan 5.8).
    e.sink().local_get(mxcsr).i32_wrap_i64();
    e.set_control(Control::Mxcsr);
    by_stack_position(e, at, IMAGE_SLOT, |e, n, slot| {
        load_register(e, n, slot, IMAGE_REGISTERS)
    });
    for x in 0..crate::guest::XMM_COUNT as u8 {
        let offset = IMAGE_XMM + u64::from(x) * IMAGE_SLOT;
        e.sink()
            .local_get(at)
            .v128_load(MemArg { offset, ..memory() });
        e.set_vector(x);
    }
}
