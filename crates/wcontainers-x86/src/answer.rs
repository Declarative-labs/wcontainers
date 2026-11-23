//! The inside's answers to instructions of the Kernel route (plan 3.5), in plain Rust over the thread's block, as
//! their meanings give them.

use crate::cpuid::{self, ECX, Registers, XSAVE_AVX, XSAVE_SSE, XSAVE_X87};
use crate::user_segments::{
    CR0, DUMMY_GDT_BASE, DUMMY_IDT_BASE, DUMMY_TABLE_LIMIT, NO_LOCAL_TABLE, SEGMENTS,
    SELECTOR_PRIVILEGE, TASK_STATE, USER_CS, USER_DS, USER32_CS,
};
use crate::x87::{
    Code, ENVIRONMENT_16, ENVIRONMENT_32, ENVIRONMENT_32_BYTES, EXCEPTIONS, NOT_STORED,
    OPCODE_BITS, REGISTER_MASK, STORED_STATUS_SHIFT, STORED_TAGS_SHIFT, TOP_SHIFT, Unit, VALUE_LOW,
};
use crate::{
    ALIGNMENT_CHECK, ARITHMETIC_FLAGS, DEFAULT32, DF, FCW_AT_START, Fxsave, ID, MXCSR_WRITABLE, NT,
    Offer, QUIET32, RAX, RBP, RBX, RCX, RDX, RSP, SegmentRegister, TF, X86Block, XMM_BYTES, ZF,
    is_canonical,
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
    /// Whether `bytes` bytes at `at` may be read, or written with `write`, without reaching them.
    fn check(&mut self, at: u64, bytes: u64, write: bool) -> Result<(), u64>;
}

/// The XSAVE header's XSTATE_BV after the legacy area, and the header's words after it that the standard form
/// of `xrstor` requires to be zero (XCOMP_BV and the next; Intel SDM Vol. 1 13.8.1).
pub(crate) const XSTATE_BV: usize = size_of::<Fxsave>();
pub(crate) const XSAVE_HEADER_ZERO: [usize; 2] = [
    XSTATE_BV + size_of::<u64>(),
    XSTATE_BV + 2 * size_of::<u64>(),
];
/// The alignment an XSAVE area needs, and the bytes `xsave` and `xrstor` reach: every component built here.
pub(crate) const XSAVE_ALIGNMENT: u64 = 64;
pub(crate) const XSTATE_BYTES: usize =
    cpuid::xsave_bytes(XSAVE_X87 | XSAVE_SSE | XSAVE_AVX) as usize;
/// The x87 header's bits an image keeps: all but the reserved byte after the abridged tag word.
pub(crate) const HEADER_KEPT: u64 =
    !((u8::MAX as u64) << (core::mem::offset_of!(Fxsave, reserved0) as u32 * u8::BITS));

/// The clock `rdtsc` reads: nanoseconds, which the guest sees as a 1 GHz counter.
pub trait Clock {
    fn nanoseconds(&mut self) -> u64;
}

/// The slots of a far transfer's frame from its lowest, each of the operand size.
#[derive(Clone, Copy)]
pub(crate) enum FrameSlot {
    Rip,
    Cs,
    Rflags,
    Rsp,
    Ss,
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
    /// `rdtsc`, or `rdtscp` with `processor`.
    TimeStamp {
        processor: bool,
    },
    /// `wrfsbase` or `wrgsbase` from a general register.
    WriteBase {
        gs: bool,
        from: Place,
    },
    /// `retf` with slots of `bytes` bytes, releasing `release` bytes more of the stack.
    FarReturn {
        bytes: u8,
        release: u16,
    },
    /// `iret` with slots of `bytes` bytes.
    InterruptReturn {
        bytes: u8,
    },
    /// A far `jmp`, or with `call` a far `call`, through the pointer at `at` with an offset of `bytes` bytes;
    /// `length` is the instruction's, the return address a call pushes being its end.
    FarBranch {
        call: bool,
        bytes: u8,
        at: Operand,
        length: u8,
    },
    /// `xsave`, or `xrstor` with `restore`, of the area at `at`, its x87 pointers of 64 bits with `wide`.
    XState {
        restore: bool,
        wide: bool,
        at: Operand,
    },
    /// `enter` of a frame of `size` bytes at nesting level `level` (already taken modulo 32), each push `bytes` long.
    Enter {
        size: u16,
        level: u8,
        bytes: u8,
    },
    /// A store of ST(0) to the `bytes` at `at`, run by the unit as its code says.
    X87Store {
        code: Code,
        at: Operand,
        bytes: u8,
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
    /// General protection for a far transfer to Linux's 32-bit user code, which is not provided.
    FarTransferTo32Bit,
}

/// What the inside reports of a finished answer, beside the guest's new state: the meaning's warnings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Notes {
    /// `cpuid` read the features AVX is told by, while the process is told no AVX.
    pub avx_asked_and_told_no: bool,
    /// `popf` or `iret` set TF: single stepping starts after this instruction.
    pub trap_flag_set: bool,
    /// `popf` or `iret` asked for alignment checking, which is never held.
    pub alignment_check_not_provided: bool,
    /// A `mov` to ss: a trap is held back over the next instruction.
    pub step_held: bool,
    /// A far transfer: rip is already its target, and the caller leaves it.
    pub transferred: bool,
    /// MXCSR's flags were read while they are not kept.
    pub mxcsr_flags_read_not_kept: bool,
}

