//! The segment registers and far transfers user mode may run: moves to and from segment registers, `push` and
//! `pop` of fs and gs, `lfs`, `lgs` and `lss`, far `call` and `jmp` through memory, `retf` and `iret`, against
//! Linux's user segments as the reference shows them.

use crate::decode::Instruction;
use crate::descriptors::{SEGMENTS, SELECTOR_PRIVILEGE};
use crate::guest::{Control, Reason, Trap};
use crate::meaning::{canonical_target, checked, load_rflags, read, write};
use crate::operand::*;
use crate::{NT, SegmentRegister, USER_CS, USER_DS, USER32_CS, X86_64};
use iced_x86::{Code, Mnemonic, OpKind, Register};
use wasm_encoder::{BlockType, ValType};
use wcontainers_guest_cpu::{Access, Emitter, Target};

/// The bits of one selector.
const SELECTOR_BITS: u32 = u16::BITS;

pub fn written(i: &Instruction) -> bool {
    use Mnemonic::*;
    match i.iced.mnemonic() {
        Mov | Push | Pop => (0..i.iced.op_count()).any(|n| segment_register_of(i, n).is_some()),
        Lfs | Lgs | Lss | Retf | Iret | Iretd | Iretq => true,
        Call | Jmp => is_far(i),
        _ => false,
    }
}

/// A far `call` or `jmp` through memory: a selector after the offset.
pub fn is_far(i: &Instruction) -> bool {
    matches!(i.iced.mnemonic(), Mnemonic::Call | Mnemonic::Jmp)
        && is_memory(i, 0)
        && memory_bytes(i) != SLOT
}

/// Where a far transfer may go, by its selector.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Transfer {
    /// `call` and `jmp`: a code segment with any requested privilege level, which the CPU replaces by 3.
    Branch,
    /// `retf` and `iret`: privilege level 3 exactly, since a return never goes to a more privileged level.
    Return,
}

/// The segment register operand `n` names, or `None` for any other operand.
fn segment_register_of(i: &Instruction, n: u32) -> Option<Register> {
    let ins = &i.iced;
    (ins.op_kind(n) == OpKind::Register
        && matches!(
            ins.op_register(n),
            Register::ES | Register::CS | Register::SS | Register::DS | Register::FS | Register::GS
        ))
    .then(|| ins.op_register(n))
}

/// The register `lfs`, `lgs` and `lss` load.
fn loaded_by(m: Mnemonic) -> Register {
    match m {
        Mnemonic::Lfs => Register::FS,
        Mnemonic::Lgs => Register::GS,
        _ => Register::SS,
    }
}

/// The block's place of a register a program can load.
fn kept(r: Register) -> Option<SegmentRegister> {
    match r {
        Register::DS => Some(SegmentRegister::Ds),
        Register::ES => Some(SegmentRegister::Es),
        Register::FS => Some(SegmentRegister::Fs),
        Register::GS => Some(SegmentRegister::Gs),
        _ => None,
    }
}

/// The bytes of an operand size: the far transfers' offsets and slots, `lfs`' offset.
fn operand_bytes(i: &Instruction) -> u32 {
    use Code::*;
    match i.iced.code() {
        Retfw | Retfw_imm16 | Iretw | Call_m1616 | Jmp_m1616 => size_of::<u16>() as u32,
        Retfd | Retfd_imm16 | Iretd | Call_m1632 | Jmp_m1632 => size_of::<u32>() as u32,
        Retfq | Retfq_imm16 | Iretq | Call_m1664 | Jmp_m1664 => SLOT,
        // `lfs`, `lgs` and `lss`: the destination register's.
        _ => memory_bytes(i) - size_of::<u16>() as u32,
    }
}

/// The slots of the frame a far transfer reads or writes at rsp, as (offset, bytes) in the order the reference
/// makes them: `retf` reads cs then rip; `iret` rflags, cs, rip, ss, then rsp; a far `call` writes cs then
/// the return address below rsp. Each slot is an access of its own, faulting by itself (a frame whose last
/// slot is not canonical faults first at its rflags slot on the reference); a return reads the selector's 16
/// bits alone of the cs and ss slots, as `pop` of a segment register does.
pub fn frame_slots(i: &Instruction) -> Option<Vec<(i64, u32)>> {
    use Mnemonic::*;
    use Slot::*;
    let s = operand_bytes(i);
    let selector = size_of::<u16>() as u32;
    let read = |slot: Slot| {
        let bytes = if matches!(slot, Cs | Ss) { selector } else { s };
        (slot as i64 * i64::from(s), bytes)
    };
    Some(match i.iced.mnemonic() {
        Retf => [Cs, Rip].map(read).to_vec(),
        Iret | Iretd | Iretq => [Rflags, Cs, Rip, Ss, Rsp].map(read).to_vec(),
        // Below rsp: cs in the highest slot, the return address under it, each written whole.
        Call if is_far(i) => [Cs, Rip]
            .map(|slot| ((slot as i64 - Cs as i64 - 1) * i64::from(s), s))
            .to_vec(),
        _ => return None,
    })
}

