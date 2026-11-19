//! What an exception taken from translated code comes to for the guest (docs/native.md section 4): the span
//! holding the host instruction, the relocations put back, the fix-ups made, the count less its lead, and the
//! calls into the inside answered. The inside resolves its guest threads' stops with this, and the tests
//! that run pieces on the floor resolve theirs with the same code.

use alloc::boxed::Box;

use aarch64_cpu::registers::ESR_EL1;
use wcontainers_abi::{PAGE_SIZE, RegisterBlock};
use wcontainers_arm64::mirror::{MIRROR, SLOT_BYTES, SLOT_SHIFT};
use wcontainers_arm64::thread_state::{
    COUNT, Exclusive, FPCR_BITS, FPSR_BITS, PAGE, RegisterPage, STACK_ALIGNMENT, TAG_LOW, WITHHELD,
    untag_address,
};
use wcontainers_guest_cpu::position::{
    Base, Fixup, Held, InsideCall, Operand, Relocated, Resume, Side, Span, Stopping, span_at,
};

use crate::syndrome::{ALIGNMENT_FAULT, CACHE_MAINTENANCE, FAULT_STATUS, WRITE_NOT_READ};

/// The general registers.
pub const GENERAL: usize = RegisterBlock::ZERO.x.len();

/// The cumulative saturation bit of FPSR, the highest the CPU we present has.
const QC: u32 = 1 << (u32::BITS - 1 - FPSR_BITS.leading_zeros());

/// FPCR's default: round to nearest, nothing flushed, not-a-numbers propagated.
const DEFAULT_MODE: u32 = 0;

/// The condition flags of PSTATE (NZCV).
pub fn condition_flags() -> u64 {
    use aarch64_cpu::registers::SPSR_EL1;
    SPSR_EL1::N::SET.value
        | SPSR_EL1::Z::SET.value
        | SPSR_EL1::C::SET.value
        | SPSR_EL1::V::SET.value
}

/// The guest's level as the hardware holds it at an exception, apart from the vector registers and the
/// thread pointer, which no resolution touches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interrupted {
    pub x: [u64; GENERAL],
    pub sp: u64,
    pub pc: u64,
    /// The saved PSTATE (SPSR_EL1).
    pub state: u64,
    pub fpsr: u32,
    pub fpcr: u32,
}

/// The guest's float status as a thread keeps it (as the WebAssembly output's thread block does).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FloatStatus {
    pub fpsr: u32,
    pub fpcr: u32,
    /// The program wrote FPSR, so its flags are kept exact.
    pub tracking: bool,
}

impl FloatStatus {
    /// The rule at every entry into the inside (docs/native.md section 3.4): when the flags are kept or the
    /// mode is not the default the thread takes the hardware's, else it keeps its own and the hardware's QC.
    pub fn enter(&mut self, hardware: u32) {
        if self.kept() {
            self.fpsr = hardware;
        } else {
            self.fpsr |= hardware & QC;
        }
    }

    /// Whether the flags are exact: the program wrote FPSR, or the mode is not the default.
    pub fn kept(&self) -> bool {
        self.tracking || self.fpcr != DEFAULT_MODE
    }

    /// Takes the hardware's status whole.
    fn take(&mut self, fpsr: u32, fpcr: u32) {
        self.fpsr = fpsr;
        self.fpcr = fpcr & FPCR_BITS;
    }
}

/// The guest's own general state at a stop; its vector registers and thread pointer are the hardware's,
/// and its float status the thread's [`FloatStatus`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuestState {
    pub x: [u64; GENERAL],
    pub sp: u64,
    pub pc: u64,
    /// NZCV, in PSTATE's bits.
    pub flags: u64,
    /// The progress count of definition E.
    pub count: u64,
    /// The exclusive monitor, in [`RegisterPage::exclusive`]'s order.
    pub exclusive: Exclusive,
}

