//! What the native exits program and the monitor's exits test say to each other, after the inside's own
//! requests: one definition, read by the program and included by the test.

// Each side uses its own half.
#![allow(dead_code)]

use wcontainers_abi::{Access, Exit, Resume, SystemCallRequest};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

wcontainers_inside::requests! {
    first: wcontainers_inside::host::Request::END;
    /// The next order, with what the last one gave in the link's bytes. Answered with an [`Order`], whose
    /// bytes are in the link.
    Next {} -> Order;
    /// A slow path the CPU side warned of: its reason in the link's bytes.
    Warned { count, detail } -> ();
}

/// One thing for the program to do with its one process and thread.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct Order {
    /// An [`OrderKind`]'s word.
    pub kind: u64,
    pub address: u64,
    pub length: u64,
    pub value: u64,
}

/// What an [`Order`] asks.
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderKind {
    /// Start a new process with one thread, in place of the last one. The first makes the CPU side, the
    /// progress count in its pieces when `value` is not 0.
    Begin,
    /// Write the link's bytes to the process's memory at `address`.
    Write,
    /// Give `length` bytes at `address` the permissions whose bits are `value`.
    Protect,
    /// Give back `length` bytes of memory from `address`.
    Read,
    /// Set the thread's registers to the register block in the link's bytes.
    SetRegisters,
    /// Run the thread from the resume whose [`ResumeWords`] are `address` and `value`; give back the exit's
    /// [`ExitWords`], then the register block.
    Run,
    /// Stop.
    Finish,
}

impl OrderKind {
    pub const ALL: [OrderKind; 7] = [
        OrderKind::Begin,
        OrderKind::Write,
        OrderKind::Protect,
        OrderKind::Read,
        OrderKind::SetRegisters,
        OrderKind::Run,
        OrderKind::Finish,
    ];

    pub fn from_word(word: u64) -> Option<OrderKind> {
        Self::ALL.into_iter().find(|k| *k as u64 == word)
    }
}

/// A [`Resume`] as a kind and a value.
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResumeWords {
    Return,
    Continue,
    FromRegisters,
}

impl ResumeWords {
    pub fn words(resume: Resume) -> (u64, u64) {
        match resume {
            Resume::Return(value) => (ResumeWords::Return as u64, value),
            Resume::Continue => (ResumeWords::Continue as u64, 0),
            Resume::FromRegisters => (ResumeWords::FromRegisters as u64, 0),
        }
    }

    pub fn resume(kind: u64, value: u64) -> Resume {
        match kind {
            k if k == ResumeWords::Return as u64 => Resume::Return(value),
            k if k == ResumeWords::Continue as u64 => Resume::Continue,
            _ => Resume::FromRegisters,
        }
    }
}

/// An [`Exit`] as words: its kind, then its fields.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct ExitWords {
    pub kind: u64,
    pub address: u64,
    /// The access, the breakpoint's comment, or the system call's number.
    pub detail: u64,
    pub arguments: [u64; ARGUMENTS],
}

/// A system call's arguments: the words of a [`SystemCallRequest`] after its number.
const ARGUMENTS: usize = size_of::<SystemCallRequest>() / size_of::<u64>() - 1;

/// The kinds of [`ExitWords`], in the order of [`Exit`]'s variants.
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExitKind {
    Syscall,
    Stop,
    MemoryFault,
    MemoryFaultWithoutRegisters,
    AlignmentFault,
    IllegalInstruction,
    Breakpoint,
}

const ACCESSES: [Access; 3] = [Access::Read, Access::Write, Access::Execute];

impl ExitWords {
    pub fn of(exit: Exit) -> ExitWords {
        let words = |kind: ExitKind| ExitWords {
            kind: kind as u64,
            ..ExitWords::default()
        };
        match exit {
            Exit::Syscall(call) => ExitWords {
                detail: call.number,
                arguments: call.arguments,
                ..words(ExitKind::Syscall)
            },
            Exit::Stop => words(ExitKind::Stop),
            Exit::MemoryFault { address, access } => ExitWords {
                address,
                detail: ACCESSES
                    .iter()
                    .position(|a| *a == access)
                    .unwrap_or_default() as u64,
                ..words(ExitKind::MemoryFault)
            },
            Exit::MemoryFaultWithoutRegisters => words(ExitKind::MemoryFaultWithoutRegisters),
            Exit::AlignmentFault { address } => ExitWords {
                address,
                ..words(ExitKind::AlignmentFault)
            },
            Exit::IllegalInstruction => words(ExitKind::IllegalInstruction),
            Exit::Breakpoint { comment } => ExitWords {
                detail: comment.into(),
                ..words(ExitKind::Breakpoint)
            },
        }
    }

    pub fn exit(&self) -> Exit {
        match self.kind {
            k if k == ExitKind::Syscall as u64 => Exit::Syscall(SystemCallRequest {
                number: self.detail,
                arguments: self.arguments,
            }),
            k if k == ExitKind::Stop as u64 => Exit::Stop,
            k if k == ExitKind::MemoryFault as u64 => Exit::MemoryFault {
                address: self.address,
                access: ACCESSES[self.detail as usize],
            },
            k if k == ExitKind::AlignmentFault as u64 => Exit::AlignmentFault {
                address: self.address,
            },
            k if k == ExitKind::IllegalInstruction as u64 => Exit::IllegalInstruction,
            k if k == ExitKind::Breakpoint as u64 => Exit::Breakpoint {
                comment: self.detail as u16,
            },
            _ => Exit::MemoryFaultWithoutRegisters,
        }
    }
}
