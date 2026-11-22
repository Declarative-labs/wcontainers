//! Lets the chip test and the record tool link with the hardware executor on an SDK that lacks its later calls.

use std::env;

use wcontainers_executor_hardware_virtualization_inside::build_environment::{
    LATER_HYPERVISOR_CALLS, LEAVE_UNDEFINED, LINK_ARG, MACOS, RERUN_IF_ENV_CHANGED, TARGET_OS,
};

fn main() {
    println!("{RERUN_IF_ENV_CHANGED}{TARGET_OS}");
    if env::var(TARGET_OS).is_ok_and(|os| os == MACOS) {
        for call in LATER_HYPERVISOR_CALLS {
            println!("{LINK_ARG}{LEAVE_UNDEFINED}{call}");
        }
    }
}
