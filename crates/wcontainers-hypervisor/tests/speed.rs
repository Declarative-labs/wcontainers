//! The speed bar (docs/native.md section 8): the eight bench.c workloads and calls.c, each built once as a static
//! ARM64 Linux program, run plainly on the host and in the kernel inside the machine, in alternating pairs, every run
//! its own process, measured by the cycles and instructions of the whole process (`proc_pid_rusage`). The plain run
//! maps the program's segments at their own addresses and answers its system calls (write and exit) at a
//! breakpoint put where each `svc` was in a copy of the image written before any run, so both sides run the
//! workloads' code as the same bytes at the same addresses and the plain run does nothing a plain process does not. The bar: the geometric mean over the eight workloads of each one's median cycles ratio, with a 95%
//! interval from a seeded bootstrap over the pairs, against E's cost plus ten points; when the interval holds the
//! bar, the runs go on with twice the pairs, up to sixty. The system-call part: a program making a million getppid
//! calls against the same program with a nop in each call's place, both in the kernel inside, in alternating pairs,
//! the kernel's own share of a call measured beside each pair over a CPU side that runs no instructions (layer 1's
//! method); the back end's share of a call, the per-call time less the kernel's, against 100 ns by the same rule.
//! Beside each pair every workload also runs untranslated at the guest's level in the machine (the test program
//! `native-untranslated`), recorded as what the virtual machine costs before any translation, never part of the bar.
//!
//! `WCONTAINERS_SPEED_PAIRS` sets the pairs to start with, `WCONTAINERS_SPEED_SEED` and
//! `WCONTAINERS_SPEED_RESAMPLES` the bootstrap, `WCONTAINERS_SPEED_RAW` the file every pair is appended to and
//! `WCONTAINERS_SPEED_PROGRAMS` a comma-separated list of the programs to run (all by default; the compute part
//! needs the eight workloads, and `syscalls` names the system-call part); `WCONTAINERS_NATIVE_BOUND` the look-ahead.

#[cfg(target_os = "macos")]
#[path = "../../wcontainers-inside/src/bin/native-kernel/protocol.rs"]
mod protocol;

