//! x86-64's hand-made programs whose pages change at a system call or a stop (`GuestTest::changes`): groups
//! and loops across system calls and calls, accesses below rsp and its red zone once a frame is popped, and
//! the longest
//! access onto a page changed at a stop. rbx holds `DATA` throughout.

use super::X86_64;
use crate::guest::{Change, GuestTest};
use crate::programs::{Blob, LOAD, STACK_BYTES, STACK_TOP};
use iced_x86::code_asm::*;
use iced_x86::{BlockEncoderOptions, IcedError};
use wcontainers_abi::PAGE_SIZE;
use wcontainers_x86::{R15, XMM_BYTES};

type Asm = Result<(), IcedError>;

/// The page of data after the code page of the programs.
const DATA: u64 = LOAD + PAGE_SIZE;
/// The page below the one rsp is moved to, in the middle of the stack.
const BELOW: u64 = (STACK_TOP - STACK_BYTES / 2) / PAGE_SIZE * PAGE_SIZE;
/// The bytes of the frame made and popped there.
const FRAME: i32 = 256;
/// Where below rsp the popped frame is read: past the red zone, which belongs to the running function's frame
/// as x86's calling convention makes it.
const BELOW_RED_ZONE: i32 = <X86_64 as wcontainers_guest_cpu::Guest>::RED_ZONE as i32 + WORD;
/// Turns of the loops the changes land in; the change comes at the head after the first.
const TURNS: u32 = 4;
const WORD: i32 = size_of::<u64>() as i32;

/// A system call that writes nothing.
fn write_nothing(a: &mut CodeAssembler) -> Asm {
    a.mov(eax, <X86_64 as GuestTest>::STAND_IN.write as u32)?;
    a.mov(edi, 1u32)?;
    a.mov(rsi, rbx)?;
    a.xor(edx, edx)?;
    a.syscall()
}

/// A function that makes a system call like `write_nothing` and returns, at `f`.
fn writing_nothing(a: &mut CodeAssembler, f: &mut CodeLabel) -> Asm {
    a.set_label(f)?;
    write_nothing(a)?;
    a.ret()
}

/// A program: rbx at `DATA`, the code `write` makes, then exit with 0; then two pages of data after the code
/// page. Returns the blob and the address of the label `head` names, if any.
fn program(
    name: &str,
    write: impl FnOnce(&mut CodeAssembler, &mut CodeLabel) -> Asm,
) -> (Blob, u64) {
    let mut a = CodeAssembler::new(u64::BITS).expect("a code assembler");
    let mut head = a.create_label();
    a.mov(rbx, DATA).unwrap();
    write(&mut a, &mut head).expect("assembled");
    a.mov(eax, <X86_64 as GuestTest>::STAND_IN.exit_group as u32)
        .unwrap();
    a.xor(edi, edi).unwrap();
    a.syscall().unwrap();
    let result = a
        .assemble_options(LOAD, BlockEncoderOptions::RETURN_NEW_INSTRUCTION_OFFSETS)
        .expect("assembled");
    let at = result.label_ip(&head).unwrap_or(0);
    let mut bytes = result.inner.code_buffer;
    assert!(bytes.len() as u64 <= PAGE_SIZE, "{name} fits its code page");
    bytes.resize((DATA + 2 * PAGE_SIZE - LOAD) as usize, 0);
    let blob = Blob {
        name: name.into(),
        code_bytes: PAGE_SIZE,
        bytes,
        load: LOAD,
        program_headers: None,
        stack_bytes: STACK_BYTES,
    };
    (blob, at)
}

/// Move rsp to the top of `BELOW`, then make and pop a frame there (rbp keeps the old rsp).
fn pop_a_frame_at_below(a: &mut CodeAssembler) -> Asm {
    a.mov(rbp, rsp)?;
    a.mov(rsp, BELOW + PAGE_SIZE)?;
    a.sub(rsp, FRAME)?;
    a.mov(qword_ptr(rsp), rax)?;
    a.add(rsp, FRAME)
}

