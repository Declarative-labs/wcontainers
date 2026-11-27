//! The ring 0 handler, assembled with iced-x86 at start-up: no line of this program's own text runs in the guest.
//!
//! The entry loads the slot's state and returns to ring 3 with IRETQ. Every vector has a stub that pushes its number
//! (and a zero where the chip pushes no error code) and goes on to the save, which writes what the exception left
//! into the slot and halts. A halted CPU resumes at the entry, so a parked CPU needs no register write to run again.

use std::mem::offset_of;

use iced_x86::BlockEncoderOptions;
use iced_x86::code_asm::{
    AsmRegister64, AsmRegisterSegment, CodeAssembler, CodeLabel, al, ax, cr2, cr3, dr6, ds, dword_ptr, eax, ecx, edx, es,
    fs, gs, ptr, qword_ptr, r8, r9, r10, r11, r12, r13, r14, r15, rax, rbp, rbx, rcx, rdi, rdx, rsi, rsp,
};

use crate::layout::{
    AREA_MARKER, ENTRY_BYTES, HANDLER, HANDLER_BYTES, KERNEL_TABLE, LEVELS, LOADING_CODE_OFFSET, OBSERVED_AREA,
    PHASE_INSTRUCTION, PHASE_LOADING, PHASE_READ, PHASE_READING, PHASE_STEP_FAILED, READING_CODE_OFFSET, Registers,
    SAVED_AREA, SAVED_COMPONENTS, SLOT, STACK_TOP, Slot, TILES_AFTER, TILES_BEFORE, TRACKED_AREA_BYTES, TRACKING_CODE,
    TrackingSteps, USER, pushes_error_code, to_index, to_kernel,
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
/// ES, DS, FS and GS, in the slot's order.
const DATA_SEGMENTS: [AsmRegisterSegment; 4] = [es, ds, fs, gs];
/// DR6 as reset: no condition recorded.
const DEBUG_STATUS_RESET: u32 = 0xFFFF_0FF0;
/// DR6.BS: the #DB is the trap after a single step.
const SINGLE_STEP: i32 = 1 << 14;
pub const VECTORS: usize = 256;

/// The vector the save records for SYSCALL's entry, past every interrupt vector.
pub const SYSTEM_CALL_VECTOR: u64 = VECTORS as u64;
/// The kernel's code segment SYSCALL loads, which IA32_STAR names.
pub const KERNEL_CODE_SELECTOR: u64 = 0x10;
/// Linux's user segments, which the frame of a SYSCALL entry names.
const USER_CODE_SELECTOR: i32 = 0x33;
const USER_DATA_SELECTOR: i32 = 0x2B;

/// The handler's bytes, to be placed at [`HANDLER`], and the kernel addresses of its parts.
pub struct Handler {
    pub bytes: Vec<u8>,
    pub entry: u64,
    pub stubs: [u64; VECTORS],
    /// SYSCALL's entry, which IA32_LSTAR names.
    pub system_call: u64,
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

/// XRSTOR TRACKING: the address of a field of the slot's tracking steps.
fn to_steps(offset: usize) -> u64 {
    to_slot(offset_of!(Slot, tracking) + offset)
}

/// XRSTOR TRACKING: the kernel's address of the entry that maps [`TRACKING_CODE`], whose USER bit lets ring 3 run it.
pub fn to_tracking_code_entry() -> u64 {
    to_kernel(KERNEL_TABLE + to_index(to_kernel(TRACKING_CODE), LEVELS - 1) * ENTRY_BYTES)
}

/// XRSTOR TRACKING: the loading XRSTOR64 and the reading XSAVEOPT64 of the area RBX names, each followed by UD2, at
/// their offsets in [`TRACKING_CODE`].
pub fn assemble_tracking_code() -> Vec<u8> {
    let assemble = || -> Result<Vec<u8>, iced_x86::IcedError> {
        let mut a = CodeAssembler::new(64)?;
        a.xrstor64(ptr(rbx))?;
        a.ud2()?;
        let mut loading = a.assemble(to_kernel(TRACKING_CODE) + LOADING_CODE_OFFSET)?;
        let mut b = CodeAssembler::new(64)?;
        b.xsaveopt64(ptr(rbx))?;
        b.ud2()?;
        let reading = b.assemble(to_kernel(TRACKING_CODE) + READING_CODE_OFFSET)?;
        loading.resize((READING_CODE_OFFSET - LOADING_CODE_OFFSET) as usize, 0xCC);
        loading.extend(reading);
        Ok(loading)
    };
    assemble().unwrap_or_else(|error| panic!("the tracking code assembles: {error}"))
}

/// XRSTOR TRACKING: copies `length` bytes from `from` to `to`, each an address or the field that holds it.
fn copy_area(a: &mut CodeAssembler, from: Place, to: Place, length: Place) -> Result<(), iced_x86::IcedError> {
    a.cld()?;
    for (register, place) in [(rsi, from), (rdi, to), (rcx, length)] {
        match place {
            Place::Address(address) => a.mov(register, address as i64)?,
            Place::Field(field) => a.mov(register, qword_ptr(field))?,
        }
    }
    a.rep().movsb()
}

/// XRSTOR TRACKING: an address, or a field of the slot that holds one.
#[derive(Clone, Copy)]
enum Place {
    Address(u64),
    Field(u64),
}

/// XRSTOR TRACKING: lets ring 3 run the tracking code, or no longer, and drops the old translation.
fn allow_tracking_code(a: &mut CodeAssembler, allowed: bool) -> Result<(), iced_x86::IcedError> {
    if allowed {
        a.or(qword_ptr(to_tracking_code_entry()), USER as i32)?;
    } else {
        a.and(qword_ptr(to_tracking_code_entry()), !(USER as i32))?;
    }
    a.invlpg(ptr(to_kernel(TRACKING_CODE)))
}

/// XRSTOR TRACKING: returns to ring 3 at the tracking code's offset with RBX the area and EDX:EAX the request, from
/// the fields that hold them, the trap flag set.
fn enter_tracking_code(a: &mut CodeAssembler, offset: u64, area: u64, request: u64) -> Result<(), iced_x86::IcedError> {
    allow_tracking_code(a, true)?;
    a.mov(rbx, qword_ptr(area))?;
    a.mov(eax, dword_ptr(request))?;
    a.mov(edx, dword_ptr(request + 4))?;
    a.mov(rsp, to_kernel(STACK_TOP) as i64)?;
    a.push(USER_DATA_SELECTOR)?;
    a.push(0_i32)?;
    a.push(TRACKING_FLAGS)?;
    a.push(USER_CODE_SELECTOR)?;
    a.mov(rcx, (to_kernel(TRACKING_CODE) + offset) as i64)?;
    a.push(rcx)?;
    a.iretq()
}

/// AMX TILES: where the slot's tile request names components, writes the slot's IA32_XFD and restores them from the
/// image at `image`, or saves them there and releases the tiles, so that the guest never holds TILEDATA in use while
/// a state runs that does not, nor XFD set while TILEDATA is in use; EAX, ECX and EDX are free.
fn move_tiles(a: &mut CodeAssembler, image: u64, save: bool) -> Result<(), iced_x86::IcedError> {
    let mut done = a.create_label();
    let request = to_slot(offset_of!(Slot, tile_request));
    a.cmp(qword_ptr(request), 0)?;
    a.je(done)?;
    a.xor(edx, edx)?;
    if save {
        a.mov(eax, dword_ptr(request))?;
        a.xsave64(ptr(to_kernel(image)))?;
        a.tilerelease()?;
    } else {
        a.mov(ecx, FEATURE_DISABLE)?;
        a.mov(eax, dword_ptr(to_slot(offset_of!(Slot, feature_disable))))?;
        a.wrmsr()?;
        a.mov(eax, dword_ptr(request))?;
        a.xrstor64(ptr(to_kernel(image)))?;
    }
    a.set_label(&mut done)?;
    a.nop()
}

/// IA32_XFD.
const FEATURE_DISABLE: u32 = 0x1C4;

/// XRSTOR TRACKING: RFLAGS of the tracking code: the trap flag, IF and the reserved bit.
const TRACKING_FLAGS: i32 = 1 << 8 | 1 << 9 | 1 << 1;

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
    let mut system_call = a.create_label();
    let mut instruction_entry = a.create_label();
    let mut instruction_save = a.create_label();
    let mut step_failed = a.create_label();
    let mut reading_done = a.create_label();
    let mut chosen = a.create_label();
    let mut halt = a.create_label();
    let mut stubs: Vec<CodeLabel> = (0..VECTORS).map(|_| a.create_label()).collect();

    a.set_label(&mut entry)?;
    a.mov(rax, qword_ptr(to_slot(offset_of!(Slot, page_table))))?;
    a.mov(cr3, rax)?;
    // XRSTOR64 loads the components EDX:EAX names, which the general registers loaded after overwrite.
    a.mov(eax, SAVED_COMPONENTS as u32)?;
    a.xor(edx, edx)?;
    a.xrstor64(ptr(to_slot(offset_of!(Slot, before_xsave))))?;
    move_tiles(&mut a, TILES_BEFORE, false)?;
    // XRSTOR TRACKING: the loading XRSTOR at ring 3 restores the state's own image from the area, whose bytes wait in
    // the saved page meanwhile.
    let load_area = to_steps(offset_of!(TrackingSteps, load_area));
    let phase = to_steps(offset_of!(TrackingSteps, phase));
    a.cmp(qword_ptr(load_area), 0)?;
    a.je(instruction_entry)?;
    a.mov(qword_ptr(phase), PHASE_LOADING as i32)?;
    let whole = Place::Address(TRACKED_AREA_BYTES);
    copy_area(&mut a, Place::Field(load_area), Place::Address(to_kernel(SAVED_AREA)), whole)?;
    copy_area(&mut a, Place::Address(to_slot(offset_of!(Slot, before_xsave))), Place::Field(load_area), whole)?;
    enter_tracking_code(&mut a, LOADING_CODE_OFFSET, load_area, to_steps(offset_of!(TrackingSteps, load_request)))?;
    a.set_label(&mut instruction_entry)?;
    a.mov(qword_ptr(phase), PHASE_INSTRUCTION as i32)?;
    // The selectors first, as loading FS and GS replaces their bases.
    for (index, segment) in DATA_SEGMENTS.into_iter().enumerate() {
        a.mov(rax, qword_ptr(to_before(offset_of!(Registers, data_segments) + index * size_of::<u64>())))?;
        a.mov(segment, ax)?;
    }
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

    // SYSCALL's entry: on the kernel's stack, the frame an exception from ring 3 would push, with the program's RSP,
    // R11's RFLAGS and RCX's address, then the vector.
    a.set_label(&mut system_call)?;
    a.mov(qword_ptr(to_after(to_general(STACK_POINTER))), rsp)?;
    a.mov(rsp, to_kernel(STACK_TOP) as i64)?;
    a.push(USER_DATA_SELECTOR)?;
    a.push(qword_ptr(to_after(to_general(STACK_POINTER))))?;
    a.push(r11)?;
    a.push(USER_CODE_SELECTOR)?;
    a.push(rcx)?;
    a.push(0_i32)?;
    a.push(SYSTEM_CALL_VECTOR as i32)?;
    a.jmp(save)?;

    a.set_label(&mut save)?;
    // XRSTOR TRACKING: a step ends here too; its single-step trap goes on to the instruction or to the halt.
    let reading_area = to_steps(offset_of!(TrackingSteps, reading_area));
    a.cmp(qword_ptr(to_steps(offset_of!(TrackingSteps, phase))), PHASE_INSTRUCTION as i32)?;
    a.je(instruction_save)?;
    a.mov(rax, qword_ptr(rsp))?;
    a.mov(qword_ptr(to_steps(offset_of!(TrackingSteps, step_vector))), rax)?;
    allow_tracking_code(&mut a, false)?;
    a.mov(edx, DEBUG_STATUS_RESET)?;
    a.mov(dr6, rdx)?;
    a.cmp(rax, 1)?;
    a.jne(step_failed)?;
    a.cmp(qword_ptr(to_steps(offset_of!(TrackingSteps, phase))), PHASE_LOADING as i32)?;
    a.jne(reading_done)?;
    let whole = Place::Address(TRACKED_AREA_BYTES);
    copy_area(&mut a, Place::Address(to_kernel(SAVED_AREA)), Place::Field(to_steps(offset_of!(TrackingSteps, load_area))), whole)?;
    a.jmp(instruction_entry)?;
    a.set_label(&mut reading_done)?;
    let reading_bytes = Place::Field(to_steps(offset_of!(TrackingSteps, reading_bytes)));
    copy_area(&mut a, Place::Field(reading_area), Place::Address(to_kernel(OBSERVED_AREA)), reading_bytes)?;
    copy_area(&mut a, Place::Address(to_kernel(SAVED_AREA)), Place::Field(reading_area), reading_bytes)?;
    a.mov(qword_ptr(to_steps(offset_of!(TrackingSteps, phase))), PHASE_READ as i32)?;
    a.jmp(halt)?;
    // A step that faulted: the area gets its own bytes back.
    a.set_label(&mut step_failed)?;
    a.cmp(qword_ptr(to_steps(offset_of!(TrackingSteps, phase))), PHASE_LOADING as i32)?;
    a.jne(chosen)?;
    a.mov(rax, qword_ptr(to_steps(offset_of!(TrackingSteps, load_area))))?;
    a.mov(qword_ptr(reading_area), rax)?;
    a.mov(qword_ptr(to_steps(offset_of!(TrackingSteps, reading_bytes))), TRACKED_AREA_BYTES as i32)?;
    a.set_label(&mut chosen)?;
    copy_area(&mut a, Place::Address(to_kernel(SAVED_AREA)), Place::Field(reading_area), Place::Field(to_steps(offset_of!(TrackingSteps, reading_bytes))))?;
    a.mov(qword_ptr(to_steps(offset_of!(TrackingSteps, phase))), PHASE_STEP_FAILED as i32)?;
    a.jmp(halt)?;
    a.set_label(&mut instruction_save)?;
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
    for (index, segment) in DATA_SEGMENTS.into_iter().enumerate() {
        a.mov(eax, segment)?;
        a.mov(qword_ptr(to_after(offset_of!(Registers, data_segments) + index * size_of::<u64>())), rax)?;
    }
    a.mov(eax, SAVED_COMPONENTS as u32)?;
    a.xor(edx, edx)?;
    a.xsave64(ptr(to_slot(offset_of!(Slot, after_xsave))))?;
    move_tiles(&mut a, TILES_AFTER, true)?;
    // XRSTOR TRACKING: the reading XSAVEOPT saves to the area, filled with the marker, while its bytes wait.
    let mut excepted = a.create_label();
    let mut reading = a.create_label();
    let reading_request = to_steps(offset_of!(TrackingSteps, reading_request));
    let reading_bytes = to_steps(offset_of!(TrackingSteps, reading_bytes));
    a.cmp(qword_ptr(to_slot(offset_of!(Slot, vector))), 1)?;
    a.jne(excepted)?;
    a.test(qword_ptr(to_slot(offset_of!(Slot, debug_status))), SINGLE_STEP)?;
    a.jz(excepted)?;
    a.mov(rax, qword_ptr(to_steps(offset_of!(TrackingSteps, completed_area))))?;
    a.mov(rcx, qword_ptr(to_steps(offset_of!(TrackingSteps, completed_request))))?;
    a.mov(rdx, qword_ptr(to_steps(offset_of!(TrackingSteps, completed_bytes))))?;
    a.jmp(reading)?;
    a.set_label(&mut excepted)?;
    a.mov(rax, qword_ptr(to_steps(offset_of!(TrackingSteps, excepted_area))))?;
    a.mov(rcx, -1_i64)?;
    a.mov(edx, TRACKED_AREA_BYTES as u32)?;
    a.set_label(&mut reading)?;
    a.test(rax, rax)?;
    a.jz(halt)?;
    a.mov(qword_ptr(reading_area), rax)?;
    a.mov(qword_ptr(reading_request), rcx)?;
    a.mov(qword_ptr(reading_bytes), rdx)?;
    a.mov(qword_ptr(to_steps(offset_of!(TrackingSteps, phase))), PHASE_READING as i32)?;
    copy_area(&mut a, Place::Field(reading_area), Place::Address(to_kernel(SAVED_AREA)), Place::Field(reading_bytes))?;
    a.mov(rdi, qword_ptr(reading_area))?;
    a.mov(al, AREA_MARKER as u32)?;
    a.mov(rcx, qword_ptr(reading_bytes))?;
    a.rep().stosb()?;
    enter_tracking_code(&mut a, READING_CODE_OFFSET, reading_area, reading_request)?;
    a.set_label(&mut halt)?;
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
        system_call: assembled.label_ip(&system_call)?,
    })
}
