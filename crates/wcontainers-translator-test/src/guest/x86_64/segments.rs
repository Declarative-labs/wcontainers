//! Layer 1's cases of the segment register and far-transfer forms: the selectors ds, es, fs and gs start with,
//! and the selectors, far pointers and return frames the instruction reads, placed over the data.

use super::*;
use wcontainers_x86::{
    CPU_NUMBER, KERNEL_CS, SELECTOR_INDEX, TASK_STATE, USER_CS, USER_DS, USER32_CS,
};

/// The segment registers an instruction may name.
pub const SEGMENT_REGISTERS: [Register; 6] = [
    Register::ES,
    Register::CS,
    Register::SS,
    Register::DS,
    Register::FS,
    Register::GS,
];

/// In a hundred: cases whose ds, es, fs and gs start with drawn selectors, selectors and frames placed where
/// the instruction reads them, of those a selector of the segment the instruction needs, an offset that is a
/// canonical address, and `iret` cases starting with NT clear.
const DRAWN_SELECTORS: u64 = 50;
const PLACED: u64 = 75;
const NEEDED: u64 = 70;
const CANONICAL_OFFSETS: u64 = 70;
const NT_CLEAR: u64 = 80;
/// In a hundred drawn selectors: null ones, and of the rest any 16 bits.
const NULL_SELECTORS: u64 = 25;
const ANY_SELECTORS: u64 = 30;

/// Selectors Linux's tables name or refuse, each with any requested privilege level.
const NAMED: [u16; 6] = [
    USER32_CS, USER_DS, USER_CS, CPU_NUMBER, TASK_STATE, KERNEL_CS,
];

pub fn is_segment_form(instruction: &Instruction) -> bool {
    use Mnemonic::*;
    match instruction.mnemonic() {
        Mov | Push | Pop => (0..instruction.op_count()).any(|n| {
            instruction.op_kind(n) == OpKind::Register
                && SEGMENT_REGISTERS.contains(&instruction.op_register(n))
        }),
        Lfs | Lgs | Lss | Retf | Iret | Iretd | Iretq => true,
        Call | Jmp => {
            use iced_x86::MemorySize::{SegPtr16, SegPtr32, SegPtr64};
            instruction.op0_kind() == OpKind::Memory
                && matches!(instruction.memory_size(), SegPtr16 | SegPtr32 | SegPtr64)
        }
        _ => false,
    }
}

/// A selector: null, one of Linux's, or anything, with a drawn privilege level and table bit.
fn selector(draw: &mut Draw) -> u16 {
    if draw.chance(NULL_SELECTORS) {
        draw.below(SELECTOR_INDEX.into()) as u16
    } else if draw.chance(ANY_SELECTORS) {
        draw.next() as u16
    } else {
        NAMED[draw.below(NAMED.len() as u64) as usize] & !(SELECTOR_INDEX - 1)
            | draw.below(SELECTOR_INDEX.into()) as u16
    }
}

/// A selector ds, es, fs or gs can hold after a return to user mode: 0 or a readable segment, with any
/// privilege level (the CPU makes a null selector with a privilege level 0 on every `iret` to user mode, so
/// a case cannot start with one).
fn kept(draw: &mut Draw) -> u16 {
    match loadable(draw) {
        null if null & !wcontainers_x86::SELECTOR_PRIVILEGE == 0 => 0,
        value => value,
    }
}

/// A selector ds, es, fs or gs can load: null or a readable segment, with any privilege level.
fn loadable(draw: &mut Draw) -> u16 {
    let segments = [0, USER32_CS, USER_DS, USER_CS, CPU_NUMBER];
    segments[draw.below(segments.len() as u64) as usize] & !wcontainers_x86::SELECTOR_PRIVILEGE
        | draw.below(u64::from(wcontainers_x86::SELECTOR_PRIVILEGE) + 1) as u16
}

/// The bytes of an operand size for a far transfer, `lfs` or the slots of a frame.
fn operand_bytes(instruction: &Instruction) -> usize {
    use Code::*;
    match instruction.code() {
        Retfw | Retfw_imm16 | Iretw | Call_m1616 | Jmp_m1616 => size_of::<u16>(),
        Retfd | Retfd_imm16 | Iretd | Call_m1632 | Jmp_m1632 => size_of::<u32>(),
        Retfq | Retfq_imm16 | Iretq | Call_m1664 | Jmp_m1664 => size_of::<u64>(),
        _ => memory_size(instruction) as usize - size_of::<u16>(),
    }
}

