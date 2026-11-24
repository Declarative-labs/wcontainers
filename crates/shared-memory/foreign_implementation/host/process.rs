//! Copies and word methods while another process, which maps the same memory file, writes the same bytes, or the bytes
//! beside them, or counts and swaps on the same words. The other process is this test binary run again, with only
//! `the_other_process` selected, the path of the file in `PATH`, and what it does in `TASK`.
#![cfg(all(target_os = "linux", not(miri)))]

use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};
use wcontainers_shared_memory::{HostMapping, SharedMemory};
use wcontainers_shared_memory_conformance::{
    CACHE_LINE, background, check_counted_and_swapped, count_and_swap, read_values_each_byte_held,
    use_words_checking_values_held, write_between_edges, write_mixed_sizes, write_own_edges,
};

/// The variable that names the memory file to the other process.
const PATH: &str = "SHARED_MEMORY_PATH";

/// The variable that tells the other process which of the tasks below to do.
const TASK: &str = "SHARED_MEMORY_TASK";

/// The bytes the writer of mixed sizes changes: two cache lines and a word.
const CHANGED: usize = 2 * CACHE_LINE + size_of::<u64>();

/// The bytes of each memory: room for two slots of copies of up to a page and more, and the two words below.
const LENGTH: usize = 4 * 4096;

/// The offset of the word the other process sets once it starts, which the parent waits for.
const STARTED: u64 = (LENGTH - 2 * size_of::<u64>()) as u64;

/// The offset of the word the parent sets to stop the other process.
const STOP: u64 = (LENGTH - size_of::<u64>()) as u64;

/// The rounds of the parent's copies and word methods while the other process runs.
const ROUNDS: usize = 200_000;

/// The rounds each process counts and swaps.
const COUNTS: usize = 200_000;

/// The bytes after the memory where the other process leaves the tokens it swapped out.
const TOKENS: usize = COUNTS * size_of::<u64>();

/// The longest the parent waits for the other process to start.
const PATIENCE: Duration = Duration::from_secs(30);

/// In the other process only: does the task named, until told to stop.
#[test]
fn the_other_process() {
    let (Ok(path), Ok(task)) = (std::env::var(PATH), std::env::var(TASK)) else {
        return;
    };
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .expect("the parent's memory file opens");
    let memory = HostMapping::file(file.as_fd(), 0, LENGTH).expect("the host maps the file");
    memory.store(STARTED, 1_u64).expect("inside");
    let stopped = || memory.load::<u64>(STOP) != Ok(0);
    match task.as_str() {
        "mixed" => write_mixed_sizes(&memory, CHANGED, stopped),
        "edges" => {
            let checks = write_own_edges(&memory, stopped);
            eprintln!("the other process checked its bytes beside the copies {checks} times");
        }
        "count" => {
            let out = count_and_swap(&memory, COUNTS as u64, COUNTS);
            let tokens: Vec<u8> = out.iter().flat_map(|token| token.to_ne_bytes()).collect();
            HostMapping::file(file.as_fd(), LENGTH as u64, TOKENS)
                .expect("the host maps the file's tokens")
                .write(0, &tokens)
                .expect("inside");
        }
        other => panic!("no task {other}"),
    }
}

/// Returns a sealed memory file of the memory's length and room for the tokens, and the parent's mapping of the memory.
fn shared() -> (OwnedFd, HostMapping) {
    let file = memfd_create("shared-memory-process", MemfdFlags::ALLOW_SEALING).expect("a memory file");
    ftruncate(&file, (LENGTH + TOKENS) as u64).expect("the file grows");
    fcntl_add_seals(&file, SealFlags::SHRINK).expect("the file takes the seal");
    let memory = HostMapping::file(file.as_fd(), 0, LENGTH).expect("the host maps the file");
    (file, memory)
}

/// Starts the other process on the task, and returns once it has started.
fn start(file: &OwnedFd, memory: &HostMapping, task: &str) -> Child {
    let child = Command::new(std::env::current_exe().expect("this test binary"))
        .args(["--exact", "the_other_process", "--nocapture", "--test-threads=1"])
        .env(PATH, format!("/proc/{}/fd/{}", std::process::id(), file.as_raw_fd()))
        .env(TASK, task)
        .spawn()
        .expect("the other process starts");
    let deadline = Instant::now() + PATIENCE;
    while memory.load::<u64>(STARTED) == Ok(0) {
        assert!(Instant::now() < deadline, "the other process never started");
        std::thread::yield_now();
    }
    child
}

/// Tells the other process to stop, and checks that it ended well.
fn stop(memory: &HostMapping, mut child: Child) {
    memory.store(STOP, 1_u64).expect("inside");
    let status = child.wait().expect("the other process ends");
    assert!(status.success(), "the other process failed: {status}");
}

#[test]
fn reads_only_values_bytes_held_while_another_process_writes_mixed_sizes() {
    let (file, memory) = shared();
    let before: Vec<u8> = (0..CHANGED).map(background).collect();
    memory.write(0, &before).expect("inside");
    let child = start(&file, &memory, "mixed");
    let mut changed = 0;
    let mut checked = 0;
    for _ in 0..ROUNDS / 1000 {
        checked += read_values_each_byte_held(&memory, CHANGED, 1000);
        let mut now = vec![0; CHANGED];
        memory.read(0, &mut now).expect("inside");
        changed += now.iter().zip(&before).filter(|(now, before)| now != before).count();
    }
    stop(&memory, child);
    eprintln!("{checked} bytes read while another process wrote; {changed} bytes seen changed");
    assert!(changed > 0, "the other process changed no byte the reader saw");
}

#[test]
fn uses_words_holding_only_values_held_while_another_process_writes_mixed_sizes() {
    let (file, memory) = shared();
    let before: Vec<u8> = (0..CHANGED).map(background).collect();
    memory.write(0, &before).expect("inside");
    let child = start(&file, &memory, "mixed");
    let rounds = use_words_checking_values_held(&memory, CHANGED, ROUNDS);
    stop(&memory, child);
    eprintln!("{rounds} rounds of word methods while another process wrote");
}

#[test]
fn copies_in_keeping_the_bytes_beside_them_that_another_process_writes() {
    let (file, memory) = shared();
    let child = start(&file, &memory, "edges");
    let copied = write_between_edges(&memory, ROUNDS / 4);
    stop(&memory, child);
    eprintln!("{copied} bytes copied in beside the bytes another process wrote");
}

#[test]
fn counts_and_swaps_without_losing_a_value_across_processes() {
    let (file, memory) = shared();
    let child = start(&file, &memory, "count");
    let mut swapped_out = count_and_swap(&memory, 0, COUNTS);
    stop(&memory, child);
    let mut theirs = vec![0; TOKENS];
    HostMapping::file(file.as_fd(), LENGTH as u64, TOKENS)
        .expect("the host maps the file's tokens")
        .read(0, &mut theirs)
        .expect("inside");
    swapped_out.extend(
        theirs
            .as_chunks::<{ size_of::<u64>() }>()
            .0
            .iter()
            .map(|&token| u64::from_ne_bytes(token)),
    );
    check_counted_and_swapped(&memory, swapped_out, 2 * COUNTS as u64);
}