/// The slots of a far transfer's frame from its lowest, each of the operand size.
#[derive(Clone, Copy)]
enum Slot {
    Rip,
    Cs,
    Rflags,
    Rsp,
    Ss,
}

/// The bytes a far `call` pushes: the old cs and the return address, each of the operand size.
pub fn far_call_push(i: &Instruction) -> u32 {
    2 * operand_bytes(i)
}

/// Whether a real CPU may answer otherwise: a null selector with a privilege level reads back 0 once an
/// interrupt or signal has returned to user mode; an AMD CPU keeps fs' or gs' base on a null selector where
/// Intel clears it; a selector of 32-bit code switches a real CPU to 32-bit mode, which this phase does not
/// have; Intel leaves the upper half of a 32-bit far call's pushed cs either zero or unwritten; and AMD reads
/// `lss`'s operand with REX.W as m16:32 where Intel reads m16:64.
pub fn may_vary(i: &Instruction) -> bool {
    use Mnemonic::*;
    match i.iced.mnemonic() {
        Mov | Pop => segment_register_of(i, 0).is_some_and(|r| kept(r).is_some()),
        Lfs | Lgs | Retf | Iret | Iretd | Iretq => true,
        Lss => i.iced.code() == Code::Lss_r64_m1664,
        Call | Jmp => is_far(i),
        _ => false,
    }
}

/// Emit the meaning; true when control carries on with the next instruction.
pub fn emit<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) -> bool {
    use Mnemonic::*;
    let ins = &i.iced;
    match ins.mnemonic() {
        Mov if segment_register_of(i, 1).is_some() => {
            let mem = checked(e, i);
            let value = selector(e, ins.op_register(1));
            write(e, i, 0, mem, value);
        }
        Mov => {
            let to = ins.op_register(0);
            // cs is loaded only by a far transfer.
            if to == Register::CS {
                e.trap(Trap::Undefined);
                return false;
            }
            let mem = checked(e, i);
            let value = read(e, i, 1, mem);
            low_selector(e, value);
            load_segment(e, to, value);
            // A trap is held back over the instruction after a load of ss.
            if to == Register::SS {
                e.sink().i32_const(1);
                e.set_control(Control::StepHeld);
            }
        }
        Push => {
            let value = selector(e, ins.op_register(0));
            crate::meaning::push_value(e, i, None, value);
        }
        Pop => {
            // The CPU reads the selector's 16 bits of the slot alone.
            let read = size_of::<u16>() as u32;
            let from = e.local(ValType::I64);
            e.get(RSP);
            e.sink().local_set(from);
            e.check(from, read, Access::Read, 0);
            let value = e.local(ValType::I64);
            load(e, from, read);
            e.sink().local_set(value);
            load_segment(e, ins.op_register(0), value);
            e.sink()
                .local_get(from)
                .i64_const(stack_bytes(i).into())
                .i64_add();
            e.set(RSP);
        }
        Lfs | Lgs | Lss => {
            let (offset, value) = far_pointer(e, i);
            load_segment(e, loaded_by(ins.mnemonic()), value);
            write(e, i, 0, None, offset);
        }
        Jmp | Call => {
            let (target, value) = far_pointer(e, i);
            far_selector(e, value, Transfer::Branch);
            if ins.mnemonic() == Call {
                far_call(e, i, target);
                return true;
            }
            canonical_target(e, target);
            e.jump(Target::Address(target));
            return false;
        }
        Retf => {
            far_return(e, i);
            return false;
        }
        _ => {
            interrupt_return(e, i);
            return false;
        }
    }
    true
}

/// Keep the low 16 bits of the i64 in `value`, in place: a selector.
fn low_selector<E: Emitter<X86_64> + ?Sized>(e: &mut E, value: u32) {
    e.sink()
        .local_get(value)
        .i64_const(u16::MAX.into())
        .i64_and()
        .local_set(value);
}