/// An instruction of the Float status route (plan 3.5), which the hardware runs between the inside's two calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloatStatus {
    /// `ldmxcsr` or `vldmxcsr`.
    LoadMxcsr,
    /// `stmxcsr` or `vstmxcsr`.
    StoreMxcsr,
    /// `fxsave`, or `fxrstor` with `restore`, of the image at `at`, its x87 pointers of 64 bits with `wide`.
    Image {
        restore: bool,
        wide: bool,
        at: Operand,
    },
    /// `fnstenv`, or `fnsave` with `registers`; `fldenv` and `frstor` with `restore`; the 28-byte form with `wide`.
    Environment {
        restore: bool,
        registers: bool,
        wide: bool,
        at: Operand,
    },
}

/// The Float status route's second call (plan 3.5). `block` holds the hardware's state just after the instruction
/// but for the x87 pointers and FOP, which are the meaning's from before it (the hardware's hold host addresses and
/// are never read); its rip is the instruction's own. A save's image gets the pointers, FOP and MXCSR's mask as the
/// meaning writes them; a load leaves them in the block as the meaning loads them; `ldmxcsr` keeps MXCSR's flags
/// from then on. A fault is the image's, made unreachable since the hardware reached it.
pub fn float_status_after(
    status: FloatStatus,
    block: &mut X86Block,
    tracking: &mut bool,
    memory: &mut impl Memory,
) -> Result<Notes, Fault> {
    const FOP: usize = core::mem::offset_of!(Fxsave, fop);
    const FIP: usize = core::mem::offset_of!(Fxsave, fip);
    const FDP: usize = core::mem::offset_of!(Fxsave, fdp);
    const MXCSR_MASK: usize = core::mem::offset_of!(Fxsave, mxcsr_mask);
    let mut notes = Notes::default();
    match status {
        FloatStatus::LoadMxcsr => *tracking = true,
        FloatStatus::StoreMxcsr => notes.mxcsr_flags_read_not_kept = !*tracking,
        FloatStatus::Image { restore, wide, at } => {
            let at = address(block, &at);
            let pointer = |p: u64| if wide { p } else { p & u64::from(u32::MAX) };
            let f = &mut block.fxsave;
            // FOP, then the two pointers, are one run of the image's bytes.
            let mut run = [0; FDP + size_of::<u64>() - FOP];
            if restore {
                memory
                    .read(at.wrapping_add(FOP as u64), &mut run)
                    .map_err(Fault::Memory)?;
                let word = |from: usize| {
                    u64::from_le_bytes(run[from - FOP..][..size_of::<u64>()].try_into().unwrap())
                };
                f.fop = u16::from_le_bytes([run[0], run[1]]);
                (f.fip, f.fdp) = (pointer(word(FIP)), pointer(word(FDP)));
            } else {
                notes.mxcsr_flags_read_not_kept = !*tracking;
                run[..size_of::<u16>()].copy_from_slice(&f.fop.to_le_bytes());
                run[FIP - FOP..][..size_of::<u64>()].copy_from_slice(&pointer(f.fip).to_le_bytes());
                run[FDP - FOP..].copy_from_slice(&pointer(f.fdp).to_le_bytes());
                memory
                    .write(at.wrapping_add(FOP as u64), &run)
                    .map_err(Fault::Memory)?;
                memory
                    .write(
                        at.wrapping_add(MXCSR_MASK as u64),
                        &MXCSR_WRITABLE.to_le_bytes(),
                    )
                    .map_err(Fault::Memory)?;
            }
        }
        FloatStatus::Environment {
            restore,
            registers,
            wide,
            at,
        } => {
            let layout = if wide {
                &ENVIRONMENT_32
            } else {
                &ENVIRONMENT_16
            };
            let at = address(block, &at);
            let word = layout.word as usize;
            // The instruction pointer, its selector, the data pointer and its selector are one run of words.
            let first = at.wrapping_add(layout.ip);
            let mut bytes = [0; ENVIRONMENT_32_BYTES as usize];
            let run = &mut bytes[..(layout.size - layout.ip) as usize];
            let field = |offset: u64| (offset - layout.ip) as usize;
            let f = &mut block.fxsave;
            if restore {
                memory.read(first, run).map_err(Fault::Memory)?;
                let read = |offset: u64| {
                    let mut bytes = [0; size_of::<u64>()];
                    bytes[..word].copy_from_slice(&run[field(offset)..][..word]);
                    u64::from_le_bytes(bytes)
                };
                f.fip = read(layout.ip);
                f.fdp = read(layout.data);
                f.fop = if wide {
                    (read(layout.selector) >> u16::BITS) as u16 & ((1 << OPCODE_BITS) - 1)
                } else {
                    0
                };
            } else {
                // The selectors are zero: the reference deprecates FCS and FDS.
                let selector = if wide {
                    u64::from(f.fop) << u16::BITS
                } else {
                    0
                };
                for (offset, value) in [
                    (layout.ip, f.fip),
                    (layout.selector, selector),
                    (layout.data, f.fdp),
                    (layout.data_selector, layout.reserved().into()),
                ] {
                    run[field(offset)..][..word].copy_from_slice(&value.to_le_bytes()[..word]);
                }
                memory.write(first, run).map_err(Fault::Memory)?;
                if registers {
                    // `fnsave` initialises the unit after its store.
                    (f.fip, f.fdp, f.fop) = (0, 0, 0);
                }
            }
        }
    }
    Ok(notes)
}