/// Why the guest stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// It goes on at this address, which has no piece loaded where it was.
    Landed(u64),
    /// A system call; pc is after it.
    Syscall,
    Breakpoint(u16),
    /// An instruction the CPU we present does not have.
    Illegal,
    /// An access or a jump that must be aligned was not, at this address.
    Alignment(u64),
    /// An access the page refused, at this address.
    MemoryFault {
        address: u64,
        write: bool,
    },
}

/// What an exception comes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved {
    /// The guest stopped there.
    Stop(Stop, GuestState),
    /// The inside answered; the guest's level goes on from this.
    Again(Interrupted, RegisterPage),
    /// As [`Resolved::Again`], before a read of the float status while its flags are not kept: a slow path.
    FloatStatusNotKept(Interrupted, RegisterPage),
    /// A counter read: the guest's level goes on from this once the clock's reading is in general register
    /// `into` (none: nowhere), through [`set_register`].
    Clock {
        into: Option<u8>,
        then: Interrupted,
        page: RegisterPage,
    },
    /// The guest announced that the code at `address` changed: the guest's level goes on from `then` once the
    /// translations of that line are dropped, or stops at `at` when it may not read there.
    CodeChanged {
        address: u64,
        at: Box<GuestState>,
        then: Interrupted,
        page: RegisterPage,
    },
}

/// The exception that does not resolve: one translated code never raises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unexpected {
    pub syndrome: u64,
    pub fault: u64,
    pub pc: u64,
}

/// One exception taken from translated code at the guest's level.
pub struct Exception<'a> {
    pub syndrome: u64,
    /// FAR_EL1.
    pub fault: u64,
    pub hardware: &'a Interrupted,
    pub page: &'a RegisterPage,
}

/// The loaded piece the exception came from, or whose jump the exception ended.
pub struct Piece<'a> {
    /// Its first host instruction's address.
    pub base: u64,
    pub length: usize,
    pub spans: &'a [Span],
}

/// Writes a general register of the guest, wherever the guest's value is kept.
pub fn set_register(register: u8, value: u64, hardware: &mut Interrupted, page: &mut RegisterPage) {
    match WITHHELD.iter().position(|&w| w == register) {
        Some(k) => page.withheld[k] = value,
        None => hardware.x[usize::from(register)] = value,
    }
}

/// A held guest value.
fn held(h: Held, hardware: &Interrupted, page: &RegisterPage) -> u64 {
    match h {
        Held::Register(r) => hardware.x[usize::from(r)],
        Held::Page(offset) => page.word(offset),
    }
}

/// The guest's state at a stop at `span`, before its fix-ups.
pub fn guest_state(span: &Span, hardware: &Interrupted, page: &RegisterPage) -> GuestState {
    state_of(&span.relocated, span.lead, span.guest, hardware, page)
}

/// The guest's state with these values relocated, the count this far ahead, at `pc`.
fn state_of(
    relocated: &[Relocated],
    lead: u32,
    pc: u64,
    hardware: &Interrupted,
    page: &RegisterPage,
) -> GuestState {
    let mut x = hardware.x;
    for (k, &r) in WITHHELD.iter().enumerate() {
        x[usize::from(r)] = page.withheld[k];
    }
    for r in relocated {
        x[usize::from(r.guest)] = held(r.held, hardware, page);
    }
    GuestState {
        x,
        sp: hardware.sp,
        pc,
        flags: hardware.state & condition_flags(),
        count: hardware.x[usize::from(COUNT)] - u64::from(lead),
        exclusive: page.exclusive(),
    }
}

