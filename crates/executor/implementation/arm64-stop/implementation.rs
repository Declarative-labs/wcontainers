//! What an exception taken from the native output's ARM64 code comes to for the guest, in every way of running
//! it (docs/native.md section 4, docs/native-without-virtual-machine.md section 3.3): the span holding the host
//! instruction, the relocations put back, the fix-ups made, the count less its lead, and the calls into the
//! inside answered. In process (section 3.2 item 1) the guest's stack pointer is the host's less the window's base and
//! the guest values of the way's two more withheld registers, and of the two a piece keeps in their place, are the
//! words just after the register page: [`in_process`] takes them.

#![no_std]

extern crate alloc;

use alloc::boxed::Box;

use wcontainers_abi::{PAGE_SIZE, RegisterBlock};
use wcontainers_arm64::Way;
use wcontainers_arm64::mirror::SLOT_BYTES;
use wcontainers_arm64::thread_state::{
    COUNT, Exclusive, FPCR_BITS, FPSR_BITS, PAGE, RegisterPage, SCRATCH, STACK_ALIGNMENT, TAG_LOW,
    WITHHELD, untag_address,
};
use wcontainers_arm64::way::{AFTER_PAGE_WORDS, BASE, IN_PROCESS_WITHHELD_AT};
use wcontainers_guest_cpu::position::{
    Base, Fixup, Held, InsideCall, Operand, Relocated, Resume, Side, Span, Stopping, span_at,
};

/// The general registers.
pub const GENERAL: usize = RegisterBlock::ZERO.x.len();

/// The general register a system call's number is in (Linux's ARM64 calling convention).
pub const SYSCALL_NUMBER: usize = 8;

/// The words after the register page in process: the guest values of the in-process way's withheld registers past
/// the other ways' three, in the way's order, then of the two a piece keeps in their place.
pub const AFTER_PAGE: usize = AFTER_PAGE_WORDS;

/// The words after the register page in process.
pub type AfterPage = [u64; AFTER_PAGE];

/// What the ways whose withheld registers are all in the page keep after it: nothing.
const NOTHING_AFTER: AfterPage = [0; AFTER_PAGE];

/// The cumulative saturation bit of FPSR, the highest the CPU we present has.
const QC: u32 = 1 << (u32::BITS - 1 - FPSR_BITS.leading_zeros());

/// FPCR's default: round to nearest, nothing flushed, not-a-numbers propagated.
const DEFAULT_MODE: u32 = 0;

