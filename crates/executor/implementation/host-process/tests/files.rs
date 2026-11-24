//! The embedder's reads and writes of the stubs' memories on a real host: they are the stubs' own bytes at the
//! guest's addresses, and many large memories add no host mapping to the embedder.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use std::os::unix::fs::FileExt;

use rustix::mm::ProtFlags;
use wcontainers_executor_host_process::files;
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

/// A seeded generator for the test's offsets and bytes (splitmix64).
struct Seeded(u64);

impl Seeded {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

fn seed() -> u64 {
    let seed = knob("WCHP_SEED", 1) as u64;
    eprintln!("WCHP_SEED={seed}");
    seed
}

/// A stub's memory at a guest address, read from outside through /proc (the test may; the executor never does).
fn read_guest(stub: &Stub, address: u64, length: usize) -> Vec<u8> {
    let memory =
        std::fs::File::open(format!("/proc/{}/mem", stub.process().as_raw_nonzero())).expect("mem");
    let mut bytes = vec![0; length];
    memory.read_exact_at(&mut bytes, address).expect("read");
    bytes
}

fn map(stub: &Stub, address: u64, length: u64) {
    let command = Command::Map {
        address: address as usize,
        length: length as usize,
        permissions: ProtFlags::READ | ProtFlags::WRITE,
        offset: address as usize,
    };
    assert_eq!(
        stub.turn(0, &[command]),
        Ok(TurnEnd::Done {
            commands_applied: 1
        })
    );
}

fn end(zygote: &Zygote, stub: &Stub) {
    if stub.end(0).is_err() {
        zygote.kill(stub.process()).expect("kill");
    }
    assert_eq!(zygote.reap(stub.process()), Ok(Ended::Exited(0)));
}

/// The host mappings of this process, as /proc counts them (the test may read /proc; the executor never does).
fn own_mappings() -> usize {
    std::fs::read_to_string("/proc/self/maps")
        .expect("maps")
        .lines()
        .count()
}

/// The host's huge page, across whose edges the writes reach.
const HUGE_PAGE: u64 = 2048 * wcontainers_abi::KIB;

/// Host mappings the tests running beside one in this process may add meanwhile (their zygotes' reservations).
const BESIDE: usize = 64;

#[test]
fn the_embedder_s_writes_are_the_stub_s_bytes() {
    let zygote = zygote();
    let stub = zygote.fork_stub().expect("fork");
    let start = 1_u64 << 40;
    let pages = 16;
    map(&stub, start, pages * HUGE_PAGE);
    let mut seeded = Seeded(seed());
    let mut written = Vec::new();
    for page in 0..pages {
        // Each write crosses into the next huge page where it can.
        let offset = start + page * HUGE_PAGE + HUGE_PAGE - 5;
        let length = if page + 1 < pages { 10 } else { 5 };
        let bytes: Vec<u8> = (0..length).map(|_| seeded.next() as u8).collect();
        files::write(stub.memory(), offset, &bytes).expect("write");
        written.push((offset, bytes));
    }
    for (offset, bytes) in &written {
        assert_eq!(&read_guest(&stub, *offset, bytes.len()), bytes);
        let mut back = vec![0; bytes.len()];
        files::read(stub.memory(), *offset, &mut back).expect("read");
        assert_eq!(&back, bytes);
    }
    // A write by another path to the memfd is read back.
    let (offset, _) = written[written.len() - 1];
    std::fs::File::from(stub.memory().try_clone_to_owned().expect("dup"))
        .write_all_at(b"other", offset)
        .expect("pwrite");
    let mut back = [0; 5];
    files::read(stub.memory(), offset, &mut back).expect("read");
    assert_eq!(&back, b"other");
    end(&zygote, &stub);
}

#[test]
fn different_memories_never_mix() {
    let zygote = zygote();
    let stubs: Vec<Stub> = (0..3).map(|_| zygote.fork_stub().expect("fork")).collect();
    let address = 1_u64 << 41;
    for (index, stub) in stubs.iter().enumerate() {
        map(stub, address, HUGE_PAGE);
        files::write(stub.memory(), address, &[index as u8 + 1; 8]).expect("write");
    }
    for (index, stub) in stubs.iter().enumerate() {
        assert_eq!(read_guest(stub, address, 8), vec![index as u8 + 1; 8]);
        let mut back = [0; 8];
        files::read(stub.memory(), address, &mut back).expect("read");
        assert_eq!(back, [index as u8 + 1; 8]);
    }
    for stub in &stubs {
        end(&zygote, stub);
    }
}

/// The plan's case: 64 guest memories of 2^43 bytes each, touched across all of it by one embedder that maps none of
/// them. Knobs: WCHP_MEMORIES, WCHP_TOUCHES (per memory), WCHP_SEED.
#[test]
fn many_large_memories_add_no_host_mapping() {
    let zygote = zygote();
    let memories = knob("WCHP_MEMORIES", 64);
    let touches = knob("WCHP_TOUCHES", 512);
    let start = 1_u64 << 40;
    let size = 1_u64 << 43;
    let stubs: Vec<Stub> = (0..memories)
        .map(|_| zygote.fork_stub().expect("fork"))
        .collect();
    for stub in &stubs {
        map(stub, start, size);
    }
    let mut seeded = Seeded(seed());
    let mut written = Vec::with_capacity(memories * touches);
    let before = own_mappings();
    let mut most = 0;
    for round in 0..touches {
        for stub in &stubs {
            let offset = start + (seeded.next() % (size / 8)) * 8;
            let word = seeded.next();
            files::write(stub.memory(), offset, &word.to_ne_bytes()).expect("write");
            written.push((offset, word));
        }
        if round % 64 == 0 {
            most = most.max(own_mappings());
        }
    }
    most = most.max(own_mappings());
    eprintln!(
        "{memories} memories, {touches} touches each: at most {most} host mappings, {before} before"
    );
    assert!(most <= before + BESIDE, "{most} host mappings");
    // Two touches of one word in a memory come with odds near 2^-23 at the default counts, so each word is its own.
    for (index, (offset, word)) in written.iter().enumerate() {
        let stub = &stubs[index % memories];
        let mut back = [0; 8];
        files::read(stub.memory(), *offset, &mut back).expect("read");
        assert_eq!(u64::from_ne_bytes(back), *word);
        if index % 61 == 0 {
            assert_eq!(read_guest(stub, *offset, 8), word.to_ne_bytes());
        }
    }
    for stub in &stubs {
        end(&zygote, stub);
    }
}

#[test]
fn a_range_past_the_memory_s_end_is_refused() {
    let zygote = zygote();
    let stub = zygote.fork_stub().expect("fork");
    let end_of_file = rustix::fs::fstat(stub.memory()).expect("fstat").st_size as u64;
    let mut back = [0; 8];
    assert_eq!(
        files::read(stub.memory(), end_of_file - 4, &mut back),
        Err(rustix::io::Errno::FAULT)
    );
    end(&zygote, &stub);
}
