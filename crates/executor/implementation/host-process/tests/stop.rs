//! The stop signal on a real host: it ends a running turn as stopped, after the turn's commands and never inside
//! them; a stop that lands in the wait, in the publishing of an exit or in a thread's end changes nothing.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rustix::mm::ProtFlags;
use wcontainers_executor_host_process::layout::GUEST_START;
use wcontainers_executor_host_process::pages::{Command, Resume};
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

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

const PAGE: usize = 4096;

/// How long stops are sent back to back to a thread told to end.
const FLOOD: Duration = Duration::from_secs(1);

/// How soon after such a flood the thread has ended.
const AFTER_FLOOD: Duration = Duration::from_millis(100);

fn map_one_page() -> Command {
    Command::Map {
        address: GUEST_START,
        length: PAGE,
        permissions: ProtFlags::READ | ProtFlags::WRITE,
        offset: GUEST_START,
    }
}

fn protect_one_page(permissions: ProtFlags) -> Command {
    Command::Protect {
        address: GUEST_START,
        length: PAGE,
        permissions,
    }
}

/// Stops a spinning turn, sending the stop again each millisecond until the turn ends (an early one can land in the
/// wait, before the thread took the turn); the turn's end and how many stops were sent.
fn stop_turn(stub: &Stub, thread: usize, turn: u32) -> (TurnEnd, u32) {
    for sent in 1..=10_000 {
        stub.stop(thread).expect("stop");
        if let Some(end) = stub
            .wait_within(thread, turn, Duration::from_millis(1))
            .expect("wait")
        {
            return (end, sent);
        }
    }
    panic!("thread {thread} ignored 10,000 stops");
}

fn host_thread_count(stub: &Stub) -> usize {
    std::fs::read_dir(format!("/proc/{}/task", stub.process().as_raw_nonzero()))
        .expect("tasks")
        .count()
}

#[test]
fn a_running_turn_is_stopped_after_its_commands() {
    let zygote = zygote();
    let mut stub = zygote.fork_stub().expect("fork");
    let made = stub.new_thread().expect("an index");
    assert_eq!(
        stub.turn(0, &[made]),
        Ok(TurnEnd::Done {
            commands_applied: 1
        })
    );
    for thread in [0, 1] {
        for round in 0..knob("WCHP_STOPS", 1000) {
            let commands: &[Command] = if round % 2 == 0 {
                &[]
            } else {
                &[map_one_page(), protect_one_page(ProtFlags::READ)]
            };
            let turn = stub.begin(thread, commands, Resume::Spin).expect("begin");
            let (end, _) = stop_turn(&stub, thread, turn);
            assert_eq!(
                end,
                TurnEnd::Stopped {
                    commands_applied: commands.len() as u64
                }
            );
            assert_eq!(
                stub.turn(thread, &[]),
                Ok(TurnEnd::Done {
                    commands_applied: 0
                })
            );
        }
    }
    zygote.kill(stub.process()).expect("kill");
    assert!(matches!(zygote.reap(stub.process()), Ok(Ended::Killed(_))));
}

#[test]
fn stops_sent_to_a_waiting_thread_are_stale() {
    let zygote = zygote();
    let stub = zygote.fork_stub().expect("fork");
    for _ in 0..knob("WCHP_STOPS", 1000) {
        stub.stop(0).expect("stop");
    }
    // Each stop went back to the wait without publishing: the next turns are answered once each, in order.
    for _ in 0..100 {
        assert_eq!(
            stub.turn(0, &[]),
            Ok(TurnEnd::Done {
                commands_applied: 0
            })
        );
    }
    stub.end(0).expect("end");
    assert_eq!(zygote.reap(stub.process()).expect("reap"), Ended::Exited(0));
}

