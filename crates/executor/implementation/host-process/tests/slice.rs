//! Slices on a real host: a turn that outlasts its slice is stopped, a stop the thread does not take is sent again
//! every 100 ms, and a thread that has not come back 10 s after its stop has its process killed.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use std::sync::Arc;
use std::time::{Duration, Instant};

use rustix::process::Signal;
use wcontainers_abi::{Access, MemoryId, ThreadId};
use wcontainers_executor_host_process::executor::{HostProcessExecutor, Touch, Touched};
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_executor_host_process::stub::STOP_IGNORED;

/// One host for the whole test process, as an embedder has, so the runners of tests running at once spread over CPUs.
static HOST: std::sync::OnceLock<Arc<HostProcessHost>> = std::sync::OnceLock::new();

fn process() -> (HostProcessExecutor, MemoryId, ThreadId) {
    let mut cpu = HostProcessExecutor::new(
        HOST.get_or_init(|| Arc::new(HostProcessHost::new().expect("the host is refused")))
            .clone(),
    );
    let memory = cpu.memory_new().expect("memory");
    let thread = cpu.thread_new(memory).expect("thread");
    (cpu, memory, thread)
}

/// Where no page is mapped: a turn that comes back at once.
const UNMAPPED: u64 = 0x10_0000;

#[test]
fn a_turn_that_outlasts_its_slice_is_stopped_once_the_slice_has_passed() {
    let (mut cpu, memory, thread) = process();
    for milliseconds in [1, 5, 20] {
        let slice = Duration::from_millis(milliseconds);
        let started = Instant::now();
        assert_eq!(
            cpu.touch(thread, Touch::Spin, Some(slice)),
            Ok(Touched::Stopped)
        );
        let took = started.elapsed();
        assert!(
            took >= slice && took < slice + Duration::from_millis(100),
            "{took:?} for a slice of {slice:?}"
        );
    }
    assert_eq!(cpu.counters().stops_sent, 3);
    assert_eq!(
        cpu.touch(thread, Touch::Load(UNMAPPED), Some(Duration::from_secs(1))),
        Ok(Touched::Fault {
            address: UNMAPPED,
            access: Access::Read
        })
    );
    assert_eq!(
        cpu.counters().stops_sent,
        3,
        "a turn back in time is not stopped"
    );
    cpu.thread_drop(thread);
    cpu.memory_drop(memory);
}

/// The stub is held stopped by the host past several resends, then let go: the turn ends once, as stopped, and the
/// stops sent meanwhile land as one.
#[test]
fn a_stop_not_taken_is_sent_again_every_100_ms() {
    let (mut cpu, memory, thread) = process();
    let stub = cpu.stub_process(memory);
    rustix::process::kill_process(stub, Signal::STOP).expect("hold the stub");
    let held = Duration::from_millis(350);
    let release = std::thread::spawn(move || {
        std::thread::sleep(held);
        rustix::process::kill_process(stub, Signal::CONT).expect("let the stub go");
    });
    let started = Instant::now();
    assert_eq!(
        cpu.touch(thread, Touch::Spin, Some(Duration::from_millis(1))),
        Ok(Touched::Stopped)
    );
    assert!(started.elapsed() >= held);
    release.join().expect("release");
    let sent = cpu.counters().stops_sent;
    assert!(sent >= 4, "{sent} stops sent in {held:?}");
    let _ = cpu.touch(thread, Touch::Load(UNMAPPED), None);
    let landed = cpu.stops_landed(thread);
    assert_eq!(landed.in_turn, 1, "{landed:?}");
    // The stops sent while the stub was held were one pending signal.
    assert!(
        landed.in_turn + landed.in_publish + landed.in_wait < sent,
        "{landed:?} of {sent}"
    );
    assert_eq!(cpu.counters().stops_ignored, 0);
    cpu.thread_drop(thread);
    cpu.memory_drop(memory);
}

#[test]
fn a_thread_that_does_not_come_back_10_s_after_its_stop_has_its_process_killed() {
    let (mut cpu, memory, thread) = process();
    let stub = cpu.stub_process(memory);
    rustix::process::kill_process(stub, Signal::STOP).expect("hold the stub");
    let started = Instant::now();
    assert_eq!(
        cpu.touch(thread, Touch::Spin, Some(Duration::from_millis(1))),
        Ok(Touched::StubEnded)
    );
    let took = started.elapsed();
    assert!(
        took >= STOP_IGNORED && took < STOP_IGNORED + Duration::from_secs(1),
        "{took:?}"
    );
    assert_eq!(cpu.counters().stops_ignored, 1);
    let sent = cpu.counters().stops_sent;
    assert!(
        (95..=102).contains(&sent),
        "{sent} stops sent, one every 100 ms"
    );
    cpu.thread_drop(thread);
    cpu.memory_drop(memory);
    assert_eq!(
        rustix::process::test_kill_process(stub),
        Err(rustix::io::Errno::SRCH),
        "the stub is killed and reaped"
    );
}
