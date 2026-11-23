//! The inside's answers to instructions of the Kernel route (plan 3.5), in plain Rust over the thread's block, as
//! their meanings give them.

use crate::cpuid::{self, ECX, Registers};
use crate::user_segments::{
    CR0, DUMMY_GDT_BASE, DUMMY_IDT_BASE, DUMMY_TABLE_LIMIT, NO_LOCAL_TABLE, SEGMENTS,
    SELECTOR_PRIVILEGE, TASK_STATE, USER_CS, USER_DS,
};
use crate::x87::{Code, EXCEPTIONS, Unit};
use crate::{
    ALIGNMENT_CHECK, ARITHMETIC_FLAGS, DEFAULT32, DF, ID, NT, Offer, QUIET32, RAX, RBX, RCX, RDX,
    RSP, SegmentRegister, TF, X86Block, XMM_BYTES, ZF,
};
use wcontainers_guest_cpu::position::{Base, Operand, Segment};
use zerocopy::IntoBytes;

/// astro-float's cache of constants, which the transcendental instructions' answers work with.
pub use astro_float_num::Consts;

/// The fraction bits of each format, the exponent biases, and single precision's sign and exponent bits.
pub(crate) const FRACTION32: u32 = f32::MANTISSA_DIGITS - 1;
pub(crate) const FRACTION64: u32 = f64::MANTISSA_DIGITS - 1;
pub(crate) const BIAS32: i32 = f32::MAX_EXP - 1;
pub(crate) const BIAS64: i64 = f64::MAX_EXP as i64 - 1;
pub(crate) const SIGN32: u32 = 1 << (u32::BITS - 1);
pub(crate) const EXPONENT32: u32 = f32::INFINITY.to_bits();
/// The bits of Intel's approximations, and how many of the operand's top fraction bits choose the
/// interval they are worked out at (Intel SDM Vol. 2 RCPPS, RSQRTPS: |relative error| <= 1.5 * 2^-12).
pub(crate) const ESTIMATE_BITS: u32 = 12;
pub(crate) const RCP_INDEX_BITS: u32 = 11;
pub(crate) const RSQRT_INDEX_BITS: u32 = 10;
/// The bits outside the six flags that `popf` writes at CPL 3; it leaves IOPL, IF, RF, VIF and VIP, and AC
/// is never held (section 5.1 of the x86 plan).
pub(crate) const POPPED_CONTROL: u64 = TF | DF | NT | ID;

/// The guest's memory as an answer reaches it; an access that cannot be made gives the first address it may not
/// make and changes nothing.
pub trait Memory {
    fn read(&mut self, at: u64, into: &mut [u8]) -> Result<(), u64>;
    fn write(&mut self, at: u64, bytes: &[u8]) -> Result<(), u64>;
}

/// Where an operand of an answered instruction is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    General {
        number: u8,
        bits: u8,
    },
    /// An xmm register, or the ymm register of the same number.
    Vector(u8),
    Memory(Operand),
}

/// How a vector instruction is encoded, which says what it leaves of the registers' upper halves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VectorEncoding {
    Legacy,
    Vex128,
    Vex256,
}

/// The descriptor-table instructions user mode may run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescriptorRead {
    LocalTable,
    TaskRegister,
    MachineStatus,
    GlobalTable,
    InterruptTable,
    /// `lar` into this general register.
    Rights {
        number: u8,
        bits: u8,
    },
    /// `lsl` into this general register.
    Limit {
        number: u8,
        bits: u8,
    },
    Readable,
    Writable,
}

/// `rcpps`, `rcpss`, `rsqrtps`, `rsqrtss` or one of their VEX forms, whose scalar forms take the other lanes from
/// `merged`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reciprocal {
    pub root: bool,
    pub scalar: bool,
    pub encoding: VectorEncoding,
    pub destination: u8,
    pub merged: u8,
    pub source: Place,
}

/// A segment register as an answer names it: cs and ss always hold Linux's 64-bit user segments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentName {
    Code,
    Stack,
    Kept(SegmentRegister),
}

