//! The native reference runner, for real ARM64 Linux.
//!
//! `wcontainers-reference cases` reads layer 1 cases on stdin and writes results on stdout. Each case runs one
//! instruction with every register exact: a signal handler rewrites its own frame to the guest's registers
//! and returns into the instruction; wherever control then lands raises the next signal, whose frame holds
//! the registers after it. System calls made from the instruction are caught by seccomp and never run.
//!
//! `wcontainers-reference blobs` runs flat blobs, as `SERVING_MODE` describes, each from its first byte with all
//! registers zero and the starting stack of `initial_stack`, for real; its output and exit status are the
//! program's.
//!
//! `wcontainers-reference time <runs> <file> <load address> <stack top> <stack bytes>` runs the blob that many
//! times, each in a child process, and writes each run's time in seconds to stderr: from just before the child
//! is made until it has ended, so the reference's own start-up is not counted.

#![allow(clippy::missing_safety_doc)]

use core::ptr::{addr_of, addr_of_mut};
use std::io::{BufRead, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};

include!("../../src/layout.rs");

const PAGE: u64 = 4096;
/// What a served blob reads as its stdin.
const EMPTY_INPUT: &str = "/dev/null";
const NZCV: u64 = 0xf000_0000;

/// The registers the guest starts with: x0 to x30, sp, pc, pstate, thread pointer.
static mut GUEST: [u64; LANDED_REGISTERS] = [0; LANDED_REGISTERS];
/// The registers where control landed, in the same order.
static mut LANDED: [u64; LANDED_REGISTERS] = [0; LANDED_REGISTERS];
/// v0 to v31 the guest starts with, and where control landed.
static mut GUEST_VECTORS: [u128; VECTOR_REGISTERS] = [0; VECTOR_REGISTERS];
static mut LANDED_VECTORS: [u128; VECTOR_REGISTERS] = [0; VECTOR_REGISTERS];
/// FPSR and FPCR the guest starts with, and where control landed.
static mut GUEST_FLOAT: [u32; FLOAT_REGISTERS] = [0; FLOAT_REGISTERS];
static mut LANDED_FLOAT: [u32; FLOAT_REGISTERS] = [0; FLOAT_REGISTERS];
static mut SIGNAL: [u64; 3] = [0; 3];
static mut HOST: core::mem::MaybeUninit<libc::mcontext_t> = core::mem::MaybeUninit::uninit();
static mut HOST_THREAD_POINTER: u64 = 0;

fn thread_pointer() -> u64 {
    let value: u64;
    unsafe { core::arch::asm!("mrs {}, tpidr_el0", out(reg) value) };
    value
}

fn set_thread_pointer(value: u64) {
    unsafe { core::arch::asm!("msr tpidr_el0, {}", in(reg) value) };
}

/// v0 to v31 in a signal frame: in the FPSIMD record that starts the frame's extra space, which Linux puts
/// 288 bytes into `uc_mcontext` (`struct sigcontext`).
unsafe fn frame_vectors(uc: *mut libc::ucontext_t) -> *mut [u128; VECTOR_REGISTERS] {
    const FPSIMD_MAGIC: u32 = 0x4650_8001;
    unsafe {
        let record = (&raw mut (*uc).uc_mcontext as *mut u8).add(288);
        assert_eq!(
            (record as *const u32).read(),
            FPSIMD_MAGIC,
            "the signal frame starts with the FPSIMD record"
        );
        record.add(16) as *mut [u128; VECTOR_REGISTERS]
    }
}

/// FPSR and FPCR in a signal frame: right after the FPSIMD record's header.
unsafe fn frame_float(uc: *mut libc::ucontext_t) -> *mut [u32; FLOAT_REGISTERS] {
    unsafe {
        (frame_vectors(uc) as *mut u8).sub(size_of::<[u32; FLOAT_REGISTERS]>())
            as *mut [u32; FLOAT_REGISTERS]
    }
}

