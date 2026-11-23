//! A stub on a real host: its turns, its mapping commands at the guest's own addresses, and its filter, which kills
//! it for any host call outside its own.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use std::os::unix::fs::FileExt;
use std::time::Instant;

use rustix::io::Errno;
use rustix::mm::ProtFlags;
use rustix::process::{Pid, Signal};
use rustix::thread::CpuSet;
use wcontainers_executor_host_process::layout::{
    GUEST_START, OUR_AREA, REGISTER_PAGES, SIGNAL_STACKS, THREAD_PAGES,
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

/// A test's knob, from the environment.
fn knob(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

const READ_WRITE: ProtFlags = ProtFlags::READ.union(ProtFlags::WRITE);
const WINDOW: usize = 64 * 1024;

fn map(address: usize, length: usize) -> Command {
    Command::Map {
        address,
        length,
        permissions: READ_WRITE,
        offset: address,
    }
}

/// A stub's memory at a guest address, read from outside through /proc (the test may; the executor never does).
fn read_guest(stub: &Stub, address: usize, length: usize) -> Result<Vec<u8>, std::io::Error> {
    let memory = std::fs::File::open(format!("/proc/{}/mem", stub.process().as_raw_nonzero()))?;
    let mut bytes = vec![0; length];
    memory.read_exact_at(&mut bytes, address as u64)?;
    Ok(bytes)
}

/// The permissions /proc shows for the mapping holding `address`.
fn permissions_at(stub: &Stub, address: usize) -> Option<String> {
    let maps = std::fs::read_to_string(format!("/proc/{}/maps", stub.process().as_raw_nonzero()))
        .expect("maps");
    maps.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let (start, end) = fields.next()?.split_once('-')?;
        let start = usize::from_str_radix(start, 16).ok()?;
        let end = usize::from_str_radix(end, 16).ok()?;
        (start <= address && address < end).then(|| fields.next().map(str::to_owned))?
    })
}

/// A turn that must end `Done`; a stub that ended instead is reaped, and how it ended is reported.
fn done(zygote: &Zygote, stub: &Stub, commands: &[Command]) -> u64 {
    match stub.turn(0, commands) {
        Ok(TurnEnd::Done { commands_applied }) => commands_applied,
        Ok(TurnEnd::StubEnded) => panic!("the stub ended: {:?}", zygote.reap(stub.process())),
        other => panic!("the turn ended {other:?}"),
    }
}

fn end_and_reap(zygote: &Zygote, stub: &Stub) -> Ended {
    if stub.end(0).is_err() {
        zygote.kill(stub.process()).expect("kill");
    }
    zygote.reap(stub.process()).expect("reap")
}

#[test]
fn a_stub_answers_every_turn_and_reports_its_thread() {
    let zygote = zygote();
    let stub = zygote.fork_stub().expect("fork");
    for _ in 0..knob("WCHP_TURNS", 10_000) {
        assert_eq!(done(&zygote, &stub, &[]), 0);
    }
    // Thread 0 is the process's first thread, whose id is the process id.
    assert_eq!(
        Some(stub.reported_thread_id(0)),
        u32::try_from(stub.process().as_raw_nonzero().get()).ok()
    );
    assert_eq!(end_and_reap(&zygote, &stub), Ended::Exited(0));
}

#[test]
fn mapping_commands_place_the_memory_at_the_guest_s_own_addresses() {
    let zygote = zygote();
    let stub = zygote.fork_stub().expect("fork");
    let address = 1 << 40;
    let memory = std::fs::File::from(stub.memory().try_clone_to_owned().expect("dup"));
    memory
        .write_all_at(b"written by the embedder", address as u64)
        .expect("write");
    let lowest = GUEST_START;
    let highest = OUR_AREA - WINDOW;
    assert_eq!(
        done(
            &zygote,
            &stub,
            &[
                map(address, WINDOW),
                map(lowest, WINDOW),
                map(highest, WINDOW)
            ]
        ),
        3
    );
    assert_eq!(
        read_guest(&stub, address, 23).expect("read").as_slice(),
        b"written by the embedder"
    );
    assert_eq!(permissions_at(&stub, address).as_deref(), Some("rw-s"));
    assert_eq!(permissions_at(&stub, lowest).as_deref(), Some("rw-s"));
    assert_eq!(permissions_at(&stub, highest).as_deref(), Some("rw-s"));
    let protect = Command::Protect {
        address,
        length: WINDOW,
        permissions: ProtFlags::READ,
    };
    assert_eq!(
        stub.turn(0, &[protect]),
        Ok(TurnEnd::Done {
            commands_applied: 1
        })
    );
    assert_eq!(permissions_at(&stub, address).as_deref(), Some("r--s"));
    let unmap = Command::Unmap {
        address,
        length: WINDOW,
    };
    assert_eq!(
        stub.turn(0, &[unmap]),
        Ok(TurnEnd::Done {
            commands_applied: 1
        })
    );
    assert_eq!(permissions_at(&stub, address), None);
    assert!(read_guest(&stub, address, 1).is_err());
    assert_eq!(end_and_reap(&zygote, &stub), Ended::Exited(0));
}