/// An instruction of the Kernel route that [`answer`] answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Question {
    Cpuid,
    Xgetbv,
    /// An x87 instruction with no memory operand, run by the unit as its code says.
    X87(Code),
    Reciprocal(Reciprocal),
    /// `popf` of this many bytes.
    PopFlags {
        bytes: u8,
    },
    /// A descriptor-table instruction; `operand` is the selector it reads, or where it stores.
    Descriptor {
        read: DescriptorRead,
        operand: Place,
    },
    /// `mov r/m, sreg`.
    SegmentRead {
        from: SegmentName,
        to: Place,
    },
    /// `mov sreg, r/m` of a segment register other than cs, which iced decodes as no instruction.
    SegmentWrite {
        to: SegmentName,
        from: Place,
    },
    /// `push` of fs or gs, of this many bytes.
    SegmentPush {
        from: SegmentName,
        bytes: u8,
    },
    /// `pop` of fs or gs, of this many bytes.
    SegmentPop {
        to: SegmentName,
        bytes: u8,
    },
    /// `lfs`, `lgs` or `lss`: the offset of `offset_bytes` bytes into a general register, then the selector after
    /// it into `to`.
    FarPointer {
        to: SegmentName,
        number: u8,
        offset_bytes: u8,
        at: Operand,
    },
}

/// How an answered instruction stops instead of finishing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    GeneralProtection,
    /// A pending x87 exception, with the status word's flags that are raised and unmasked.
    FloatException(u16),
    /// The first address an access may not make, which the guest's `memory_fault` turns into its fault.
    Memory(u64),
}

/// What the inside reports of a finished answer, beside the guest's new state: the meaning's warnings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Notes {
    /// `cpuid` read the features AVX is told by, while the process is told no AVX.
    pub avx_asked_and_told_no: bool,
    /// `popf` set TF: single stepping starts after this instruction.
    pub trap_flag_set: bool,
    /// `popf` asked for alignment checking, which is never held.
    pub alignment_check_not_provided: bool,
    /// A `mov` to ss: a trap is held back over the next instruction.
    pub step_held: bool,
}

/// Answers `question` on `block` for a process told of `offer`'s features. On `Ok` the instruction has finished
/// and the caller moves rip past it; on a fault nothing has changed. Rip is the instruction's own address.
pub fn answer(
    question: Question,
    block: &mut X86Block,
    offer: Offer,
    constants: &mut Consts,
    memory: &mut impl Memory,
) -> Result<Notes, Fault> {
    let general = |n: u8| usize::from(n);
    let mut notes = Notes::default();
    match question {
        Question::Cpuid => {
            let (leaf, sub_leaf) = (
                cpuid::effective_leaf(block.gpr[general(RAX)] as u32),
                block.gpr[general(RCX)] as u32,
            );
            for (n, value) in [RAX, RBX, RCX, RDX]
                .into_iter()
                .zip(told(offer, leaf, sub_leaf))
            {
                block.gpr[general(n)] = value.into();
            }
            notes.avx_asked_and_told_no = !offer.avx() && cpuid::asks_for_avx(leaf, sub_leaf);
        }
        Question::Xgetbv => {
            if block.gpr[general(RCX)] as u32 != 0 {
                return Err(Fault::GeneralProtection);
            }
            block.gpr[general(RAX)] = cpuid::enabled_state(offer).into();
            block.gpr[general(RDX)] = 0;
        }
        Question::X87(code) => {
            let area = &block.fxsave;
            let raised = area.fsw & !area.fcw & EXCEPTIONS;
            if raised != 0 {
                return Err(Fault::FloatException(raised));
            }
            Unit::new(block.fxsave.as_mut_bytes(), code, 0, constants).run(0, 0);
        }
        Question::Reciprocal(form) => reciprocal(block, memory, form)?,
        Question::PopFlags { bytes } => {
            let from = block.gpr[general(RSP)];
            let mut word = [0; size_of::<u64>()];
            memory
                .read(from, &mut word[..usize::from(bytes)])
                .map_err(Fault::Memory)?;
            let value = u64::from_le_bytes(word);
            block.gpr[general(RSP)] = from.wrapping_add(bytes.into());
            let written = POPPED_CONTROL & low_bits(u32::from(bytes) * u8::BITS);
            block.rflags = (block.rflags & !written) | (value & written);
            block.flags = (value & ARITHMETIC_FLAGS) as u32;
            notes.trap_flag_set = value & TF != 0;
            notes.alignment_check_not_provided = value & ALIGNMENT_CHECK != 0;
        }
        Question::Descriptor { read, operand } => descriptor(block, memory, read, operand)?,
        Question::SegmentRead { from, to } => {
            let value = selector(block, from).into();
            match to {
                Place::General { number, bits } => set_general(block, number, bits, value),
                Place::Memory(at) => memory
                    .write(address(block, &at), &(value as u16).to_le_bytes())
                    .map_err(Fault::Memory)?,
                Place::Vector(_) => return Err(Fault::GeneralProtection),
            }
        }
        Question::SegmentWrite { to, from } => {
            let value = match from {
                Place::General { number, .. } => block.gpr[usize::from(number)] as u16,
                Place::Memory(at) => read_selector(memory, address(block, &at))?,
                Place::Vector(_) => return Err(Fault::GeneralProtection),
            };
            load_segment(block, to, value)?;
            notes.step_held = to == SegmentName::Stack;
        }
        Question::SegmentPush { from, bytes } => {
            let value = u64::from(selector(block, from)).to_le_bytes();
            let at = block.gpr[general(RSP)].wrapping_sub(bytes.into());
            memory
                .write(at, &value[..usize::from(bytes)])
                .map_err(Fault::Memory)?;
            block.gpr[general(RSP)] = at;
        }
        Question::SegmentPop { to, bytes } => {
            let from = block.gpr[general(RSP)];
            let value = read_selector(memory, from)?;
            load_segment(block, to, value)?;
            block.gpr[general(RSP)] = from.wrapping_add(bytes.into());
        }
        Question::FarPointer {
            to,
            number,
            offset_bytes,
            at,
        } => {
            let at = address(block, &at);
            let mut offset = [0; size_of::<u64>()];
            memory
                .read(at, &mut offset[..usize::from(offset_bytes)])
                .map_err(Fault::Memory)?;
            let value = read_selector(memory, at.wrapping_add(offset_bytes.into()))?;
            load_segment(block, to, value)?;
            set_general(
                block,
                number,
                offset_bytes * u8::BITS as u8,
                u64::from_le_bytes(offset),
            );
        }
    }
    Ok(notes)
}

