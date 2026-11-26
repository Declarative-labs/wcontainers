//! The ring 0 handler, assembled with iced-x86 at start-up: no line of this program's own text runs in the guest.
//!
//! The entry loads the slot's state and returns to ring 3 with IRETQ. Every vector has a stub that pushes its number
//! (and a zero where the chip pushes no error code) and goes on to the save, which writes what the exception left
//! into the slot and halts. A halted CPU resumes at the entry, so a parked CPU needs no register write to run again.

use std::mem::offset_of;

use iced_x86::BlockEncoderOptions;
use iced_x86::code_asm::{
    AsmRegister64, CodeAssembler, CodeLabel, cr2, cr3, dr6, eax, edx, ptr, qword_ptr, r8, r9, r10, r11, r12, r13,
    r14, r15, rax, rbp, rbx, rcx, rdi, rdx, rsi, rsp,
};

use crate::layout::{
    HANDLER, HANDLER_BYTES, Registers, SAVED_COMPONENTS, SLOT, STACK_TOP, Slot, pushes_error_code, to_kernel,
};

/// The general registers but RSP, by their numbers.
const LOADED: [(usize, AsmRegister64); 15] = [
    (0, rax),
    (1, rcx),
    (2, rdx),
    (3, rbx),
    (5, rbp),
    (6, rsi),
    (7, rdi),
    (8, r8),
    (9, r9),
    (10, r10),
    (11, r11),
    (12, r12),
    (13, r13),
    (14, r14),
    (15, r15),
];
const STACK_POINTER: usize = 4;
/// DR6 as reset: no condition recorded.
const DEBUG_STATUS_RESET: u32 = 0xFFFF_0FF0;
pub const VECTORS: usize = 256;

/// The handler's bytes, to be placed at [`HANDLER`], and the kernel addresses of its parts.
pub struct Handler {
    pub bytes: Vec<u8>,
    pub entry: u64,
    pub stubs: [u64; VECTORS],
    /// PROBE-COST TIMING: a halt that halts again when resumed.
    pub floor: u64,
}

fn to_slot(offset: usize) -> u64 {
    to_kernel(SLOT) + offset as u64
}

fn to_before(offset: usize) -> u64 {
    to_slot(offset_of!(Slot, before) + offset)
}

fn to_after(offset: usize) -> u64 {
    to_slot(offset_of!(Slot, after) + offset)
}

fn to_general(number: usize) -> usize {
    offset_of!(Registers, general) + number * size_of::<u64>()
}

pub fn assemble() -> Handler {
    match try_assemble() {
        Ok(handler) => handler,
        Err(error) => panic!("the handler assembles: {error}"),
    }
}

fn try_assemble() -> Result<Handler, iced_x86::IcedError> {
    let mut a = CodeAssembler::new(64)?;
    let mut entry = a.create_label();
    let mut save = a.create_label();
    let mut floor = a.create_label();
    let mut stubs: Vec<CodeLabel> = (0..VECTORS).map(|_| a.create_label()).collect();

    a.set_label(&mut entry)?;
    a.mov(rax, qword_ptr(to_slot(offset_of!(Slot, page_table))))?;
    a.mov(cr3, rax)?;
    // XRSTOR64 loads the components EDX:EAX names, which the general registers loaded after overwrite.
    a.mov(eax, SAVED_COMPONENTS as u32)?;
    a.xor(edx, edx)?;
    a.xrstor64(ptr(to_slot(offset_of!(Slot, before_xsave))))?;
    a.mov(rax, qword_ptr(to_before(offset_of!(Registers, fs_base))))?;
    a.wrfsbase(rax)?;
    a.mov(rax, qword_ptr(to_before(offset_of!(Registers, gs_base))))?;
    a.wrgsbase(rax)?;
    a.mov(rsp, to_kernel(STACK_TOP) as i64)?;
    for offset in [
        offset_of!(Registers, stack_segment),
        to_general(STACK_POINTER),
        offset_of!(Registers, flags),
        offset_of!(Registers, code_segment),
        offset_of!(Registers, instruction_pointer),
    ] {
        a.push(qword_ptr(to_before(offset)))?;
    }
    for (number, register) in LOADED {
        a.mov(register, qword_ptr(to_before(to_general(number))))?;
    }
    a.iretq()?;

    for (vector, stub) in stubs.iter_mut().enumerate() {
        a.set_label(stub)?;
        if !pushes_error_code(vector as u64) {
            a.push(0_i32)?;
        }
        a.push(vector as i32)?;
        a.jmp(save)?;
    }

    a.set_label(&mut save)?;
    for (number, register) in LOADED {
        a.mov(qword_ptr(to_after(to_general(number))), register)?;
    }
    // The stub's vector and error code, then the frame the chip pushed.
    for target in [
        to_slot(offset_of!(Slot, vector)),
        to_slot(offset_of!(Slot, error_code)),
        to_after(offset_of!(Registers, instruction_pointer)),
        to_after(offset_of!(Registers, code_segment)),
        to_after(offset_of!(Registers, flags)),
        to_after(to_general(STACK_POINTER)),
        to_after(offset_of!(Registers, stack_segment)),
    ] {
        a.pop(rax)?;
        a.mov(qword_ptr(target), rax)?;
    }
    a.mov(rax, cr2)?;
    a.mov(qword_ptr(to_slot(offset_of!(Slot, fault_address))), rax)?;
    a.mov(rax, dr6)?;
    a.mov(qword_ptr(to_slot(offset_of!(Slot, debug_status))), rax)?;
    a.mov(eax, DEBUG_STATUS_RESET)?;
    a.mov(dr6, rax)?;
    a.rdfsbase(rax)?;
    a.mov(qword_ptr(to_after(offset_of!(Registers, fs_base))), rax)?;
    a.rdgsbase(rax)?;
    a.mov(qword_ptr(to_after(offset_of!(Registers, gs_base))), rax)?;
    a.mov(eax, SAVED_COMPONENTS as u32)?;
    a.xor(edx, edx)?;
    a.xsave64(ptr(to_slot(offset_of!(Slot, after_xsave))))?;
    a.hlt()?;
    a.jmp(entry)?;
    a.set_label(&mut floor)?;
    a.hlt()?;
    a.jmp(floor)?;

    let assembled = a.assemble_options(to_kernel(HANDLER), BlockEncoderOptions::RETURN_NEW_INSTRUCTION_OFFSETS)?;
    let bytes = assembled.inner.code_buffer.clone();
    assert!(bytes.len() as u64 <= HANDLER_BYTES, "the handler fits its pages");
    let mut addresses = [0; VECTORS];
    for (address, stub) in addresses.iter_mut().zip(&stubs) {
        *address = assembled.label_ip(stub)?;
    }
    Ok(Handler {
        bytes,
        entry: assembled.label_ip(&entry)?,
        stubs: addresses,
        floor: assembled.label_ip(&floor)?,
    })
}
