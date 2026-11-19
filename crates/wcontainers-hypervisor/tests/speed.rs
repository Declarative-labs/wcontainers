//! The speed bar (docs/native.md section 8): the eight bench.c workloads and calls.c, each built once as a static
//! ARM64 Linux program, run plainly on the host and in the kernel inside the machine, in alternating pairs, every run
//! its own process, measured by the cycles and instructions of the whole process (`proc_pid_rusage`). The plain run
//! maps the program's segments at their own addresses and answers its system calls (write and exit) at a
//! breakpoint put where each `svc` was, so both sides run the workloads' code as the same bytes at the same
//! addresses. The bar: the geometric mean over the eight workloads of each one's median cycles ratio, with a 95%
//! interval from a seeded bootstrap over the pairs, against E's cost plus ten points; when the interval holds the
//! bar, the runs go on with twice the pairs, up to sixty.
//!
//! `WCONTAINERS_SPEED_PAIRS` sets the pairs to start with, `WCONTAINERS_SPEED_SEED` and
//! `WCONTAINERS_SPEED_RESAMPLES` the bootstrap, `WCONTAINERS_SPEED_RAW` the file every pair is appended to and
//! `WCONTAINERS_SPEED_PROGRAMS` a comma-separated list of the programs to run (all by default; the bar needs the
//! eight workloads); `WCONTAINERS_NATIVE_BOUND` the look-ahead.

#[cfg(target_os = "macos")]
#[path = "../../wcontainers-inside/src/bin/native-kernel/protocol.rs"]
mod protocol;

#[cfg(target_os = "macos")]
mod common;

#[cfg(target_os = "macos")]
mod container;

