//! The dynamic register uses of docs/native.md section 3.3: every program of the speed test run under
//! unicorn-engine's ARM64 emulator, each executed instruction's named general registers counted, so the withheld
//! registers can be the ones the benchmark set names least. Prints the share of each program's executed instructions
//! that name each register, and the mean over the programs; a measurement, with no verdict.
//!
//! `WCONTAINERS_REGISTERS_SCALE` sets bench.c's size (0 by default) and `WCONTAINERS_REGISTERS_CALLS` calls.c's
//! number of calls.

#[cfg(target_os = "macos")]
mod common;

#[cfg(target_os = "macos")]
fn main() {
    macos::main();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!(
        "the register count builds its programs with the speed test's tools; nothing to run on this host"
    );
}

#[cfg(target_os = "macos")]
mod macos {
    use std::cell::RefCell;
    use std::rc::Rc;

    use linux_raw_sys::general::{__NR_exit, __NR_exit_group, __NR_write, _STK_LIM};
    use object::Endianness;
    use object::elf::{PF_X, PT_LOAD};
    use object::read::elf::{ElfFile64, FileHeader, ProgramHeader};
    use unicorn_engine::{Arch, Arm64CpuModel, Mode, Prot, RegisterARM64, Unicorn};
    use wcontainers_abi::PAGE_SIZE;
    use wcontainers_arm64::{Arm64, Host, Instruction, Reg, decode};
    use wcontainers_guest_cpu::{Guest, effects};

    use crate::common::{BENCH, CALLS, IMAGE_BASE, PROGRAMS_DIRECTORY, freestanding, setting};

    /// bench.c's workloads, in the order of its `WORKLOAD` numbers, which start at one.
    const WORKLOADS: [&str; 8] = [
        "fib",
        "sieve",
        "hash",
        "sort",
        "interpret",
        "matrix",
        "walk",
        "search",
    ];

    /// The general registers an instruction can name: x0 to x30.
    const GENERAL: u8 = 31;

    /// Where the stack goes: under the image, clear of it.
    const STACK_TOP: u64 = IMAGE_BASE / 2;

    /// Sets bench.c's size.
    const SCALE_VARIABLE: &str = "WCONTAINERS_REGISTERS_SCALE";
    /// Sets how many calls calls.c makes.
    const CALLS_VARIABLE: &str = "WCONTAINERS_REGISTERS_CALLS";
    /// calls.c's calls by default: enough to dwarf its start.
    const CALLS_MADE: u64 = 10_u64.pow(6);

    /// What the float meanings compute is no matter here: it changes no register an instruction names.
    const HOST: Host = Host {
        fused_multiply_add: true,
        arm_nans: true,
    };

    /// The general registers each instruction word of an executable segment names, by its offset from `base`.
    struct Named {
        base: u64,
        masks: Vec<u32>,
    }

    impl Named {
        fn new(base: u64, bytes: &[u8]) -> Named {
            let masks = bytes
                .as_chunks::<{ size_of::<u32>() }>()
                .0
                .iter()
                .map(|&w| {
                    let word = u32::from_le_bytes(w);
                    let e = effects::<Arm64>(&decode(word), HOST, ());
                    let both = e.uses.union(e.defs);
                    (0..GENERAL)
                        .filter(|&n| Arm64::reg_bit(Reg::X(n)).is_some_and(|bit| both.has_bit(bit)))
                        .fold(0, |mask, n| mask | 1 << n)
                })
                .collect();
            Named { base, masks }
        }
    }

    /// Executed instructions, and how many of them named each general register.
    #[derive(Default)]
    struct Counts {
        instructions: u64,
        named: [u64; GENERAL as usize],
    }