/// The address of the first byte `operand` names, the instruction's own address in rip.
pub fn address(block: &X86Block, operand: &Operand) -> u64 {
    let value = |base: Base| match base {
        Base::Register(n) => block.gpr[usize::from(n)],
        Base::StackPointer => block.gpr[usize::from(RSP)],
        Base::Zero => 0,
        Base::Pc => block.rip,
    };
    let mut sum = value(operand.base).wrapping_add_signed(operand.offset);
    if let Some(index) = operand.index {
        let unused = u64::BITS - u32::from(index.bits);
        let moved = value(index.register) << unused;
        let extended = if index.signed {
            ((moved as i64) >> unused) as u64
        } else {
            moved >> unused
        };
        sum = sum.wrapping_add(extended << index.shift);
    }
    sum &= low_bits(operand.address_bits.into());
    sum.wrapping_add(match operand.segment {
        Some(Segment::Fs) => block.fs_base,
        Some(Segment::Gs) => block.gs_base,
        None => 0,
    })
}

/// The selector in a segment register.
fn selector(block: &X86Block, name: SegmentName) -> u16 {
    match name {
        SegmentName::Code => USER_CS,
        SegmentName::Stack => USER_DS,
        SegmentName::Kept(r) => block.selectors[r as usize],
    }
}

/// The 16 bits of a selector at `at`, the only ones the CPU reads of its slot.
fn read_selector(memory: &mut impl Memory, at: u64) -> Result<u16, Fault> {
    let mut word = [0; size_of::<u16>()];
    memory.read(at, &mut word).map_err(Fault::Memory)?;
    Ok(u16::from_le_bytes(word))
}

/// Loads a segment register with `value`, or general protection where Linux's tables give user mode no segment
/// it may load there: a null selector or a readable segment for ds, es, fs and gs, the data segment at privilege
/// level 3 alone for ss. Loading fs or gs makes its base 0, every user segment's base, and Intel's for a null
/// selector.
fn load_segment(block: &mut X86Block, name: SegmentName, value: u16) -> Result<(), Fault> {
    let names = |selector: u16| value & !SELECTOR_PRIVILEGE == selector & !SELECTOR_PRIVILEGE;
    let allowed = match name {
        SegmentName::Stack => value == USER_DS,
        _ => names(0) || SEGMENTS.iter().any(|segment| names(segment.selector)),
    };
    if !allowed {
        return Err(Fault::GeneralProtection);
    }
    if let SegmentName::Kept(r) = name {
        block.selectors[r as usize] = value;
        match r {
            SegmentRegister::Fs => block.fs_base = 0,
            SegmentRegister::Gs => block.gs_base = 0,
            SegmentRegister::Ds | SegmentRegister::Es => {}
        }
    }
    Ok(())
}

