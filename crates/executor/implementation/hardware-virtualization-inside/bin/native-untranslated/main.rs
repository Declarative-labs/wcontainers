//! The untranslated run: a test program run by the monitor's speed test, never shipped. It maps a static
//! program's segments at their own addresses, as the floor maps guest memory but with its code runnable at the
//! guest's level, runs the program's own instructions there and answers its write and exit calls, so the speed
//! test can tell what the virtual machine costs before any translation.

#![cfg_attr(target_os = "none", no_std, no_main)]

#[cfg(target_os = "none")]
extern crate alloc;

#[cfg(target_os = "none")]
mod protocol;

#[cfg(target_os = "none")]
mod untranslated {
    use alloc::vec::Vec;

    use aarch64_cpu::registers::{ESR_EL1, FAR_EL1, Read, SPSR_EL1};
    use aarch64_paging::descriptor::El1Attributes;
    use wcontainers_abi::PAGE_SIZE;
    use wcontainers_executor_hardware_virtualization_inside::floor::{
        AddressSpace, Doorbell, FIRST_PROCESS_IDENTIFIER, Floor, Full, Taken, UserRegisters,
        enter_user, guest_read_only, guest_read_write, make_code_visible, with_frames,
    };
    use wcontainers_executor_hardware_virtualization_inside::layout::{BLOCK_SIZE, linear};
    use wcontainers_executor_hardware_virtualization_inside::link::{Ring, Start};
    use wcontainers_executor_hardware_virtualization_inside::stop::SYSCALL_NUMBER;
    use wcontainers_kernel_linux::linux::general::{__NR_exit, __NR_exit_group, __NR_write};

    use crate::protocol::{Exited, Load, Output, Segment, Segments};

    /// A range of the program's memory and the guest-physical frames that hold it, one after the other.
    struct Region {
        address: u64,
        length: u64,
        frame: u64,
    }

    /// Takes frames for `length` bytes at `address`, placed so that the address and its frame agree within a
    /// block, so every whole block of the range takes a block entry, and maps them.
    fn map(space: &AddressSpace, address: u64, length: u64, attributes: El1Attributes) -> Region {
        let within = address % BLOCK_SIZE;
        let frame = with_frames(|frames| {
            let run = frames.take_blocks((within + length).div_ceil(BLOCK_SIZE))?;
            space
                .user
                .map_range(frames, address, run + within, length, attributes)?;
            Ok::<u64, Full>(run + within)
        })
        .expect("the program fits guest-physical memory");
        Region {
            address,
            length,
            frame,
        }
    }

    /// The bytes of the program's memory at `address`.
    fn bytes(regions: &[Region], address: u64, length: u64) -> &'static [u8] {
        let region = regions
            .iter()
            .find(|r| address >= r.address && address + length <= r.address + r.length)
            .expect("the program wrote from its own memory");
        // SAFETY: the region's frames are one run, reached through the linear map.
        unsafe {
            core::slice::from_raw_parts(
                linear(region.frame + (address - region.address)) as *const u8,
                length as usize,
            )
        }
    }

    #[unsafe(no_mangle)]
    extern "C" fn floor_main(start: &Start) -> ! {
        let mut floor = Floor::boot(start);
        let global = floor.global;
        let space =
            with_frames(|frames| AddressSpace::new(frames, &global, FIRST_PROCESS_IDENTIFIER))
                .expect("frames for the address space");
        let program = Doorbell.ask(&Segments {}, &[]);
        let mut regions = Vec::new();
        let mut end = 0;
        for index in 0..program.segments {
            let segment: Segment = Doorbell.ask(&Load { index }, &[]);
            let first = segment.address & !(PAGE_SIZE - 1);
            let length =
                (segment.address + segment.memory_length).next_multiple_of(PAGE_SIZE) - first;
            let attributes = if segment.writable != 0 {
                guest_read_write()
            } else if segment.executable != 0 {
                guest_read_only() - El1Attributes::UXN
            } else {
                guest_read_only()
            };
            let region = map(&space, first, length, attributes);
            let at = region.frame + (segment.address - first);
            // SAFETY: the region's frames are one fresh run, reached through the linear map.
            let into = unsafe {
                core::slice::from_raw_parts_mut(linear(at) as *mut u8, segment.file_length as usize)
            };
            Doorbell.take(into);
            if segment.executable != 0 {
                make_code_visible(linear(at), segment.file_length);
            }
            end = end.max(first + length);
            regions.push(region);
        }
        // The stack after the program, a block apart, as the plain run's stack is a mapping of its own.
        let stack = end.next_multiple_of(BLOCK_SIZE) + BLOCK_SIZE;
        regions.push(map(&space, stack, program.stack, guest_read_write()));
        let mut registers = UserRegisters {
            sp: stack + program.stack,
            pc: program.entry,
            pstate: SPSR_EL1::M::EL0t.value,
            ..UserRegisters::default()
        };
        space.switch();
        loop {
            match enter_user(&mut registers) {
                Taken::Synchronous => {}
                Taken::Interrupt => {
                    floor.take_interrupt();
                    continue;
                }
                Taken::Other => panic!("an unexpected exception at {:#x}", registers.pc),
            }
            assert_eq!(
                ESR_EL1.read(ESR_EL1::EC),
                ESR_EL1::EC::Value::SVC64 as u64,
                "the program stopped at {:#x}: ESR {:#x}, FAR {:#x}",
                registers.pc,
                ESR_EL1.get(),
                FAR_EL1.get()
            );
            let [first, buffer, length] = [0, 1, 2].map(|k| registers.x[k]);
            let number = registers.x[SYSCALL_NUMBER] as u32;
            if number == __NR_write {
                Doorbell.ask(&Output {}, bytes(&regions, buffer, length));
                registers.x[0] = length;
            } else if number == __NR_exit || number == __NR_exit_group {
                loop {
                    Doorbell.ask(&Exited { status: first }, &[]);
                }
            } else {
                panic!("the untranslated run answers write and exit, not system call {number}");
            }
        }
    }
}

/// Built for the host this program is empty: it only runs inside the virtual machine.
#[cfg(not(target_os = "none"))]
fn main() {}