/// A fresh i64 local with the selector in segment register `r`: cs and ss always Linux's 64-bit user
/// segments, the others from the block.
fn selector<E: Emitter<X86_64> + ?Sized>(e: &mut E, r: Register) -> u32 {
    let value = e.local(ValType::I64);
    match kept(r) {
        Some(at) => {
            e.get_control(Control::Selectors);
            e.sink()
                .i64_const((at as u32 * SELECTOR_BITS).into())
                .i64_shr_u()
                .i64_const(u16::MAX.into())
                .i64_and();
        }
        None => {
            e.sink()
                .i64_const(if r == Register::CS { USER_CS } else { USER_DS }.into());
        }
    }
    e.sink().local_set(value);
    value
}

/// Load segment register `r` with the selector in `value`, or stop with general protection when Linux's
/// tables give user mode no segment it may load there: a null selector or a readable segment for ds, es, fs and
/// gs, the data segment at privilege level 3 alone for ss. Loading fs or gs makes its base 0, every user
/// segment's base, and Intel's for a null selector.
fn load_segment<E: Emitter<X86_64> + ?Sized>(e: &mut E, r: Register, value: u32) {
    let allowed = e.local(ValType::I32);
    let mut s = e.sink();
    if r == Register::SS {
        s.local_get(value).i64_const(USER_DS.into()).i64_eq();
    } else {
        s.local_get(value)
            .i64_const((!SELECTOR_PRIVILEGE).into())
            .i64_and()
            .i64_eqz();
        for segment in &SEGMENTS {
            s.local_get(value)
                .i64_const((!SELECTOR_PRIVILEGE).into())
                .i64_and()
                .i64_const((segment.selector & !SELECTOR_PRIVILEGE).into())
                .i64_eq()
                .i32_or();
        }
    }
    s.local_set(allowed);
    e.sink().local_get(allowed).i32_eqz().if_(BlockType::Empty);
    e.trap(Trap::GeneralProtection);
    e.sink().end();
    let Some(at) = kept(r) else {
        return;
    };
    let shift = i64::from(at as u32 * SELECTOR_BITS);
    e.get_control(Control::Selectors);
    e.sink()
        .i64_const(!(i64::from(u16::MAX) << shift))
        .i64_and()
        .local_get(value)
        .i64_const(shift)
        .i64_shl()
        .i64_or();
    e.set_control(Control::Selectors);
    match at {
        SegmentRegister::Fs => {
            e.sink().i64_const(0);
            e.set(FS);
        }
        SegmentRegister::Gs => {
            e.sink().i64_const(0);
            e.set(GS);
        }
        _ => {}
    }
}

/// The offset of `offset_bytes` bytes at `at` and the selector after it, into fresh i64 locals.
fn far_pointer<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) -> (u32, u32) {
    let [offset, value] = pointer_pieces(i).map(|(by, bytes)| {
        // The selector's address wraps by itself under an address-size prefix.
        let mut piece = *i;
        piece
            .iced
            .set_memory_displacement64(i.iced.memory_displacement64().wrapping_add(by.into()));
        let at = address(e, &piece);
        e.check(at, bytes, Access::Read, 0);
        let value = e.local(ValType::I64);
        load(e, at, bytes);
        e.sink().local_set(value);
        value
    });
    (offset, value)
}

/// The two reads of a far pointer, as (offset in the operand, bytes): the offset, then the selector after it;
/// each faults by itself.
pub fn pointer_pieces(i: &Instruction) -> [(u32, u32); 2] {
    let offset_bytes = operand_bytes(i);
    [(0, offset_bytes), (offset_bytes, size_of::<u16>() as u32)]
}

/// Whether the instruction's memory operand is a far pointer.
pub fn reads_pointer(i: &Instruction) -> bool {
    is_far(i)
        || matches!(
            i.iced.mnemonic(),
            Mnemonic::Lfs | Mnemonic::Lgs | Mnemonic::Lss
        )
}

/// Stop with general protection unless the selector in `value` names 64-bit code a transfer of this kind may
/// go to; a selector of Linux's 32-bit user code is counted as what this phase does not provide.
fn far_selector<E: Emitter<X86_64> + ?Sized>(e: &mut E, value: u32, transfer: Transfer) {
    let ignored = match transfer {
        Transfer::Branch => SELECTOR_PRIVILEGE,
        Transfer::Return => 0,
    };
    let named = |e: &mut E, selector: u16| {
        e.sink()
            .local_get(value)
            .i64_const((!ignored).into())
            .i64_and()
            .i64_const((selector & !ignored).into())
            .i64_eq();
    };
    named(e, USER32_CS);
    e.sink().if_(BlockType::Empty);
    e.warn(Reason::FarTransferTo32Bit);
    e.trap(Trap::GeneralProtection);
    e.sink().end();
    named(e, USER_CS);
    e.sink().i32_eqz().if_(BlockType::Empty);
    e.trap(Trap::GeneralProtection);
    e.sink().end();
}