/// Entry: keep the host's frame, put the guest's registers in its place.
extern "C" fn enter(_: libc::c_int, _: *mut libc::siginfo_t, context: *mut libc::c_void) {
    unsafe {
        let uc = &mut *(context as *mut libc::ucontext_t);
        (*addr_of_mut!(HOST)).write(core::ptr::read(&uc.uc_mcontext));
        HOST_THREAD_POINTER = thread_pointer();
        let guest = &*addr_of!(GUEST);
        uc.uc_mcontext
            .regs
            .copy_from_slice(&guest[..GENERAL_REGISTERS]);
        uc.uc_mcontext.sp = guest[CASE_SP];
        uc.uc_mcontext.pc = guest[LANDED_PC];
        uc.uc_mcontext.pstate = (uc.uc_mcontext.pstate & !NZCV) | (guest[LANDED_FLAGS] & NZCV);
        frame_vectors(uc).write_unaligned(*addr_of!(GUEST_VECTORS));
        frame_float(uc).write_unaligned(*addr_of!(GUEST_FLOAT));
        set_thread_pointer(guest[LANDED_THREAD_POINTER]);
    }
}

/// Exit: record where control landed, then put the host's frame back.
extern "C" fn land(signal: libc::c_int, info: *mut libc::siginfo_t, context: *mut libc::c_void) {
    unsafe {
        let guest_thread_pointer = thread_pointer();
        set_thread_pointer(HOST_THREAD_POINTER);
        let uc = &mut *(context as *mut libc::ucontext_t);
        let landed = &mut *addr_of_mut!(LANDED);
        landed[..GENERAL_REGISTERS].copy_from_slice(&uc.uc_mcontext.regs);
        landed[CASE_SP] = uc.uc_mcontext.sp;
        landed[LANDED_PC] = uc.uc_mcontext.pc;
        landed[LANDED_FLAGS] = uc.uc_mcontext.pstate & NZCV;
        landed[LANDED_THREAD_POINTER] = guest_thread_pointer;
        *addr_of_mut!(LANDED_VECTORS) = frame_vectors(uc).read_unaligned();
        *addr_of_mut!(LANDED_FLOAT) = frame_float(uc).read_unaligned();
        SIGNAL = [
            signal as u64,
            (*info).si_code as u64,
            (*info).si_addr() as u64,
        ];
        uc.uc_mcontext = (*addr_of!(HOST)).assume_init_read();
    }
}

fn check(result: libc::c_int, what: &str) {
    if result != 0 {
        panic!("{what}: {}", std::io::Error::last_os_error());
    }
}

fn map(address: u64, length: u64, protection: libc::c_int) {
    let at = unsafe {
        libc::mmap(
            address as *mut libc::c_void,
            length as usize,
            protection,
            libc::MAP_PRIVATE
                | libc::MAP_ANONYMOUS
                | libc::MAP_FIXED_NOREPLACE
                | libc::MAP_NORESERVE,
            -1,
            0,
        )
    };
    if at as u64 != address {
        let error = std::io::Error::last_os_error();
        let maps = std::fs::read_to_string("/proc/self/maps").unwrap_or_default();
        panic!("map {address:#x}: {error}\n{maps}");
    }
}

fn protect(address: u64, length: u64, protection: libc::c_int) {
    check(
        unsafe { libc::mprotect(address as *mut libc::c_void, length as usize, protection) },
        "mprotect",
    );
}

fn handle(
    signal: libc::c_int,
    handler: extern "C" fn(libc::c_int, *mut libc::siginfo_t, *mut libc::c_void),
) {
    unsafe {
        let mut action: libc::sigaction = core::mem::zeroed();
        action.sa_sigaction = handler as usize;
        action.sa_flags = libc::SA_SIGINFO | libc::SA_ONSTACK;
        check(
            libc::sigaction(signal, &action, core::ptr::null_mut()),
            "sigaction",
        );
    }
}

fn alternate_stack() {
    const SIZE: usize = 1 << 16;
    let memory = Box::leak(vec![0u8; SIZE].into_boxed_slice());
    let stack = libc::stack_t {
        ss_sp: memory.as_mut_ptr() as *mut libc::c_void,
        ss_flags: 0,
        ss_size: SIZE,
    };
    check(
        unsafe { libc::sigaltstack(&stack, core::ptr::null_mut()) },
        "sigaltstack",
    );
}

