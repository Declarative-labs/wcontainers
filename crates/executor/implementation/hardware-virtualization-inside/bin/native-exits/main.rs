//! The native CPU side's exits: a test program run by the monitor's exits test, never shipped. It holds one
//! process and one thread on [`NativeCpu`] and does what the host orders: writes and protects memory, sets
//! registers, runs the thread to its next exit and gives back the exit and the registers.

#![cfg_attr(target_os = "none", no_std, no_main)]

#[cfg(target_os = "none")]
extern crate alloc;

#[cfg(target_os = "none")]
mod protocol;

#[cfg(target_os = "none")]
mod exits {
    use alloc::vec::Vec;

    use wcontainers_abi::{Cpu, Permissions, RegisterBlock};
    use wcontainers_executor_hardware_virtualization_inside::floor::{self, Floor, counter};
    use wcontainers_executor_hardware_virtualization_inside::link::{LINK_BYTES, Start};
    use wcontainers_executor_hardware_virtualization_inside::native::NativeCpu;
    use zerocopy::{FromBytes, IntoBytes};

    use crate::protocol::{ExitWords, Next, Order, OrderKind, ResumeWords, Warned};

    #[unsafe(no_mangle)]
    extern "C" fn floor_main(start: &Start) -> ! {
        let floor = Floor::boot(start);
        let mut cpu: Option<NativeCpu> = None;
        let mut process = None;
        let mut result: Vec<u8> = Vec::new();
        loop {
            let (order, bytes): (Order, Vec<u8>) =
                floor::ask_link(&Next {}, &result, |link| (link.get(), link.data().to_vec()));
            result.clear();
            let kind = OrderKind::from_word(order.kind).expect("an order of the protocol");
            if kind == OrderKind::Finish {
                break;
            }
            if kind == OrderKind::Begin {
                let cpu = cpu.get_or_insert_with(|| {
                    NativeCpu::new(floor.global, order.value != 0).expect("frames for the CPU side")
                });
                if let Some((memory, thread)) = process.take() {
                    cpu.thread_drop(thread);
                    cpu.memory_drop(memory);
                }
                let memory = cpu.memory_new().expect("a memory");
                let thread = cpu.thread_new(memory).expect("a thread");
                process = Some((memory, thread));
                continue;
            }
            let cpu = cpu.as_mut().expect("begun");
            let (memory, thread) = process.expect("begun");
            match kind {
                OrderKind::Write => cpu.write(memory, order.address, &bytes).expect("written"),
                OrderKind::Protect => cpu
                    .set_permissions(
                        memory,
                        order.address,
                        order.length,
                        Permissions::from_bits_truncate(order.value as u8),
                    )
                    .expect("protected"),
                OrderKind::Read => {
                    result.resize(order.length.min(LINK_BYTES as u64) as usize, 0);
                    cpu.read(memory, order.address, &mut result).expect("read");
                }
                OrderKind::SetRegisters => {
                    let block = RegisterBlock::read_from_bytes(&bytes).expect("a register block");
                    cpu.set_registers(thread, &block);
                }
                OrderKind::Run => {
                    let resume = ResumeWords::resume(order.address, order.value);
                    let exit = cpu.run(thread, resume, &mut counter, None);
                    cpu.send_warnings(&mut |warning| {
                        floor::ask(
                            &Warned {
                                count: warning.count,
                                detail: warning.detail,
                            },
                            warning.reason.as_bytes(),
                        )
                    });
                    let mut block = RegisterBlock::ZERO;
                    cpu.registers(thread, &mut block);
                    result.extend_from_slice(ExitWords::of(exit).as_bytes());
                    result.extend_from_slice(block.as_bytes());
                }
                OrderKind::Begin | OrderKind::Finish => {}
            }
        }
        loop {
            floor::ask(&Next {}, &[]);
        }
    }
}

/// Built for the host this program is empty: it only runs inside the virtual machine.
#[cfg(not(target_os = "none"))]
fn main() {}