/// The hardware and the register page with a span's fix-ups made: they name host registers, so they come
/// before the relocations are read. `memory` reads guest memory.
pub fn fixed_up(
    span: &Span,
    h: &Interrupted,
    page: &RegisterPage,
    float: &mut FloatStatus,
    memory: &mut dyn FnMut(u64, &mut [u8]),
) -> (Interrupted, RegisterPage) {
    let (mut hardware, mut fixed) = (h.clone(), page.clone());
    for fixup in &span.fixups {
        match *fixup {
            Fixup::Constant { register, value } => hardware.x[usize::from(register)] = value,
            Fixup::Move { register, from } => {
                let value = h.x[usize::from(from)];
                match hardware.x.get_mut(usize::from(register)) {
                    Some(x) => *x = value,
                    None => hardware.sp = value,
                }
            }
            Fixup::Forget { status, read } => {
                let remembered = [page.exclusive_low, page.exclusive_high];
                let swapped = read[0].is_some()
                    && read
                        .iter()
                        .zip(remembered)
                        .all(|(r, m)| r.is_none_or(|r| held(r, h, page) == m));
                hardware.x[usize::from(status)] = u64::from(!swapped);
                fixed.exclusive_bytes = 0;
            }
            Fixup::Remember {
                bytes,
                pair,
                address,
            } => {
                let at = held(address, h, page);
                let word = size_of::<u64>();
                let total = usize::from(bytes) * if pair { 2 } else { 1 };
                let mut read = |offset: usize, length: usize| {
                    let mut value = [0u8; size_of::<u64>()];
                    memory(at + offset as u64, &mut value[..length]);
                    u64::from_le_bytes(value)
                };
                let two_words = pair && usize::from(bytes) == word;
                fixed.exclusive_bytes = total as u64;
                fixed.exclusive_address = at;
                fixed.exclusive_low = read(0, total.min(word));
                fixed.exclusive_high = if two_words { read(word, word) } else { 0 };
            }
            Fixup::FloatStatus => float.take(h.fpsr, h.fpcr),
        }
    }
    (hardware, fixed)
}

/// Where the guest goes on after a span's commit point.
fn resume_address(span: &Span, hardware: &Interrupted, page: &RegisterPage) -> u64 {
    match span.resume {
        Resume::At(at) => at,
        Resume::Through(h) => untag_address(held(h, hardware, page)),
        Resume::Slot(h) => slot_target(held(h, hardware, page)),
    }
}

/// The guest address whose entry slot is at `slot`.
fn slot_target(slot: u64) -> u64 {
    (slot & !MIRROR) >> SLOT_SHIFT
}

/// The first byte of a guest access at `pc` as its operand gives it, tag and all.
pub fn access_first(operand: &Operand, pc: u64, x: &[u64; GENERAL], sp: u64) -> u64 {
    let reg = |b: Base| match b {
        Base::Register(n) => x[usize::from(n)],
        Base::StackPointer => sp,
        Base::Zero => 0,
        Base::Pc => pc,
    };
    let index = operand.index.map_or(0, |i| {
        let value = reg(i.register);
        let bits = u32::from(i.bits);
        let extended = if bits >= u64::BITS {
            value
        } else if i.signed {
            (((value << (u64::BITS - bits)) as i64) >> (u64::BITS - bits)) as u64
        } else {
            value & ((1 << bits) - 1)
        };
        extended << i.shift
    });
    reg(operand.base)
        .wrapping_add_signed(operand.offset)
        .wrapping_add(index)
}

/// Where a fault's address is reported: the access's first byte when it is in the page that refused, else
/// the first byte of the next page. The pages are told apart by the address bits below the tag, which the
/// hardware's fault address shares with the guest's whatever the host code untagged.
pub fn fault_address(first: u64, fault: u64) -> u64 {
    let below_tag = (1 << TAG_LOW) - 1;
    if (first ^ fault) & below_tag & !(PAGE_SIZE - 1) == 0 {
        first
    } else {
        (first | (PAGE_SIZE - 1)).wrapping_add(1)
    }
}

/// The alignment fault of an access: the stack pointer's own, as Linux has the CPU check it first, when it is
/// the base and not a multiple of 16, else at the access's first byte.
fn misaligned(operand: &Operand, pc: u64, x: &[u64; GENERAL], sp: u64) -> Stop {
    if operand.base == Base::StackPointer && !sp.is_multiple_of(STACK_ALIGNMENT) {
        Stop::Alignment(sp)
    } else {
        Stop::Alignment(untag_address(access_first(operand, pc, x, sp)))
    }
}