/// The condition flags of PSTATE (NZCV).
pub fn condition_flags() -> u64 {
    wcontainers_abi::pstate::NZCV
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
    /// Only FPSR's own bits are taken: some processors keep what a write puts in the others (Neoverse V2 keeps
    /// those above QC), which read as zero by the architecture.
    pub fn enter(&mut self, hardware: u32) {
        if self.kept() {
            self.fpsr = hardware & FPSR_BITS;
        } else {
            self.fpsr |= hardware & QC;
        }
    }

    /// Whether the flags are exact: the program wrote FPSR, or the mode is not the default.
    pub fn kept(&self) -> bool {
        self.tracking || self.fpcr != DEFAULT_MODE
    }

    /// Takes the hardware's status, its own bits only.
    fn take(&mut self, fpsr: u32, fpcr: u32) {
        self.fpsr = fpsr & FPSR_BITS;
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
    /// A jump's first miss, at its call into the inside at host address `call`: the guest's level goes on
    /// from `then` once `target` is made the jump's last target.
    LastTarget {
        call: u64,
        target: u64,
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

/// What the host says an exception taken from translated code was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Taken {
    /// A call into the inside, with its code.
    Call(u16),
    /// The stack pointer, as a base, was not a multiple of 16.
    StackAlignment,
    /// A jump's target, or the slot it reached, was not aligned.
    JumpAlignment,
    /// An access the page refused at `fault`, or one that was not aligned.
    Access {
        alignment: bool,
        fault: u64,
        write: bool,
    },
    /// Anything else, which translated code never raises.
    Other,
}

/// The loaded piece the exception came from, or whose jump the exception ended.
pub struct Piece<'a> {
    /// Its first host instruction's address.
    pub base: u64,
    pub length: usize,
    pub spans: &'a [Span],
}

/// Writes a general register of the guest, wherever the guest's value is kept, in the ways whose withheld registers
/// are all in the page.
pub fn set_register(register: u8, value: u64, hardware: &mut Interrupted, page: &mut RegisterPage) {
    let mut after = NOTHING_AFTER;
    set_in(
        Way::Machine,
        &[],
        register,
        value,
        hardware,
        page,
        &mut after,
    );
}

fn set_in(
    way: Way,
    relocated: &[Relocated],
    register: u8,
    value: u64,
    hardware: &mut Interrupted,
    page: &mut RegisterPage,
    after: &mut AfterPage,
) {
    if let Some(r) = relocated.iter().find(|r| r.guest == register) {
        match r.held {
            Held::Register(h) => hardware.x[usize::from(h)] = value,
            Held::Page(offset) => *held_page_mut(offset, page, after) = value,
            Held::Rebased(_) => unreachable!("arm64 translated code rebases no register"),
        }
        return;
    }
    match way.withheld().iter().position(|&w| w == register) {
        Some(k) => *withheld_mut(k, page, after) = value,
        None => hardware.x[usize::from(register)] = value,
    }
}

/// The word a guest value is held in at a byte offset of the register page or past it.
fn held_page_mut<'a>(
    offset: u16,
    page: &'a mut RegisterPage,
    after: &'a mut AfterPage,
) -> &'a mut u64 {
    if let Some(past) = usize::from(offset).checked_sub(IN_PROCESS_WITHHELD_AT) {
        return &mut after[past / size_of::<u64>()];
    }
    let RegisterPage {
        withheld,
        saved,
        link,
        ..
    } = page;
    withheld
        .iter_mut()
        .chain(saved)
        .chain(core::iter::once(link))
        .nth(usize::from(offset) / size_of::<u64>())
        .expect("a held guest value is a word before the exclusive monitor")
}

/// The guest value of the way's `k`th withheld register.
fn withheld(k: usize, page: &RegisterPage, after: &AfterPage) -> u64 {
    match page.withheld.get(k) {
        Some(&value) => value,
        None => after[k - WITHHELD.len()],
    }
}

fn withheld_mut<'a>(k: usize, page: &'a mut RegisterPage, after: &'a mut AfterPage) -> &'a mut u64 {
    match page.withheld.get_mut(k) {
        Some(value) => value,
        None => &mut after[k - WITHHELD.len()],
    }
}

/// A held guest value.
fn held(h: Held, hardware: &Interrupted, page: &RegisterPage, after: &AfterPage) -> u64 {
    match h {
        Held::Register(r) => hardware.x[usize::from(r)],
        Held::Page(offset) => match usize::from(offset).checked_sub(IN_PROCESS_WITHHELD_AT) {
            Some(past) => after[past / size_of::<u64>()],
            None => page.word(offset),
        },
        Held::Rebased(_) => unreachable!("arm64 translated code rebases no register"),
    }
}

/// The guest's state at a stop at `span`, before its fix-ups, in the ways whose withheld registers are all in the
/// page.
pub fn guest_state(span: &Span, hardware: &Interrupted, page: &RegisterPage) -> GuestState {
    state_at(Way::Machine, span, hardware, page, &NOTHING_AFTER)
}

fn state_at(
    way: Way,
    span: &Span,
    hardware: &Interrupted,
    page: &RegisterPage,
    after: &AfterPage,
) -> GuestState {
    state_of(
        way,
        &span.relocated,
        span.lead,
        span.guest,
        hardware,
        page,
        after,
    )
}

/// The guest's state with these values relocated, the count this far ahead, at `pc`.
fn state_of(
    way: Way,
    relocated: &[Relocated],
    lead: u32,
    pc: u64,
    hardware: &Interrupted,
    page: &RegisterPage,
    after: &AfterPage,
) -> GuestState {
    let mut x = hardware.x;
    for (k, &r) in way.withheld().iter().enumerate() {
        x[usize::from(r)] = withheld(k, page, after);
    }
    for r in relocated {
        x[usize::from(r.guest)] = held(r.held, hardware, page, after);
    }
    GuestState {
        x,
        sp: hardware.sp.wrapping_sub(way.stack_base()),
        pc,
        flags: hardware.state & condition_flags(),
        count: hardware.x[usize::from(COUNT)] - u64::from(lead),
        exclusive: page.exclusive(),
    }
}