/// A mask of the low `bits` bits.
fn low_bits(bits: u32) -> u64 {
    u64::MAX.checked_shr(u64::BITS - bits).unwrap_or(0)
}

/// Writes `value` to a general register of `bits` bits as an instruction does: a 32-bit write clears the upper
/// half, a narrower one keeps the bits above it.
fn set_general(block: &mut X86Block, number: u8, bits: u8, value: u64) {
    let register = &mut block.gpr[usize::from(number)];
    let kept = if u32::from(bits) >= u32::BITS {
        0
    } else {
        !low_bits(bits.into())
    };
    *register = (*register & kept) | (value & low_bits(bits.into()));
}

/// The xmm register `n` and the upper half of its ymm register, side by side.
fn vector(block: &X86Block, n: u8) -> [u8; 2 * XMM_BYTES as usize] {
    let mut out = [0; 2 * XMM_BYTES as usize];
    let (low, high) = out.split_at_mut(XMM_BYTES as usize);
    low.copy_from_slice(&block.fxsave.xmm[usize::from(n)]);
    high.copy_from_slice(&block.ymm_high[usize::from(n)]);
    out
}

fn reciprocal(
    block: &mut X86Block,
    memory: &mut impl Memory,
    form: Reciprocal,
) -> Result<(), Fault> {
    let Reciprocal {
        root,
        scalar,
        encoding,
        destination,
        merged,
        source,
    } = form;
    let lane = size_of::<u32>();
    let lanes = if scalar {
        1
    } else if encoding == VectorEncoding::Vex256 {
        2 * XMM_BYTES as usize / lane
    } else {
        XMM_BYTES as usize / lane
    };
    let input = match source {
        Place::Vector(n) => vector(block, n),
        Place::Memory(operand) => {
            let at = address(block, &operand);
            if encoding == VectorEncoding::Legacy
                && !scalar
                && !at.is_multiple_of(u64::from(XMM_BYTES))
            {
                return Err(Fault::GeneralProtection);
            }
            let mut bytes = [0; 2 * XMM_BYTES as usize];
            memory
                .read(at, &mut bytes[..lanes * lane])
                .map_err(Fault::Memory)?;
            bytes
        }
        Place::General { .. } => return Err(Fault::GeneralProtection),
    };
    let mut out = vector(block, merged);
    for k in 0..lanes {
        let x = u32::from_le_bytes(input[k * lane..][..lane].try_into().expect("a lane"));
        out[k * lane..][..lane].copy_from_slice(&estimate(x, root).to_le_bytes());
    }
    let (low, high) = out.split_at(XMM_BYTES as usize);
    block.fxsave.xmm[usize::from(destination)].copy_from_slice(low);
    match encoding {
        VectorEncoding::Legacy => {}
        VectorEncoding::Vex128 => {
            block.ymm_high[usize::from(destination)] = [0; XMM_BYTES as usize]
        }
        VectorEncoding::Vex256 => block.ymm_high[usize::from(destination)].copy_from_slice(high),
    }
    Ok(())
}