/// The operand of a span's access, which a fault or an alignment call needs.
fn operand(span: &Span) -> Option<&Operand> {
    match &span.stopping {
        Stopping::Access(operand) => Some(operand),
        _ => None,
    }
}

/// The hardware after a call into the inside, going on at the next host instruction.
fn after_call(h: &Interrupted) -> Interrupted {
    Interrupted {
        pc: h.pc + size_of::<u32>() as u64,
        ..h.clone()
    }
}

/// What an exception taken from `piece` comes to. `memory` reads guest memory (for a jump's fix-ups).
pub fn resolve(
    piece: &Piece<'_>,
    exception: &Exception<'_>,
    float: &mut FloatStatus,
    memory: &mut dyn FnMut(u64, &mut [u8]),
) -> Result<Resolved, Unexpected> {
    let h = exception.hardware;
    let page = exception.page;
    let class = ESR_EL1::EC.read(exception.syndrome);
    let unexpected = Unexpected {
        syndrome: exception.syndrome,
        fault: exception.fault,
        pc: h.pc,
    };
    let offset = h.pc.wrapping_sub(piece.base);
    let in_piece = offset < piece.length as u64;
    if !in_piece {
        // A jump that reached its entry slot: the guest lands at the slot's address, the jump done (no value
        // relocated, the count's add behind it).
        float.enter(h.fpsr);
        let target = slot_target(h.pc);
        let state = state_of(&[], 0, target, h, page);
        let stop = if class == ESR_EL1::EC::Value::PCAlignmentFault as u64 {
            Stop::Alignment(target)
        } else {
            Stop::Landed(target)
        };
        return Ok(Resolved::Stop(stop, state));
    }
    let span = span_at(piece.spans, piece.length, offset as u32).ok_or(unexpected)?;
    if class == ESR_EL1::EC::Value::Brk64 as u64 {
        let call = InsideCall::from_code(ESR_EL1::ISS.read(exception.syndrome) as u16)
            .ok_or(unexpected)?;
        match call {
            InsideCall::FloatBefore => {
                float.enter(h.fpsr);
                let mut next = after_call(h);
                next.fpsr = float.fpsr;
                if span.stopping == Stopping::FloatStatusRead && !float.kept() {
                    return Ok(Resolved::FloatStatusNotKept(next, page.clone()));
                }
                return Ok(Resolved::Again(next, page.clone()));
            }
            InsideCall::FloatAfter => {
                float.take(h.fpsr, h.fpcr);
                if span.stopping == Stopping::FloatStatusWrite {
                    float.tracking = true;
                }
                return Ok(Resolved::Again(after_call(h), page.clone()));
            }
            _ => {}
        }
        float.enter(h.fpsr);
        let mut state = guest_state(span, h, page);
        let stop = match call {
            InsideCall::Exit => {
                let to = resume_address(span, h, page);
                state.pc = to;
                Stop::Landed(to)
            }
            InsideCall::Answer => match span.stopping {
                Stopping::Syscall => {
                    state.pc = span.guest + size_of::<u32>() as u64;
                    Stop::Syscall
                }
                Stopping::CodeChanged(register) => {
                    return Ok(Resolved::CodeChanged {
                        address: register.map_or(0, |r| state.x[usize::from(r)]),
                        at: Box::new(state),
                        then: after_call(h),
                        page: page.clone(),
                    });
                }
                Stopping::Counter(into) => {
                    return Ok(Resolved::Clock {
                        into,
                        then: after_call(h),
                        page: page.clone(),
                    });
                }
                _ => return Err(unexpected),
            },
            InsideCall::Trap => match span.stopping {
                Stopping::Breakpoint(comment) => Stop::Breakpoint(comment),
                _ => Stop::Illegal,
            },
            InsideCall::Misaligned => misaligned(
                operand(span).ok_or(unexpected)?,
                span.guest,
                &state.x,
                state.sp,
            ),
            InsideCall::Unreachable => {
                let (fixed, fixed_page) = fixed_up(span, h, page, float, memory);
                state = guest_state(span, &fixed, &fixed_page);
                let to = resume_address(span, h, page);
                state.pc = to;
                if to.is_multiple_of(size_of::<u32>() as u64) {
                    Stop::Landed(to)
                } else {
                    Stop::Alignment(to)
                }
            }
            InsideCall::FloatBefore | InsideCall::FloatAfter => return Err(unexpected),
        };
        return Ok(Resolved::Stop(stop, state));
    }
    float.enter(h.fpsr);
    let state = guest_state(span, h, page);
    if span.side != Side::Before {
        return Err(unexpected);
    }
    let status = ESR_EL1::ISS.read(exception.syndrome) & FAULT_STATUS;
    let data_abort = class == ESR_EL1::EC::Value::DataAbortLowerEL as u64;
    let stop = if class == ESR_EL1::EC::Value::SPAlignmentFault as u64 {
        Stop::Alignment(state.sp)
    } else if data_abort {
        let operand = operand(span).ok_or(unexpected)?;
        let first = untag_address(access_first(operand, span.guest, &state.x, state.sp));
        if status == ALIGNMENT_FAULT {
            Stop::Alignment(first)
        } else {
            Stop::MemoryFault {
                address: fault_address(first, exception.fault),
                write: exception.syndrome & (WRITE_NOT_READ | CACHE_MAINTENANCE) == WRITE_NOT_READ,
            }
        }
    } else {
        return Err(unexpected);
    };
    Ok(Resolved::Stop(stop, state))
}

