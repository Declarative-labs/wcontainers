//! The handlers this process had before a run's: a signal of a run that comes outside one is theirs, and a run's
//! handler stays for every later run. A process of its own, whose handlers are set before the first preparation.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use wcontainers_linux_process::{LinuxProcess, RUN_STOP_SIGNAL, RunEnd, RunFrame, Signal};
use wcontainers_linux_process_direct::DirectLinuxProcess;

/// Where the instructions are mapped: far above any heap or library of a test process.
const INSTRUCTIONS_AT: usize = 1 << 41;
/// `brk #0` ([Arm ARM, C6.2](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/BRK--Breakpoint-instruction-)).
const BREAKPOINT: u32 = 0xd420_0000;

fn sent_to_this_thread(signal: Signal) {
    DirectLinuxProcess
        .tgkill(rustix::process::getpid(), rustix::thread::gettid(), signal)
        .expect("this thread is signalled");
}

#[test]
fn a_signal_outside_a_run_goes_to_the_handler_before_and_a_run_s_handler_stays() {
    let linux = DirectLinuxProcess;
    let trapped = Arc::new(AtomicBool::new(false));
    let stopped = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(Signal::TRAP.as_raw(), Arc::clone(&trapped))
        .expect("a handler for the trap");
    signal_hook::flag::register(RUN_STOP_SIGNAL.as_raw(), Arc::clone(&stopped))
        .expect("a handler for the stop signal");
    let page = rustix::param::page_size();
    let mut instructions = vec![0_u8; page];
    instructions[..size_of::<u32>()].copy_from_slice(&BREAKPOINT.to_le_bytes());
    let mapping = linux
        .map_instructions(INSTRUCTIONS_AT, &instructions, 0..0, true)
        .expect("the instructions are mapped");
    std::thread::scope(|scope| {
        let (prepared_sender, prepared) = std::sync::mpsc::channel();
        let (signalled_sender, signalled) = std::sync::mpsc::channel::<()>();
        let mapping = &mapping;
        let runner = scope.spawn(move || {
            let mut thread = linux.prepare_run_thread().expect("the thread is prepared");
            prepared_sender
                .send(rustix::thread::gettid())
                .expect("the test waits");
            signalled.recv().expect("the test signals");
            let mut frame = RunFrame::default();
            frame.registers.pc = INSTRUCTIONS_AT as u64;
            // The stop that reached this prepared thread between runs ends the next before it starts.
            let first = linux.run_instructions(&mut thread, mapping, &mut frame);
            let second = linux.run_instructions(&mut thread, mapping, &mut frame);
            (first, second)
        });
        let runner_thread = prepared.recv().expect("the runner is prepared");
        // This thread was never prepared: both signals are the handlers' before, each time.
        for _ in 0..2 {
            trapped.store(false, Ordering::Release);
            stopped.store(false, Ordering::Release);
            sent_to_this_thread(Signal::TRAP);
            sent_to_this_thread(RUN_STOP_SIGNAL);
            assert!(
                trapped.load(Ordering::Acquire),
                "the trap reached the earlier handler"
            );
            assert!(
                stopped.load(Ordering::Acquire),
                "the stop reached the earlier handler"
            );
        }
        stopped.store(false, Ordering::Release);
        trapped.store(false, Ordering::Release);
        linux
            .tgkill(rustix::process::getpid(), runner_thread, RUN_STOP_SIGNAL)
            .expect("the runner is signalled");
        signalled_sender.send(()).expect("the runner waits");
        let (first, second) = runner.join().expect("the runner ends");
        // A run's own breakpoint ends it, whatever the handler before would do: the run's handler stayed.
        assert_eq!(
            (first, second),
            (RunEnd::NotEntered, RunEnd::Raised(Signal::TRAP))
        );
        assert!(
            !stopped.load(Ordering::Acquire),
            "a stop to a prepared thread is the run's"
        );
        assert!(
            !trapped.load(Ordering::Acquire),
            "a run's breakpoint is the run's"
        );
    });
}