#[test]
fn a_command_the_host_refuses_is_reported_after_those_before_it() {
    let zygote = zygote();
    let stub = zygote.fork_stub().expect("fork");
    let address = 1 << 40;
    let misaligned = address + WINDOW + 1;
    assert_eq!(
        stub.turn(0, &[map(address, WINDOW), map(misaligned, WINDOW)]),
        Ok(TurnEnd::CommandRefused {
            index: 1,
            errno: Errno::INVAL
        })
    );
    assert_eq!(permissions_at(&stub, address).as_deref(), Some("rw-s"));
    assert_eq!(
        stub.turn(0, &[]),
        Ok(TurnEnd::Done {
            commands_applied: 0
        })
    );
    assert_eq!(end_and_reap(&zygote, &stub), Ended::Exited(0));
}

#[test]
fn a_stub_making_a_host_call_outside_its_filter_is_killed() {
    let zygote = zygote();
    let executable = ProtFlags::READ | ProtFlags::EXEC;
    let forbidden = [
        (
            "its control pages made writable",
            Command::Protect {
                address: THREAD_PAGES,
                length: 4096,
                permissions: READ_WRITE,
            },
        ),
        (
            "its register pages unmapped",
            Command::Unmap {
                address: REGISTER_PAGES,
                length: 4096,
            },
        ),
        ("its signal stacks replaced", map(SIGNAL_STACKS, WINDOW)),
        (
            "guest memory made executable",
            Command::Protect {
                address: 1 << 40,
                length: WINDOW,
                permissions: executable,
            },
        ),
        (
            "guest memory mapped executable",
            Command::Map {
                address: 1 << 40,
                length: WINDOW,
                permissions: executable,
                offset: 1 << 40,
            },
        ),
        (
            "memory at another offset than its address",
            Command::Map {
                address: 1 << 40,
                length: WINDOW,
                permissions: READ_WRITE,
                offset: (1 << 40) + WINDOW,
            },
        ),
        (
            "memory at an offset equal only in its low word",
            Command::Map {
                address: 1 << 40,
                length: WINDOW,
                permissions: READ_WRITE,
                offset: 1 << 41,
            },
        ),
        (
            "a mapping across the guest area's end",
            map(OUR_AREA - WINDOW, 2 * WINDOW),
        ),
        ("a mapping below the guest's lowest address", map(0, WINDOW)),
        (
            "a length that wraps",
            map(1 << 40, usize::MAX - (1 << 40) + WINDOW + 1),
        ),
    ];
    for (what, command) in forbidden {
        let stub = zygote.fork_stub().expect("fork");
        assert_eq!(
            done(&zygote, &stub, &[]),
            0,
            "{what}: the stub did not start"
        );
        assert_eq!(
            stub.turn(0, &[command]),
            Ok(TurnEnd::StubEnded),
            "{what}: the stub was not stopped"
        );
        assert_eq!(
            zygote.reap(stub.process()),
            Ok(Ended::Killed(Signal::SYS.as_raw().unsigned_abs())),
            "{what}"
        );
    }
}

fn map_shared(address: usize, permissions: ProtFlags) -> Command {
    Command::MapShared {
        address,
        length: WINDOW,
        permissions,
        offset: 0,
    }
}