/// Make every system call made from the instruction under test raise SIGSYS instead of running.
fn trap_test_system_calls() {
    const LOW: u32 = 8;
    const HIGH: u32 = 12;
    let statement = |code: u16, jt: u8, jf: u8, k: u32| libc::sock_filter { code, jt, jf, k };
    let load = (libc::BPF_LD | libc::BPF_W | libc::BPF_ABS) as u16;
    let jeq = (libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K) as u16;
    let jge = (libc::BPF_JMP | libc::BPF_JGE | libc::BPF_K) as u16;
    let ret = (libc::BPF_RET | libc::BPF_K) as u16;
    let filter = [
        statement(load, 0, 0, HIGH),
        statement(jeq, 0, 3, (CODE >> 32) as u32),
        statement(load, 0, 0, LOW),
        statement(jge, 0, 1, CODE as u32),
        statement(jge, 0, 1, CODE as u32 + 8),
        statement(ret, 0, 0, libc::SECCOMP_RET_ALLOW),
        statement(ret, 0, 0, libc::SECCOMP_RET_TRAP),
    ];
    let program = libc::sock_fprog {
        len: filter.len() as u16,
        filter: filter.as_ptr() as *mut libc::sock_filter,
    };
    unsafe {
        check(
            libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0),
            "no new privileges",
        );
        check(
            libc::prctl(
                libc::PR_SET_SECCOMP,
                libc::SECCOMP_MODE_FILTER,
                &program as *const _ as usize,
                0,
                0,
            ),
            "seccomp",
        );
    }
}

/// Make the instruction just written to the code page visible to instruction fetch.
fn synchronise(address: u64) {
    unsafe {
        core::arch::asm!(
            "dc cvau, {0}", "dsb ish", "ic ivau, {0}", "dsb ish", "isb",
            in(reg) address,
        )
    };
}

fn timer(microseconds: i64) {
    let value = libc::itimerval {
        it_interval: libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
        it_value: libc::timeval {
            tv_sec: 0,
            tv_usec: microseconds as _,
        },
    };
    check(
        unsafe { libc::setitimer(libc::ITIMER_REAL, &value, core::ptr::null_mut()) },
        "setitimer",
    );
}

fn word(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..][..size_of::<u64>()].try_into().unwrap())
}

fn cases() {
    map(DATA, DATA_BYTES, libc::PROT_READ | libc::PROT_WRITE);
    map(
        CODE - CODE_SPAN,
        2 * CODE_SPAN,
        libc::PROT_READ | libc::PROT_EXEC,
    );
    alternate_stack();
    handle(libc::SIGUSR1, enter);
    for signal in [
        libc::SIGILL,
        libc::SIGSEGV,
        libc::SIGBUS,
        libc::SIGTRAP,
        libc::SIGSYS,
        libc::SIGFPE,
        libc::SIGALRM,
    ] {
        handle(signal, land);
    }
    trap_test_system_calls();

    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let mut request = [0u8; REQUEST_BYTES];
    let mut expected = vec![0u8; DATA_BYTES as usize];
    while input.read_exact(&mut request).is_ok() {
        let instruction = u32::from_le_bytes(request[..4].try_into().unwrap());
        let seed = word(&request, REQUEST_SEED_AT);
        fill(seed, &mut expected);
        protect(
            DATA + DATA_WRITABLE,
            DATA_READ_ONLY,
            libc::PROT_READ | libc::PROT_WRITE,
        );
        unsafe {
            core::ptr::copy_nonoverlapping(expected.as_ptr(), DATA as *mut u8, expected.len())
        };
        protect(DATA + DATA_WRITABLE, DATA_READ_ONLY, libc::PROT_READ);
        protect(CODE, PAGE, libc::PROT_READ | libc::PROT_WRITE);
        unsafe { (CODE as *mut u32).write_volatile(instruction) };
        protect(CODE, PAGE, libc::PROT_READ | libc::PROT_EXEC);
        synchronise(CODE);

        unsafe {
            let guest = &mut *addr_of_mut!(GUEST);
            let register = |i: usize| word(&request, REQUEST_REGISTERS_AT + size_of::<u64>() * i);
            for (i, value) in guest[..=CASE_SP].iter_mut().enumerate() {
                *value = register(i);
            }
            guest[LANDED_PC] = CODE;
            guest[LANDED_FLAGS] = register(REQUEST_FLAGS);
            guest[LANDED_THREAD_POINTER] = register(REQUEST_THREAD_POINTER);
            let vectors = &mut *addr_of_mut!(GUEST_VECTORS);
            for (i, value) in vectors.iter_mut().enumerate() {
                let at = REQUEST_VECTORS_AT + size_of::<u128>() * i;
                *value =
                    u128::from_le_bytes(request[at..][..size_of::<u128>()].try_into().unwrap());
            }
            for (i, value) in (*addr_of_mut!(GUEST_FLOAT)).iter_mut().enumerate() {
                let at = REQUEST_FLOAT_AT + size_of::<u32>() * i;
                *value = u32::from_le_bytes(request[at..][..size_of::<u32>()].try_into().unwrap());
            }
            timer(500_000);
            libc::raise(libc::SIGUSR1);
            timer(0);
        }

        let data = unsafe { core::slice::from_raw_parts(DATA as *const u8, DATA_BYTES as usize) };
        let changed: Vec<(u32, u8)> = data
            .iter()
            .zip(&expected)
            .enumerate()
            .filter(|(_, (now, before))| now != before)
            .map(|(i, (now, _))| (i as u32, *now))
            .collect();
        let mut result = Vec::with_capacity(RESULT_HEADER_BYTES + CHANGE_BYTES * changed.len());
        let signal = unsafe { *addr_of!(SIGNAL) };
        result.extend_from_slice(&(signal[0] as u32).to_le_bytes());
        result.extend_from_slice(&(signal[1] as u32).to_le_bytes());
        result.extend_from_slice(&signal[2].to_le_bytes());
        for value in unsafe { *addr_of!(LANDED) } {
            result.extend_from_slice(&value.to_le_bytes());
        }
        for value in unsafe { *addr_of!(LANDED_VECTORS) } {
            result.extend_from_slice(&value.to_le_bytes());
        }
        for value in unsafe { *addr_of!(LANDED_FLOAT) } {
            result.extend_from_slice(&value.to_le_bytes());
        }
        result.extend_from_slice(&(changed.len() as u32).to_le_bytes());
        for (offset, value) in changed {
            result.extend_from_slice(&offset.to_le_bytes());
            result.push(value);
        }
        output.write_all(&result).unwrap();
        output.flush().unwrap();
    }
}