/// The rest of a far `call` once its selector is tested: push the old cs and the return address, each of the
/// operand size, test the offset is canonical, then call it.
fn far_call<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, target: u32) {
    let size = operand_bytes(i);
    let stack = Instruction {
        address32: false,
        ..*i
    };
    let slots = frame_slots(i).expect("a frame");
    let [code_at, at] = [slots[0], slots[1]].map(|(by, _)| register_address(e, &stack, RSP, by));
    // cs is pushed first: where only the return address's slot is not allowed, the reference CPU faults
    // there with cs already written, rsp unchanged.
    e.check(code_at, size, Access::Write, 1);
    let back = e.local(ValType::I64);
    e.sink().i64_const(USER_CS.into()).local_set(back);
    store(e, code_at, size, back);
    e.check(at, size, Access::Write, 1);
    e.this_address(i.iced.len() as i64);
    e.sink().local_set(back);
    store(e, at, size, back);
    canonical_target(e, target);
    e.sink().local_get(at);
    e.set(RSP);
    e.call(Target::Address(target));
}

/// Read a slot of the frame at `from` (`frame_slots`' offset and bytes), zero-extended, into a fresh i64 local.
fn frame_slot<E: Emitter<X86_64> + ?Sized>(e: &mut E, from: u32, (by, bytes): (i64, u32)) -> u32 {
    let (at, value) = (e.local(ValType::I64), e.local(ValType::I64));
    e.sink()
        .local_get(from)
        .i64_const(by)
        .i64_add()
        .local_set(at);
    e.check(at, bytes, Access::Read, 0);
    load(e, at, bytes);
    e.sink().local_set(value);
    value
}

/// `retf` and `retf imm16`: cs and the offset popped, cs tested, then the return.
fn far_return<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let slots = frame_slots(i).expect("a frame");
    let from = e.local(ValType::I64);
    e.get(RSP);
    e.sink().local_set(from);
    let mut slots = slots.into_iter();
    let mut next = |e: &mut E| frame_slot(e, from, slots.next().expect("a slot"));
    let code = next(e);
    let target = next(e);
    low_selector(e, code);
    far_selector(e, code, Transfer::Return);
    canonical_target(e, target);
    let extra = if i.iced.op_count() > 0 {
        i.iced.immediate16()
    } else {
        0
    };
    // `ret` moves rsp up by one slot of its own.
    let popped = i64::from(2 * operand_bytes(i)) + i64::from(extra) - i64::from(SLOT);
    if popped != 0 {
        e.sink().local_get(from).i64_const(popped).i64_add();
        e.set(RSP);
    }
    e.ret(target);
}

/// `iret`: general protection with NT set, before anything is read; then the frame in the reference's order,
/// cs tested once rip is read, ss after its slot and rsp's, the rip's canonical test last; then rflags as
/// `popf` loads it, rsp and the return.
fn interrupt_return<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let slots = frame_slots(i).expect("a frame");
    e.get_control(Control::Rflags);
    e.sink()
        .i64_const(NT as i64)
        .i64_and()
        .i64_const(0)
        .i64_ne()
        .if_(BlockType::Empty);
    e.trap(Trap::GeneralProtection);
    e.sink().end();
    let from = e.local(ValType::I64);
    e.get(RSP);
    e.sink().local_set(from);
    let mut slots = slots.into_iter();
    let mut next = |e: &mut E| frame_slot(e, from, slots.next().expect("a slot"));
    let flags = next(e);
    let code = next(e);
    let target = next(e);
    low_selector(e, code);
    far_selector(e, code, Transfer::Return);
    let stack = next(e);
    let pointer = next(e);
    low_selector(e, stack);
    e.sink()
        .local_get(stack)
        .i64_const(USER_DS.into())
        .i64_ne()
        .if_(BlockType::Empty);
    e.trap(Trap::GeneralProtection);
    e.sink().end();
    canonical_target(e, target);
    load_rflags(e, flags, operand_bytes(i));
    // `ret` moves rsp up by one slot of its own.
    e.sink()
        .local_get(pointer)
        .i64_const(-i64::from(SLOT))
        .i64_add();
    e.set(RSP);
    e.ret(target);
}