/// Answers `question` on `block` for a process told of `offer`'s features. On `Ok` the instruction has finished
/// and the caller moves rip past it unless it `transferred`; on a fault nothing has changed but the slots a far
/// `call` wrote before it, as on the CPU. Rip is the instruction's own address.
pub fn answer(
    question: Question,
    block: &mut X86Block,
    offer: Offer,
    constants: &mut Consts,
    memory: &mut (impl Memory + Clock),
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
            pending(block)?;
            Unit::new(block.fxsave.as_mut_bytes(), code, 0, constants).run(0, 0);
            block.fxsave.fip = block.rip;
        }
        Question::X87Store { code, at, bytes } => {
            pending(block)?;
            let at = address(block, &at);
            let [low, high] = Unit::new(block.fxsave.as_mut_bytes(), code, at, constants)
                .run(0, 0)
                .map(|result| result as u64);
            if high & NOT_STORED == 0 {
                let value = (u128::from(high as u16) << u64::BITS | u128::from(low)).to_le_bytes();
                let bytes = usize::from(bytes);
                // An 80-bit value goes in two parts, its first eight bytes written before the last two are reached.
                let first = bytes.min(VALUE_LOW as usize);
                memory.write(at, &value[..first]).map_err(Fault::Memory)?;
                memory
                    .write(at.wrapping_add(first as u64), &value[first..bytes])
                    .map_err(Fault::Memory)?;
                block.fxsave.fsw = (high >> STORED_STATUS_SHIFT) as u16;
                block.fxsave.ftw = (high >> STORED_TAGS_SHIFT) as u8;
            }
            block.fxsave.fip = block.rip;
        }
        Question::Enter { size, level, bytes } => {
            // Each push writes below the last and faults by itself; the registers change only at the end.
            let width = u64::from(bytes);
            let mut frame = block.gpr[general(RSP)];
            let mut frame_pointer = block.gpr[general(RBP)];
            let mut push = |memory: &mut _, value| {
                frame = frame.wrapping_sub(width);
                write_bytes(memory, frame, value, bytes).map(|()| frame)
            };
            let pushed = push(&mut *memory, frame_pointer)?;
            if level > 0 {
                for _ in 1..level {
                    frame_pointer = frame_pointer.wrapping_sub(width);
                    let copied = read_bytes(memory, frame_pointer, bytes)?;
                    push(&mut *memory, copied)?;
                }
                push(&mut *memory, pushed)?;
            }
            // The CPU then checks a write at the final stack pointer (Intel SDM Vol. 2 ENTER).
            let top = frame.wrapping_sub(size.into());
            memory.check(top, width, true).map_err(Fault::Memory)?;
            // A word-sized `enter` writes bp alone, over what the copies left in rbp.
            block.gpr[general(RBP)] = frame_pointer;
            set_general(block, RBP, bytes * u8::BITS as u8, pushed);
            block.gpr[general(RSP)] = top;
        }
        Question::Reciprocal(form) => reciprocal(block, memory, form)?,
        Question::PopFlags { bytes } => {
            let from = block.gpr[general(RSP)];
            let mut word = [0; size_of::<u64>()];
            memory
                .read(from, &mut word[..usize::from(bytes)])
                .map_err(Fault::Memory)?;
            block.gpr[general(RSP)] = from.wrapping_add(bytes.into());
            load_rflags(block, u64::from_le_bytes(word), bytes, &mut notes);
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
            let (offset, value) = far_pointer(block, memory, at, offset_bytes)?;
            load_segment(block, to, value)?;
            set_general(block, number, offset_bytes * u8::BITS as u8, offset);
        }
        Question::TimeStamp { processor } => {
            let ticks = memory.nanoseconds();
            let half = u32::BITS as u8;
            set_general(block, RAX, half, ticks);
            set_general(block, RDX, half, ticks >> half);
            if processor {
                set_general(block, RCX, half, 0);
            }
        }
        Question::WriteBase { gs, from } => {
            let Place::General { number, bits } = from else {
                return Err(Fault::GeneralProtection);
            };
            let value = block.gpr[usize::from(number)] & low_bits(bits.into());
            canonical(value)?;
            *if gs {
                &mut block.gs_base
            } else {
                &mut block.fs_base
            } = value;
        }
        Question::FarReturn { bytes, release } => {
            let from = block.gpr[general(RSP)];
            let slot = |slot: FrameSlot| frame_slot(from, slot, bytes);
            let code = read_selector(memory, slot(FrameSlot::Cs))?;
            let target = read_bytes(memory, slot(FrameSlot::Rip), bytes)?;
            far_selector(code, false)?;
            canonical(target)?;
            block.gpr[general(RSP)] = from.wrapping_add(2 * u64::from(bytes) + u64::from(release));
            block.rip = target;
            notes.transferred = true;
        }
        Question::InterruptReturn { bytes } => {
            if block.rflags & NT != 0 {
                return Err(Fault::GeneralProtection);
            }
            let from = block.gpr[general(RSP)];
            let slot = |slot: FrameSlot| frame_slot(from, slot, bytes);
            let flags = read_bytes(memory, slot(FrameSlot::Rflags), bytes)?;
            let code = read_selector(memory, slot(FrameSlot::Cs))?;
            let target = read_bytes(memory, slot(FrameSlot::Rip), bytes)?;
            far_selector(code, false)?;
            let stack = read_selector(memory, slot(FrameSlot::Ss))?;
            let pointer = read_bytes(memory, slot(FrameSlot::Rsp), bytes)?;
            if stack != USER_DS {
                return Err(Fault::GeneralProtection);
            }
            canonical(target)?;
            load_rflags(block, flags, bytes, &mut notes);
            block.gpr[general(RSP)] = pointer;
            block.rip = target;
            notes.transferred = true;
        }
        Question::FarBranch {
            call,
            bytes,
            at,
            length,
        } => {
            let (target, code) = far_pointer(block, memory, at, bytes)?;
            far_selector(code, true)?;
            if call {
                // cs first, then the return address under it, each faulting by itself; the target's test after.
                let stack = block.gpr[general(RSP)];
                let below = |slot: FrameSlot| {
                    stack.wrapping_sub((FrameSlot::Cs as u64 + 1 - slot as u64) * u64::from(bytes))
                };
                let back = block.rip.wrapping_add(length.into());
                for (slot, value) in [(FrameSlot::Cs, u64::from(USER_CS)), (FrameSlot::Rip, back)] {
                    memory
                        .write(below(slot), &value.to_le_bytes()[..usize::from(bytes)])
                        .map_err(Fault::Memory)?;
                }
                canonical(target)?;
                block.gpr[general(RSP)] = below(FrameSlot::Rip);
            } else {
                canonical(target)?;
            }
            block.rip = target;
            notes.transferred = true;
        }
        Question::XState { restore, wide, at } => xstate(block, memory, offer, restore, wide, at)?,
    }
    Ok(notes)
}

