//! The carrying code of the native output: host instructions written around a guest instruction, from iced's
//! encoder (docs/x86-native-output.md sections 3 and 4).

use iced_x86::{Code, Encoder, Instruction as Iced, MemoryOperand, Register};

use crate::area::GUEST_GS_BASE;
use crate::decode::Instruction;
use crate::native::Way;
use crate::operand::{R10, R11, R15, general};

/// The withheld registers: the count, and two scratch registers (section 3.4's first guess).
pub const COUNT: Register = Register::R10;
pub const T: Register = Register::R11;
pub const U: Register = Register::R15;
/// u's low half, written by a 32-bit instruction that zero-extends it.
const U_LOW: Register = Register::R15D;

/// Whether the instruction names a withheld register, which a stand-in must then replace.
pub fn names_withheld(i: &Iced) -> bool {
    let withheld = |r: Register| general(r).is_some_and(|g| matches!(g.number, R10 | R11 | R15));
    (0..i.op_count())
        .any(|n| i.op_kind(n) == iced_x86::OpKind::Register && withheld(i.op_register(n)))
        || withheld(i.memory_base())
        || withheld(i.memory_index())
}

fn encode(code: &mut Vec<u8>, i: &Iced) {
    let mut encoder = Encoder::new(crate::MODE_BITS);
    encoder
        .encode(i, 0)
        .expect("the carrying code's instructions encode");
    code.extend_from_slice(&encoder.take_buffer());
}

/// The host code of an instruction whose memory operand is on the thread area's segment, in the machine or a
/// host process (section 3.1): t gets the guest's segment base from the register area, and the access is made
/// at t plus the operand's offset part, taken at the operand's own address size, so under the guest's 0x67 the
/// part is wrapped to 32 bits before the base is added. None where the instruction names a withheld register
/// or is RIP-relative, whose forms are not built yet, and in process, where the access is the window's.
pub fn area_operand(way: Way, i: &Instruction) -> Option<Vec<u8>> {
    let ins = i.iced;
    let built = matches!(way, Way::Machine | Way::Process)
        && ins.memory_segment() == way.area_segment()
        && !ins.is_ip_rel_memory_operand()
        && !names_withheld(&ins)
        && (ins.memory_index() == Register::None || general(ins.memory_index()).is_some());
    if !built {
        return None;
    }
    let mut code = Vec::new();
    let base = MemoryOperand::with_base_displ(
        Register::None,
        i64::try_from(GUEST_GS_BASE).expect("a small offset"),
    );
    let load = Iced::with2(
        Code::Mov_r64_rm64,
        T,
        MemoryOperand {
            displ_size: 8,
            segment_prefix: way.area_segment(),
            ..base
        },
    )
    .expect("a load from the register area");
    encode(&mut code, &load);
    let (guest_base, index) = (ins.memory_base(), ins.memory_index());
    let mut access = ins;
    access.set_segment_prefix(Register::None);
    if i.address32 {
        let part = MemoryOperand::new(
            guest_base,
            index,
            ins.memory_index_scale(),
            i64::from(ins.memory_displacement32() as i32),
            ins.memory_displ_size(),
            false,
            Register::None,
        );
        let offset = Iced::with2(Code::Lea_r32_m, U_LOW, part).expect("the operand's 32-bit part");
        encode(&mut code, &offset);
        access.set_memory_base(T);
        access.set_memory_index(U);
        access.set_memory_index_scale(1);
        access.set_memory_displacement64(0);
        access.set_memory_displ_size(0);
    } else if guest_base == Register::None {
        access.set_memory_base(T);
    } else if index == Register::None {
        access.set_memory_index(T);
        access.set_memory_index_scale(1);
    } else {
        let add = Iced::with2(
            Code::Lea_r64_m,
            T,
            MemoryOperand::with_base_index(guest_base, T),
        )
        .expect("t plus the operand's base");
        encode(&mut code, &add);
        access.set_memory_base(T);
    }
    encode(&mut code, &access);
    Some(code)
}