/// An offset a far transfer goes to: in reach of the instruction, or any canonical address, or anything.
fn offset(draw: &mut Draw, bytes: usize) -> u64 {
    let wide = if draw.chance(CANONICAL_OFFSETS) {
        match draw.below(2) {
            0 => near_target(draw),
            _ => draw.next() & (USER_END - 1),
        }
    } else {
        draw.next()
    };
    wide & (u64::MAX >> (u64::BITS as usize - bytes * u8::BITS as usize))
}

/// The low bytes of `value`.
fn slot(value: u64, bytes: usize) -> Vec<u8> {
    value.to_le_bytes()[..bytes].to_vec()
}

/// Draw what the case's segment registers start with and what the instruction of `length` bytes reads; the
/// bytes placed over the data, or `None` for a case this layer leaves out: one whose far transfer would go to
/// 32-bit code (answered by design, plan 5.3), into the reference runner or into its own bytes.
pub fn draw(
    draw: &mut Draw,
    instruction: &Instruction,
    length: usize,
    state: &mut [u128],
    aimed: Option<u64>,
    seed: u64,
) -> Option<(u32, Vec<u8>)> {
    use Mnemonic::*;
    let through = |r: Register| {
        (0..instruction.op_count()).any(|n| instruction.op_kind(n) == OpKind::Memory)
            && instruction.memory_segment() == r
    };
    if draw.chance(DRAWN_SELECTORS) {
        for (n, segment) in wcontainers_x86::SEGMENT_REGISTER_ORDER
            .into_iter()
            .enumerate()
        {
            let (base, prefix) = match segment {
                wcontainers_x86::SegmentRegister::Ds => (None, Register::DS),
                wcontainers_x86::SegmentRegister::Es => (None, Register::ES),
                wcontainers_x86::SegmentRegister::Fs => (Some(FS), Register::FS),
                wcontainers_x86::SegmentRegister::Gs => (Some(GS), Register::GS),
            };
            // A loaded fs or gs selector has a base of 0, so only where no operand goes through it.
            let value = kept(draw);
            if let Some(field) = base {
                if value == 0 || through(prefix) {
                    continue;
                }
                state[field] = 0;
            }
            state[SELECTORS + n] = value.into();
        }
    }
    let m = instruction.mnemonic();
    if m == Mov && instruction.op1_kind() == OpKind::Register {
        if let Some(field) = field_of(instruction.op1_register())
            && draw.chance(NEEDED)
        {
            let value = if draw.chance(NEEDED) {
                loadable(draw)
            } else {
                selector(draw)
            };
            state[field] = state[field] & !u128::from(u16::MAX) | u128::from(value);
        }
        return Some(Default::default());
    }
    let size = operand_bytes(instruction);
    let needed = |draw: &mut Draw, transfer: bool| {
        if !draw.chance(NEEDED) {
            selector(draw)
        } else if transfer {
            USER_CS
        } else if m == Lss {
            USER_DS
        } else {
            loadable(draw)
        }
    };
    // Where the read starts and the bytes there: the frame of a return, the pointer of a far branch or a
    // segment load, the selector of `mov` or `pop`.
    let (at, bytes) = match m {
        Retf => {
            let mut bytes = slot(offset(draw, size), size);
            bytes.extend(slot(needed(draw, true).into(), size));
            (state[RSP] as u64, bytes)
        }
        Iret | Iretd | Iretq => {
            if draw.chance(NT_CLEAR) {
                state[RFLAGS] &= !u128::from(wcontainers_x86::NT);
            }
            let ss = if draw.chance(NEEDED) {
                USER_DS
            } else {
                selector(draw)
            };
            let mut bytes = slot(offset(draw, size), size);
            for value in [
                needed(draw, true).into(),
                draw.value(),
                draw.value(),
                ss.into(),
            ] {
                bytes.extend(slot(value, size));
            }
            (state[RSP] as u64, bytes)
        }
        Pop => (
            state[RSP] as u64,
            slot(needed(draw, false).into(), size_of::<u16>()),
        ),
        Mov | Lfs | Lgs | Lss | Call | Jmp => {
            let Some(at) = aimed else {
                return Some(Default::default());
            };
            let far = matches!(m, Call | Jmp);
            let mut bytes = if m == Mov {
                Vec::new()
            } else if far {
                slot(offset(draw, size), size)
            } else {
                slot(draw.value(), size)
            };
            bytes.extend(slot(needed(draw, far).into(), size_of::<u16>()));
            (at, bytes)
        }
        _ => return Some(Default::default()),
    };
    let placed = if draw.chance(PLACED)
        && at >= DATA
        && at.wrapping_add(bytes.len() as u64) <= DATA + DATA_BYTES
    {
        ((at - DATA) as u32, bytes)
    } else {
        Default::default()
    };
    // What the instruction reads there: the data with the placed bytes over it.
    let mut data = vec![0; DATA_BYTES as usize];
    crate::layout::fill(seed, &mut data);
    crate::layout::place(&placed, &mut data);
    let read = |by: usize, bytes: usize| -> Option<u64> {
        let from = at.wrapping_add(by as u64).checked_sub(DATA)? as usize;
        let mut word = [0; size_of::<u64>()];
        word[..bytes].copy_from_slice(data.get(from..from + bytes)?);
        Some(u64::from_le_bytes(word))
    };
    let (target, code) = match m {
        Retf | Iret | Iretd | Iretq => (read(0, size), read(size, size_of::<u16>())),
        Call | Jmp => (read(0, size), read(size, size_of::<u16>())),
        _ => (None, None),
    };
    // A return to 32-bit code needs privilege level 3; a branch takes any.
    let ignored = if matches!(m, Call | Jmp) {
        wcontainers_x86::SELECTOR_PRIVILEGE
    } else {
        0
    };
    if code.is_some_and(|c| c as u16 & !ignored == USER32_CS & !ignored) {
        return None;
    }
    // An `iret` that loads AC from an unaligned frame takes an alignment check on its later reads, which
    // Linux takes for a split lock and retries for ever (seen on the reference): left out.
    let iret = matches!(m, Iret | Iretd | Iretq);
    if iret
        && read(size * 2, size).unwrap_or(0) & wcontainers_x86::ALIGNMENT_CHECK != 0
        && at % size as u64 != 0
    {
        return None;
    }
    let runner = crate::layout::x86_64::RUNNER;
    if target.is_some_and(|t| runner.contains(&t)) {
        return None;
    }
    // A transfer into its own bytes, which our side runs again where the reference stops there.
    let own = CODE..CODE + length as u64;
    if code.is_some() && target.is_some_and(|t| own.contains(&t)) {
        return None;
    }
    Some(placed)
}

