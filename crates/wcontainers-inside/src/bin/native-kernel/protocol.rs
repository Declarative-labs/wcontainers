//! What the native kernel program and the monitor's kernel test say to each other, after the inside's own
//! requests: one definition, read by the program and included by the test.

// Each side uses its own half.
#![allow(dead_code)]

use wcontainers_abi::Next;

wcontainers_inside::requests! {
    first: wcontainers_inside::container::Request::END;
    /// The container started: what the kernel runs next, as [`NextKind`]'s word with its thread and status, and
    /// the thread's register block in the link's bytes when it is one to run.
    Started { kind, thread, status } -> ();
}

/// What a [`Started`] says the kernel does next.
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NextKind {
    Run,
    Idle,
    Finished,
}

impl Started {
    pub fn of(next: Next) -> Started {
        match next {
            Next::Run { thread, .. } => Started {
                kind: NextKind::Run as u64,
                thread: thread.0.into(),
                status: 0,
            },
            Next::Idle { .. } => Started {
                kind: NextKind::Idle as u64,
                ..Started::default()
            },
            Next::Finished { status } => Started {
                kind: NextKind::Finished as u64,
                status: i64::from(status) as u64,
                ..Started::default()
            },
        }
    }
}