/// The hardware and the register page with a span's fix-ups made: they name host registers, so they come
/// before the relocations are read. `memory` reads guest memory. For the ways whose withheld registers are all in
/// the page.
pub fn fixed_up(
    span: &Span,
    h: &Interrupted,
    page: &RegisterPage,
    float: &mut FloatStatus,
    memory: &mut dyn FnMut(u64, &mut [u8]),
) -> (Interrupted, RegisterPage) {
    fixed_up_in(span, h, page, &NOTHING_AFTER, float, memory)
}

fn fixed_up_in(
    span: &Span,
    h: &Interrupted,
    page: &RegisterPage,
    after: &AfterPage,
    float: &mut FloatStatus,
    memory: &mut dyn FnMut(u64, &mut [u8]),
) -> (Interrupted, RegisterPage) {
    let (mut hardware, mut fixed) = (h.clone(), page.clone());
    for fixup in &span.fixups {
        match *fixup {
            Fixup::Constant { register, value } | Fixup::Address { register, value } => {
                hardware.x[usize::from(register)] = value
            }
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
                        .all(|(r, m)| r.is_none_or(|r| held(r, h, page, after) == m));
                hardware.x[usize::from(status)] = u64::from(!swapped);
                fixed.exclusive_bytes = 0;
            }
            Fixup::Remember {
                bytes,
                pair,
                address,
            } => {
                let at = held(address, h, page, after);
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
            // x86's alone: ARM64 code never carries them.
            Fixup::X87Pointer(_) => {}
            Fixup::Offset { .. } => {
                unreachable!("arm64 translated code moves no register by a fix-up")
            }
        }
    }
    (hardware, fixed)
}

