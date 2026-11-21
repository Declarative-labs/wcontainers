//! What the harness needs of one guest CPU beyond `Guest`: its names, layer 1's forms and cases, the state
//! both sides compare, and the stand-in's numbers. Every test and the rest of the harness are written once
//! over `G: GuestTest`; each CPU fills it in here, so test-only dependencies never reach the CPU crates.

pub mod arm64;
pub mod x86_64;

use crate::layer2::{Model, Program};
use crate::programs::Blob;
use crate::reference::Target;
use proptest::strategy::BoxedStrategy;
use wcontainers_guest_cpu::{Guest, Personality};
use wcontainers_translator::contract::Reason;

/// What the harness needs of one guest beyond `Guest`. Its thread block can be compared and shown.
pub trait GuestTest: Guest + Personality<Block: PartialEq + core::fmt::Debug> {
    // --- names ---
    /// How the reference is reached: the runner's Rust target, Docker's platform, the SSH host's variable
    /// and the state's field names in wire order.
    const REFERENCE: Target;
    /// zig's and clang's target, such as `aarch64-linux-musl`.
    const C_TARGET: &'static str;
    /// Compiler flags for code with general registers only.
    const GENERAL_ONLY: &'static [&'static str];
    /// The suffix of this CPU's names, as in `layer1_arm64`.
    const NAME: &'static str;
    /// ELF's `e_machine` for this CPU.
    const ELF_MACHINE: object::elf::Machine;
    /// musl's loader inside images, such as `/lib/ld-musl-aarch64.so.1`.
    const MUSL_LOADER: &'static str;
    /// What a dynamic relocation type of a real binary does.
    fn relocation(kind: object::elf::RelocationType) -> Relocation;
    /// One instruction as text for failure messages.
    fn disassemble(code: &[u8]) -> String;

    /// Every answer the probe module can give, so a test can translate for hosts other than this one.
    const PROBE_ANSWERS: &'static [i32];
    /// The features the CPU offers, by name, and those a program is told of (decoded from what it reads,
    /// such as ARM64's HWCAP): a test checks they are the same set.
    fn offered() -> Vec<&'static str>;
    fn told() -> Vec<&'static str>;

    // --- layer 1: the forms ---
    type Form: Clone + core::fmt::Debug + Send + Sync + 'static;
    /// Every form layer 1 draws from, in a fixed order.
    fn forms() -> &'static [Self::Form];
    /// The seed key, stable across runs, and what failures are listed under.
    fn form_name(form: &Self::Form) -> &str;
    /// The group a form's failures are counted under in the summary.
    fn form_group(form: &Self::Form) -> &str;
    /// The random cases of a form, whole: the instruction with its operands shaped and the starting state
    /// from this CPU's own value strategies. The state's pc is `CODE`.
    fn cases(form: &Self::Form) -> BoxedStrategy<Case>;
    /// Cases of random bytes that are no instruction of any form, from a plain state, with data seed `seed`.
    fn no_form(seed: u64) -> BoxedStrategy<Case>;
    /// The forms of features the CPU does not offer by default, in a fixed order, drawn by `cases`: each
    /// must stop as illegal instruction.
    fn unoffered_forms() -> &'static [Self::Form];
    /// Whether the reference CPU has an unoffered form's feature, so that its run is recorded, not compared.
    fn reference_has(form: &Self::Form) -> bool;
    /// Hand-made cases: each run on both sides and compared, unless it is answered by design, and checked
    /// against what it must do.
    fn hand_cases() -> Vec<Designed>;

    // --- the code area ---
    /// The byte the code area is filled with: an instruction that stops wherever control lands.
    const FILLER: u8;
    /// How far the code area reaches either side of `CODE`.
    const CODE_SPAN: u64;

    // --- the comparable state ---
    /// The field that holds the instruction pointer, the stack pointer's, and the general registers'.
    const PC_FIELD: usize;
    const SP_FIELD: usize;
    const GENERAL_FIELDS: core::ops::Range<usize>;
    /// The thread state (`BLOCK_SIZE + PRIVATE_BYTES` bytes) as field values in `REFERENCE.fields` order,
    /// and back.
    fn to_state(thread: &[u8]) -> State;
    fn from_state(state: &State, thread: &mut [u8]);
    /// Keep the float exception flags exact from now on, as after a program wrote them.
    fn track(thread: &mut [u8]);
    /// Whether the float exception flags are kept in this thread state.
    fn tracked(thread: &[u8]) -> bool;
    /// Whether a program's code reads the float exception flags, so that they are kept exact from its start.
    fn reads_float_status(code: &[u8]) -> bool;
    /// Make the reference's outcome what the translated one must equal: take over what the architecture
    /// leaves open (registers and memory after a faulting access), and the float flags not kept.
    fn reconcile(case: &Case, expected: &mut Outcome, actual: &Outcome);
    /// Why the reference's outcome, reconciled, differing from the translated one is a known failure of the
    /// reference CPU (where it is documented not to give the exact result), or none.
    fn known_failure(case: &Case, expected: &Outcome, actual: &Outcome) -> Option<&'static str>;
    /// A reference signal as a generic stop, given where the instruction was and its length.
    fn reference_stop(signal: &Signal, length: u8) -> Stop;
    /// The `si_code` Linux gives the breakpoint trap whose detail is `detail`.
    fn breakpoint_code(detail: u16) -> u32;

    /// A reason to leave an instruction out of what is compared, as one whose behaviour the architecture
    /// leaves open, else `None`.
    fn excluded(code: &[u8]) -> Option<&'static str>;

    // --- blobs ---
    /// A flat blob's own auxiliary vector entries (what the CPU offers), in Linux's order.
    const AUXILIARY: &'static [(u64, u64)];
    /// The stand-in's system call numbers.
    const STAND_IN: StandIn;

    // --- the hand-made programs of the smoke, checks and changes tests ---
    fn smoke() -> Smoke;
    fn checks() -> Vec<Check>;
    fn changes() -> Vec<Change>;

    // --- layer 2 ---
    /// The registers layer 2's programs use.
    const MODEL: Model;
    /// A program in this CPU's code, as a flat blob that starts at its first byte.
    fn assemble(program: &Program) -> Blob;
}

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
