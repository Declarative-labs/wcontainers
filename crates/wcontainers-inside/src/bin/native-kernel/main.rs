//! The kernel inside: a test program run by the monitor's kernel test, never shipped. It takes a container's
//! files and start through the link, starts its first process in the kernel on [`NativeCpu`] with the host's
//! services through the link, and says what the kernel runs next.

#![cfg_attr(target_os = "none", no_std, no_main)]

#[cfg(target_os = "none")]
extern crate alloc;

#[cfg(target_os = "none")]
mod protocol;

#[cfg(target_os = "none")]
mod kernel {
    use wcontainers_abi::{Cpu, Next, RegisterBlock};
    use wcontainers_inside::container;
    use wcontainers_inside::floor::{self, Doorbell, Floor};
    use wcontainers_inside::host::LinkHost;
    use wcontainers_inside::link::Start;
    use wcontainers_inside::native::NativeCpu;
    use wcontainers_kernel::Kernel;
    use zerocopy::IntoBytes;

    use crate::protocol::Started;

    #[unsafe(no_mangle)]
    extern "C" fn floor_main(start: &Start) -> ! {
        let floor = Floor::boot(start);
        let mut cpu = NativeCpu::new(floor.global, false).expect("frames for the CPU side");
        let mut host = LinkHost { ring: Doorbell };
        let mut kernel = Kernel::new();
        if let Err(refused) = container::start(&mut Doorbell, &mut kernel, &mut cpu, &mut host) {
            panic!("the kernel refused a file: {refused:?}");
        }
        let next = kernel.next();
        let mut block = RegisterBlock::ZERO;
        if let Next::Run { thread, .. } = next {
            cpu.registers(thread, &mut block);
        }
        loop {
            floor::ask(&Started::of(next), block.as_bytes());
        }
    }
}

/// Built for the host this program is empty: it only runs inside the virtual machine.
#[cfg(not(target_os = "none"))]
fn main() {}