/// The guest's state at an interrupt that landed at the hardware's pc while `piece` ran: before a commit
/// point the instruction has not happened, after it its fix-ups finish it. The thread takes the hardware's
/// float status as at every entry. `memory` reads guest memory for the fix-ups.
pub fn interrupted(
    piece: &Piece<'_>,
    h: &Interrupted,
    page: &RegisterPage,
    float: &mut FloatStatus,
    memory: &mut dyn FnMut(u64, &mut [u8]),
) -> Option<GuestState> {
    float.enter(h.fpsr);
    let offset = h.pc.wrapping_sub(piece.base);
    if offset >= piece.length as u64 {
        // The jump is done and the guest is at its target's entry slot, or in it: an aligned target's slot is
        // run from its start, a misaligned one's address keeps the target's low bits.
        let slot = if h.pc.is_multiple_of(size_of::<u32>() as u64) {
            h.pc & !(SLOT_BYTES - 1)
        } else {
            h.pc
        };
        return Some(state_of(&[], 0, slot_target(slot), h, page));
    }
    let span = span_at(piece.spans, piece.length, offset as u32)?;
    if span.side == Side::Before {
        return Some(guest_state(span, h, page));
    }
    let (fixed, fixed_page) = fixed_up(span, h, page, float, memory);
    let mut state = guest_state(span, &fixed, &fixed_page);
    state.pc = resume_address(span, h, page);
    Some(state)
}

/// The hardware and the register page a guest state goes on from, as the inside returns to it: the withheld
/// registers' guest values in the page, the count in its register, the page's address `register_page` in
/// its. The pc is left for the caller to set to the piece's host address.
pub fn rejoin(
    state: &GuestState,
    mut page: RegisterPage,
    float: &FloatStatus,
    register_page: u64,
) -> (Interrupted, RegisterPage) {
    page.set_exclusive(state.exclusive);
    let mut x = state.x;
    for (slot, &r) in page.withheld.iter_mut().zip(&WITHHELD) {
        *slot = x[usize::from(r)];
        x[usize::from(r)] = 0;
    }
    x[usize::from(COUNT)] = state.count;
    x[usize::from(PAGE)] = register_page;
    let hardware = Interrupted {
        x,
        sp: state.sp,
        pc: 0,
        state: state.flags,
        fpsr: float.fpsr,
        fpcr: float.fpcr,
    };
    (hardware, page)
}