/// The image's offsets (Intel SDM Vol. 1 10.5.1, table 10-2), and the x87 header's bytes.
const IMAGE_POINTER: usize = core::mem::offset_of!(Fxsave, fip);
const IMAGE_DATA: usize = core::mem::offset_of!(Fxsave, fdp);
const IMAGE_MXCSR: usize = core::mem::offset_of!(Fxsave, mxcsr);
const IMAGE_MXCSR_MASK: usize = core::mem::offset_of!(Fxsave, mxcsr_mask);
const IMAGE_REGISTERS: usize = core::mem::offset_of!(Fxsave, st);
const IMAGE_XMM: usize = core::mem::offset_of!(Fxsave, xmm);
pub(crate) const IMAGE_SLOT: usize = XMM_BYTES as usize;
const MXCSR_BYTES: usize = size_of::<u32>();
const HEADER: usize = IMAGE_POINTER;
/// A register's 64-bit significand, and the sign and exponent above it, as the block keeps them.
const SIGNIFICAND: usize = size_of::<u64>();
const EXPONENT: usize = size_of::<u16>();
const HIGH_KEPT: usize = size_of::<u32>();

/// `xsave` and `xrstor` as `fpu::image` and `fpu::xstate_image` give them: a non-canonical first byte faults
/// first, then a misaligned address, then a non-canonical last byte, then the last byte (wrapping within the
/// address size, before the segment base), then the whole area; then the components RFBM (rax with XCR0) names.
fn xstate(
    block: &mut X86Block,
    memory: &mut impl Memory,
    offer: Offer,
    restore: bool,
    wide: bool,
    operand: Operand,
) -> Result<(), Fault> {
    let bytes = XSTATE_BYTES as u64;
    let at = address(block, &operand);
    let mut reach = |at: u64, bytes: u64| memory.check(at, bytes, !restore).map_err(Fault::Memory);
    if !is_canonical(at) {
        reach(at, bytes)?;
    }
    if !at.is_multiple_of(XSAVE_ALIGNMENT) {
        return Err(Fault::GeneralProtection);
    }
    if !is_canonical(at.wrapping_add(bytes - 1)) {
        reach(at, bytes)?;
    }
    let base = match operand.segment {
        Some(Segment::Fs) => block.fs_base,
        Some(Segment::Gs) => block.gs_base,
        None => 0,
    };
    let last = (at.wrapping_sub(base).wrapping_add(bytes - 1)
        & low_bits(operand.address_bits.into()))
    .wrapping_add(base);
    reach(last, 1)?;
    reach(at, bytes)?;
    let mut area = [0; XSTATE_BYTES];
    memory.read(at, &mut area).map_err(Fault::Memory)?;
    let rfbm = block.gpr[usize::from(RAX)] & u64::from(cpuid::enabled_state(offer));
    let named = |component: u32| rfbm & u64::from(component) != 0;
    let word = |area: &[u8], at: usize| {
        u64::from_le_bytes(area[at..][..size_of::<u64>()].try_into().expect("a word"))
    };
    let top = |block: &X86Block| usize::from(block.fxsave.fsw >> TOP_SHIFT);
    // ST(k) is the physical register (k + TOP) mod 8.
    let slot =
        |n: usize, top: usize| IMAGE_REGISTERS + (n.wrapping_sub(top) & REGISTER_MASK) * IMAGE_SLOT;
    let fxsave = &mut block.fxsave;
    if !restore {
        if named(XSAVE_X87) {
            area[..HEADER].copy_from_slice(&fxsave.as_bytes()[..HEADER]);
            for (offset, pointer) in [(IMAGE_POINTER, fxsave.fip), (IMAGE_DATA, fxsave.fdp)] {
                let kept = if wide {
                    pointer
                } else {
                    pointer & low_bits(u32::BITS)
                };
                area[offset..][..size_of::<u64>()].copy_from_slice(&kept.to_le_bytes());
            }
            let top = top(block);
            for (n, register) in block.fxsave.st.iter().enumerate() {
                let image = &mut area[slot(n, top)..][..IMAGE_SLOT];
                image.fill(0);
                image[..SIGNIFICAND + HIGH_KEPT]
                    .copy_from_slice(&register[..SIGNIFICAND + HIGH_KEPT]);
            }
        }
        let fxsave = &block.fxsave;
        if named(XSAVE_SSE | XSAVE_AVX) {
            area[IMAGE_MXCSR..][..MXCSR_BYTES].copy_from_slice(&fxsave.mxcsr.to_le_bytes());
            area[IMAGE_MXCSR_MASK..][..MXCSR_BYTES].copy_from_slice(&MXCSR_WRITABLE.to_le_bytes());
        }
        let avx = cpuid::AVX_STATE_OFFSET as usize;
        for (component, offset, registers) in [
            (XSAVE_SSE, IMAGE_XMM, &fxsave.xmm),
            (XSAVE_AVX, avx, &block.ymm_high),
        ] {
            if named(component) {
                area[offset..][..registers.as_bytes().len()].copy_from_slice(registers.as_bytes());
            }
        }
        let in_use = components_in_use(block);
        let bv = (word(&area, XSTATE_BV) & !rfbm) | (in_use & rfbm);
        area[XSTATE_BV..][..size_of::<u64>()].copy_from_slice(&bv.to_le_bytes());
        return memory.write(at, &area).map_err(Fault::Memory);
    }
    // The checks, before anything changes.
    let bv = word(&area, XSTATE_BV);
    let mxcsr = u32::from_le_bytes(
        area[IMAGE_MXCSR..][..MXCSR_BYTES]
            .try_into()
            .expect("a word"),
    );
    let header = (bv & !u64::from(cpuid::enabled_state(offer)))
        | XSAVE_HEADER_ZERO
            .iter()
            .fold(0, |all, &at| all | word(&area, at));
    if header != 0 || mxcsr & !MXCSR_WRITABLE != 0 && named(XSAVE_SSE | XSAVE_AVX) {
        return Err(Fault::GeneralProtection);
    }
    let loaded = |component: u32| bv & u64::from(component) != 0;
    if named(XSAVE_X87) {
        let from = loaded(XSAVE_X87).then_some(&area);
        let header = from.map_or(FCW_AT_START.into(), |area| word(area, 0) & HEADER_KEPT);
        fxsave.as_mut_bytes()[..HEADER].copy_from_slice(&header.to_le_bytes());
        let pointer_bytes = if wide {
            size_of::<u64>()
        } else {
            size_of::<u32>()
        };
        for (offset, pointer) in [
            (IMAGE_POINTER, &mut fxsave.fip),
            (IMAGE_DATA, &mut fxsave.fdp),
        ] {
            let mut value = [0; size_of::<u64>()];
            if let Some(area) = from {
                value[..pointer_bytes].copy_from_slice(&area[offset..][..pointer_bytes]);
            }
            *pointer = u64::from_le_bytes(value);
        }
        // TOP as just restored places the registers.
        let top = usize::from(fxsave.fsw >> TOP_SHIFT);
        for (n, register) in fxsave.st.iter_mut().enumerate() {
            register[..SIGNIFICAND + HIGH_KEPT].fill(0);
            if let Some(area) = from {
                register[..SIGNIFICAND + EXPONENT]
                    .copy_from_slice(&area[slot(n, top)..][..SIGNIFICAND + EXPONENT]);
            }
        }
    }
    if named(XSAVE_SSE | XSAVE_AVX) {
        fxsave.mxcsr = mxcsr;
    }
    let avx = cpuid::AVX_STATE_OFFSET as usize;
    for (component, offset, registers) in [
        (XSAVE_SSE, IMAGE_XMM, &mut block.fxsave.xmm),
        (XSAVE_AVX, avx, &mut block.ymm_high),
    ] {
        if named(component) {
            let into = registers.as_mut_bytes();
            if loaded(component) {
                into.copy_from_slice(&area[offset..][..into.len()]);
            } else {
                into.fill(0);
            }
        }
    }
    Ok(())
}

