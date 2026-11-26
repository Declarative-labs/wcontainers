//! The x87 state in the legacy area of the XSAVE image, as FXSAVE64 lays it out: the registers in stack order from
//! TOP, so that ST(i) is physical register (TOP + i) mod 8.

use wcontainers_processor_architecture_x86_64::{X87, X87_INITIAL, X87_REGISTERS};

use crate::layout::{
    FX_CONTROL_WORD, FX_DATA_POINTER, FX_INSTRUCTION_POINTER, FX_OPCODE, FX_STATUS_WORD, FX_TAG_WORD, FX_X87_REGISTERS,
    X87_COMPONENT,
};

/// The bytes of one register's value, of the 16 its slot has.
const REGISTER_BYTES: usize = 10;
const SLOT_BYTES: usize = 16;
const TOP_SHIFT: u16 = 11;
const TOP_MASK: u16 = 7;

fn to_physical(status: u16, stack_index: usize) -> usize {
    (usize::from((status >> TOP_SHIFT) & TOP_MASK) + stack_index) % X87_REGISTERS
}

pub fn write_x87(image: &mut [u8], x87: &X87) {
    image[FX_CONTROL_WORD..FX_CONTROL_WORD + 2].copy_from_slice(&x87.control.to_le_bytes());
    image[FX_STATUS_WORD..FX_STATUS_WORD + 2].copy_from_slice(&x87.status.to_le_bytes());
    image[FX_TAG_WORD] = x87.tags;
    image[FX_OPCODE..FX_OPCODE + 2].copy_from_slice(&x87.opcode.to_le_bytes());
    image[FX_INSTRUCTION_POINTER..FX_INSTRUCTION_POINTER + 8].copy_from_slice(&x87.instruction_pointer.to_le_bytes());
    image[FX_DATA_POINTER..FX_DATA_POINTER + 8].copy_from_slice(&x87.data_pointer.to_le_bytes());
    for stack_index in 0..X87_REGISTERS {
        let at = FX_X87_REGISTERS + stack_index * SLOT_BYTES;
        let value = x87.registers[to_physical(x87.status, stack_index)];
        image[at..at + REGISTER_BYTES].copy_from_slice(&value.to_le_bytes()[..REGISTER_BYTES]);
    }
}

/// The x87 state the image holds, or the initial state where XSTATE_BV says the component is in it.
pub fn read_x87(image: &[u8], saved: u64) -> X87 {
    if saved & X87_COMPONENT == 0 {
        return X87_INITIAL;
    }
    let word = |at: usize| u16::from_le_bytes([image[at], image[at + 1]]);
    let quadword = |at: usize| u64::from_le_bytes(image[at..at + 8].try_into().expect("eight bytes"));
    let status = word(FX_STATUS_WORD);
    let mut registers = [0; X87_REGISTERS];
    for stack_index in 0..X87_REGISTERS {
        let at = FX_X87_REGISTERS + stack_index * SLOT_BYTES;
        let mut bytes = [0; 16];
        bytes[..REGISTER_BYTES].copy_from_slice(&image[at..at + REGISTER_BYTES]);
        registers[to_physical(status, stack_index)] = u128::from_le_bytes(bytes);
    }
    X87 {
        registers,
        control: word(FX_CONTROL_WORD),
        status,
        tags: image[FX_TAG_WORD],
        opcode: word(FX_OPCODE),
        instruction_pointer: quadword(FX_INSTRUCTION_POINTER),
        data_pointer: quadword(FX_DATA_POINTER),
    }
}

/// Whether the state is one the chip gives back as loaded: each register 80 bits, the opcode 11 bits, the control
/// word's reserved bits clear, and FSW's ES and B each set exactly when an exception flag is set that FCW does not
/// mask, as the chip makes them once it loads FSW.
pub fn is_holdable(x87: &X87) -> bool {
    const OPCODE_BITS: u32 = 11;
    const CONTROL_RESERVED: u16 = 0xE000;
    const BUSY: u16 = 1 << 15;
    const SUMMARY: u16 = 1 << 7;
    const EXCEPTION_FLAGS: u16 = 0x3F;
    let pending = x87.status & !x87.control & EXCEPTION_FLAGS != 0;
    x87.registers.iter().all(|register| register >> (REGISTER_BYTES * 8) == 0)
        && x87.opcode >> OPCODE_BITS == 0
        && x87.control & CONTROL_RESERVED == 0
        && (x87.status & BUSY != 0) == pending
        && (x87.status & SUMMARY != 0) == pending
}