/// Where the guest goes on after a span's commit point.
fn resume_address(
    way: Way,
    span: &Span,
    hardware: &Interrupted,
    page: &RegisterPage,
    after: &AfterPage,
) -> u64 {
    match span.resume {
        Resume::At(at) => at,
        Resume::Through(h) => untag_address(held(h, hardware, page, after)),
        Resume::Slot(h) => way.slot_target(held(h, hardware, page, after)),
    }
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

/// What an exception `taken` from `piece`, run in `way`, comes to with the hardware `h` and the register page
/// as it left them, its pc the host instruction that took it; none for one translated code never raises.
/// `memory` reads guest memory (for a jump's fix-ups). In process, [`in_process::resolve`].
pub fn resolve(
    way: Way,
    piece: &Piece<'_>,
    taken: Taken,
    h: &Interrupted,
    page: &RegisterPage,
    float: &mut FloatStatus,
    memory: &mut dyn FnMut(u64, &mut [u8]),
) -> Option<Resolved> {
    debug_assert!(!matches!(way, Way::InProcess { .. }), "in_process::resolve");
    resolve_in(way, piece, taken, h, page, &NOTHING_AFTER, float, memory)
}

#[expect(
    clippy::too_many_arguments,
    reason = "resolve's, and the words after the page"
)]
fn resolve_in(
    way: Way,
    piece: &Piece<'_>,
    taken: Taken,
    h: &Interrupted,
    page: &RegisterPage,
    after: &AfterPage,
    float: &mut FloatStatus,
    memory: &mut dyn FnMut(u64, &mut [u8]),
) -> Option<Resolved> {
    let offset = h.pc.wrapping_sub(piece.base);
    let in_piece = offset < piece.length as u64;
    if !in_piece {
        // A jump that reached its entry slot: the guest lands at the slot's address, the jump done (no value
        // relocated, the count's add behind it).
        float.enter(h.fpsr);
        let target = way.slot_target(h.pc);
        let state = state_of(way, &[], 0, target, h, page, after);
        let stop = if taken == Taken::JumpAlignment {
            Stop::Alignment(target)
        } else {
            Stop::Landed(target)
        };
        return Some(Resolved::Stop(stop, state));
    }
    let span = span_at(piece.spans, piece.length, offset as u32)?;
    if let Taken::Call(code) = taken {
        let call = InsideCall::from_code(code)?;
        match call {
            InsideCall::FloatBefore => {
                float.enter(h.fpsr);
                let mut next = after_call(h);
                next.fpsr = float.fpsr;
                if span.stopping == Stopping::FloatStatusRead && !float.kept() {
                    return Some(Resolved::FloatStatusNotKept(next, page.clone()));
                }
                return Some(Resolved::Again(next, page.clone()));
            }
            InsideCall::FloatAfter => {
                float.take(h.fpsr, h.fpcr);
                if span.stopping == Stopping::FloatStatusWrite {
                    float.tracking = true;
                }
                return Some(Resolved::Again(after_call(h), page.clone()));
            }
            InsideCall::LastTarget => {
                return Some(Resolved::LastTarget {
                    call: h.pc,
                    // While no target is made, the compare left the target itself in t.
                    target: h.x[usize::from(SCRATCH)],
                    then: after_call(h),
                    page: page.clone(),
                });
            }
            _ => {}
        }
        float.enter(h.fpsr);
        let mut state = state_at(way, span, h, page, after);
        let stop = match call {
            InsideCall::Exit => {
                let to = resume_address(way, span, h, page, after);
                state.pc = to;
                Stop::Landed(to)
            }
            InsideCall::Answer => match span.stopping {
                Stopping::Syscall => {
                    state.pc = span.guest + size_of::<u32>() as u64;
                    Stop::Syscall
                }
                Stopping::CodeChanged(register) => {
                    return Some(Resolved::CodeChanged {
                        address: register.map_or(0, |r| state.x[usize::from(r)]),
                        at: Box::new(state),
                        then: after_call(h),
                        page: page.clone(),
                    });
                }
                Stopping::Counter(into) => {
                    return Some(Resolved::Clock {
                        into,
                        then: after_call(h),
                        page: page.clone(),
                    });
                }
                _ => return None,
            },
            InsideCall::Trap => match span.stopping {
                Stopping::Breakpoint(comment) => Stop::Breakpoint(comment),
                _ => Stop::Illegal,
            },
            InsideCall::Misaligned => misaligned(operand(span)?, span.guest, &state.x, state.sp),
            InsideCall::Unreachable => {
                let (fixed, fixed_page) = fixed_up_in(span, h, page, after, float, memory);
                state = state_at(way, span, &fixed, &fixed_page, after);
                let to = resume_address(way, span, h, page, after);
                state.pc = to;
                if to.is_multiple_of(size_of::<u32>() as u64) {
                    Stop::Landed(to)
                } else {
                    Stop::Alignment(to)
                }
            }
            // ARM64 code makes none: x86's two, and the replay check's, which ARM64's output does not write yet.
            InsideCall::X87Control | InsideCall::Step | InsideCall::Stop => return None,
            InsideCall::FloatBefore | InsideCall::FloatAfter | InsideCall::LastTarget => {
                return None;
            }
        };
        return Some(Resolved::Stop(stop, state));
    }
    float.enter(h.fpsr);
    let state = state_at(way, span, h, page, after);
    if span.side != Side::Before {
        return None;
    }
    let stop = match taken {
        Taken::StackAlignment => Stop::Alignment(state.sp),
        Taken::Access {
            alignment,
            fault,
            write,
        } => {
            let operand = operand(span)?;
            let first = untag_address(access_first(operand, span.guest, &state.x, state.sp));
            if alignment {
                Stop::Alignment(first)
            } else {
                Stop::MemoryFault {
                    address: fault_address(first, fault.wrapping_sub(way.window_base())),
                    write,
                }
            }
        }
        _ => return None,
    };
    Some(Resolved::Stop(stop, state))
}

