//! The actions this process had before a run's that are not handlers of three arguments: a default action is given
//! back, so that the signal then does what it did before, and a handler of one argument is called. Each in a child
//! process of this test, which a variable tells which it is.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitStatus};

use rustix::process::{Resource, Rlimit, setrlimit};
use wcontainers_linux_process::{LinuxProcess, Signal};
use wcontainers_linux_process_direct::DirectLinuxProcess;

/// The variable that makes this test's binary a child, and its values.
const CHILD: &str = "WCONTAINERS_EARLIER_ACTION";
const DEFAULT_ACTION: &str = "default";
const ONE_ARGUMENT: &str = "one-argument";
/// The variable that loads a library before the child.
const PRELOAD: &str = "LD_PRELOAD";
/// The test a child runs.
const CHILD_TEST: &str = "a_child_s_signals";
/// The status the handler of one_argument_handler.c ends its process with.
const ONE_ARGUMENT_EXIT: i32 = 71;
/// How many times a child signals itself: the first reaches a run's handler, the second whatever that gave back.
const SIGNALS: usize = 2;

/// Runs this test's binary as the child `part`, with `preloaded` loaded before it.
fn child(part: &str, preloaded: Option<&str>) -> ExitStatus {
    let mut command = Command::new(std::env::current_exe().expect("this test's binary"));
    command
        .args(["--exact", CHILD_TEST, "--test-threads=1"])
        .env(CHILD, part);
    if let Some(library) = preloaded {
        command.env(PRELOAD, library);
    }
    command.status().expect("the child runs")
}

/// In a child only: a run's handlers are installed (a thread is prepared, and its handle dropped), then this thread,
/// never prepared, sends itself the signal of its part, twice.
#[test]
fn a_child_s_signals() {
    let Ok(part) = std::env::var(CHILD) else {
        return;
    };
    let signal = if part == DEFAULT_ACTION {
        Signal::ILL
    } else {
        Signal::TRAP
    };
    let no_core = Rlimit {
        current: Some(0),
        maximum: Some(0),
    };
    setrlimit(Resource::Core, no_core).expect("no core file");
    let linux = DirectLinuxProcess;
    std::thread::scope(|scope| {
        scope
            .spawn(|| linux.prepare_run_thread().map(drop))
            .join()
            .expect("the thread ends")
            .expect("the thread is prepared");
    });
    for _ in 0..SIGNALS {
        linux
            .tgkill(rustix::process::getpid(), rustix::thread::gettid(), signal)
            .expect("this thread is signalled");
    }
}

/// A process whose undefined-instruction signal did the default before a run's handler ends at it, as that default
/// does ([signal(7)](https://man7.org/linux/man-pages/man7/signal.7.html): SIGILL, Core).
#[test]
fn an_earlier_default_action_is_given_back() {
    assert_eq!(
        child(DEFAULT_ACTION, None).signal(),
        Some(Signal::ILL.as_raw())
    );
}

/// A handler of one argument this process had for the breakpoint signal before a run's is called.
#[test]
fn an_earlier_handler_of_one_argument_is_called() {
    let status = child(ONE_ARGUMENT, Some(env!("ONE_ARGUMENT_HANDLER")));
    assert_eq!(status.code(), Some(ONE_ARGUMENT_EXIT));
}
