//! Why a guest thread came back from the CPU side, and how the kernel sends it off again.

use crate::ThreadId;

/// One system call as the guest made it: the number from x8 and the six raw values from x0 to x5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Syscall {
    pub number: u64,
    pub arguments: [u64; 6],
}

/// What a memory access was trying to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    /// Running code from the page.
    Execute,
}

/// Why a thread came back. Its registers on the CPU side are exact, with `pc` at the instruction named
/// below, unless the variant says otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// The guest ran a system call instruction. `pc` is the instruction after it.
    Syscall(Syscall),
    /// The stop flag was raised. The CPU side has already cleared it.
    Stop,
    /// An access the page's permissions do not allow. `pc` is the instruction that tried.
    MemoryFault { address: u64, access: Access },
    /// An access outside the whole address space, stopped by the host's engine. The registers are lost, so
    /// the process can only be killed; no handler may run.
    MemoryFaultWithoutRegisters,
    /// An instruction the CPU we present does not have. `pc` is that instruction.
    IllegalInstruction,
    /// A breakpoint instruction (`brk`), with its 16 bit comment field. `pc` is that instruction.
    Breakpoint { comment: u16 },
}

/// How a thread continues.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resume {
    /// Only after [`Exit::Syscall`]: put this value in x0 and carry on after the system call. No other
    /// register has changed.
    Return(u64),
    /// Only after [`Exit::Stop`]: nothing has changed, carry on.
    Continue,
    /// The kernel has replaced the thread's registers (a new thread, a new program, a signal handler, a
    /// fork child, a restored snapshot). Whatever the CPU side had cached for the thread, such as a
    /// WebAssembly stack, is thrown away and the thread starts from its saved registers.
    FromRegisters,
}

/// What the kernel wants done next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    /// Run this thread until it comes back.
    Run {
        thread: ThreadId,
        resume: Resume,
        /// If set, raise the thread's stop flag after this many nanoseconds, because another thread is
        /// waiting for a turn. If not set, no timer is needed.
        slice_nanoseconds: Option<u64>,
    },
    /// Nothing can run. Come back when an [`crate::Event`] arrives, or at this time on the monotonic clock
    /// if one is given. The driver must not spin.
    Idle { until_nanoseconds: Option<u64> },
    /// The container's first process has ended with this wait status (as Linux's `wait` reports it).
    Finished { status: i32 },
}
