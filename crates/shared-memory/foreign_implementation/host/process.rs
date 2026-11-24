//! Copies and word methods while another process, which maps the same memory file, writes the same bytes, or the bytes
//! beside them, or counts and swaps on the same words. The other process is this test binary run again, with only
//! `the_other_process` selected, the path of the file in `PATH`, what it does in `TASK` and its parent in `PARENT`.
#![cfg(all(target_os = "linux", not(miri)))]

use std::io::{BufRead, BufReader};
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};
use rustix::process::{Pid, Signal, getppid, kill_process, set_parent_process_death_signal};
use wcontainers_shared_memory::{HostMapping, SharedMemory};
use wcontainers_shared_memory_conformance::{
    CACHE_LINE, background, check_counted_and_swapped, count_and_swap, read_values_each_byte_held,
    use_words_checking_values_held, write_between_edges, write_mixed_sizes, write_own_edges,
};

/// The variable that names the memory file to the other process.
const PATH: &str = "SHARED_MEMORY_PATH";

/// The variable that tells the other process which of the tasks below to do.
const TASK: &str = "SHARED_MEMORY_TASK";

/// The variable that names the process that started the other process.
const PARENT: &str = "SHARED_MEMORY_PARENT";

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

/// In the other process only: does the task named, until told to stop, or until the test thread that started it ends.
#[test]
fn the_other_process() {
    let Ok(task) = std::env::var(TASK) else {
        return;
    };
    // The kernel sends the signal when the thread that started this process ends, so a test that fails or crashes
    // before `stop` leaves no writer behind holding its output open.
    set_parent_process_death_signal(Some(Signal::KILL)).expect("the parent-death signal is set");
    let parent = std::env::var(PARENT).expect("the parent is named");
    assert_eq!(
        Pid::as_raw(getppid()).to_string(),
        parent,
        "the parent ended before the parent-death signal was set"
    );
    if task == "parent" {
        let (file, memory) = shared();
        #[expect(
            clippy::zombie_processes,
            reason = "this process is killed while it parks, so that the test sees whether the writer outlives it"
        )]
        let writer = start(&file, &memory, "mixed");
        println!("writer {}", writer.id());
        loop {
            std::thread::park();
        }
    }
    let path = std::env::var(PATH).expect("the memory file is named");
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
        .env(PARENT, std::process::id().to_string())
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

/// Returns whether the process has ended: it is gone, or only its exit status is left.
fn has_ended(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .is_none_or(|stat| {
            stat.rsplit_once(") ")
                .is_some_and(|(_, rest)| rest.starts_with(['Z', 'X']))
        })
}

#[test]
fn the_other_process_ends_when_the_process_that_started_it_is_killed() {
    let mut parent = Command::new(std::env::current_exe().expect("this test binary"))
        .args(["--exact", "the_other_process", "--nocapture", "--test-threads=1"])
        .env(TASK, "parent")
        .env(PARENT, std::process::id().to_string())
        .stdout(Stdio::piped())
        .spawn()
        .expect("the parent process starts");
    let output = parent.stdout.take().expect("the parent's output is piped");
    let (named, writer) = std::sync::mpsc::channel();
    // The test harness prints the test's name on the line where the parent's own output starts.
    std::thread::spawn(move || {
        let writer = BufReader::new(output)
            .lines()
            .map_while(Result::ok)
            .find_map(|line| line.split_once("writer ")?.1.trim().parse::<u32>().ok());
        named.send(writer).ok();
    });
    let writer = writer.recv_timeout(PATIENCE).ok().flatten();
    parent.kill().expect("the parent is killed");
    let writer = writer.expect("the parent names the writer it started");
    parent.wait().expect("the parent ends");
    let deadline = Instant::now() + PATIENCE;
    while !has_ended(writer) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let ended = has_ended(writer);
    if !ended && let Some(pid) = i32::try_from(writer).ok().and_then(Pid::from_raw) {
        kill_process(pid, Signal::KILL).ok();
    }
    assert!(ended, "the writer {writer} outlived the process that started it");
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
