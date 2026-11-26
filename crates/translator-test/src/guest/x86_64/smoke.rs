//! x86-64's smoke programs (`part::smoke`), each started with every register 0 but rip, and rsp at the
//! top of a writable page.

use super::{Code, Decoder, DecoderOptions};
use crate::x86_64::guest::{Smoke, UNKNOWN_SYSTEM_CALL};
use iced_x86::IcedError;
use iced_x86::code_asm::*;

/// The status the program that makes it exits with.
const AFTER_UNKNOWN: u32 = 3;
const EXIT_STATUS: u32 = 42;
/// The loop around a call adds this down to 1.
const ADDED_FROM: u32 = 10;
/// The long loop's turns, 2^30.
const LONG_TURNS: u32 = 1 << 30;

fn assembled(write: impl FnOnce(&mut CodeAssembler) -> Result<(), IcedError>) -> Vec<u8> {
    let mut a = CodeAssembler::new(u64::BITS).expect("a code assembler");
    write(&mut a).expect("assembled");
    a.assemble(0).expect("assembled")
}

/// `exit(edi)`.
fn exit(a: &mut CodeAssembler) -> Result<(), IcedError> {
    a.mov(eax, super::STAND_IN.exit as u32)?;
    a.syscall()
}

pub(super) fn smoke() -> Smoke {
    let loop_and_call = assembled(|a| {
        let (mut top, mut add) = (a.create_label(), a.create_label());
        a.xor(eax, eax)?;
        a.mov(ecx, ADDED_FROM)?;
        a.set_label(&mut top)?;
        a.call(add)?;
        a.dec(ecx)?;
        a.jne(top)?;
        a.mov(edi, eax)?;
        exit(a)?;
        a.set_label(&mut add)?;
        a.add(eax, ecx)?;
        a.ret()
    });
    let long_loop = assembled(|a| {
        let mut top = a.create_label();
        a.mov(edi, LONG_TURNS)?;
        a.set_label(&mut top)?;
        a.dec(edi)?;
        a.jne(top)?;
        exit(a)
    });
    // The loop's instructions: the two after the first.
    let mut offsets = Decoder::new(u64::BITS, &long_loop, DecoderOptions::NONE)
        .into_iter()
        .scan(0, |at, i| {
            let here = *at;
            *at += i.len() as u64;
            Some(here)
        });
    let long_loop_body = offsets.by_ref().skip(1).take(2).collect();

    Smoke {
        exit: assembled(|a| {
            a.mov(edi, EXIT_STATUS)?;
            exit(a)
        }),
        loop_and_call,
        unknown_call: assembled(|a| {
            a.mov(eax, UNKNOWN_SYSTEM_CALL as u32)?;
            a.syscall()?;
            a.mov(eax, UNKNOWN_SYSTEM_CALL as u32)?;
            a.syscall()?;
            a.mov(edi, AFTER_UNKNOWN)?;
            exit(a)
        }),
        long_loop,
        long_loop_counter: usize::from(wcontainers_x86::RDI),
        long_loop_body,
        mark: assembled(|a| a.mov(eax, 1)),
        marked: usize::from(wcontainers_x86::RAX),
        illegal: vec![
            // AVX, not offered by default.
            (assembled(|a| a.vaddps(ymm0, ymm1, ymm2)), 1),
            // AES, never offered.
            (assembled(|a| a.aesenc(xmm0, xmm1)), 1),
            // `push es`, in no form in 64-bit mode.
            (vec![Code::Pushw_ES.op_code().op_code() as u8], 0),
        ],
    }
}