/// The blob ran out of time: the status of a process killed by SIGALRM.
extern "C" fn timed_out(_: libc::c_int, _: *mut libc::siginfo_t, _: *mut libc::c_void) {
    unsafe { libc::_exit(SIGNALLED + libc::SIGALRM) };
}

/// Run a flat blob at `load`; `headers` is where its program headers are and how many.
fn blob(path: &str, load: u64, stack_top: u64, stack_bytes: u64, headers: Option<(u64, u64)>) {
    let bytes = std::fs::read(path).expect("read the blob");
    let length = (bytes.len() as u64).next_multiple_of(PAGE);
    // One mapping for code and data, as a flat blob needs.
    map(
        load,
        length,
        libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
    );
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), load as *mut u8, bytes.len()) };
    let mut address = load;
    while address < load + length {
        synchronise(address);
        address += 64;
    }
    map(
        stack_top - stack_bytes,
        stack_bytes,
        libc::PROT_READ | libc::PROT_WRITE,
    );
    let (sp, stack) = initial_stack(stack_top, headers);
    unsafe { core::ptr::copy_nonoverlapping(stack.as_ptr(), sp as *mut u8, stack.len()) };
    handle(libc::SIGUSR1, enter);
    unsafe {
        let guest = &mut *addr_of_mut!(GUEST);
        *guest = [0; LANDED_REGISTERS];
        guest[CASE_SP] = sp;
        guest[LANDED_PC] = load;
        // A program that does not end in this time ends as if killed by SIGALRM (random programs may
        // loop). In Docker the runner is the first process, which a signal's default action never kills,
        // so a handler ends it.
        handle(libc::SIGALRM, timed_out);
        libc::alarm(BLOB_SECONDS);
        libc::raise(libc::SIGUSR1);
    }
    unreachable!("the blob returned without exiting");
}