/// XSTATE_BV's bits of the x87, SSE and AVX components not in their initial configuration (Intel SDM Vol. 1
/// 13.6), as `fpu::components_in_use` gives them; MXCSR is in neither.
fn components_in_use(block: &X86Block) -> u64 {
    let fxsave = &block.fxsave;
    let header = u64::from_le_bytes(fxsave.as_bytes()[..HEADER].try_into().expect("a word"));
    let registers = fxsave
        .st
        .iter()
        .any(|r| r[..SIGNIFICAND + HIGH_KEPT].iter().any(|&b| b != 0));
    let x87 = header != u64::from(FCW_AT_START) || fxsave.fip != 0 || fxsave.fdp != 0 || registers;
    let any = |registers: &[[u8; IMAGE_SLOT]]| registers.as_flattened().iter().any(|&b| b != 0);
    [
        (XSAVE_X87, x87),
        (XSAVE_SSE, any(&fxsave.xmm)),
        (XSAVE_AVX, any(&block.ymm_high)),
    ]
    .into_iter()
    .filter(|&(_, used)| used)
    .fold(0, |set, (component, _)| set | u64::from(component))
}

/// The rflags bits user mode may write, from `value` of `bytes` bytes, as `popf` and `iret` load them at CPL 3.
fn load_rflags(block: &mut X86Block, value: u64, bytes: u8, notes: &mut Notes) {
    let written = POPPED_CONTROL & low_bits(u32::from(bytes) * u8::BITS);
    block.rflags = (block.rflags & !written) | (value & written);
    block.flags = (value & ARITHMETIC_FLAGS) as u32;
    notes.trap_flag_set = value & TF != 0;
    notes.alignment_check_not_provided = value & ALIGNMENT_CHECK != 0;
}

