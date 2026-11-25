//! An embedder killed on a real host leaves nothing behind: its zygote sees its socket hang up and ends, and the
//! stubs end with the zygote.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use std::io::{BufRead, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use wcontainers_abi::Cpu;
use wcontainers_executor_host_process::executor::{HostProcessExecutor, Touch, Touched};
use wcontainers_executor_host_process::host::HostProcessHost;

/// Set in the copy of this test binary that plays the embedder.
const PLAY_EMBEDDER: &str = "WCHP_PLAY_EMBEDDER";

/// The embedder's part: a host and three guest processes with a thread each, their process ids printed, then waiting
/// to be killed.
fn embedder() -> ! {
    let host = Arc::new(HostProcessHost::new().expect("the host is refused"));
    let mut cpu = HostProcessExecutor::new(host.clone());
    let mut stubs = Vec::new();
    for _ in 0..3 {
        let memory = cpu.memory_new().expect("memory");
        let thread = cpu.thread_new(memory).expect("thread");
        assert_eq!(
            cpu.touch(thread, Touch::Spin, Some(Duration::from_millis(1))),
            Ok(Touched::Stopped)
        );
        stubs.push(cpu.stub_process(memory).as_raw_nonzero().get());
    }
    let zygote = host
        .zygote_process()
        .expect("a zygote")
        .as_raw_nonzero()
        .get();
    // Written past the test harness's capture of `println!`.
    let mut out = std::io::stdout();
    writeln!(out, "zygote {zygote} stubs {stubs:?}").expect("print");
    out.flush().expect("print");
    loop {
        std::thread::park();
    }
}

/// The embedder's process, killed if the test ends early.
struct Embedder(std::process::Child);

impl Drop for Embedder {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Whether a process is gone: no longer there, or ended and waiting for its parent to reap it.
fn gone(process: i32) -> bool {
    match std::fs::read_to_string(format!("/proc/{process}/stat")) {
        Err(_) => true,
        Ok(stat) => stat
            .rsplit_once(')')
            .and_then(|(_, rest)| rest.split_whitespace().next())
            .is_some_and(|state| state == "Z" || state == "X"),
    }
}

/// A stub killed while its thread runs ends the turn as ended within 150 ms, and once dropped it is reaped.
#[test]
fn a_stub_killed_from_outside_ends_its_turn_and_is_reaped() {
    let mut cpu = HostProcessExecutor::new(Arc::new(
        HostProcessHost::new().expect("the host is refused"),
    ));
    let memory = cpu.memory_new().expect("memory");
    let thread = cpu.thread_new(memory).expect("thread");
    let stub = cpu.stub_process(memory);
    let killer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        rustix::process::kill_process(stub, rustix::process::Signal::KILL).expect("kill");
        Instant::now()
    });
    assert_eq!(cpu.touch(thread, Touch::Spin, None), Ok(Touched::StubEnded));
    let seen = Instant::now();
    let killed = killer.join().expect("killer");
    assert!(
        seen.duration_since(killed) <= Duration::from_millis(150),
        "seen {:?} after the kill",
        seen.duration_since(killed)
    );
    cpu.thread_drop(thread);
    cpu.memory_drop(memory);
    assert_eq!(
        rustix::process::test_kill_process(stub),
        Err(rustix::io::Errno::SRCH),
        "the zygote holds no zombie"
    );
}

#[test]
fn an_embedder_killed_leaves_no_zygote_or_stub_behind() {
    if std::env::var_os(PLAY_EMBEDDER).is_some() {
        embedder();
    }
    let mut child = Embedder(
        Command::new(std::env::current_exe().expect("this test binary"))
            .args([
                "--exact",
                "an_embedder_killed_leaves_no_zygote_or_stub_behind",
                "--nocapture",
                "--test-threads",
                "1",
            ])
            .env(PLAY_EMBEDDER, "1")
            .stdout(Stdio::piped())
            .spawn()
            .expect("the embedder"),
    );
    let output = child.0.stdout.take().expect("its output");
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        // The test harness prints its own words first, on the same line too.
        let found = std::io::BufReader::new(output)
            .lines()
            .map_while(Result::ok)
            .find_map(|line| line.find("zygote ").map(|at| line[at..].to_owned()));
        let _ = send.send(found);
    });
    let line = receive
        .recv_timeout(Duration::from_secs(30))
        .expect("the embedder printed nothing in 30 s")
        .expect("its process ids");
    let numbers: Vec<i32> = line
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|number| number.parse().ok())
        .collect();
    assert_eq!(numbers.len(), 4, "{line}");
    for process in &numbers {
        assert!(!gone(*process), "{process} ended early");
    }
    child.0.kill().expect("kill the embedder");
    child.0.wait().expect("reap the embedder");
    let deadline = Instant::now() + Duration::from_secs(5);
    for process in numbers {
        while !gone(process) {
            assert!(Instant::now() < deadline, "{process} outlived its embedder");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