/// A block's memfd is received without waiting, mapped as guest memory only, and closed.
#[test]
fn a_shared_block_is_mapped_only_once_sent_and_only_as_guest_memory() {
    let zygote = zygote();
    let block =
        rustix::fs::memfd_create("a test's block", rustix::fs::MemfdFlags::CLOEXEC).expect("memfd");
    rustix::fs::ftruncate(&block, WINDOW as u64).expect("size");
    rustix::io::pwrite(&block, &5u64.to_ne_bytes(), 8).expect("write the block");
    let stub = zygote.fork_stub().expect("fork");
    assert_eq!(
        stub.turn(0, &[map_shared(1 << 40, READ_WRITE)]),
        Ok(TurnEnd::CommandRefused {
            index: 0,
            errno: Errno::AGAIN
        }),
        "a block not sent is refused, not waited for"
    );
    stub.send_block(rustix::fd::AsFd::as_fd(&block))
        .expect("send");
    assert_eq!(done(&zygote, &stub, &[map_shared(1 << 40, READ_WRITE)]), 1);
    assert_eq!(
        stub.load(0, (1 << 40) + 8),
        Ok(TurnEnd::Loaded { value: 5 })
    );
    assert_eq!(end_and_reap(&zygote, &stub), Ended::Exited(0));
    let executable = ProtFlags::READ | ProtFlags::EXEC;
    for (what, command) in [
        ("a block mapped executable", map_shared(1 << 40, executable)),
        (
            "a block over the signal stacks",
            map_shared(SIGNAL_STACKS, READ_WRITE),
        ),
        (
            "a block below the guest's lowest address",
            map_shared(0, READ_WRITE),
        ),
    ] {
        let stub = zygote.fork_stub().expect("fork");
        assert_eq!(
            done(&zygote, &stub, &[]),
            0,
            "{what}: the stub did not start"
        );
        stub.send_block(rustix::fd::AsFd::as_fd(&block))
            .expect("send");
        assert_eq!(stub.turn(0, &[command]), Ok(TurnEnd::StubEnded), "{what}");
        assert_eq!(
            zygote.reap(stub.process()),
            Ok(Ended::Killed(Signal::SYS.as_raw().unsigned_abs())),
            "{what}"
        );
    }
}

#[test]
fn a_stub_killed_from_outside_is_seen_ended_within_its_wait() {
    let zygote = zygote();
    let stub = zygote.fork_stub().expect("fork");
    rustix::process::kill_process(stub.process(), Signal::STOP).expect("stop");
    let killer = {
        let process = stub.process();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            rustix::process::kill_process(process, Signal::KILL).expect("kill");
            Instant::now()
        })
    };
    assert_eq!(stub.turn(0, &[]), Ok(TurnEnd::StubEnded));
    let seen = Instant::now();
    let killed = killer.join().expect("killer");
    assert!(
        seen.duration_since(killed) <= std::time::Duration::from_millis(150),
        "its end was seen {:?} after its death",
        seen.duration_since(killed)
    );
    assert_eq!(
        zygote.reap(stub.process()),
        Ok(Ended::Killed(Signal::KILL.as_raw().unsigned_abs()))
    );
}

fn pin(process: Option<Pid>, cpu: usize) {
    let mut set = CpuSet::new();
    set.set(cpu);
    rustix::thread::sched_setaffinity(process, &set).expect("pin");
}

/// The turn's round trip on one CPU, the stub's process and the test's thread pinned together: the median over
/// batches of the mean per turn. Knobs: WCHP_CPU, WCHP_PER_BATCH, WCHP_BATCHES. Run with `--ignored --nocapture`.
#[test]
#[ignore = "a measurement, run on its own"]
fn turn_round_trip_on_one_cpu() {
    let zygote = zygote();
    let stub = zygote.fork_stub().expect("fork");
    let cpu = knob("WCHP_CPU", 1);
    let per_batch = knob("WCHP_PER_BATCH", 10_000);
    let batches = knob("WCHP_BATCHES", 50);
    pin(None, cpu);
    pin(Some(stub.process()), cpu);
    for _ in 0..per_batch {
        stub.turn(0, &[]).expect("warm");
    }
    let mut means: Vec<f64> = (0..batches)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..per_batch {
                stub.turn(0, &[]).expect("turn");
            }
            start.elapsed().as_secs_f64() * 1e9 / per_batch as f64
        })
        .collect();
    means.sort_by(f64::total_cmp);
    println!(
        "turn round trip on cpu {cpu}: median {:.0} ns, lowest {:.0} ns, highest {:.0} ns ({batches} batches of {per_batch})",
        means[means.len() / 2],
        means[0],
        means[means.len() - 1]
    );
    assert_eq!(end_and_reap(&zygote, &stub), Ended::Exited(0));
}
