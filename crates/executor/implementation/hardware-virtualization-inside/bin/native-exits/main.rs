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

    use wcontainers_abi::{Cpu, PAGE_SIZE, Permissions, RegisterBlock, SharedId};
    use wcontainers_executor_hardware_virtualization_inside::floor::{
        self, Floor, counter, with_frames,
    };
    use wcontainers_executor_hardware_virtualization_inside::link::{LINK_BYTES, Start};
    use wcontainers_executor_hardware_virtualization_inside::native::NativeCpu;
    use zerocopy::{FromBytes, IntoBytes};

    use crate::protocol::{ExitWords, Next, Order, OrderKind, ResumeWords, Warned};

    #[expect(
        unsafe_code,
        reason = "exports floor_main under its own name for the entry assembly to call, a name nothing else defines; test program only, a sym operand would remove it"
    )]
    #[unsafe(no_mangle)]
    extern "C" fn floor_main(start: &Start) -> ! {
        let floor = Floor::boot(start);
        let mut cpu: Option<NativeCpu> = None;
        let mut process = None;
        let mut result: Vec<u8> = Vec::new();
        // The frames `TakeEveryFrame` took, until `GiveFramesBack`.
        let mut held: Vec<u64> = Vec::new();
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
                OrderKind::TryProtect => {
                    let done = cpu.set_permissions(
                        memory,
                        order.address,
                        order.length,
                        Permissions::from_bits_truncate(order.value as u8),
                    );
                    result.extend_from_slice(u64::from(done.is_err()).as_bytes());
                }
                OrderKind::TakeEveryFrame => {
                    // Room for every frame first, once: the heap takes frames, so it may not grow under their
                    // lock, nor once they are all taken.
                    held.reserve_exact(((start.memory / PAGE_SIZE) as usize).saturating_sub(held.len()));
                    let before = held.len();
                    with_frames(|frames| {
                        while held.len() < held.capacity()
                            && let Ok(frame) = frames.take()
                        {
                            held.push(frame);
                        }
                    });
                    result.extend_from_slice(((held.len() - before) as u64).as_bytes());
                }
                OrderKind::GiveFramesBack => {
                    with_frames(|frames| held.drain(..).for_each(|frame| frames.give(frame)));
                }
                OrderKind::SharedNew => {
                    let SharedId(shared) = cpu.shared_new(order.length).expect("a shared block");
                    result.extend_from_slice(u64::from(shared).as_bytes());
                }
                OrderKind::SharedMap => cpu
                    .shared_map(
                        SharedId(order.value as u32),
                        0,
                        memory,
                        order.address,
                        order.length,
                    )
                    .expect("shown"),
                OrderKind::SharedWrite => cpu
                    .shared_write(SharedId(order.value as u32), order.address, &bytes)
                    .expect("written to the block"),
                OrderKind::SharedDrop => cpu.shared_drop(SharedId(order.value as u32)),
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