#[cfg(target_os = "macos")]
#[path = "../../wcontainers-inside/src/bin/native-untranslated/protocol.rs"]
mod untranslated;

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

    use linux_raw_sys::general::{
        __NR_exit, __NR_exit_group, __NR_getppid, __NR_write, _STK_LIM, S_IRWXU,
    };
    use object::Endianness;
    use object::elf::{PF_R, PF_W, PF_X, PT_LOAD};
    use object::read::elf::{ElfFile64, FileHeader, ProgramHeader};
    use wcontainers_abi::{Exit, Next, Resume, Syscall};
    use wcontainers_arm64::encode::{self, form};
    use wcontainers_arm64::{Instruction, decode};
    use wcontainers_hypervisor::{
        Exit as VcpuExit, Register, VirtualCpu, VirtualMachine, boot, link as link_page,
    };
    use wcontainers_inside::link::{self, Fatal, Parts};
    use wcontainers_inside::stop::SYSCALL_NUMBER;
    use wcontainers_kernel::{Kernel, Start};
    use wcontainers_kernel_test::buffer::BufferCpu;
    use wcontainers_kernel_test::driver::TestHost;
    use wcontainers_kernel_test::driver::exit_code;
    use wcontainers_kernel_test::image::{File, Kind, add_files};

    use crate::common::{
        BENCH, CALLS, IMAGE_BASE, MEMORY, ONE_CPU, OWN_GUEST, PROGRAMS_DIRECTORY, be_signed, build,
        compiled, freestanding, load_average, look_ahead, machine, page_size, setting,
    };
    use crate::container::{
        DEFAULT_SEED, Policy, REALTIME_NANOSECONDS, native_at, one_program, program,
    };
    use crate::untranslated::{Exited, Load, Program as Loadable, Request, Segment};

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

    /// Which side a child process runs: [`PLAIN`], [`NATIVE`] or [`UNTRANSLATED`].
    const RUN_VARIABLE: &str = "WCONTAINERS_SPEED_RUN";
    const PLAIN: &str = "plain";
    const NATIVE: &str = "native";
    const UNTRANSLATED: &str = "untranslated";
    /// What the names of the dynamic loader's variables begin with, which no child inherits.
    const LOADER_VARIABLE_PREFIX: &str = "DYLD_";

    /// The program a child runs, and the image of the test program inside for the other two sides.
    const IMAGE_VARIABLE: &str = "WCONTAINERS_SPEED_IMAGE";
    const INSIDE_VARIABLE: &str = "WCONTAINERS_SPEED_INSIDE";

    /// The files the inside's images are written to for the children: the kernel's and the untranslated run's.
    const INSIDE_IMAGE: &str = "speed-inside";
    const UNTRANSLATED_IMAGE: &str = "speed-untranslated";

    /// The test program inside that runs a program untranslated.
    const UNTRANSLATED_PROGRAM: &str = "native-untranslated";

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

    /// The system-call part's program, which makes getppid calls, and its build with a nop in each call's place.
    const ANSWERED: &str = "answered";
    const NOTHING: &str = "nothing";
    /// The define that builds [`NOTHING`].
    const NOTHING_DEFINE: &str = "NOTHING";
    /// The calls each run of the system-call part makes, as section 8's method has it.
    const SYSTEM_CALLS_MADE: u64 = 10_u64.pow(6);
    /// The back end's share of a system call answered inside, at most (section 8's second part).
    const SYSTEM_CALL_BAR_NANOSECONDS: f64 = 100.0;
    /// The name [`PROGRAMS_VARIABLE`] gives the system-call part.
    const SYSTEM_CALLS: &str = "syscalls";

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
            Ok(PLAIN) => {
                let path = image_path();
                assert_eq!(
                    path.extension().and_then(|e| e.to_str()),
                    Some(PLAIN),
                    "the plain run takes the copy plain_image wrote"
                );
                run_plain(&std::fs::read(path).expect("the program"))
            }
            Ok(NATIVE) => run_native(),
            Ok(UNTRANSLATED) => {
                run_untranslated(&std::fs::read(image_path()).expect("the program"))
            }
            Ok(other) => panic!("{RUN_VARIABLE}={other}: {PLAIN}, {NATIVE} or {UNTRANSLATED}"),
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

    /// Runs [`plain_image`]'s copy of the program at EL0 on the host: its segments at their addresses with their
    /// permissions, each breakpoint's handler answering the call whose `svc` it stands for.
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

    /// Writes beside `image` the plain run's copy of it, a breakpoint over every `svc` of its code, so that the
    /// plain run's own process decodes nothing.
    fn plain_image(image: &Path) -> PathBuf {
        let mut bytes = std::fs::read(image).expect("the program");
        let code: Vec<std::ops::Range<usize>> = {
            let elf = ElfFile64::<Endianness>::parse(&*bytes).expect("an ELF image");
            let endian = elf.endian();
            elf.elf_program_headers()
                .iter()
                .filter(|h| h.p_type(endian) == PT_LOAD && h.p_flags(endian) & PF_X != 0)
                .map(|h| {
                    let start = h.p_offset(endian) as usize;
                    start..start + h.p_filesz(endian) as usize
                })
                .collect()
        };
        let breakpoint = encode::exception(form::BRK, 0).to_le_bytes();
        for range in code {
            assert!(
                range.start.is_multiple_of(size_of::<u32>()),
                "code is word aligned"
            );
            for word in bytes[range].as_chunks_mut().0 {
                if matches!(decode(u32::from_le_bytes(*word)), Instruction::Syscall) {
                    *word = breakpoint;
                }
            }
        }
        let path = image.with_extension(PLAIN);
        std::fs::write(&path, bytes).expect("writing the plain run's image");
        path
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
        let policy = Policy {
            seed: DEFAULT_SEED,
            wall_start: REALTIME_NANOSECONDS,
            turn_events: None,
        };
        let outcome = native_at(&inside, look_ahead(), policy, &files, &starting);
        report(exit_code(outcome.status), &outcome.host.stdout)
    }

    /// Runs the program's own instructions at the guest's level in the machine, its segments mapped by the test
    /// program `native-untranslated`, which hands its write and exit calls here.
    fn run_untranslated(image: &[u8]) -> ! {
        let inside = std::fs::read(std::env::var_os(INSIDE_VARIABLE).expect("the inside's image"))
            .expect("the inside's image");
        let elf = ElfFile64::<Endianness>::parse(image).expect("an ELF image");
        let endian = elf.endian();
        let segments: Vec<_> = elf
            .elf_program_headers()
            .iter()
            .filter(|header| header.p_type(endian) == PT_LOAD)
            .collect();
        let (machine, memory, loaded, start) = machine(&inside, MEMORY, ONE_CPU);
        let mut cpu = machine.cpu().expect("a virtual CPU");
        boot(&mut cpu, &memory, &loaded, 0, start).expect("booting");
        let mut parts = Parts::default();
        let mut output = Vec::new();
        loop {
            match cpu.run().expect("running") {
                VcpuExit::Doorbell => {}
                other => panic!(
                    "the inside stopped: {other:?} at {:#x}",
                    cpu.get(Register::Pc).unwrap_or_default()
                ),
            }
            let link = link_page(&memory, &loaded, 0);
            let Some(bytes) = parts.request(link) else {
                continue;
            };
            let word = link.head.request;
            if let Some(request) = link::Request::from_word(word) {
                match request {
                    link::Request::Print => {
                        println!("inside: {}", String::from_utf8_lossy(link.data()))
                    }
                    link::Request::Fatal => {
                        let Fatal { esr, elr, far } = link.get();
                        panic!("the inside faulted: ESR {esr:#x} ELR {elr:#x} FAR {far:#x}");
                    }
                    request => panic!("no {request:?} here"),
                }
                continue;
            }
            match Request::from_word(word).expect("a request of the untranslated run") {
                Request::Segments => link.put(&Loadable {
                    segments: segments.len() as u64,
                    entry: elf.elf_header().e_entry(endian),
                    stack: u64::from(_STK_LIM),
                }),
                Request::Load => {
                    let Load { index } = link.get();
                    let header = segments[index as usize];
                    let flags = header.p_flags(endian);
                    link.put(&Segment {
                        address: header.p_vaddr(endian),
                        memory_length: header.p_memsz(endian),
                        file_length: header.p_filesz(endian),
                        writable: u64::from(flags & PF_W != 0),
                        executable: u64::from(flags & PF_X != 0),
                    });
                    let data = header.data(endian, image).expect("the segment's bytes");
                    parts.answer(link, data.to_vec());
                }
                Request::Output => {
                    output.extend_from_slice(&bytes);
                    parts.answer(link, Vec::new());
                }
                Request::Exited => {
                    let Exited { status } = link.get();
                    report(status as i32, &output)
                }
            }
        }
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
        let mut command = Command::new(std::env::current_exe().expect("this test's path"));
        // The loader's variables a build shell sets (a library path) make every child's start do work no plain
        // program does.
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with(LOADER_VARIABLE_PREFIX) {
                command.env_remove(name);
            }
        }
        let started = Instant::now();
        let out = command
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

    /// A plain run and a native run of one program, one after the other, with the untranslated run beside them.
    struct Pair {
        plain: Run,
        native: Run,
        untranslated: Run,
    }

    impl Pair {
        fn ratio(&self, of: impl Fn(&Run) -> f64) -> f64 {
            of(&self.native) / of(&self.plain)
        }

        fn untranslated_ratio(&self, of: impl Fn(&Run) -> f64) -> f64 {
            of(&self.untranslated) / of(&self.plain)
        }
    }

    /// The images of the test programs inside: the kernel's and the untranslated run's.
    struct Insides {
        kernel: PathBuf,
        untranslated: PathBuf,
    }

    impl Insides {
        fn of(&self, side: &str) -> &Path {
            if side == UNTRANSLATED {
                &self.untranslated
            } else {
                &self.kernel
            }
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
        plain_image: PathBuf,
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
    /// `raw`. The untranslated run goes last in a pair that starts plain and first in one that starts native.
    fn run_pairs(program: &mut Program, wanted: u64, insides: &Insides, raw: &mut std::fs::File) {
        let sides = [PLAIN, NATIVE, UNTRANSLATED];
        let image = |side: &str| {
            if side == PLAIN {
                &program.plain_image
            } else {
                &program.image
            }
        };
        if program.pairs.is_empty() {
            for side in sides {
                run(side, image(side), insides.of(side));
            }
        }
        while (program.pairs.len() as u64) < wanted {
            let index = program.pairs.len();
            let order = if index.is_multiple_of(2) {
                sides
            } else {
                [UNTRANSLATED, NATIVE, PLAIN]
            };
            let load = load_average();
            let mut runs: Vec<(&str, Run)> = order
                .into_iter()
                .map(|side| (side, run(side, image(side), insides.of(side))))
                .collect();
            let mut take = |side: &str| {
                let at = runs
                    .iter()
                    .position(|(s, _)| *s == side)
                    .expect("each side ran");
                runs.swap_remove(at).1
            };
            let (plain, native, untranslated) = (take(PLAIN), take(NATIVE), take(UNTRANSLATED));
            let expected = program
                .expected
                .get_or_insert_with(|| (plain.status, plain.output.clone()));
            for (side, run) in [
                (PLAIN, &plain),
                (NATIVE, &native),
                (UNTRANSLATED, &untranslated),
            ] {
                assert_eq!(
                    (&run.status, &run.output),
                    (&expected.0, &expected.1),
                    "{}: the {side} run's exit code and output",
                    program.name
                );
            }
            writeln!(
                raw,
                "{{\"program\":\"{}\",\"pair\":{index},\"first\":\"{}\",\"look_ahead\":\"{:?}\",\"load\":\"{load}\",\"plain\":{},\"native\":{},\"untranslated\":{}}}",
                program.name,
                order[0],
                look_ahead(),
                json(&plain),
                json(&native),
                json(&untranslated)
            )
            .expect("writing the raw pairs");
            program.pairs.push(Pair {
                plain,
                native,
                untranslated,
            });
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
        let untranslated = |of: fn(&Run) -> f64| {
            percent(median(
                &mut program
                    .pairs
                    .iter()
                    .map(|p| p.untranslated_ratio(of))
                    .collect::<Vec<_>>(),
            ))
        };
        let plain_cycles = median(
            &mut program
                .pairs
                .iter()
                .map(|p| cycles(&p.plain))
                .collect::<Vec<_>>(),
        );
        println!(
            "speed: {} over {} pairs, medians native against plain: {}; untranslated against plain: cycles {:+.1}%, instructions {:+.1}%; plain {plain_cycles:.0} cycles; on performance cores {:.0}% plain, {:.0}% native, {:.0}% untranslated",
            program.name,
            program.pairs.len(),
            medians.join(", "),
            untranslated(cycles),
            untranslated(|r| r.counts.instructions as f64),
            share(|p| &p.plain),
            share(|p| &p.native),
            share(|p| &p.untranslated),
        );
    }

    fn measure() {
        let insides = Insides {
            kernel: compiled(INSIDE_IMAGE),
            untranslated: compiled(UNTRANSLATED_IMAGE),
        };
        std::fs::write(&insides.kernel, program()).expect("writing the inside's image");
        std::fs::write(&insides.untranslated, build(UNTRANSLATED_PROGRAM))
            .expect("writing the untranslated run's image");
        let inside = &insides.kernel;
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
            .map(|(name, source, defines, in_bar)| {
                let image =
                    freestanding(PROGRAMS_DIRECTORY, name, source, &defines, Some(IMAGE_BASE));
                Program {
                    name: name.to_owned(),
                    plain_image: plain_image(&image),
                    image,
                    in_bar,
                    pairs: Vec::new(),
                    expected: None,
                }
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
        let compute = loop {
            for program in &mut programs {
                run_pairs(program, wanted, &insides, &mut raw);
                summarize(program);
            }
            if programs.iter().filter(|p| p.in_bar).count() != WORKLOADS.len() {
                println!("speed: not every workload ran, so no verdict on the compute part");
                break None;
            }
            let (estimate, lower, upper) = bar_estimate(&programs, seed, resamples);
            let said = verdict(percent(lower), percent(upper), bar);
            println!(
                "speed: geometric mean of the median cycles ratios {:+.2}%, {:.0}% interval [{:+.2} .. {:+.2}] (seed {seed:#x}, {resamples} resamples, {wanted} pairs), bar {bar:+.1}%, load {}: {said:?}",
                percent(estimate),
                CONFIDENCE * PERCENT,
                percent(lower),
                percent(upper),
                load_average()
            );
            if said != Verdict::Undecided || wanted * 2 > MOST_PAIRS {
                break Some(said);
            }
            wanted *= 2;
        };
        let system_calls =
            wanted_program(SYSTEM_CALLS).then(|| system_calls(inside, seed, resamples, &mut raw));
        for (part, said) in [("compute", compute), ("system calls", system_calls)] {
            match said {
                Some(Verdict::Fails) => panic!("speed: the {part} part of the bar fails"),
                Some(Verdict::Undecided) => panic!(
                    "speed: the {part} part is undecided at {MOST_PAIRS} pairs; run it on a quieter machine"
                ),
                Some(Verdict::Passes) | None => {}
            }
        }
    }

    /// What the decision rule makes of an interval against a bar.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Verdict {
        Passes,
        Fails,
        Undecided,
    }

    fn verdict(lower: f64, upper: f64, bar: f64) -> Verdict {
        if upper <= bar {
            Verdict::Passes
        } else if lower > bar {
            Verdict::Fails
        } else {
            Verdict::Undecided
        }
    }

    /// The kernel's own share of a getppid call, in nanoseconds, by layer 1's method: the calls handed straight to
    /// the kernel over a CPU side that runs no instructions, the first process being `image`.
    fn kernel_share(image: &[u8]) -> f64 {
        let mut cpu = BufferCpu::new();
        let mut host = TestHost::new(DEFAULT_SEED);
        let mut kernel = Kernel::new();
        add_files(
            &mut kernel,
            &[File {
                path: ANSWERED.to_owned(),
                mode: S_IRWXU,
                uid: 0,
                gid: 0,
                mtime: 0,
                kind: Kind::Regular(image.to_vec()),
            }],
        );
        let (_, starting) = one_program(ANSWERED, image.to_vec(), &[]);
        let arguments: Vec<&[u8]> = starting.arguments.iter().map(Vec::as_slice).collect();
        let environment: Vec<&[u8]> = starting.environment.iter().map(Vec::as_slice).collect();
        kernel.start(
            &mut cpu,
            &mut host,
            &Start {
                path: &starting.path,
                arguments: &arguments,
                environment: &environment,
                working_directory: &starting.working_directory,
                hostname: &starting.hostname,
                groups: &starting.groups,
                backend: None,
            },
        );
        assert_eq!(kernel.start_failure(), None, "starting {ANSWERED}");
        let next = kernel.next();
        assert!(
            matches!(next, Next::Run { .. }),
            "{ANSWERED} runs: {next:?}"
        );
        let call = Exit::Syscall(Syscall {
            number: __NR_getppid.into(),
            arguments: Default::default(),
        });
        let started = Instant::now();
        for _ in 0..SYSTEM_CALLS_MADE {
            kernel.exit(&mut cpu, &mut host, call);
            match kernel.next() {
                Next::Run {
                    resume: Resume::Return(_),
                    ..
                } => {}
                other => panic!("getppid answered with {other:?}"),
            }
        }
        started.elapsed().as_nanos() as f64 / SYSTEM_CALLS_MADE as f64
    }

    /// One pair of the system-call part: the per-call time of the native runs, and the kernel's share beside it,
    /// in nanoseconds.
    struct CallPair {
        per_call: f64,
        kernel: f64,
    }

    /// The back end's share of a call over `pairs`: the median per-call time less the kernel's share's median.
    fn back_end_share(pairs: &[&CallPair]) -> f64 {
        let mut per_call: Vec<f64> = pairs.iter().map(|p| p.per_call).collect();
        let mut kernel: Vec<f64> = pairs.iter().map(|p| p.kernel).collect();
        median(&mut per_call) - median(&mut kernel)
    }

    /// The system-call part of the bar (section 8): `answered`'s getppid calls in the kernel inside against the same
    /// program with a nop in each call's place, in alternating pairs, each pair's difference of wall times over the
    /// calls made being the per-call time, with the kernel's own share measured beside each pair; the back end's
    /// share is the per-call time less the kernel's, with a seeded bootstrap's interval over the pairs.
    fn system_calls(inside: &Path, seed: u64, resamples: u64, raw: &mut std::fs::File) -> Verdict {
        let build = |name: &str, nothing: bool| {
            let mut defines = vec![
                format!("CALLS={SYSTEM_CALLS_MADE}"),
                format!("GETPPID={__NR_getppid}"),
                format!("EXIT_GROUP={__NR_exit_group}"),
            ];
            defines.extend(nothing.then(|| NOTHING_DEFINE.to_owned()));
            freestanding(OWN_GUEST, name, ANSWERED, &defines, None)
        };
        let answered = build(ANSWERED, false);
        let nothing = build(NOTHING, true);
        let image = std::fs::read(&nothing).expect("the built program");
        for side in [&answered, &nothing] {
            run(NATIVE, side, inside);
        }
        let mut pairs: Vec<CallPair> = Vec::new();
        let mut wanted = setting(PAIRS_VARIABLE, DEFAULT_PAIRS);
        loop {
            while (pairs.len() as u64) < wanted {
                let index = pairs.len();
                let order = if index.is_multiple_of(2) {
                    [&answered, &nothing]
                } else {
                    [&nothing, &answered]
                };
                let load = load_average();
                let [first, second] = order.map(|image| run(NATIVE, image, inside));
                let (calls, none) = if order[0] == &answered {
                    (first, second)
                } else {
                    (second, first)
                };
                // Only answered calls give the loop a parent to add up, and its exit status says whether it did.
                assert_eq!(
                    (calls.status, none.status),
                    (i32::from(true), i32::from(false)),
                    "{ANSWERED}'s exit codes"
                );
                let per_call = (calls.wall_nanoseconds as f64 - none.wall_nanoseconds as f64)
                    / SYSTEM_CALLS_MADE as f64;
                let kernel = kernel_share(&image);
                writeln!(
                    raw,
                    "{{\"program\":\"{SYSTEM_CALLS}\",\"pair\":{index},\"load\":\"{load}\",\"calls\":{},\"nothing\":{},\"kernel_ns\":{kernel}}}",
                    json(&calls),
                    json(&none)
                )
                .expect("writing the raw pairs");
                pairs.push(CallPair { per_call, kernel });
            }
            let all: Vec<&CallPair> = pairs.iter().collect();
            let estimate = back_end_share(&all);
            let mut random = fastrand::Rng::with_seed(seed);
            let mut shares: Vec<f64> = (0..resamples)
                .map(|_| {
                    let drawn: Vec<&CallPair> = (0..pairs.len())
                        .map(|_| &pairs[random.usize(..pairs.len())])
                        .collect();
                    back_end_share(&drawn)
                })
                .collect();
            shares.sort_by(f64::total_cmp);
            let tail = (1.0 - CONFIDENCE) / 2.0;
            let at = |share: f64| shares[((shares.len() - 1) as f64 * share).round() as usize];
            let (lower, upper) = (at(tail), at(1.0 - tail));
            let said = verdict(lower, upper, SYSTEM_CALL_BAR_NANOSECONDS);
            let mut per_call: Vec<f64> = pairs.iter().map(|p| p.per_call).collect();
            let mut kernel: Vec<f64> = pairs.iter().map(|p| p.kernel).collect();
            println!(
                "speed: system calls over {} pairs: {:.1} ns a call (median), the kernel's share {:.1} ns (median), the back end's {estimate:.1} ns, {:.0}% interval [{lower:.1} .. {upper:.1}] (seed {seed:#x}, {resamples} resamples), bar {SYSTEM_CALL_BAR_NANOSECONDS} ns, load {}: {said:?}",
                pairs.len(),
                median(&mut per_call),
                median(&mut kernel),
                CONFIDENCE * PERCENT,
                load_average()
            );
            if said != Verdict::Undecided || wanted * 2 > MOST_PAIRS {
                return said;
            }
            wanted *= 2;
        }
    }
}
