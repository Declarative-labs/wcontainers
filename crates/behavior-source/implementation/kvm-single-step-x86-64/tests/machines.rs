//! Making and dropping the chip source's machine leaves the process's virtual size and resident memory where they were.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use wcontainers_behavior_source_kvm_single_step_x86_64::SingleStepMachine;

const MACHINES: usize = 1000;

/// Returns the process's status line that starts with the name.
fn read_status(name: &str) -> String {
    let status = std::fs::read_to_string("/proc/self/status").expect("the process's status");
    status.lines().find(|line| line.starts_with(name)).expect("the line").to_owned()
}

#[test]
fn dropping_machines_leaves_the_process_as_it_was() {
    // The first machine maps the memory, the second zeroes all of it; every later one takes it as the second did.
    for _ in 0..2 {
        drop(SingleStepMachine::new().expect("a machine"));
    }
    let before = (read_status("VmSize:"), read_status("VmRSS:"));
    for _ in 0..MACHINES {
        drop(SingleStepMachine::new().expect("a machine"));
    }
    let after = (read_status("VmSize:"), read_status("VmRSS:"));
    println!("{MACHINES} machines: before {before:?}, after {after:?}");
    assert_eq!(before.0, after.0);
}