/// Far transfers to Linux's 32-bit user code, which this phase does not provide: general protection at the
/// instruction, nothing changed (plan 5.3). Each reads its pointer or frame at rsp, placed there.
pub fn far_to_32_bit_cases() -> Vec<Designed> {
    let slots = |values: &[u64], bytes: usize| -> Vec<u8> {
        values.iter().flat_map(|&v| slot(v, bytes)).collect()
    };
    let landing = CODE + PAGE_SIZE;
    let eight = size_of::<u64>();
    let at_rsp = |code: Code, bytes: Vec<u8>| {
        let instruction = match code {
            Code::Jmp_m1664 | Code::Call_m1664 => {
                Instruction::with1(code, MemoryOperand::with_base(Register::RSP))
                    .expect("a far branch")
            }
            _ => Instruction::with(code),
        };
        let mut encoder = Encoder::new(wcontainers_x86::MODE_BITS);
        encoder.encode(&instruction, CODE).expect("an encoding");
        let mut case = plain(&encoder.take_buffer());
        case.placed = ((case.state.0[RSP] as u64 - DATA) as u32, bytes);
        Designed {
            name: Box::leak(format!("{code:?}").into_boxed_str()),
            case,
            stop: Some(Stop::GeneralProtection),
            fields: vec![(RIP, CODE.into())],
            compared: false,
        }
    };
    let mut pointer = slot(landing, eight);
    pointer.extend(slot(USER32_CS.into(), size_of::<u16>()));
    vec![
        at_rsp(Code::Jmp_m1664, pointer.clone()),
        at_rsp(Code::Call_m1664, pointer),
        at_rsp(Code::Retfq, slots(&[landing, USER32_CS.into()], eight)),
        at_rsp(
            Code::Iretq,
            slots(
                &[
                    landing,
                    USER32_CS.into(),
                    wcontainers_x86::ALWAYS,
                    DATA,
                    USER_DS.into(),
                ],
                eight,
            ),
        ),
    ]
}
