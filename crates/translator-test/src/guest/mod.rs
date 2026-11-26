//! What the harness keeps of one guest beyond `Guest`: the cases, states, stops and programs both sides
//! compare. Written once and compiled for each guest in `arm64` and `x86_64`, beside that guest's own part
//! (`part`), so test-only dependencies never reach the CPU crates.

use super::programs::Blob;
use wcontainers_translator::contract::Reason;

/// What a dynamic relocation does: add the load address, or put a symbol's address, or something the
/// starts of functions are not found through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relocation {
    Relative,
    Absolute,
    Other,
}

/// An instruction's bytes: the first `length` of `bytes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Encoded {
    pub bytes: [u8; 16],
    pub length: u8,
}

impl Encoded {
    pub fn new(code: &[u8]) -> Self {
        let mut bytes = [0; 16];
        bytes[..code.len()].copy_from_slice(code);
        Encoded {
            bytes,
            length: code.len() as u8,
        }
    }

    pub fn code(&self) -> &[u8] {
        &self.bytes[..usize::from(self.length)]
    }
}

/// One value per field of the guest's `REFERENCE.fields`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State(pub Vec<u128>);

/// One layer 1 case: the instruction, the state to start from, whether the float flags are kept exact, the
/// data area's seed, and bytes placed over the seed's data from an offset (a frame or a pointer the
/// instruction reads; none when empty).
#[derive(Clone, Debug)]
pub struct Case {
    pub code: Encoded,
    pub state: State,
    pub tracking: bool,
    pub seed: u64,
    pub placed: (u32, Vec<u8>),
}

/// What the reference reported: the signal, its `si_code` and `si_addr`, and the pc in the frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Signal {
    pub number: u32,
    pub code: u32,
    pub address: u64,
    pub pc: u64,
}

/// Where control went or why the thread stopped, in terms both sides share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Control reached this address and the instruction finished.
    Landed(u64),
    Illegal,
    /// A breakpoint trap, with its `si_code`.
    Breakpoint(u32),
    Syscall,
    MemoryFault(u64),
    /// SIGBUS with BUS_ADRALN and this `si_addr`: ARM64 the faulting address, x86-64 always 0.
    Alignment(u64),
    GeneralProtection,
    StackFault,
    DivideError,
    /// SIGFPE with this `si_code`.
    FloatException(u32),
    /// Anything that has no counterpart on the other side.
    Other(u32, u64),
}

/// How a case ended on one side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub stop: Stop,
    pub state: State,
    /// Changed data bytes: offset from `DATA`, new value.
    pub changed: Vec<(u32, u8)>,
}

/// A hand-made case and what it must do: its stop when given, these fields' values, and its float flags' keeping
/// and slow paths after.
pub struct Designed {
    pub name: &'static str,
    pub case: Case,
    pub stop: Option<Stop>,
    pub fields: Vec<(usize, u128)>,
    /// Compared with the reference too; false for what is answered by design.
    pub compared: bool,
    /// Whether the float exception flags must be kept after it, when given.
    pub tracking: Option<bool>,
    /// The slow paths it must have counted, by name, and how often.
    pub counted: Vec<(&'static str, u64)>,
}

/// The system call `Smoke::unknown_call` makes, which the stand-in does not have.
pub const UNKNOWN_SYSTEM_CALL: u64 = 999;

/// The smoke test's programs, each started at its first byte with every register 0 but the pc.
pub struct Smoke {
    /// Exits with 42.
    pub exit: Vec<u8>,
    /// Adds 10 down to 1 in a loop around a call and exits with the sum, 55.
    pub loop_and_call: Vec<u8>,
    /// Makes system call `UNKNOWN_SYSTEM_CALL` from two places in a row, then exits with 3.
    pub unknown_call: Vec<u8>,
    /// Counts the field `long_loop_counter` down from 2^30 to 0 in a loop of the instructions at the offsets
    /// `long_loop_body`, then exits with 0.
    pub long_loop: Vec<u8>,
    pub long_loop_counter: usize,
    pub long_loop_body: Vec<u64>,
    /// Sets the field `marked` to 1; each of `illegal` runs after it, with the times it counts as unsupported:
    /// instructions of features not offered, and bytes in no form.
    pub mark: Vec<u8>,
    pub marked: usize,
    pub illegal: Vec<(Vec<u8>, u64)>,
}

/// How many times a counter must read.
#[derive(Clone, Copy, Debug)]
pub enum Counted {
    Exactly(u64),
    /// At least once.
    Some,
}

/// A program of the checks test: Linux, none and full agree on it, and full ends as none does.
pub struct Check {
    pub blob: Blob,
    /// A counter of the runs at full and what it must read.
    pub counted: Option<(Reason, Counted)>,
    /// A check fails on a function's first instruction, so it runs in Chrome too.
    pub at_entry: bool,
}

/// A program of the changes test: at its first system call, or at the first stop where the pc and a field
/// (`at`: pc, field, value) have these values, the page at `page` is made to allow nothing; full must fault
/// exactly as none does.
pub struct Change {
    pub blob: Blob,
    pub at: Option<(u64, usize, u128)>,
    pub page: u64,
}

/// The stand-in kernel's system call numbers.
pub struct StandIn {
    pub write: u64,
    pub writev: u64,
    pub exit: u64,
    pub exit_group: u64,
    pub brk: u64,
    pub mmap: u64,
    pub munmap: u64,
    /// The call through which the C library sets its thread pointer, where the kernel sets it.
    pub set_base: Option<SetBase>,
}

/// A system call that sets a base in the thread's block (x86-64's `arch_prctl`): for each code in its first
/// argument, the block offset of the base its second argument goes to. Linux refuses a base at or above
/// `limit` with EPERM; any other code is a call the stand-in does not have.
pub struct SetBase {
    pub number: u64,
    pub bases: &'static [(u64, usize)],
    pub limit: u64,
}
