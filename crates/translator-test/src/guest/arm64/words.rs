//! ARM64's layer 1 cases as one instruction word and ARM64's own register arrays, for runners that take
//! ARM64's state as it is (the native back end's tests): each converts to and from the harness's
//! generic [`super::Case`] and [`super::Outcome`], whose fields are ARM64's in the reference's order.

use super::{FPCR, FPSR, NZCV, PC, SP, TPIDR, V, word};
use crate::arm64::guest::{self, Encoded, State};
use proptest::prelude::*;
use wcontainers_arm64::forms::Form;

pub use super::{covered, excluded, of_feature_not_offered};

/// v0 to v31.
const VECTORS: usize = FPSR - V;

/// One case (the fields up to the thread pointer, but pc): the instruction word, x0 to x30, sp, the flags (at their `pstate` bits) and the thread pointer,
/// v0 to v31, FPSR and FPCR, whether the float flags are kept exact, and the data area's seed.
#[derive(Clone, Debug)]
pub struct Case {
    pub word: u32,
    pub registers: [u64; TPIDR],
    pub vectors: [u128; VECTORS],
    /// FPSR and FPCR.
    pub float: [u32; 2],
    /// The flags are kept exact, as after a program wrote FPSR.
    pub tracking: bool,
    pub seed: u64,
}

/// How a case ended (the fields before the vectors): the stop, x0 to x30, sp, pc, the flags and the thread pointer, v0 to v31, FPSR and
/// FPCR, and the data bytes it changed (offset from `DATA`, new value).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub stop: Stop,
    pub registers: [u64; V],
    pub vectors: [u128; VECTORS],
    /// FPSR and FPCR.
    pub float: [u32; 2],
    pub changed: Vec<(u32, u8)>,
}

/// Where control went or why the thread stopped, as ARM64 can.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Control reached this address and the instruction finished.
    Landed(u64),
    Illegal,
    Breakpoint,
    Syscall,
    MemoryFault(u64),
    /// SIGBUS with BUS_ADRALN and this `si_addr`.
    Alignment(u64),
    /// Anything that has no counterpart on the other side.
    Other(u32, u64),
}

impl From<guest::Stop> for Stop {
    fn from(stop: guest::Stop) -> Stop {
        match stop {
            guest::Stop::Landed(to) => Stop::Landed(to),
            guest::Stop::Illegal => Stop::Illegal,
            guest::Stop::Breakpoint(_) => Stop::Breakpoint,
            guest::Stop::Syscall => Stop::Syscall,
            guest::Stop::MemoryFault(address) => Stop::MemoryFault(address),
            guest::Stop::Alignment(address) => Stop::Alignment(address),
            guest::Stop::Other(kind, detail) => Stop::Other(kind, detail),
            other => panic!("{other:?} is no stop of ARM64's"),
        }
    }
}

impl Case {
    /// The harness's case of the same instruction and state.
    pub fn generic(&self) -> guest::Case {
        let mut state: Vec<u128> = self.registers[..=SP].iter().map(|&r| r.into()).collect();
        state.push(crate::layout::CODE.into());
        state.extend(self.registers[SP + 1..].iter().map(|&r| u128::from(r)));
        state.extend(self.vectors);
        state.extend(self.float.map(u128::from));
        debug_assert_eq!(state.len(), FPCR + 1);
        guest::Case {
            code: Encoded::new(&self.word.to_le_bytes()),
            state: State(state),
            tracking: self.tracking,
            seed: self.seed,
            placed: Default::default(),
        }
    }

    /// A harness's case of ARM64, which places no bytes over the data.
    pub fn of(case: &guest::Case) -> Case {
        assert!(case.placed.1.is_empty(), "an ARM64 case places no bytes");
        let s = &case.state.0;
        let mut registers = [0u64; TPIDR];
        for (to, from) in registers.iter_mut().zip(&s[..=SP]) {
            *to = *from as u64;
        }
        registers[SP + 1] = s[NZCV] as u64;
        registers[SP + 2] = s[TPIDR] as u64;
        Case {
            word: word(case.code.code()),
            registers,
            vectors: core::array::from_fn(|i| s[V + i]),
            float: [s[FPSR] as u32, s[FPCR] as u32],
            tracking: case.tracking,
            seed: case.seed,
        }
    }
}

impl Outcome {
    /// An outcome of the harness's, of ARM64's fields.
    pub fn of(outcome: &guest::Outcome) -> Outcome {
        let s = &outcome.state.0;
        debug_assert_eq!(PC, SP + 1);
        Outcome {
            stop: outcome.stop.into(),
            registers: core::array::from_fn(|i| s[i] as u64),
            vectors: core::array::from_fn(|i| s[V + i]),
            float: [s[FPSR] as u32, s[FPCR] as u32],
            changed: outcome.changed.clone(),
        }
    }
}

/// Cases for one form, as layer 1 draws them.
pub fn cases(form: Form) -> impl Strategy<Value = Case> {
    super::cases(&form).prop_map(|case| Case::of(&case))
}

/// Run a case through the translator.
pub fn translated(case: &Case) -> Outcome {
    Outcome::of(&crate::arm64::layer1::translated(&case.generic()))
}