/// Serve blobs (`SERVING_MODE`) until stdin ends.
fn serve_blobs() {
    let mut requests = std::io::stdin().lock();
    let mut answers = std::io::stdout().lock();
    let mut request = Vec::new();
    loop {
        request.clear();
        if requests
            .read_until(REQUEST_END, &mut request)
            .expect("read a request")
            == 0
        {
            return;
        }
        let arguments: Vec<String> = request
            .strip_suffix(&[REQUEST_END])
            .expect("a whole request")
            .split(|&byte| byte == REQUEST_SEPARATOR)
            .map(|argument| String::from_utf8_lossy(argument).into_owned())
            .collect();
        // As in a container of its own: stdin empty, stdout and stderr pipes.
        let [stdout, stderr] = [(); 2].map(|()| {
            let mut ends = [0; 2];
            let made = unsafe { libc::pipe2(ends.as_mut_ptr(), libc::O_CLOEXEC) };
            assert_eq!(
                made,
                0,
                "a pipe for the output: {}",
                std::io::Error::last_os_error()
            );
            ends.map(|end| unsafe { std::fs::File::from_raw_fd(end) })
        });
        let nothing = std::fs::File::open(EMPTY_INPUT).expect("open the empty input");
        let child = unsafe { libc::fork() };
        if child == 0 {
            let [[_, out], [_, errors]] = [&stdout, &stderr];
            for (from, to) in [
                (nothing.as_raw_fd(), libc::STDIN_FILENO),
                (out.as_raw_fd(), libc::STDOUT_FILENO),
                (errors.as_raw_fd(), libc::STDERR_FILENO),
            ] {
                let made = unsafe { libc::dup2(from, to) };
                assert_eq!(made, to, "dup2: {}", std::io::Error::last_os_error());
            }
            run_blob(&arguments);
        }
        assert!(child > 0, "fork: {}", std::io::Error::last_os_error());
        let [outputs, errors] = [stdout, stderr].map(|[mut read, write]| {
            drop(write);
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                read.read_to_end(&mut bytes)
                    .expect("read the blob's output");
                bytes
            })
        });
        let mut status = libc::c_int::default();
        assert_eq!(unsafe { libc::waitpid(child, &mut status, 0) }, child);
        let status = if libc::WIFSIGNALED(status) {
            SIGNALLED + libc::WTERMSIG(status)
        } else {
            libc::WEXITSTATUS(status)
        };
        answers.write_all(&status.to_le_bytes()).unwrap();
        for output in [outputs, errors] {
            let bytes = output.join().expect("the output's reader");
            let length = u32::try_from(bytes.len()).expect("output that fits its length");
            answers.write_all(&length.to_le_bytes()).unwrap();
            answers.write_all(&bytes).unwrap();
        }
        answers.flush().unwrap();
    }
}

/// Run a blob from its arguments: file, load address, stack top, stack bytes, and optional program headers.
fn run_blob(arguments: &[String]) -> ! {
    let [path, load, stack_top, stack_bytes, headers_and_count @ ..] = arguments else {
        panic!("a blob's arguments: {arguments:?}");
    };
    blob(
        path,
        number(load),
        number(stack_top),
        number(stack_bytes),
        headers(headers_and_count),
    );
    unreachable!("the blob returned without exiting");
}

fn time(
    runs: u32,
    path: &str,
    load: u64,
    stack_top: u64,
    stack_bytes: u64,
    headers: Option<(u64, u64)>,
) {
    for _ in 0..runs {
        let start = std::time::Instant::now();
        match unsafe { libc::fork() } {
            0 => blob(path, load, stack_top, stack_bytes, headers),
            -1 => panic!("fork failed"),
            child => {
                let mut status = 0;
                assert_eq!(unsafe { libc::waitpid(child, &mut status, 0) }, child);
                eprintln!("{:.6}", start.elapsed().as_secs_f64());
            }
        }
    }
}

/// The optional address and count of a blob's program headers.
fn headers(arguments: &[String]) -> Option<(u64, u64)> {
    match arguments {
        [at, count] => Some((number(at), count.parse().expect("a count"))),
        _ => None,
    }
}

fn number(text: &str) -> u64 {
    u64::from_str_radix(text.trim_start_matches("0x"), 16).expect("a hexadecimal number")
}

fn main() {
    // A failure of the runner itself ends with status 101 after this message on stderr, which `programs::native`
    // fails the case on. (An abort is not enough: in Docker the runner is the first process, which ignores its
    // own SIGABRT, so musl's abort ends it with SIGSEGV, as if the guest had faulted.)
    std::panic::set_hook(Box::new(|info| {
        eprintln!("{}: {info}", env!("CARGO_PKG_NAME"));
        unsafe { libc::_exit(101) };
    }));
    let arguments: Vec<String> = std::env::args().collect();
    match arguments.get(1).map(String::as_str) {
        Some("cases") => cases(),
        Some(SERVING_MODE) => serve_blobs(),
        Some("time") => time(
            arguments[2].parse().expect("a number of runs"),
            &arguments[3],
            number(&arguments[4]),
            number(&arguments[5]),
            number(&arguments[6]),
            headers(&arguments[7..]),
        ),
        _ => panic!(
            "usage: {} cases | {SERVING_MODE} | time <runs> <file> <load> <stack top> <stack bytes> [<headers> <count>]",
            env!("CARGO_PKG_NAME")
        ),
    }
}
