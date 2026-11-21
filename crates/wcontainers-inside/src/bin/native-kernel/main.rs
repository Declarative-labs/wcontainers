//! The kernel inside: a test program run by the monitor's kernel test, never shipped. It takes a container's
//! files and start through the link, starts its first process in the kernel on [`NativeCpu`], and runs it to
//! its end with the runtime's loop and the kernel test crate's policy, the host's services through the link,
//! then sends the root's files back.

#![cfg_attr(target_os = "none", no_std, no_main)]

#[cfg(target_os = "none")]
extern crate alloc;

#[cfg(target_os = "none")]
mod protocol;

#[cfg(target_os = "none")]
mod kernel {
    use alloc::collections::BTreeMap;

    use wcontainers_inside::container;
    use wcontainers_inside::floor::{self, Doorbell, Floor};
    use wcontainers_inside::host::LinkHost;
    use wcontainers_inside::link::Start;
    use wcontainers_inside::native::NativeCpu;
    use wcontainers_kernel_linux::Kernel;
    use wcontainers_kernel_test_policy::{TestPolicy, Warned as Kept};
    use wcontainers_runtime_single_kernel::{Ending, System as _, drive};

    use crate::protocol::{Ended, Policy, PolicySeeds, Warned};

    #[unsafe(no_mangle)]
    extern "C" fn floor_main(start: &Start) -> ! {
        let floor = Floor::boot(start);
        // The test crate's policy counts its turns, which needs the count in every piece.
        let mut cpu = NativeCpu::new(floor.global, true).expect("frames for the CPU side");
        let PolicySeeds {
            seed,
            wall_start,
            turn_events,
        } = floor::ask(&Policy {}, &[]);
        let mut policy = TestPolicy::new(LinkHost::new(Doorbell), seed, wall_start);
        policy.turn_events = (turn_events != 0).then_some(turn_events);
        let mut kernel = Kernel::new();
        if let Err(refused) = container::start(&mut Doorbell, &mut kernel, &mut cpu, &policy) {
            panic!("the kernel refused a file: {refused:?}");
        }
        let status = match drive(&mut kernel, &mut cpu, &mut policy) {
            Ok(Ending::Finished { status }) => status,
            Ok(Ending::Ended(never)) => match never {},
            Err(refused) => panic!("the native back end refused: {refused:?}"),
        };
        container::send_root(&mut Doorbell, &kernel, &mut cpu);
        cpu.send_warnings(&mut |warning| policy.cpu_warning(warning));
        // Each reason's last warning, which carries its count.
        let warnings = policy.warnings();
        let last: BTreeMap<&str, &Kept> = warnings
            .iter()
            .map(|warned| (warned.reason.as_str(), warned))
            .collect();
        for warned in last.into_values() {
            floor::ask(
                &Warned {
                    count: warned.count,
                    detail: warned.detail,
                },
                warned.reason.as_bytes(),
            );
        }
        let (stops, positions) = cpu.positions();
        loop {
            floor::ask(
                &Ended {
                    status: i64::from(status) as u64,
                    turns: policy.turns(),
                    stops,
                    positions,
                    events: cpu.last_count(),
                },
                &[],
            );
        }
    }
}

/// Built for the host this program is empty: it only runs inside the virtual machine.
#[cfg(not(target_os = "none"))]
fn main() {}