    /// Runs the static program `image` under the emulator to its exit and counts what it executed.
    fn count(image: &[u8]) -> Counts {
        let elf = ElfFile64::<Endianness>::parse(image).expect("an ELF image");
        let endian = elf.endian();
        let mut unicorn = Unicorn::new(Arch::ARM64, Mode::ARM).expect("an ARM64 emulator");
        unicorn
            .ctl_set_cpu_model(Arm64CpuModel::MAX as i32)
            .expect("the most capable CPU model");
        let mut named = Vec::new();
        for header in elf.elf_program_headers() {
            if header.p_type(endian) != PT_LOAD {
                continue;
            }
            let address = header.p_vaddr(endian);
            let start = address / PAGE_SIZE * PAGE_SIZE;
            let length = (address + header.p_memsz(endian)).next_multiple_of(PAGE_SIZE) - start;
            unicorn
                .mem_map(start, length, Prot::ALL)
                .expect("the segment's pages");
            let bytes = header.data(endian, image).expect("the segment's bytes");
            unicorn.mem_write(address, bytes).expect("the segment");
            if header.p_flags(endian) & PF_X != 0 {
                named.push(Named::new(address, bytes));
            }
        }
        let stack = u64::from(_STK_LIM);
        unicorn
            .mem_map(STACK_TOP - stack, stack, Prot::READ | Prot::WRITE)
            .expect("the stack");
        unicorn
            .reg_write(RegisterARM64::SP, STACK_TOP)
            .expect("the stack pointer");
        let counts = Rc::new(RefCell::new(Counts::default()));
        let into = counts.clone();
        unicorn
            .add_code_hook(1, 0, move |_, address, _| {
                let mut c = into.borrow_mut();
                c.instructions += 1;
                let segment = named
                    .iter()
                    .find(|n| {
                        (n.base..n.base + n.masks.len() as u64 * Arm64::MAX_LENGTH)
                            .contains(&address)
                    })
                    .expect("code in an executable segment");
                let mask = segment.masks[((address - segment.base) / Arm64::MAX_LENGTH) as usize];
                for n in 0..GENERAL {
                    c.named[usize::from(n)] += u64::from(mask >> n & 1);
                }
            })
            .expect("the code hook");
        unicorn
            .add_intr_hook(|unicorn, _| {
                let pc = unicorn.reg_read(RegisterARM64::PC).expect("pc");
                let mut word = [0; size_of::<u32>()];
                unicorn
                    .mem_read(pc - Arm64::MAX_LENGTH, &mut word)
                    .expect("the instruction before the stop");
                assert!(
                    matches!(decode(u32::from_le_bytes(word)), Instruction::Syscall),
                    "an exception other than svc at {pc:#x}"
                );
                let number = unicorn.reg_read(RegisterARM64::X8).expect("x8");
                let number = u32::try_from(number).expect("a call number");
                if number == __NR_write {
                    let length = unicorn.reg_read(RegisterARM64::X2).expect("x2");
                    unicorn
                        .reg_write(RegisterARM64::X0, length)
                        .expect("write's result");
                } else if number == __NR_exit || number == __NR_exit_group {
                    unicorn.emu_stop().expect("stopped");
                } else {
                    panic!("the program called {number}, which a freestanding program does not");
                }
            })
            .expect("the system call hook");
        let entry = elf.elf_header().e_entry(endian);
        unicorn.emu_start(entry, 0, 0, 0).expect("the program runs");
        counts.take()
    }

    pub fn main() {
        let scale = setting(SCALE_VARIABLE, 0);
        let calls = setting(CALLS_VARIABLE, CALLS_MADE);
        let programs = WORKLOADS
            .iter()
            .zip(1..)
            .map(|(&name, workload)| {
                (
                    name,
                    BENCH,
                    vec![format!("SCALE={scale}"), format!("WORKLOAD={workload}")],
                )
            })
            .chain(std::iter::once((
                CALLS,
                CALLS,
                vec![format!("CALLS={calls}")],
            )));
        let mut rates: Vec<[f64; GENERAL as usize]> = Vec::new();
        for (name, source, defines) in programs {
            let path = freestanding(
                PROGRAMS_DIRECTORY,
                &format!("registers-{name}"),
                source,
                &defines,
                Some(IMAGE_BASE),
            );
            let counts = count(&std::fs::read(path).expect("the built program"));
            let rate = counts.named.map(|n| n as f64 / counts.instructions as f64);
            println!(
                "registers: {name}, {} instructions, share of them naming each register: {}",
                counts.instructions,
                listed(&rate)
            );
            rates.push(rate);
        }
        let mean: [f64; GENERAL as usize] =
            std::array::from_fn(|n| rates.iter().map(|r| r[n]).sum::<f64>() / rates.len() as f64);
        let mut order: Vec<usize> = (0..usize::from(GENERAL)).collect();
        order.sort_by(|&a, &b| mean[a].total_cmp(&mean[b]));
        println!(
            "registers: the mean over {} programs, fewest first: {}",
            rates.len(),
            order
                .iter()
                .map(|&n| format!("x{n} {:.4}", mean[n]))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    fn listed(rate: &[f64]) -> String {
        rate.iter()
            .enumerate()
            .map(|(n, r)| format!("x{n} {r:.4}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}