/// General protection unless `target` is canonical, as for a branch to it.
fn canonical(target: u64) -> Result<(), Fault> {
    if is_canonical(target) {
        Ok(())
    } else {
        Err(Fault::GeneralProtection)
    }
}

/// The address of a slot of the frame at `from` whose slots are of `bytes` bytes.
fn frame_slot(from: u64, slot: FrameSlot, bytes: u8) -> u64 {
    from.wrapping_add(slot as u64 * u64::from(bytes))
}

/// `bytes` bytes at `at`, zero-extended.
fn read_bytes(memory: &mut impl Memory, at: u64, bytes: u8) -> Result<u64, Fault> {
    let mut word = [0; size_of::<u64>()];
    memory
        .read(at, &mut word[..usize::from(bytes)])
        .map_err(Fault::Memory)?;
    Ok(u64::from_le_bytes(word))
}

/// The low `bytes` bytes of `value` written at `at`.
fn write_bytes(memory: &mut impl Memory, at: u64, value: u64, bytes: u8) -> Result<(), Fault> {
    memory
        .write(at, &value.to_le_bytes()[..usize::from(bytes)])
        .map_err(Fault::Memory)
}

/// The floating point exception an x87 instruction that waits meets first, when a raised one is unmasked.
fn pending(block: &X86Block) -> Result<(), Fault> {
    let area = &block.fxsave;
    match area.fsw & !area.fcw & EXCEPTIONS {
        0 => Ok(()),
        raised => Err(Fault::FloatException(raised)),
    }
}

/// The offset of `offset_bytes` bytes at `at` and the selector after it, each read faulting by itself; the
/// selector's address wraps by itself under an address-size prefix.
fn far_pointer(
    block: &X86Block,
    memory: &mut impl Memory,
    at: Operand,
    offset_bytes: u8,
) -> Result<(u64, u16), Fault> {
    let offset = read_bytes(memory, address(block, &at), offset_bytes)?;
    let after = Operand {
        offset: at.offset.wrapping_add(offset_bytes.into()),
        ..at
    };
    Ok((offset, read_selector(memory, address(block, &after))?))
}

/// General protection unless `value` names Linux's 64-bit user code, with any requested privilege level for a
/// branch and level 3 exactly for a return, which never goes to a more privileged level.
fn far_selector(value: u16, branch: bool) -> Result<(), Fault> {
    let ignored = if branch { SELECTOR_PRIVILEGE } else { 0 };
    let names = |selector: u16| value & !ignored == selector & !ignored;
    if names(USER32_CS) {
        return Err(Fault::FarTransferTo32Bit);
    }
    if !names(USER_CS) {
        return Err(Fault::GeneralProtection);
    }
    Ok(())
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