pub(super) fn changes() -> Vec<Change> {
    let mut out = Vec::new();
    let mut at_syscall = |name: &str, page, write: &dyn Fn(&mut CodeAssembler) -> Asm| {
        let (blob, _) = program(name, |a, _| write(a));
        out.push(Change {
            blob,
            at: None,
            page,
        });
    };
    // Two loads through one base with a system call between them: the second is checked again.
    at_syscall("group-across-syscall", DATA, &|a| {
        a.mov(r8, qword_ptr(rbx))?;
        write_nothing(a)?;
        a.mov(r9, qword_ptr(rbx + WORD))
    });
    // A loop that loads through a base and then makes a system call: the next turn's load is checked again.
    at_syscall("loop-holding-syscall", DATA, &|a| {
        let mut top = a.create_label();
        a.mov(r15d, TURNS - 1)?;
        a.set_label(&mut top)?;
        a.mov(r8, qword_ptr(rbx + r15 * WORD))?;
        a.add(r10, r8)?;
        write_nothing(a)?;
        a.dec(r15)?;
        a.jne(top)
    });
    // Two loads through one base with a call between them whose callee makes a system call.
    at_syscall("group-across-call", DATA, &|a| {
        let (mut f, mut end) = (a.create_label(), a.create_label());
        a.mov(r8, qword_ptr(rbx))?;
        a.call(f)?;
        a.mov(r9, qword_ptr(rbx + WORD))?;
        a.jmp(end)?;
        writing_nothing(a, &mut f)?;
        a.set_label(&mut end)?;
        a.nop()
    });
    // A loop over memory holding a call whose callee makes a system call.
    at_syscall("loop-holding-call", DATA, &|a| {
        let (mut f, mut top, mut end) = (a.create_label(), a.create_label(), a.create_label());
        a.mov(r15d, TURNS - 1)?;
        a.set_label(&mut top)?;
        a.mov(r8, qword_ptr(rbx + r15 * WORD))?;
        a.add(r10, r8)?;
        a.call(f)?;
        a.dec(r15)?;
        a.jne(top)?;
        a.jmp(end)?;
        writing_nothing(a, &mut f)?;
        a.set_label(&mut end)?;
        a.nop()
    });
    // Below rsp once a frame is popped is no frame: a change there at a system call is seen at the next
    // access.
    at_syscall("below-sp-across-syscall", BELOW, &|a| {
        pop_a_frame_at_below(a)?;
        write_nothing(a)?;
        a.mov(r9, qword_ptr(rsp - BELOW_RED_ZONE))?;
        a.mov(rsp, rbp)
    });
    // The same with the access in a loop and the change at its first stop.
    let (blob, head) = program("below-sp-in-a-loop", |a, head| {
        pop_a_frame_at_below(a)?;
        a.mov(r15d, TURNS)?;
        a.set_label(head)?;
        a.mov(r9, qword_ptr(rsp - BELOW_RED_ZONE))?;
        a.add(r10, r9)?;
        a.dec(r15)?;
        a.jne(*head)?;
        a.mov(rsp, rbp)
    });
    out.push(Change {
        blob,
        at: Some((head, usize::from(R15), u128::from(TURNS - 1))),
        page: BELOW,
    });
    // A counted loop of 16 byte accesses walking to the end of `DATA`, the last one half onto the page
    // after, which is made to allow nothing at the first stop: the loop's proof fails and its checked copy
    // keeps the run of pages it finds allowed, which must not let that last access through.
    let step = XMM_BYTES as i32;
    let first = DATA + PAGE_SIZE - u64::from(TURNS) * XMM_BYTES as u64 + XMM_BYTES as u64 / 2;
    for load in [true, false] {
        let name = if load {
            "longest-access-load"
        } else {
            "longest-access-store"
        };
        let (blob, head) = program(name, |a, head| {
            a.mov(rdi, first)?;
            a.mov(r15d, TURNS)?;
            a.set_label(head)?;
            if load {
                a.movdqu(xmm0, xmmword_ptr(rdi))?;
            } else {
                a.movdqu(xmmword_ptr(rdi), xmm0)?;
            }
            a.add(rdi, step)?;
            a.dec(r15)?;
            a.jne(*head)
        });
        out.push(Change {
            blob,
            at: Some((head, usize::from(R15), u128::from(TURNS - 1))),
            page: DATA + PAGE_SIZE,
        });
    }
    out
}
