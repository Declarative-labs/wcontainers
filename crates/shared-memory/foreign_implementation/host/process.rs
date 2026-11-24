//! Reads while another process, which maps the same memory file, writes the same bytes with single bytes, 4-byte words
//! and 8-byte words. The other process is this test binary run again, with only `the_other_process_writes` selected and
//! the path of the file in `WRITER`.
#![cfg(all(target_os = "linux", not(miri)))]

use std::os::fd::{AsFd, AsRawFd};
use std::process::Command;
use std::time::{Duration, Instant};

use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};
use wcontainers_shared_memory::{HostMapping, SharedMemory};
use wcontainers_shared_memory_conformance::{
    CACHE_LINE, background, read_values_each_byte_held, write_mixed_sizes,
};

/// The variable that names the memory file to the other process.
const WRITER: &str = "SHARED_MEMORY_WRITER";

/// The bytes the writer changes: two cache lines and a word.
const CHANGED: usize = 2 * CACHE_LINE + size_of::<u64>();

/// The offset of the word the writer sets once it writes, and the reader sets to stop it.
const STARTED: u64 = 2 * CHANGED as u64;
const STOP: u64 = STARTED + size_of::<u64>() as u64;

/// The rounds of reads while the other process writes.
const ROUNDS: usize = 200_000;

/// The longest the reader waits for the writer to start or to end.
const PATIENCE: Duration = Duration::from_secs(30);

/// Returns the bytes of the memory: the bytes changed, then room for the two words.
fn length() -> usize {
    3 * CHANGED
}

/// In the other process only: writes until told to stop.
#[test]
fn the_other_process_writes() {
    let Ok(path) = std::env::var(WRITER) else {
        return;
    };
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .expect("the reader's memory file opens");
    let memory = HostMapping::file(file.as_fd(), 0, length()).expect("the host maps the file");
    memory.store(STARTED, 1_u64).expect("inside");
    write_mixed_sizes(&memory, CHANGED, || memory.load::<u64>(STOP) != Ok(0));
}

#[test]
fn reads_only_values_bytes_held_while_another_process_writes_mixed_sizes() {
    let file =
        memfd_create("shared-memory-process", MemfdFlags::ALLOW_SEALING).expect("a memory file");
    ftruncate(&file, length() as u64).expect("the file grows");
    fcntl_add_seals(&file, SealFlags::SHRINK).expect("the file takes the seal");
    let memory = HostMapping::file(file.as_fd(), 0, length()).expect("the host maps the file");
    let before: Vec<u8> = (0..CHANGED).map(background).collect();
    memory.write(0, &before).expect("inside");
    let mut writer = Command::new(std::env::current_exe().expect("this test binary"))
        .args([
            "--exact",
            "the_other_process_writes",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(
            WRITER,
            format!("/proc/{}/fd/{}", std::process::id(), file.as_raw_fd()),
        )
        .spawn()
        .expect("the other process starts");
    let deadline = Instant::now() + PATIENCE;
    while memory.load::<u64>(STARTED) == Ok(0) {
        assert!(
            Instant::now() < deadline,
            "the other process never started writing"
        );
        std::thread::yield_now();
    }
    let mut changed = 0;
    let mut checked = 0;
    for _ in 0..ROUNDS / 1000 {
        checked += read_values_each_byte_held(&memory, CHANGED, 1000);
        let mut now = vec![0; CHANGED];
        memory.read(0, &mut now).expect("inside");
        changed += now
            .iter()
            .zip(&before)
            .filter(|(now, before)| now != before)
            .count();
    }
    memory.store(STOP, 1_u64).expect("inside");
    let status = writer.wait().expect("the other process ends");
    assert!(status.success(), "the other process failed: {status}");
    eprintln!("{checked} bytes read while another process wrote; {changed} bytes seen changed");
    assert!(
        changed > 0,
        "the other process changed no byte the reader saw"
    );
}