/// Intel's `rcpps` (or `rsqrtps`) of one single precision lane by plan 5.8's formula, as `float::reciprocal`
/// gives it: the estimate at the middle of the operand's interval, rounded to 12 bits, in f64 arithmetic.
pub fn estimate(x: u32, root: bool) -> u32 {
    let index_bits = if root {
        RSQRT_INDEX_BITS
    } else {
        RCP_INDEX_BITS
    };
    let index = ((1u64 << index_bits) - 1) << (FRACTION32 - index_bits);
    let half_step = 1u64 << (FRACTION64 - index_bits - 1);
    let scale = f64::from(1u32 << (ESTIMATE_BITS + 1));
    let fraction_scale = f64::from(1u32 << (FRACTION32 - ESTIMATE_BITS));
    let exponent = (x >> FRACTION32) & u32::from(u8::MAX);
    let wide = u64::from(x);
    let middle =
        ((wide & index) << (FRACTION64 - FRACTION32)) | half_step | ((BIAS64 as u64) << FRACTION64);
    let middle = if root {
        // 2^b, b the parity of the unbiased exponent: set when the exponent field is even.
        let even = ((wide >> FRACTION32) & 1) ^ 1;
        libm::sqrt(f64::from_bits(middle + (even << FRACTION64)))
    } else {
        f64::from_bits(middle)
    };
    let units = (libm::rint(scale / middle) * fraction_scale) as i32;
    let e = exponent as i32;
    let field = if root {
        (BIAS32 - 2) - ((e - BIAS32 - ((e & 1) ^ 1)) >> 1)
    } else {
        2 * BIAS32 - 2 - e
    };
    let sign = x & SIGN32;
    let mut out = (field << FRACTION32).wrapping_add(units) as u32 | sign;
    if root {
        // Negative numbers give the default not-a-number; zeros and denormals a signed infinity; +inf +0.
        if (x as i32) < 0 {
            out = DEFAULT32;
        }
        if exponent == 0 {
            out = sign | EXPONENT32;
        }
        if x == EXPONENT32 {
            out = 0;
        }
    } else {
        // Results below the normal range are a signed zero; zeros and denormals give a signed infinity.
        if exponent >= (2 * BIAS32 - 1) as u32 {
            out = sign;
        }
        if exponent == 0 {
            out = sign | EXPONENT32;
        }
    }
    if f32::from_bits(x).is_nan() {
        out = x | QUIET32;
    }
    out
}

fn descriptor(
    block: &mut X86Block,
    memory: &mut impl Memory,
    read: DescriptorRead,
    operand: Place,
) -> Result<(), Fault> {
    use DescriptorRead::*;
    let stored = match read {
        LocalTable => Some(u64::from(NO_LOCAL_TABLE)),
        TaskRegister => Some(u64::from(TASK_STATE)),
        MachineStatus => Some(u64::from(CR0)),
        _ => None,
    };
    if let Some(stored) = stored {
        match operand {
            Place::General { number, bits } => set_general(block, number, bits, stored),
            Place::Memory(at) => memory
                .write(address(block, &at), &(stored as u16).to_le_bytes())
                .map_err(Fault::Memory)?,
            Place::Vector(_) => return Err(Fault::GeneralProtection),
        }
        return Ok(());
    }
    if let GlobalTable | InterruptTable = read {
        let Place::Memory(at) = operand else {
            return Err(Fault::GeneralProtection);
        };
        let base = if read == GlobalTable {
            DUMMY_GDT_BASE
        } else {
            DUMMY_IDT_BASE
        };
        let mut image = [0; size_of::<u16>() + size_of::<u64>()];
        let (limit, rest) = image.split_at_mut(size_of::<u16>());
        limit.copy_from_slice(&DUMMY_TABLE_LIMIT.to_le_bytes());
        rest.copy_from_slice(&base.to_le_bytes());
        return memory
            .write(address(block, &at), &image)
            .map_err(Fault::Memory);
    }
    let selector = match operand {
        Place::General { number, .. } => block.gpr[usize::from(number)] as u16,
        Place::Memory(at) => read_selector(memory, address(block, &at))?,
        Place::Vector(_) => return Err(Fault::GeneralProtection),
    };
    let named = SEGMENTS.iter().find(|segment| {
        (read != Writable || segment.writable)
            && selector & !SELECTOR_PRIVILEGE == segment.selector & !SELECTOR_PRIVILEGE
    });
    if let Some(segment) = named {
        match read {
            Rights { number, bits } => set_general(block, number, bits, segment.rights.into()),
            Limit { number, bits } => set_general(block, number, bits, segment.limit.into()),
            _ => {}
        }
    }
    let zero = ZF as u32;
    block.flags = (block.flags & !zero) | if named.is_some() { zero } else { 0 };
    Ok(())
}

/// `cpuid`'s registers for an effective leaf: a leaf of no table none, a sub-leaf past its table the table's
/// last answer with ecx echoing its low byte where the leaf does.
fn told(offer: Offer, leaf: u32, sub_leaf: u32) -> Registers {
    if !cpuid::leaves().any(|l| l == leaf) {
        return Registers::default();
    }
    let subs = cpuid::sub_leaves(leaf);
    if sub_leaf < subs {
        return cpuid::answer(offer, leaf, sub_leaf);
    }
    let mut out = cpuid::answer(offer, leaf, subs);
    if cpuid::echoes_sub_leaf(leaf) {
        out[ECX] = sub_leaf & u32::from(u8::MAX);
    }
    out
}
