//! The one dispatch table: an entry for every ARM64 Linux system call, by number.
//!
//! Each implemented call is one typed Rust function. A call Linux has that is not built yet returns
//! `ENOSYS`, is counted and warns. So does a number Linux does not have.

mod io;
mod list;
mod process;

use wcontainers_abi::Syscall;

use crate::counters::reason;
use crate::process::Tid;
use crate::{Context, Errno, Kernel};

/// How a system call ended for its thread.
pub(crate) enum Outcome {
    /// Put this in x0 and carry on.
    Return(u64),
    /// The thread no longer exists.
    Gone,
}

impl From<Result<u64, Errno>> for Outcome {
    fn from(result: Result<u64, Errno>) -> Self {
        Outcome::Return(match result {
            Ok(value) => value,
            Err(errno) => errno.to_return(),
        })
    }
}

/// One system call: the raw call as the guest made it, and the thread that made it.
pub(crate) type Handler = fn(&mut Kernel, &mut Context<'_>, Tid, Syscall) -> Outcome;

#[derive(Clone, Copy)]
struct Entry {
    /// Linux's name for the call; empty where Linux has no call with this number.
    name: &'static str,
    handler: Handler,
    implemented: bool,
}

/// One more than the highest call number Linux has.
const LENGTH: usize = {
    let mut highest = 0;
    let mut index = 0;
    while index < list::LIST.len() {
        if list::LIST[index].0 > highest {
            highest = list::LIST[index].0;
        }
        index += 1;
    }
    highest as usize + 1
};

static TABLE: [Entry; LENGTH] = {
    use crate::linux::general::*;

    let mut table = [Entry {
        name: "",
        handler: unknown,
        implemented: false,
    }; LENGTH];
    let mut index = 0;
    while index < list::LIST.len() {
        let (number, name) = list::LIST[index];
        table[number as usize] = Entry {
            name,
            handler: not_implemented,
            implemented: false,
        };
        index += 1;
    }

    let implemented: &[(u32, Handler)] = &[
        (__NR_write, io::write),
        (__NR_exit, process::exit),
        (__NR_exit_group, process::exit_group),
    ];
    let mut index = 0;
    while index < implemented.len() {
        let (number, handler) = implemented[index];
        assert!(!table[number as usize].name.is_empty());
        table[number as usize].handler = handler;
        table[number as usize].implemented = true;
        index += 1;
    }
    table
};

pub(crate) fn dispatch(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    call: Syscall,
) -> Outcome {
    // ARM64 Linux takes the call number from the low 32 bits of x8 only.
    let number = call.number as u32;
    let handler = match TABLE.get(number as usize) {
        Some(entry) => entry.handler,
        None => unknown,
    };
    let call = Syscall {
        number: u64::from(number),
        ..call
    };
    handler(kernel, cx, tid, call)
}

pub(crate) fn all() -> impl Iterator<Item = crate::SystemCall> {
    TABLE
        .iter()
        .enumerate()
        .filter(|(_, entry)| !entry.name.is_empty())
        .map(|(number, entry)| crate::SystemCall {
            number: number as u64,
            name: entry.name,
            implemented: entry.implemented,
        })
}

pub(crate) fn name(number: u64) -> Option<&'static str> {
    TABLE
        .get(usize::try_from(number).ok()?)
        .map(|entry| entry.name)
        .filter(|name| !name.is_empty())
}

fn not_implemented(kernel: &mut Kernel, cx: &mut Context<'_>, _: Tid, call: Syscall) -> Outcome {
    kernel
        .counters
        .count(cx.host, reason::NOT_IMPLEMENTED, call.number);
    Err(Errno::ENOSYS).into()
}

fn unknown(kernel: &mut Kernel, cx: &mut Context<'_>, _: Tid, call: Syscall) -> Outcome {
    kernel
        .counters
        .count(cx.host, reason::UNKNOWN_SYSCALL, call.number);
    Err(Errno::ENOSYS).into()
}