#[cfg(target_os = "macos")]
fn main() {
    macos::main();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("the speed test needs Hypervisor.framework; nothing to run on this host");
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::{c_int, c_void};
    use std::fmt::Write as _;
    use std::io::Write as _;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Mutex;
    use std::time::Instant;

    use linux_raw_sys::general::{__NR_exit, __NR_exit_group, __NR_write, _STK_LIM};
    use object::Endianness;
    use object::elf::{PF_R, PF_W, PF_X, PT_LOAD};
    use object::read::elf::{ElfFile64, FileHeader, ProgramHeader};
    use wcontainers_arm64::encode::{self, form};
    use wcontainers_arm64::{Instruction, decode};
    use wcontainers_inside::stop::SYSCALL_NUMBER;
    use wcontainers_kernel_test::driver::exit_code;

    use crate::common::{
        BENCH, CALLS, IMAGE_BASE, be_signed, benchmark, compiled, load_average, look_ahead,
        page_size, setting,
    };
    use crate::container::{DEFAULT_SEED, REALTIME_NANOSECONDS, native_at, one_program, program};

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

    /// How many calls calls.c makes, as the cost rounds made it.
    const CALLS_MADE: u64 = 10_u64.pow(8);

    /// bench.c's size for timing.
    const TIMING_SCALE: u32 = 1;

    /// Which side a child process runs: [`PLAIN`] or [`NATIVE`].
    const RUN_VARIABLE: &str = "WCONTAINERS_SPEED_RUN";
    const PLAIN: &str = "plain";
    const NATIVE: &str = "native";

    /// The program a child runs, and the image of the test program inside for a native one.
    const IMAGE_VARIABLE: &str = "WCONTAINERS_SPEED_IMAGE";
    const INSIDE_VARIABLE: &str = "WCONTAINERS_SPEED_INSIDE";

    /// The file the inside's image is written to for the children.
    const INSIDE_IMAGE: &str = "speed-inside";

    /// The lines a child reports on: the program's output, then its counts and exit code.
    const OUTPUT_LINE: &str = "speed-output";
    const COUNTS_LINE: &str = "speed-counts";

    const PAIRS_VARIABLE: &str = "WCONTAINERS_SPEED_PAIRS";
    const DEFAULT_PAIRS: u64 = 15;
    /// The most pairs the decision rule runs before it calls the bar undecided.
    const MOST_PAIRS: u64 = 60;
    const SEED_VARIABLE: &str = "WCONTAINERS_SPEED_SEED";
    const DEFAULT_BOOTSTRAP_SEED: u64 = 1;
    const RESAMPLES_VARIABLE: &str = "WCONTAINERS_SPEED_RESAMPLES";
    const DEFAULT_RESAMPLES: u64 = 2000;
    const RAW_VARIABLE: &str = "WCONTAINERS_SPEED_RAW";
    const RAW_FILE: &str = "speed-raw.jsonl";
    const PROGRAMS_VARIABLE: &str = "WCONTAINERS_SPEED_PROGRAMS";
    const PROGRAMS_SEPARATOR: char = ',';

    /// E's cycles cost with the add's placement free, as N2 measured it (docs/native.md section 4), in percent.
    const COUNT_COST_PERCENT: f64 = 11.2;
    /// What the bar allows over the count's cost, in percentage points.
    const MARGIN_POINTS: f64 = 10.0;
    const PERCENT: f64 = 100.0;
    /// The share of the bootstrap's results the interval holds.
    const CONFIDENCE: f64 = 0.95;

    /// `RUSAGE_INFO_V6` of macOS's sys/resource.h.
    const RUSAGE_INFO_V6: c_int = 6;

    /// The words of `rusage_info_v6` after `ri_pcycles`: two energies, two secure times, three neural footprints and
    /// nine reserved.
    const AFTER_PERFORMANCE_CYCLES: usize = 16;

    /// macOS's `struct rusage_info_v6`: libc has the fourth version, whose fields lead it.
    #[repr(C)]
    struct RusageV6 {
        v4: libc::rusage_info_v4,
        flags: u64,
        user_performance_time: u64,
        system_performance_time: u64,
        performance_instructions: u64,
        performance_cycles: u64,
        after: [u64; AFTER_PERFORMANCE_CYCLES],
    }

    /// The general registers before the frame pointer, x29, which macOS's thread state keeps by name.
    const FRAME_POINTER: usize = 29;

    /// The start of macOS's `ucontext_t` (sys/_types/_ucontext.h), to the machine context it points to.
    #[repr(C)]
    struct Context {
        on_stack: c_int,
        mask: libc::sigset_t,
        stack: libc::stack_t,
        link: *mut c_void,
        machine_size: usize,
        machine: *mut MachineContext,
    }

    /// macOS's `__darwin_mcontext64` on ARM64 (mach/arm/_structs.h): the exception state, then the thread state.
    #[repr(C)]
    struct MachineContext {
        fault_address: u64,
        syndrome: u32,
        exception: u32,
        thread: ThreadState,
    }

    /// `__darwin_arm_thread_state64`.
    #[repr(C)]
    struct ThreadState {
        x: [u64; FRAME_POINTER],
        frame_pointer: u64,
        link_register: u64,
        stack_pointer: u64,
        pc: u64,
        cpsr: u32,
        flags: u32,
    }

    unsafe extern "C" {
        /// Makes code written through the data side visible to instruction fetch (libSystem).
        fn sys_icache_invalidate(start: *mut c_void, length: usize);
    }

    /// The whole process's counts so far.
    #[derive(Clone, Copy, Debug, Default)]
    struct Counts {
        cycles: u64,
        instructions: u64,
        performance_cycles: u64,
    }

    fn counts() -> Counts {
        // SAFETY: plain data, filled by the call.
        let mut usage: RusageV6 = unsafe { std::mem::zeroed() };
        // SAFETY: the buffer is a `rusage_info_v6`, the flavour asked for.
        let done = unsafe {
            libc::proc_pid_rusage(
                libc::getpid(),
                RUSAGE_INFO_V6,
                (&raw mut usage).cast::<libc::rusage_info_t>(),
            )
        };
        assert_eq!(done, 0, "proc_pid_rusage");
        Counts {
            cycles: usage.v4.ri_cycles,
            instructions: usage.v4.ri_instructions,
            performance_cycles: usage.performance_cycles,
        }
    }

    /// Reports a child's run to the parent and ends the process.
    fn report(status: i32, output: &[u8]) -> ! {
        let counts = counts();
        let hex = output.iter().fold(String::new(), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        });
        println!("{OUTPUT_LINE} {hex}");
        println!(
            "{COUNTS_LINE} {} {} {} {status}",
            counts.cycles, counts.instructions, counts.performance_cycles
        );
        let _ = std::io::stdout().flush();
        std::process::exit(0)
    }

    pub fn main() {
        be_signed();
        match std::env::var(RUN_VARIABLE).as_deref() {
            Ok(PLAIN) => run_plain(&std::fs::read(image_path()).expect("the program")),
            Ok(NATIVE) => run_native(),
            Ok(other) => panic!("{RUN_VARIABLE}={other}: {PLAIN} or {NATIVE}"),
            Err(_) => measure(),
        }
    }

    fn image_path() -> PathBuf {
        std::env::var_os(IMAGE_VARIABLE)
            .expect("the program's path")
            .into()
    }

    /// The program's output, kept by the plain run's system calls.
    static OUTPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

    /// Runs the program at EL0 on the host: its segments at their addresses with their permissions, each `svc` a
    /// breakpoint whose handler answers the call.
    fn run_plain(image: &[u8]) -> ! {
        let elf = ElfFile64::<Endianness>::parse(image).expect("an ELF image");
        let endian = elf.endian();
        let host_page = page_size();
        for header in elf.elf_program_headers() {
            if header.p_type(endian) != PT_LOAD {
                continue;
            }
            let address = header.p_vaddr(endian);
            let start = address / host_page * host_page;
            let length = (address + header.p_memsz(endian)).next_multiple_of(host_page) - start;
            // SAFETY: a fresh private mapping, asked for where the segment goes.
            let at = unsafe {
                libc::mmap(
                    start as *mut c_void,
                    length as usize,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_PRIVATE | libc::MAP_ANON,
                    -1,
                    0,
                )
            };
            assert_eq!(at as u64, start, "the segment's addresses are free");
            let bytes = header.data(endian, image).expect("the segment's bytes");
            // SAFETY: the mapping holds the segment.
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), address as *mut u8, bytes.len())
            };
            let flags = header.p_flags(endian);
            if flags & PF_X != 0 {
                break_at_system_calls(address, bytes.len());
            }
            let protection = [
                (PF_R, libc::PROT_READ),
                (PF_W, libc::PROT_WRITE),
                (PF_X, libc::PROT_EXEC),
            ]
            .into_iter()
            .filter(|(flag, _)| flags & flag != 0)
            .fold(libc::PROT_NONE, |all, (_, protection)| all | protection);
            // SAFETY: the mapping is this program's.
            let changed = unsafe { libc::mprotect(at, length as usize, protection) };
            assert_eq!(changed, 0, "mprotect");
            if flags & PF_X != 0 {
                // SAFETY: the range is mapped.
                unsafe { sys_icache_invalidate(at, length as usize) };
            }
        }
        // Linux's default stack limit.
        let stack_length = u64::from(_STK_LIM);
        // SAFETY: a fresh private mapping anywhere.
        let stack = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                stack_length as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANON,
                -1,
                0,
            )
        };
        assert_ne!(stack, libc::MAP_FAILED, "the stack");
        // SAFETY: plain data; the handler takes its context as SA_SIGINFO says.
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = on_breakpoint as *const () as usize;
        action.sa_flags = libc::SA_SIGINFO;
        // SAFETY: installs the handler for this process's breakpoints.
        let installed = unsafe { libc::sigaction(libc::SIGTRAP, &action, std::ptr::null_mut()) };
        assert_eq!(installed, 0, "sigaction");
        let top = stack as u64 + stack_length;
        let entry = elf.elf_header().e_entry(endian);
        // SAFETY: the program's code is mapped; it ends the process through its exit call.
        unsafe {
            std::arch::asm!(
                "mov sp, {top}",
                "br {entry}",
                top = in(reg) top,
                entry = in(reg) entry,
                options(noreturn)
            )
        }
    }

    /// Puts a breakpoint over every `svc` among the code words at `address`.
    fn break_at_system_calls(address: u64, length: usize) {
        assert!(
            address.is_multiple_of(size_of::<u32>() as u64),
            "code is word aligned"
        );
        let breakpoint = encode::exception(form::BRK, 0);
        // SAFETY: the words are the segment's, mapped writable.
        let words = unsafe {
            std::slice::from_raw_parts_mut(address as *mut u32, length / size_of::<u32>())
        };
        for word in words {
            if matches!(decode(*word), Instruction::Syscall) {
                *word = breakpoint;
            }
        }
    }

    /// Answers the system call whose `svc` the breakpoint stands for.
    extern "C" fn on_breakpoint(_: c_int, _: *mut libc::siginfo_t, context: *mut c_void) {
        // SAFETY: macOS hands a SA_SIGINFO handler its `ucontext_t`, whose machine context it points to.
        let state = unsafe { &mut (*(*context.cast::<Context>()).machine).thread };
        let [first, buffer, length] = [0, 1, 2].map(|k| state.x[k]);
        let number = state.x[SYSCALL_NUMBER] as u32;
        if number == __NR_write {
            // SAFETY: the program's own buffer, mapped in this process.
            let bytes = unsafe { std::slice::from_raw_parts(buffer as *const u8, length as usize) };
            OUTPUT.lock().expect("the output").extend_from_slice(bytes);
            state.x[0] = length;
            state.pc += size_of::<u32>() as u64;
        } else if number == __NR_exit || number == __NR_exit_group {
            let output = std::mem::take(&mut *OUTPUT.lock().expect("the output"));
            report(first as i32, &output)
        } else {
            panic!("the plain run answers write and exit, not system call {number}");
        }
    }

    /// Runs the program in the kernel inside the machine at the look-ahead the environment gives.
    fn run_native() -> ! {
        let path = image_path();
        let inside = std::fs::read(std::env::var_os(INSIDE_VARIABLE).expect("the inside's image"))
            .expect("the inside's image");
        let name = path
            .file_name()
            .expect("a file name")
            .to_string_lossy()
            .into_owned();
        let (files, starting) = one_program(&name, std::fs::read(&path).expect("the program"), &[]);
        let outcome = native_at(
            &inside,
            look_ahead(),
            DEFAULT_SEED,
            REALTIME_NANOSECONDS,
            &files,
            &starting,
        );
        report(exit_code(outcome.status), &outcome.host.stdout)
    }

    /// One child's run.
    #[derive(Clone, Debug)]
    struct Run {
        counts: Counts,
        wall_nanoseconds: u128,
        status: i32,
        output: String,
    }

    fn run(side: &str, image: &Path, inside: &Path) -> Run {
        let started = Instant::now();
        let out = Command::new(std::env::current_exe().expect("this test's path"))
            .env(RUN_VARIABLE, side)
            .env(IMAGE_VARIABLE, image)
            .env(INSIDE_VARIABLE, inside)
            .output()
            .expect("running a child");
        let wall_nanoseconds = started.elapsed().as_nanos();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "the {side} run of {}: {}\n{text}{}",
            image.display(),
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        let line = |key: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(key)?.strip_prefix(' '))
                .unwrap_or_else(|| panic!("the {side} run reports {key}:\n{text}"))
                .to_owned()
        };
        let output = line(OUTPUT_LINE);
        let numbers: Vec<i64> = line(COUNTS_LINE)
            .split(' ')
            .map(|n| n.parse().expect("a number"))
            .collect();
        let [cycles, instructions, performance_cycles, status] = numbers[..] else {
            panic!("four numbers: {numbers:?}");
        };
        Run {
            counts: Counts {
                cycles: cycles as u64,
                instructions: instructions as u64,
                performance_cycles: performance_cycles as u64,
            },
            wall_nanoseconds,
            status: status as i32,
            output,
        }
    }

    /// A plain run and a native run of one program, one after the other.
    struct Pair {
        plain: Run,
        native: Run,
    }

    impl Pair {
        fn ratio(&self, of: impl Fn(&Run) -> f64) -> f64 {
            of(&self.native) / of(&self.plain)
        }
    }

    fn cycles(run: &Run) -> f64 {
        run.counts.cycles as f64
    }

    fn median(values: &mut [f64]) -> f64 {
        values.sort_by(f64::total_cmp);
        let middle = values.len() / 2;
        if values.len().is_multiple_of(2) {
            (values[middle - 1] + values[middle]) / 2.0
        } else {
            values[middle]
        }
    }

    fn geometric_mean(values: &[f64]) -> f64 {
        (values.iter().map(|v| v.ln()).sum::<f64>() / values.len() as f64).exp()
    }

    fn percent(ratio: f64) -> f64 {
        (ratio - 1.0) * PERCENT
    }

    /// A program of the set: its name and built image, and its pairs so far.
    struct Program {
        name: String,
        image: PathBuf,
        in_bar: bool,
        pairs: Vec<Pair>,
        expected: Option<(i32, String)>,
    }

    fn json(run: &Run) -> String {
        format!(
            "{{\"cycles\":{},\"instructions\":{},\"performance_cycles\":{},\"wall_ns\":{}}}",
            run.counts.cycles,
            run.counts.instructions,
            run.counts.performance_cycles,
            run.wall_nanoseconds
        )
    }

    /// Runs `program`'s pairs up to `wanted`, the first after one discarded run of each side, appending each to
    /// `raw`.
    fn run_pairs(program: &mut Program, wanted: u64, inside: &Path, raw: &mut std::fs::File) {
        if program.pairs.is_empty() {
            for side in [PLAIN, NATIVE] {
                run(side, &program.image, inside);
            }
        }
        while (program.pairs.len() as u64) < wanted {
            let index = program.pairs.len();
            let order = if index.is_multiple_of(2) {
                [PLAIN, NATIVE]
            } else {
                [NATIVE, PLAIN]
            };
            let load = load_average();
            let [first, second] = order.map(|side| run(side, &program.image, inside));
            let (plain, native) = if order[0] == PLAIN {
                (first, second)
            } else {
                (second, first)
            };
            let expected = program
                .expected
                .get_or_insert_with(|| (plain.status, plain.output.clone()));
            for (side, run) in [(PLAIN, &plain), (NATIVE, &native)] {
                assert_eq!(
                    (&run.status, &run.output),
                    (&expected.0, &expected.1),
                    "{}: the {side} run's exit code and output",
                    program.name
                );
            }
            writeln!(
                raw,
                "{{\"program\":\"{}\",\"pair\":{index},\"first\":\"{}\",\"look_ahead\":\"{:?}\",\"load\":\"{load}\",\"plain\":{},\"native\":{}}}",
                program.name,
                order[0],
                look_ahead(),
                json(&plain),
                json(&native)
            )
            .expect("writing the raw pairs");
            program.pairs.push(Pair { plain, native });
        }
    }

    /// The bar's geometric mean of the workloads' median cycles ratios, and its interval.
    fn bar_estimate(programs: &[Program], seed: u64, resamples: u64) -> (f64, f64, f64) {
        let bar: Vec<&Program> = programs.iter().filter(|p| p.in_bar).collect();
        let estimate = geometric_mean(
            &bar.iter()
                .map(|p| {
                    median(
                        &mut p
                            .pairs
                            .iter()
                            .map(|pair| pair.ratio(cycles))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
        );
        let mut random = fastrand::Rng::with_seed(seed);
        let mut means: Vec<f64> = (0..resamples)
            .map(|_| {
                geometric_mean(
                    &bar.iter()
                        .map(|p| {
                            let mut drawn: Vec<f64> = (0..p.pairs.len())
                                .map(|_| p.pairs[random.usize(..p.pairs.len())].ratio(cycles))
                                .collect();
                            median(&mut drawn)
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        means.sort_by(f64::total_cmp);
        let tail = (1.0 - CONFIDENCE) / 2.0;
        let at = |share: f64| means[((means.len() - 1) as f64 * share).round() as usize];
        (estimate, at(tail), at(1.0 - tail))
    }

    fn summarize(program: &Program) {
        let medians = [
            (
                "cycles",
                program
                    .pairs
                    .iter()
                    .map(|p| p.ratio(cycles))
                    .collect::<Vec<_>>(),
            ),
            (
                "instructions",
                program
                    .pairs
                    .iter()
                    .map(|p| p.ratio(|r| r.counts.instructions as f64))
                    .collect(),
            ),
            (
                "wall",
                program
                    .pairs
                    .iter()
                    .map(|p| p.ratio(|r| r.wall_nanoseconds as f64))
                    .collect(),
            ),
        ]
        .map(|(what, mut ratios)| format!("{what} {:+.1}%", percent(median(&mut ratios))));
        let share = |of: fn(&Pair) -> &Run| {
            median(
                &mut program
                    .pairs
                    .iter()
                    .map(|p| of(p).counts.performance_cycles as f64 / cycles(of(p)) * PERCENT)
                    .collect::<Vec<_>>(),
            )
        };
        let plain_cycles = median(
            &mut program
                .pairs
                .iter()
                .map(|p| cycles(&p.plain))
                .collect::<Vec<_>>(),
        );
        println!(
            "speed: {} over {} pairs, medians native against plain: {}; plain {plain_cycles:.0} cycles; on performance cores {:.0}% plain, {:.0}% native",
            program.name,
            program.pairs.len(),
            medians.join(", "),
            share(|p| &p.plain),
            share(|p| &p.native),
        );
    }

    fn measure() {
        let inside = compiled(INSIDE_IMAGE);
        std::fs::write(&inside, program()).expect("writing the inside's image");
        let wanted_names: Option<Vec<String>> = std::env::var(PROGRAMS_VARIABLE)
            .ok()
            .map(|list| list.split(PROGRAMS_SEPARATOR).map(str::to_owned).collect());
        let wanted_program = |name: &str| {
            wanted_names
                .as_ref()
                .is_none_or(|names| names.iter().any(|n| n == name))
        };
        let mut programs: Vec<Program> = WORKLOADS
            .iter()
            .zip(1..)
            .map(|(&name, workload)| {
                let defines = [
                    format!("SCALE={TIMING_SCALE}"),
                    format!("WORKLOAD={workload}"),
                ];
                (name, BENCH, defines.to_vec(), true)
            })
            .chain(std::iter::once((
                CALLS,
                CALLS,
                vec![format!("CALLS={CALLS_MADE}")],
                false,
            )))
            .filter(|(name, ..)| wanted_program(name))
            .map(|(name, source, defines, in_bar)| Program {
                name: name.to_owned(),
                image: benchmark(name, source, &defines, Some(IMAGE_BASE)),
                in_bar,
                pairs: Vec::new(),
                expected: None,
            })
            .collect();
        let raw_path =
            std::env::var_os(RAW_VARIABLE).map_or_else(|| compiled(RAW_FILE), PathBuf::from);
        let mut raw = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&raw_path)
            .expect("the raw pairs' file");
        let seed = setting(SEED_VARIABLE, DEFAULT_BOOTSTRAP_SEED);
        let resamples = setting(RESAMPLES_VARIABLE, DEFAULT_RESAMPLES);
        let bar = COUNT_COST_PERCENT + MARGIN_POINTS;
        let mut wanted = setting(PAIRS_VARIABLE, DEFAULT_PAIRS);
        println!(
            "speed: look-ahead {:?}, load {} at the start, raw pairs in {}",
            look_ahead(),
            load_average(),
            raw_path.display()
        );
        loop {
            for program in &mut programs {
                run_pairs(program, wanted, &inside, &mut raw);
                summarize(program);
            }
            if programs.iter().filter(|p| p.in_bar).count() != WORKLOADS.len() {
                println!("speed: not every workload ran, so no verdict on the bar");
                return;
            }
            let (estimate, lower, upper) = bar_estimate(&programs, seed, resamples);
            println!(
                "speed: geometric mean of the median cycles ratios {:+.2}%, {:.0}% interval [{:+.2} .. {:+.2}] (seed {seed:#x}, {resamples} resamples, {wanted} pairs), bar {bar:+.1}%, load {}",
                percent(estimate),
                CONFIDENCE * PERCENT,
                percent(lower),
                percent(upper),
                load_average()
            );
            if percent(upper) <= bar {
                println!("speed: the bar passes");
                return;
            }
            assert!(percent(lower) <= bar, "speed: the bar fails");
            assert!(
                wanted * 2 <= MOST_PAIRS,
                "speed: undecided at {wanted} pairs; run it on a quieter machine"
            );
            wanted *= 2;
        }
    }
}
