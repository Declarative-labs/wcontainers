//! The inside's answers to instructions of the Kernel route (plan 3.5), in plain Rust over the thread's block, as
//! their meanings give them.

use crate::cpuid::{self, ECX, Registers};
use crate::x87::{Code, EXCEPTIONS, Unit};
use crate::{Offer, RAX, RBX, RCX, RDX, X86Block};
use zerocopy::IntoBytes;

/// astro-float's cache of constants, which the transcendental instructions' answers work with.
pub use astro_float_num::Consts;

/// An instruction of the Kernel route that [`answer`] answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Question {
    Cpuid,
    Xgetbv,
    /// An x87 instruction with no memory operand, run by the unit as its code says.
    X87(Code),
}

/// How an answered instruction stops instead of finishing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    GeneralProtection,
    /// A pending x87 exception, with the status word's flags that are raised and unmasked.
    FloatException(u16),
}

/// What the inside reports of a finished answer, beside the guest's new state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Note {
    None,
    /// `cpuid` read the features AVX is told by, while the process is told no AVX.
    AvxAskedAndToldNo,
}

/// Answers `question` on `block` for a process told of `offer`'s features. On `Ok` the instruction has finished
/// and the caller moves rip past it; on a fault nothing has changed.
pub fn answer(
    question: Question,
    block: &mut X86Block,
    offer: Offer,
    constants: &mut Consts,
) -> Result<Note, Fault> {
    let general = |n: u8| usize::from(n);
    match question {
        Question::Cpuid => {
            let (leaf, sub_leaf) = (
                cpuid::effective_leaf(block.gpr[general(RAX)] as u32),
                block.gpr[general(RCX)] as u32,
            );
            for (n, value) in [RAX, RBX, RCX, RDX]
                .into_iter()
                .zip(told(offer, leaf, sub_leaf))
            {
                block.gpr[general(n)] = value.into();
            }
            if !offer.avx() && cpuid::asks_for_avx(leaf, sub_leaf) {
                return Ok(Note::AvxAskedAndToldNo);
            }
        }
        Question::Xgetbv => {
            if block.gpr[general(RCX)] as u32 != 0 {
                return Err(Fault::GeneralProtection);
            }
            block.gpr[general(RAX)] = cpuid::enabled_state(offer).into();
            block.gpr[general(RDX)] = 0;
        }
        Question::X87(code) => {
            let area = &block.fxsave;
            let raised = area.fsw & !area.fcw & EXCEPTIONS;
            if raised != 0 {
                return Err(Fault::FloatException(raised));
            }
            Unit::new(block.fxsave.as_mut_bytes(), code, 0, constants).run(0, 0);
        }
    }
    Ok(Note::None)
}

/// `cpuid`'s registers for an effective leaf: a leaf of no table none, a sub-leaf past its table the table's
/// last answer with ecx echoing its low byte where the leaf does.
fn told(offer: Offer, leaf: u32, sub_leaf: u32) -> Registers {
    if !cpuid::leaves().any(|l| l == leaf) {
        return Registers::default();
    }
    let subs = cpuid::sub_leaves(leaf);
    if sub_leaf < subs {
        return cpuid::answer(offer, leaf, sub_leaf);
    }
    let mut out = cpuid::answer(offer, leaf, subs);
    if cpuid::echoes_sub_leaf(leaf) {
        out[ECX] = sub_leaf & u32::from(u8::MAX);
    }
    out
}