/// Stops sent without pause to one thread land in every window of its turns: none is lost or repeated, a command
/// that makes a thread is applied exactly once, and a thread told to end ends.
#[test]
fn stops_landing_anywhere_never_lose_repeat_or_split_a_turn() {
    let seed = knob("WCHP_SEED", 1);
    let rounds = knob("WCHP_ROUNDS", 20_000);
    eprintln!("WCHP_SEED={seed} WCHP_ROUNDS={rounds}");
    let mut random = fastrand::Rng::with_seed(seed);
    let zygote = zygote();
    let mut stub = zygote.fork_stub().expect("fork");
    let made = stub.new_thread().expect("an index");
    assert_eq!(
        stub.turn(0, &[made]),
        Ok(TurnEnd::Done {
            commands_applied: 1
        })
    );
    assert_eq!(
        stub.turn(1, &[map_one_page()]),
        Ok(TurnEnd::Done {
            commands_applied: 1
        })
    );
    let target = stub.stop_target(1);
    let running = AtomicBool::new(true);
    let sent = AtomicU64::new(0);
    let (mut done, mut stopped, mut sent_by_turns) = (0u64, 0u64, 0u64);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            while running.load(Ordering::Relaxed) {
                // A stop already pending takes the next one in: the host never refuses it for a full queue.
                target.stop().expect("stop");
                sent.fetch_add(1, Ordering::Relaxed);
                for _ in 0..random_pause(seed, sent.load(Ordering::Relaxed)) {
                    core::hint::spin_loop();
                }
            }
        });
        for _ in 0..rounds {
            let (commands, resume, expected) = match random.u8(0..4) {
                0 => (vec![], Resume::Ping, 0),
                1 => (vec![protect_one_page(ProtFlags::READ)], Resume::Ping, 1),
                2 => (
                    vec![
                        protect_one_page(ProtFlags::READ | ProtFlags::WRITE),
                        stub.new_thread().expect("an index"),
                    ],
                    Resume::Ping,
                    2,
                ),
                _ => (vec![], Resume::Spin, 0),
            };
            let made = commands.iter().find_map(|command| match command {
                Command::NewThread { thread, .. } => Some(*thread),
                _ => None,
            });
            let turn = stub.begin(1, &commands, resume).expect("begin");
            let end = if resume == Resume::Spin {
                let (end, sent) = stop_turn(&stub, 1, turn);
                sent_by_turns += u64::from(sent);
                end
            } else {
                stub.wait(1, turn).expect("wait")
            };
            match end {
                TurnEnd::Done { commands_applied } if commands_applied == expected => done += 1,
                TurnEnd::Stopped { commands_applied } if commands_applied == expected => {
                    stopped += 1
                }
                other => panic!("a turn of {} commands ended {other:?}", commands.len()),
            }
            if let Some(thread) = made {
                assert_eq!(
                    stub.turn(thread, &[]),
                    Ok(TurnEnd::Done {
                        commands_applied: 0
                    })
                );
                stub.end_thread(thread).expect("end");
                assert!(
                    stub.thread_gone(thread, Duration::from_secs(1))
                        .expect("wait"),
                    "thread {thread} did not end"
                );
            }
        }
        running.store(false, Ordering::Relaxed);
    });
    eprintln!("{rounds} turns: {done} done, {stopped} stopped");
    let landed = stub.stops_landed(1);
    let sent = sent.load(Ordering::Relaxed) + sent_by_turns;
    eprintln!("{sent} stops sent, landed {landed:?}");
    assert_eq!(landed.in_turn, stopped, "a stop in a turn ended it once");
    assert!(
        landed.in_publish > 0 && landed.in_wait > 0,
        "a window was never hit"
    );
    assert!(landed.in_turn + landed.in_publish + landed.in_wait + landed.in_end <= sent);
    // Threads that ended are listed by the host until they are reaped, a moment after their exit-clear word.
    let until = std::time::Instant::now() + Duration::from_secs(1);
    while host_thread_count(&stub) != 2 && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(host_thread_count(&stub), 2);
    // Thread 1 is told to end under stops sent back to back for a while: they may hold it in its handler while they
    // come faster than it runs, but none is left pending after them, so it ends soon after the last.
    let flood_sent = std::thread::scope(|scope| {
        let flood = scope.spawn(|| {
            let until = Instant::now() + FLOOD;
            let mut sent = 0u64;
            while Instant::now() < until && target.stop().is_ok() {
                sent += 1;
            }
            sent
        });
        stub.end_thread(1).expect("end");
        flood.join().expect("flood")
    });
    let gone = stub.thread_gone(1, AFTER_FLOOD).expect("wait");
    eprintln!(
        "ending under {flood_sent} stops, landed {:?}",
        stub.stops_landed(1)
    );
    assert!(
        gone,
        "thread 1 did not end within {AFTER_FLOOD:?} of the last stop"
    );
    stub.end(0).expect("end");
    assert_eq!(zygote.reap(stub.process()).expect("reap"), Ended::Exited(0));
}

/// A pause between stops, from nothing to a few microseconds, so they fall at every distance from a turn's start.
fn random_pause(seed: u64, sent: u64) -> u32 {
    fastrand::Rng::with_seed(seed ^ sent.wrapping_mul(0x9e37_79b9_7f4a_7c15)).u32(0..4000)
}
