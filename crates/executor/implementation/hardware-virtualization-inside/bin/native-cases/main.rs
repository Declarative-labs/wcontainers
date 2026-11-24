//! The native output's cases: a test program run by the monitor's native test, never shipped. It maps the
//! regions the host names, puts the vectors the host writes in place, and for each run makes the host's code
//! visible and calls the gate, from which the host runs the guest's level and reads it back.

#![cfg_attr(target_os = "none", no_std, no_main)]

#[cfg(target_os = "none")]
mod protocol;

#[cfg(target_os = "none")]
mod cases {
    use aarch64_cpu::asm::barrier::{self, SY};
    use aarch64_cpu::registers::{VBAR_EL1, Write};
    use aarch64_paging::descriptor::El1Attributes;
    use wcontainers_abi::PAGE_SIZE;
    use wcontainers_executor_hardware_virtualization_inside::floor::{
        self, AddressSpace, FIRST_PROCESS_IDENTIFIER, Floor, Frames, Full, code, guest_read_only,
        guest_read_write, inside_code, make_code_visible, register_page, with_frames,
    };
    use wcontainers_executor_hardware_virtualization_inside::link::Start;

    use crate::protocol::{Code, Done, GATE_OFFSET, Kind, Map, Next, Region};

    /// Runs `f` over the frames; the host sizes the regions to fit guest-physical memory.
    fn frames<T>(f: impl FnOnce(&mut Frames) -> Result<T, Full>) -> T {
        with_frames(f).expect("the regions fit guest-physical memory")
    }

    /// `pages` fresh frames in a row, taken before any table is, so nothing comes between them.
    fn run_of_frames(pages: u64) -> u64 {
        frames(|frames| {
            let first = frames.take()?;
            for k in 1..pages {
                let frame = frames.take()?;
                assert_eq!(frame, first + k * PAGE_SIZE, "frames in a row");
            }
            Ok(first)
        })
    }

    /// Maps one region and gives its first frame.
    fn map(floor: &Floor, space: &AddressSpace, region: Region) -> u64 {
        let kind = Kind::from_word(region.kind).expect("a kind of region");
        let pages = region.bytes.div_ceil(PAGE_SIZE);
        let (table, attributes, repeated): (_, El1Attributes, bool) = match kind {
            Kind::ReadWrite => (space.user, guest_read_write(), false),
            Kind::ReadOnly => (space.user, guest_read_only(), false),
            Kind::Repeated => (space.user, guest_read_only(), true),
            Kind::Code => (floor.global, code(), false),
            Kind::RegisterPage => (space.inside, register_page(), false),
            Kind::Gate => (floor.global, inside_code(), false),
        };
        let first = run_of_frames(if repeated { 1 } else { pages });
        frames(|frames| {
            for k in 0..pages {
                let frame = if repeated {
                    first
                } else {
                    first + k * PAGE_SIZE
                };
                table.map(frames, region.va + k * PAGE_SIZE, frame, attributes)?;
            }
            Ok(())
        });
        first
    }

    #[expect(
        unsafe_code,
        reason = "exports floor_main under its own name for the entry assembly to call, a name nothing else defines; test program only, a sym operand would remove it"
    )]
    #[unsafe(no_mangle)]
    extern "C" fn floor_main(start: &Start) -> ! {
        let floor = Floor::boot(start);
        let space =
            frames(|frames| AddressSpace::new(frames, &floor.global, FIRST_PROCESS_IDENTIFIER));
        let mut frame = 0;
        let mut gate = None;
        loop {
            let region = floor::ask(&Map { frame }, &[]);
            if region.bytes == 0 {
                break;
            }
            if region.kind == Kind::Gate as u64 {
                gate = Some(region);
            }
            frame = map(&floor, &space, region);
        }
        space.switch();
        let gate = gate.expect("the host names a gate");
        make_code_visible(gate.va, gate.bytes);
        VBAR_EL1.set(gate.va);
        barrier::isb(SY);
        // SAFETY: the host wrote the gate, which comes back as a call does: it restores every register.
        #[expect(
            unsafe_code,
            reason = "turns the address of the gate the host wrote into a function pointer; the gate comes back as a call does, restoring every register; test program only, stays a one-off"
        )]
        let call: extern "C" fn() = unsafe { core::mem::transmute(gate.va + GATE_OFFSET) };
        loop {
            let code: Code = floor::ask(&Next {}, &[]);
            if code.bytes == 0 {
                break;
            }
            make_code_visible(code.va, code.bytes);
            call();
        }
        loop {
            floor::ask(&Done {}, &[]);
        }
    }
}

/// Built for the host this program is empty: it only runs inside the virtual machine.
#[cfg(not(target_os = "none"))]
fn main() {}
