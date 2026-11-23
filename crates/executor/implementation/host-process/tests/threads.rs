//! A stub's threads on a real host: made by a command of a thread's turn, each on its own pages and signal stack,
//! ended through its exit-clear word, their indices reused only after it is zero, and never more than a stub holds.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use std::time::Duration;

use wcontainers_executor_host_process::layout::{
    SIGNAL_STACK_SIZE, SIGNAL_STACKS, THREADS_PER_PROCESS,
};
use wcontainers_executor_host_process::pages::Command;
use wcontainers_executor_host_process::probe::HostProbe;
use wcontainers_executor_host_process::stub::{Stub, TurnEnd};
use wcontainers_executor_host_process::zygote::{Ended, Zygote};

/// One embedder process has one zygote place, so tests running at once start theirs one at a time.
static STARTING: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn zygote() -> Zygote {
    let probe = HostProbe::new().expect("the host is refused");
    let _one_at_a_time = STARTING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Zygote::start(&probe).expect("the zygote did not start")
}

fn knob(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// A turn of `thread` that must end `Done`, giving the count of commands applied.
fn done(zygote: &Zygote, stub: &Stub, thread: usize, commands: &[Command]) -> u64 {
    match stub.turn(thread, commands) {
        Ok(TurnEnd::Done { commands_applied }) => commands_applied,
        Ok(TurnEnd::StubEnded) => panic!("the stub ended: {:?}", zygote.reap(stub.process())),
        other => panic!("thread {thread}'s turn ended {other:?}"),
    }
}

/// Makes a thread through thread 0's turn and gives its index.
fn new_thread(zygote: &Zygote, stub: &mut Stub) -> usize {
    let command = stub.new_thread().expect("a free index");
    let Command::NewThread { thread, .. } = command else {
        unreachable!("a new thread's command")
    };
    assert_eq!(done(zygote, stub, 0, &[command]), 1);
    thread
}

/// The host's ids of a stub's threads, from /proc (the test may; the executor never does).
fn host_threads(stub: &Stub) -> Vec<u32> {
    let mut ids: Vec<u32> =
        std::fs::read_dir(format!("/proc/{}/task", stub.process().as_raw_nonzero()))
            .expect("tasks")
            .map(|entry| {
                entry
                    .expect("task")
                    .file_name()
                    .to_str()
                    .and_then(|name| name.parse().ok())
                    .expect("a thread id")
            })
            .collect();
    ids.sort_unstable();
    ids
}

/// A waiting thread's stack pointer, the second field from the end of /proc's line for its system call; a thread
/// just published its exit and not yet back in its wait shows "running" there, so the line is read until it is in
/// a call.
fn stack_pointer(stub: &Stub, host_thread: u32) -> usize {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        let line = std::fs::read_to_string(format!(
            "/proc/{}/task/{host_thread}/syscall",
            stub.process().as_raw_nonzero()
        ))
        .expect("syscall");
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 3 {
            let sp = fields[fields.len() - 2].trim_start_matches("0x");
            return usize::from_str_radix(sp, 16).expect("a stack pointer");
        }
        assert!(
            std::time::Instant::now() < deadline,
            "thread {host_thread} never waited: {line}"
        );
        std::thread::yield_now();
    }
}

fn kill_and_reap(zygote: &Zygote, stub: &Stub) -> Ended {
    zygote.kill(stub.process()).expect("kill");
    zygote.reap(stub.process()).expect("reap")
}

