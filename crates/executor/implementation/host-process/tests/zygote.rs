//! The zygote on a real host: it holds nothing of the embedder, survives migration and signals, and ends cleanly.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use std::time::{Duration, Instant};

use rustix::io::Errno;
use rustix::process::{Pid, Signal};
use rustix::thread::CpuSet;
use wcontainers_executor_host_process::layout::{
    SIGNAL_STACKS, SIGNAL_STACKS_SIZE, THREAD_PAGES, ZYGOTE, ZYGOTE_SIZE,
};
use wcontainers_executor_host_process::probe::HostProbe;
use wcontainers_executor_host_process::zygote::{Ended, Zygote, ZygoteEnd};

/// The fourth check's count of moves between two CPUs.
const MOVES: usize = 200;

fn zygote() -> Zygote {
    let probe = HostProbe::new().expect("the host is refused");
    Zygote::start(&probe).expect("the zygote did not start")
}

fn two_cpus() -> [CpuSet; 2] {
    let allowed = rustix::thread::sched_getaffinity(None).expect("affinity");
    let mut cpus = (0..CpuSet::MAX_CPU)
        .filter(|cpu| allowed.is_set(*cpu))
        .map(|cpu| {
            let mut set = CpuSet::new();
            set.set(cpu);
            set
        });
    [
        cpus.next().expect("a first CPU"),
        cpus.next().expect("a second CPU"),
    ]
}

fn signal_number(signal: Signal) -> u32 {
    signal.as_raw().unsigned_abs()
}

#[test]
fn survives_its_own_migration() {
    let zygote = zygote();
    let cpus = two_cpus();
    for move_number in 0..MOVES {
        rustix::thread::sched_setaffinity(Some(zygote.process()), &cpus[move_number % 2])
            .expect("move");
        zygote
            .ping()
            .unwrap_or_else(|errno| panic!("no answer after move {move_number}: {errno}"));
    }
    assert_eq!(
        zygote.end(),
        Ok(Ended::Exited(ZygoteEnd::EmbedderGone as u32))
    );
}

#[test]
fn a_stub_survives_migration_and_stops_then_dies_by_its_kill() {
    let zygote = zygote();
    let stub = zygote.fork_stub().expect("fork").process();
    let cpus = two_cpus();
    for move_number in 0..MOVES {
        rustix::thread::sched_setaffinity(Some(stub), &cpus[move_number % 2]).expect("move");
        rustix::process::kill_process(stub, Signal::STOP).expect("stop");
        rustix::process::kill_process(stub, Signal::CONT).expect("continue");
    }
    zygote.ping().expect("the zygote answers");
    zygote.kill(stub).expect("kill");
    assert_eq!(
        zygote.reap(stub),
        Ok(Ended::Killed(signal_number(Signal::KILL)))
    );
}

/// Every mapping of a process, as /proc shows it (the test may read /proc; the executor never does).
fn mappings(process: Pid) -> Vec<(usize, usize, String)> {
    let maps =
        std::fs::read_to_string(format!("/proc/{}/maps", process.as_raw_nonzero())).expect("maps");
    maps.lines()
        .map(|line| {
            let range = line.split_whitespace().next().expect("a range");
            let (start, end) = range.split_once('-').expect("start-end");
            let parse = |hex| usize::from_str_radix(hex, 16).expect("hex");
            (parse(start), parse(end), line.to_owned())
        })
        .collect()
}

/// Checks that a process maps only the zygote's pages and `also`, and holds at most `descriptors` descriptors.
fn assert_holds_only_its_own(process: Pid, also: Option<(usize, usize)>, descriptors: usize) {
    for (start, end, line) in mappings(process) {
        let own = start >= ZYGOTE && end <= ZYGOTE + ZYGOTE_SIZE
            || also.is_some_and(|(from, to)| start >= from && end <= to);
        // x86-64's legacy vsyscall page lies above user space and cannot be unmapped.
        let vsyscall = line.ends_with("[vsyscall]");
        assert!(
            own || vsyscall,
            "process {} still maps {line}",
            process.as_raw_nonzero()
        );
    }
    let held = std::fs::read_dir(format!("/proc/{}/fd", process.as_raw_nonzero()))
        .expect("fds")
        .count();
    assert!(
        held <= descriptors,
        "process {} holds {held} descriptors",
        process.as_raw_nonzero()
    );
}