/// The guest's state at an interrupt that landed at the hardware's pc while `piece` ran: before a commit
/// point the instruction has not happened, after it its fix-ups finish it. The thread takes the hardware's
/// float status as at every entry. `memory` reads guest memory for the fix-ups. In process,
/// [`in_process::interrupted`].
pub fn interrupted(
    way: Way,
    piece: &Piece<'_>,
    h: &Interrupted,
    page: &RegisterPage,
    float: &mut FloatStatus,
    memory: &mut dyn FnMut(u64, &mut [u8]),
) -> Option<GuestState> {
    debug_assert!(
        !matches!(way, Way::InProcess { .. }),
        "in_process::interrupted"
    );
    interrupted_in(way, piece, h, page, &NOTHING_AFTER, float, memory)
}

fn interrupted_in(
    way: Way,
    piece: &Piece<'_>,
    h: &Interrupted,
    page: &RegisterPage,
    after: &AfterPage,
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
        return Some(state_of(way, &[], 0, way.slot_target(slot), h, page, after));
    }
    let span = span_at(piece.spans, piece.length, offset as u32)?;
    if span.side == Side::Before {
        return Some(state_at(way, span, h, page, after));
    }
    let (fixed, fixed_page) = fixed_up_in(span, h, page, after, float, memory);
    let mut state = state_at(way, span, &fixed, &fixed_page, after);
    state.pc = resume_address(way, span, h, page, after);
    Some(state)
}

/// The hardware and the register page a guest state goes on from, as the inside returns to it: the withheld
/// registers' guest values in the page, the count in its register, the page's address `register_page` in
/// its. The pc is left for the caller to set to the piece's host address. In process, [`in_process::rejoin`].
pub fn rejoin(
    state: &GuestState,
    page: RegisterPage,
    float: &FloatStatus,
    register_page: u64,
) -> (Interrupted, RegisterPage) {
    let (hardware, page, _) = rejoin_in(Way::Machine, state, page, float, register_page);
    (hardware, page)
}

fn rejoin_in(
    way: Way,
    state: &GuestState,
    mut page: RegisterPage,
    float: &FloatStatus,
    register_page: u64,
) -> (Interrupted, RegisterPage, AfterPage) {
    page.set_exclusive(state.exclusive);
    let mut after = NOTHING_AFTER;
    let mut x = state.x;
    for (k, &r) in way.withheld().iter().enumerate() {
        *withheld_mut(k, &mut page, &mut after) = x[usize::from(r)];
        x[usize::from(r)] = 0;
    }
    x[usize::from(COUNT)] = state.count;
    x[usize::from(PAGE)] = register_page;
    if let Way::InProcess { .. } = way {
        x[usize::from(BASE)] = way.window_base();
    }
    let hardware = Interrupted {
        x,
        sp: state.sp.wrapping_add(way.stack_base()),
        pc: 0,
        state: state.flags,
        fpsr: float.fpsr,
        fpcr: float.fpcr,
    };
    (hardware, page, after)
}

/// Resolution in the in-process way (docs/native-without-virtual-machine.md section 3.2 item 1): a thread's
/// guest values of [`wcontainers_arm64::way::CHECK`] and [`BASE`] are the words after its register page, its
/// guest stack pointer the host's less the window's base (the host's itself in displaced code), and a read of the
/// way's mark ([`MARK`]) is an access outside the window or a stack pointer leaving it.
pub mod in_process {
    use super::{
        AfterPage, FloatStatus, GuestState, Interrupted, Piece, Resolved, Taken, access_first,
        interrupted_in, operand, rejoin_in, resolve_in, set_in, state_at,
    };
    use wcontainers_arm64::Way;
    use wcontainers_arm64::thread_state::{RegisterPage, TAG_LOW, untag_address};
    use wcontainers_arm64::way::{MARK, WINDOW_BITS};
    use wcontainers_guest_cpu::position::{Operand, Side, Span, Stopping, span_at};

    /// The most bytes a stack probe reads below the stack pointer (rule c's `ldur wzr, [sp, #-4]`).
    const PROBE_BELOW: u64 = size_of::<u32>() as u64;

