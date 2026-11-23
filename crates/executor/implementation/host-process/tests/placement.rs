//! Placement on a real host: the runners of two containers take different CPUs, each guest thread follows its runner,
//! and a runner whose CPU another program also uses moves to a free one.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rustix::process::Pid;
use wcontainers_abi::Cpu;
use wcontainers_executor_host_process::executor::{HostProcessExecutor, Touch, Touched};
use wcontainers_executor_host_process::host::HostProcessHost;

/// One host for the whole test process, as an embedder has.
static HOST: std::sync::OnceLock<Arc<HostProcessHost>> = std::sync::OnceLock::new();

fn host() -> Arc<HostProcessHost> {
    HOST.get_or_init(|| Arc::new(HostProcessHost::new().expect("the host is refused")))
        .clone()
}

/// The only CPU a thread may run on, if it has one.
fn pinned_to(thread: Pid) -> Option<usize> {
    let set = rustix::thread::sched_getaffinity(Some(thread)).expect("affinity");
    (set.count() == 1)
        .then(|| (0..rustix::thread::CpuSet::MAX_CPU).find(|cpu| set.is_set(*cpu)))
        .flatten()
}

fn pin_here(cpu: usize) {
    let mut set = rustix::thread::CpuSet::new();
    set.set(cpu);
    rustix::thread::sched_setaffinity(None, &set).expect("pin");
}

/// A container's runner on a thread of its own: its CPU, and the one its guest thread is pinned to after a turn.
fn one_runner(host: Arc<HostProcessHost>, hold: Arc<std::sync::Barrier>) -> (usize, Option<usize>) {
    let mut cpu = HostProcessExecutor::new(host);
    let memory = cpu.memory_new().expect("memory");
    let thread = cpu.thread_new(memory).expect("thread");
    assert_eq!(
        cpu.touch(thread, Touch::Spin, Some(Duration::from_millis(1))),
        Ok(Touched::Stopped)
    );
    let runner = cpu.runner_cpu().expect("placed");
    let guest = pinned_to(cpu.stub_process(memory));
    // Both runners hold their CPUs until both have looked.
    hold.wait();
    cpu.thread_drop(thread);
    cpu.memory_drop(memory);
    (runner, guest)
}

#[test]
fn the_runners_of_two_containers_take_different_cpus_and_their_threads_follow() {
    let host = host();
    let hold = Arc::new(std::sync::Barrier::new(2));
    let runners: Vec<_> = (0..2)
        .map(|_| {
            let (host, hold) = (host.clone(), hold.clone());
            std::thread::spawn(move || one_runner(host, hold))
        })
        .collect();
    let places: Vec<_> = runners
        .into_iter()
        .map(|runner| runner.join().expect("runner"))
        .collect();
    for (runner, guest) in &places {
        assert_eq!(*guest, Some(*runner), "the guest thread follows its runner");
    }
    assert_ne!(places[0].0, places[1].0, "{places:?}");
}

#[test]
fn a_runner_whose_cpu_another_program_uses_moves_and_its_thread_follows() {
    let mut cpu = HostProcessExecutor::new(host());
    let memory = cpu.memory_new().expect("memory");
    let thread = cpu.thread_new(memory).expect("thread");
    let slice = Some(Duration::from_millis(1));
    assert_eq!(cpu.touch(thread, Touch::Spin, slice), Ok(Touched::Stopped));
    let first = cpu.runner_cpu().expect("placed");
    let spinning = Arc::new(AtomicBool::new(true));
    let other = {
        let spinning = spinning.clone();
        std::thread::spawn(move || {
            pin_here(first);
            while spinning.load(Ordering::Relaxed) {
                std::hint::spin_loop();
            }
        })
    };
    let started = Instant::now();
    let mut turns = 0;
    while cpu.counters().placement_moves == 0 && started.elapsed() < Duration::from_secs(5) {
        assert_eq!(cpu.touch(thread, Touch::Spin, slice), Ok(Touched::Stopped));
        turns += 1;
    }
    let took = started.elapsed();
    spinning.store(false, Ordering::Relaxed);
    other.join().expect("the other program");
    assert!(
        cpu.counters().placement_moves >= 1,
        "no move in {turns} turns"
    );
    let now = cpu.runner_cpu().expect("placed");
    assert_ne!(now, first);
    assert_eq!(cpu.touch(thread, Touch::Spin, slice), Ok(Touched::Stopped));
    assert_eq!(
        pinned_to(cpu.stub_process(memory)),
        Some(now),
        "the guest thread follows"
    );
    eprintln!(
        "moved from CPU {first} to {now} after {turns} turns, {took:?}; {:?}",
        cpu.counters()
    );
    cpu.thread_drop(thread);
    cpu.memory_drop(memory);
}