#[test]
fn neither_zygote_nor_stub_holds_any_memory_or_descriptor_of_the_embedder() {
    let zygote = zygote();
    assert_holds_only_its_own(zygote.process(), None, 1);
    let stub = zygote.fork_stub().expect("fork");
    // A turn is answered only once the stub has made its pages and installed its filter.
    assert!(matches!(
        stub.turn(0, &[]),
        Ok(wcontainers_executor_host_process::stub::TurnEnd::Done { .. })
    ));
    let stub = stub.process();
    // Its socket and its memory.
    assert_holds_only_its_own(
        stub,
        Some((THREAD_PAGES, SIGNAL_STACKS + SIGNAL_STACKS_SIZE)),
        2,
    );
    zygote.kill(stub).expect("kill");
    assert_eq!(
        zygote.reap(stub),
        Ok(Ended::Killed(signal_number(Signal::KILL)))
    );
}

/// The signals a process blocks, as /proc shows them.
fn blocked_signals(process: Pid) -> u64 {
    let status = std::fs::read_to_string(format!("/proc/{}/status", process.as_raw_nonzero()))
        .expect("status");
    let blocked = status
        .lines()
        .find_map(|line| line.strip_prefix("SigBlk:"))
        .expect("a SigBlk line");
    u64::from_str_radix(blocked.trim(), 16).expect("hex")
}

#[test]
fn the_zygote_blocks_every_signal_but_those_none_can() {
    let zygote = zygote();
    let bit = |signal: Signal| 1_u64 << (signal_number(signal) - 1);
    assert_eq!(
        blocked_signals(zygote.process()),
        !(bit(Signal::KILL) | bit(Signal::STOP))
    );
}

#[test]
fn many_stubs_are_forked_killed_and_reaped() {
    let zygote = zygote();
    let stubs: Vec<Pid> = (0..64)
        .map(|_| zygote.fork_stub().expect("fork").process())
        .collect();
    for stub in &stubs {
        zygote.kill(*stub).expect("kill");
    }
    for stub in stubs {
        assert_eq!(
            zygote.reap(stub),
            Ok(Ended::Killed(signal_number(Signal::KILL)))
        );
        assert_eq!(
            rustix::process::test_kill_process(stub),
            Err(Errno::SRCH),
            "a reaped stub is gone"
        );
    }
}

#[test]
fn its_stubs_die_with_it() {
    let zygote = zygote();
    let stubs: Vec<Pid> = (0..4)
        .map(|_| zygote.fork_stub().expect("fork").process())
        .collect();
    rustix::process::kill_process(zygote.process(), Signal::KILL).expect("kill the zygote");
    assert_eq!(zygote.end(), Ok(Ended::Killed(signal_number(Signal::KILL))));
    let deadline = Instant::now() + Duration::from_secs(5);
    for stub in stubs {
        while rustix::process::test_kill_process(stub).is_ok() {
            assert!(
                Instant::now() < deadline,
                "stub {} outlived its zygote",
                stub.as_raw_nonzero()
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

#[test]
fn a_zygote_started_from_a_busy_many_threaded_embedder_answers() {
    /// Stops the busy threads however the test's own loop ends.
    struct StopOnDrop<'a>(&'a std::sync::atomic::AtomicBool);
    impl Drop for StopOnDrop<'_> {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
    let stop = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                let mut kept = Vec::new();
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    kept.push(vec![0_u8; 4096]);
                    if kept.len() > 256 {
                        kept.clear();
                    }
                }
            });
        }
        let _stop = StopOnDrop(&stop);
        for _ in 0..16 {
            let zygote = zygote();
            zygote.ping().expect("ping");
            assert_eq!(
                zygote.end(),
                Ok(Ended::Exited(ZygoteEnd::EmbedderGone as u32))
            );
        }
    });
}

#[test]
fn the_probe_finds_a_user_space_top_above_our_layout() {
    let probe = HostProbe::new().expect("probe");
    eprintln!("user-space top {:#x}", probe.user_space_top());
    assert!(probe.user_space_top() > ZYGOTE + ZYGOTE_SIZE);
    #[cfg(target_arch = "aarch64")]
    {
        eprintln!("CTR_EL0 {:#x}", probe.cache_type());
        eprintln!("SVE at a system call {:?}", probe.sve_at_system_call());
        assert_ne!(
            probe.sve_at_system_call(),
            wcontainers_executor_host_process::probe::SveAtSystemCall::Kept,
            "the host keeps SVE state across a system call, against its ABI"
        );
        assert!(probe.code_maintenance_covers_every_mapping());
    }
}