    /// What an exception comes to in process.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Resolution {
        /// As in every way.
        Resolved(Resolved),
        /// The access starts outside the window, at `address`, where the guest faults: nothing is mapped there.
        /// Whether it is a write is the instruction's, which the floor reads (the mark is a read).
        Outside { address: u64, state: GuestState },
        /// The instruction at the state's pc writes a stack pointer outside the window: the floor runs it
        /// displaced (rule e).
        Displaced(GuestState),
        /// A stack probe faulted, which is never the guest's fault: the instruction is done and the guest goes
        /// on from this state, displaced where its stack pointer is outside the window (rule c).
        Probed(GuestState),
    }

    /// Whether a guest address is outside the window, its top byte ignored as the window check ignores it.
    pub fn outside(address: u64) -> bool {
        (address & ((1 << TAG_LOW) - 1)) >> WINDOW_BITS != 0
    }

    /// [`super::resolve`] for an in-process way, with `after` the words after the register page.
    #[expect(
        clippy::too_many_arguments,
        reason = "resolve's, and the words after the page"
    )]
    pub fn resolve(
        way: Way,
        piece: &Piece<'_>,
        taken: Taken,
        h: &Interrupted,
        page: &RegisterPage,
        after: &AfterPage,
        float: &mut FloatStatus,
        memory: &mut dyn FnMut(u64, &mut [u8]),
    ) -> Option<Resolution> {
        debug_assert!(matches!(way, Way::InProcess { .. }), "an in-process way");
        // Displaced code makes no stack probe.
        let displaced = way.stack_base() != way.window_base();
        let offset = h.pc.wrapping_sub(piece.base);
        if offset < piece.length as u64 {
            let span = span_at(piece.spans, piece.length, offset as u32)?;
            let mark = way.window_base() + MARK;
            let probe = !displaced && span.side != Side::Before && span.stopping == Stopping::Plain;
            match taken {
                Taken::Access { fault, .. } if fault == mark && span.side == Side::Before => {
                    float.enter(h.fpsr);
                    let state = state_at(way, span, h, page, after);
                    let Some(at) = operand(span) else {
                        return Some(Resolution::Displaced(state));
                    };
                    let first = access_first(at, span.guest, &state.x, state.sp);
                    let checked = Operand { offset: 0, ..*at };
                    let checked = access_first(&checked, span.guest, &state.x, state.sp);
                    // A structure access post-indexed by a register checks only its new stack pointer.
                    return Some(if outside(first) || outside(checked) {
                        Resolution::Outside {
                            address: untag_address(first),
                            state,
                        }
                    } else {
                        Resolution::Displaced(state)
                    });
                }
                Taken::Access {
                    fault,
                    alignment: false,
                    ..
                } if probe && h.sp.wrapping_sub(fault) <= PROBE_BELOW => {
                    return interrupted(way, piece, h, page, after, float, memory)
                        .map(Resolution::Probed);
                }
                Taken::StackAlignment if probe => {
                    return interrupted(way, piece, h, page, after, float, memory)
                        .map(Resolution::Probed);
                }
                _ => {}
            }
        }
        resolve_in(way, piece, taken, h, page, after, float, memory).map(Resolution::Resolved)
    }

    /// [`super::interrupted`] for an in-process way, with `after` the words after the register page.
    pub fn interrupted(
        way: Way,
        piece: &Piece<'_>,
        h: &Interrupted,
        page: &RegisterPage,
        after: &AfterPage,
        float: &mut FloatStatus,
        memory: &mut dyn FnMut(u64, &mut [u8]),
    ) -> Option<GuestState> {
        interrupted_in(way, piece, h, page, after, float, memory)
    }

    /// [`super::rejoin`] for an in-process way, with the words to put after the register page: the displaced way
    /// where the guest's stack pointer is outside its window.
    pub fn rejoin(
        way: Way,
        state: &GuestState,
        page: RegisterPage,
        float: &FloatStatus,
        register_page: u64,
    ) -> (Interrupted, RegisterPage, AfterPage) {
        rejoin_in(way, state, page, float, register_page)
    }

    /// [`super::set_register`] for an in-process way, at a stop in `span`, with `after` the words after the register
    /// page: a piece may keep a guest value elsewhere.
    pub fn set_register(
        way: Way,
        span: &Span,
        register: u8,
        value: u64,
        hardware: &mut Interrupted,
        page: &mut RegisterPage,
        after: &mut AfterPage,
    ) {
        set_in(way, &span.relocated, register, value, hardware, page, after);
    }
}
