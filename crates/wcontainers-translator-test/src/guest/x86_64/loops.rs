//! x86-64's counted loops of the checks test: loops whose range of accesses the loop proofs show allowed at
//! the entry, and the same loops running onto the page after the blob, which are counted as not proven.

use super::assemble::WORD;
use crate::guest::{Check, Counted, GuestTest};
use crate::programs::{Blob, LOAD, STACK_BYTES};
use iced_x86::code_asm::*;
use wcontainers_abi::PAGE_SIZE;
use wcontainers_translator::contract::Reason;
use wcontainers_x86::X86_64;

const EXIT_GROUP: u64 = <X86_64 as GuestTest>::STAND_IN.exit_group;
/// Pages of zeros after the code page that the loops walk.
const DATA_PAGES: u64 = 3;
/// The step of the counter of clang's initialising loop.
const STRIDE: i32 = 3;
/// Turns over half the data pages, and turns over one page more than they hold.
const WITHIN: i32 = (DATA_PAGES * PAGE_SIZE / WORD as u64 / 2) as i32;
const PAST: i32 = ((DATA_PAGES + 1) * PAGE_SIZE / WORD as u64) as i32;

/// The ways compilers count a loop.
#[derive(Clone, Copy, Debug)]
enum Form {
    /// A count down to zero by `dec` and `jnz`, loading through a moving base.
    Down,
    /// An index up to a bound by `inc`, `cmp` and `jb`, loading through a base and the scaled index.
    Up,
    /// clang's initialising loop: a store through a moving base, counted by a value moved by `STRIDE` and
    /// compared for inequality with its last value.
    Stride,
}

/// A loop of `turns` turns of `form` over the data pages, then exit with the low byte of the sum.
fn counted(form: Form, turns: i32) -> Result<Blob, IcedError> {
    let mut a = CodeAssembler::new(u64::BITS)?;
    let mut top = a.create_label();
    let data = LOAD + PAGE_SIZE;
    a.xor(ebx, ebx)?;
    a.mov(rcx, data)?;
    match form {
        Form::Down => {
            a.mov(esi, turns)?;
            a.set_label(&mut top)?;
            a.mov(rdx, qword_ptr(rcx))?;
            a.add(rcx, WORD)?;
            a.add(rbx, rdx)?;
            a.dec(esi)?;
            a.jnz(top)?;
        }
        Form::Up => {
            a.xor(eax, eax)?;
            a.set_label(&mut top)?;
            a.add(rbx, qword_ptr(rcx + rax * WORD))?;
            a.inc(rax)?;
            a.cmp(rax, turns)?;
            a.jb(top)?;
        }
        Form::Stride => {
            a.xor(eax, eax)?;
            a.set_label(&mut top)?;
            a.mov(qword_ptr(rcx), rax)?;
            a.add(rcx, WORD)?;
            a.add(rax, STRIDE)?;
            a.cmp(rax, turns * STRIDE)?;
            a.jne(top)?;
            a.mov(rbx, qword_ptr(rcx - WORD))?;
        }
    }
    a.mov(edi, ebx)?;
    a.mov(eax, EXIT_GROUP as u32)?;
    a.syscall()?;
    let mut bytes = a.assemble(LOAD)?;
    let code_bytes = bytes.len() as u64;
    assert!(code_bytes <= PAGE_SIZE);
    bytes.resize(((1 + DATA_PAGES) * PAGE_SIZE) as usize, 0);
    Ok(Blob {
        name: format!("{form:?}{turns}"),
        bytes,
        code_bytes,
        load: LOAD,
        link_address: Some(LOAD),
        main: None,
        program_headers: None,
        stack_bytes: STACK_BYTES,
    })
}

/// Each form within the data pages, never counted as not proven, and running past them, counted once for
/// its one entry.
pub(super) fn checks() -> Vec<Check> {
    let mut out = Vec::new();
    for form in [Form::Down, Form::Up, Form::Stride] {
        for (turns, entries) in [(WITHIN, 0), (PAST, 1)] {
            out.push(Check {
                blob: counted(form, turns).expect("assembles"),
                counted: Some((Reason::LoopNotProven, Counted::Exactly(entries))),
                at_entry: false,
            });
        }
    }
    out
}