#[test]
fn new_threads_take_turns_on_their_own_pages_and_signal_stacks() {
    let zygote = zygote();
    let mut stub = zygote.fork_stub().expect("fork");
    let threads: Vec<usize> = (0..8).map(|_| new_thread(&zygote, &mut stub)).collect();
    assert_eq!(threads, (1..=8).collect::<Vec<_>>());
    for _ in 0..knob("WCHP_TURNS", 1000) {
        for &thread in &threads {
            assert_eq!(done(&zygote, &stub, thread, &[]), 0);
        }
    }
    let mut reported: Vec<u32> = (0..=8)
        .map(|thread| stub.reported_thread_id(thread))
        .collect();
    for &thread in &threads {
        let stack = stack_pointer(&stub, stub.reported_thread_id(thread));
        let own = SIGNAL_STACKS + thread * SIGNAL_STACK_SIZE;
        assert!(
            (own..=own + SIGNAL_STACK_SIZE).contains(&stack),
            "thread {thread} waits on {stack:#x}, not its signal stack"
        );
    }
    reported.sort_unstable();
    assert_eq!(reported, host_threads(&stub));
    for &thread in &threads {
        stub.end_thread(thread).expect("end");
        assert!(
            stub.thread_gone(thread, Duration::from_secs(1))
                .expect("wait")
        );
    }
    // The host clears the word once the thread has left its memory, and lists it until it is reaped, a moment later.
    let alone = vec![stub.reported_thread_id(0)];
    let until = std::time::Instant::now() + Duration::from_secs(1);
    while host_threads(&stub) != alone && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(host_threads(&stub), alone);
    stub.end(0).expect("the last thread ends the stub");
    assert_eq!(zygote.reap(stub.process()).expect("reap"), Ended::Exited(0));
}

#[test]
fn an_index_is_reused_only_after_its_thread_is_gone() {
    let zygote = zygote();
    let mut stub = zygote.fork_stub().expect("fork");
    assert_eq!(new_thread(&zygote, &mut stub), 1);
    assert_eq!(new_thread(&zygote, &mut stub), 2);
    // Let go without an end: thread 2 still runs, so its exit-clear word holds its id and the index stays taken.
    stub.release_thread(2);
    assert_eq!(new_thread(&zygote, &mut stub), 3);
    stub.end_thread(1).expect("end");
    assert!(stub.thread_gone(1, Duration::from_secs(1)).expect("wait"));
    assert_eq!(new_thread(&zygote, &mut stub), 1);
    assert_eq!(done(&zygote, &stub, 1, &[]), 0);
    assert_eq!(done(&zygote, &stub, 2, &[]), 0);
    let until = std::time::Instant::now() + Duration::from_secs(1);
    while host_threads(&stub).len() != 4 && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(host_threads(&stub).len(), 4);
    assert!(matches!(kill_and_reap(&zygote, &stub), Ended::Killed(_)));
}

#[test]
fn a_stub_refuses_an_index_past_its_last() {
    let zygote = zygote();
    let stub = zygote.fork_stub().expect("fork");
    let past = Command::NewThread {
        thread: THREADS_PER_PROCESS,
        last_turn: 0,
    };
    assert_eq!(stub.turn(0, &[past]), Ok(TurnEnd::BadTurn));
    assert_eq!(host_threads(&stub).len(), 1);
    assert_eq!(done(&zygote, &stub, 0, &[]), 0);
    stub.end(0).expect("end");
    assert_eq!(zygote.reap(stub.process()).expect("reap"), Ended::Exited(0));
}

#[test]
fn every_index_held_refuses_the_next_thread() {
    let zygote = zygote();
    let mut stub = zygote.fork_stub().expect("fork");
    let mut made = 0;
    loop {
        let mut commands = Vec::new();
        while commands.len() < 64 {
            match stub.new_thread() {
                Some(command) => commands.push(command),
                None => break,
            }
        }
        if commands.is_empty() {
            break;
        }
        assert_eq!(done(&zygote, &stub, 0, &commands), commands.len() as u64);
        made += commands.len();
    }
    assert_eq!(made, THREADS_PER_PROCESS - 1);
    assert_eq!(stub.new_thread(), None);
    assert_eq!(host_threads(&stub).len(), THREADS_PER_PROCESS);
    assert_eq!(done(&zygote, &stub, THREADS_PER_PROCESS - 1, &[]), 0);
    assert!(matches!(kill_and_reap(&zygote, &stub), Ended::Killed(_)));
}
