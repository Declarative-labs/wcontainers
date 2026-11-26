//! The native reference runner, for real Linux on the guest's CPU: ARM64 (in Docker on an ARM64 host) or
//! x86-64 (over SSH on an x86-64 machine). One program; its per-CPU part is the module `cfg(target_arch)`
//! chooses, `machine`, whose `Cpu` gives the same items for each CPU.
//!
//! `wcontainers-reference serve` speaks the wire of `layout.rs` on stdin and stdout until stdin ends.
//!
//! A case runs one instruction with every register exact: a signal handler rewrites its own frame to the
//! case's state and returns into the instruction; wherever control then stops raises the next signal, whose
//! frame holds the state after it. System calls made from the instruction are caught by seccomp and never
//! run. Cases run in a child process that serves them until one damages it; a case that ends the child is
//! reported as the runner's failure and the next case gets a new child.
//!
//! A blob runs in a child of its own from its first byte with the blob start state and the starting stack of
//! `initial_stack`, for real; its output and exit status are the program's. When a signal ends it, its
//! signal, `si_code`, pc and general registers come back too. Timed blobs run that many times, each in a
//! child, timed from just before the child is made until it has ended.
//!
//! A program is an ordinary static executable, run as Linux starts one in a child of its own; its output and
//! exit status come back as a blob's.

#![allow(clippy::missing_safety_doc)]

use core::ptr::{addr_of, addr_of_mut};
use std::io::{Read, Write};

include!("../../src/layout.rs");

#[cfg(target_arch = "aarch64")]
#[path = "aarch64.rs"]
mod machine;
#[cfg(target_arch = "x86_64")]
#[path = "x86_64.rs"]
mod machine;
use machine::Cpu;

const PAGE: u64 = 4096;
/// How long a case may run before SIGALRM stops it.
const CASE_MICROSECONDS: i64 = 500_000;
/// The most state fields of any guest.
const MAX_FIELDS: usize = 80;
/// The most general registers a blob's stop record holds.
const MAX_STOP_REGISTERS: usize = 40;

/// The state a case starts from, and the state where control stopped.
static mut GUEST: [u128; MAX_FIELDS] = [0; MAX_FIELDS];
static mut LANDED: [u128; MAX_FIELDS] = [0; MAX_FIELDS];
/// The stopping signal, its `si_code` and `si_addr`.
static mut SIGNAL: [u64; 3] = [0; 3];
/// The case's instruction, for `Cpu::unfinished`.
static mut CODE_BYTES: [u8; 16] = [0; 16];
static mut CODE_LENGTH: usize = 0;
/// Steps left for an unfinished instruction.
static mut STEPS_LEFT: u64 = 0;
/// What Linux refused to set up for the case, if anything.
static mut REFUSED: Option<&'static str> = None;

/// Entry: keep the runner's frame, put the case's state in its place.
extern "C" fn enter(_: libc::c_int, _: *mut libc::siginfo_t, context: *mut libc::c_void) {
    unsafe {
        let fields = Cpu::FIELDS.len();
        REFUSED = Cpu::enter(
            context as *mut libc::ucontext_t,
            &(&*addr_of!(GUEST))[..fields],
        )
        .err();
    };
}

/// Exit: step on through an unfinished instruction, or record where control stopped and put the runner's
/// frame back.
extern "C" fn land(signal: libc::c_int, info: *mut libc::siginfo_t, context: *mut libc::c_void) {
    unsafe {
        let uc = context as *mut libc::ucontext_t;
        let code = &(&*addr_of!(CODE_BYTES))[..CODE_LENGTH];
        if STEPS_LEFT > 0 && signal == libc::SIGTRAP && Cpu::unfinished(code, uc, info) {
            STEPS_LEFT -= 1;
            return;
        }
        let fields = Cpu::FIELDS.len();
        Cpu::leave(
            code,
            &(&*addr_of!(GUEST))[..fields],
            uc,
            info,
            &mut (&mut *addr_of_mut!(LANDED))[..fields],
        );
        SIGNAL = [
            signal as u64,
            (*info).si_code as u64,
            (*info).si_addr() as u64,
        ];
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
    flags: libc::c_int,
) {
    unsafe {
        let mut action: libc::sigaction = core::mem::zeroed();
        action.sa_sigaction = handler as usize;
        action.sa_flags = libc::SA_SIGINFO | libc::SA_ONSTACK | flags;
        libc::sigfillset(&mut action.sa_mask);
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

/// Make every system call made from the instruction window, and every call of another architecture made
/// anywhere, raise SIGSYS instead of running.
fn trap_test_system_calls() {
    const ARCH: u32 = 4;
    const LOW: u32 = 8;
    const HIGH: u32 = 12;
    let statement = |code: u16, jt: u8, jf: u8, k: u32| libc::sock_filter { code, jt, jf, k };
    let load = (libc::BPF_LD | libc::BPF_W | libc::BPF_ABS) as u16;
    let jeq = (libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K) as u16;
    let jge = (libc::BPF_JMP | libc::BPF_JGE | libc::BPF_K) as u16;
    let ret = (libc::BPF_RET | libc::BPF_K) as u16;
    let filter = [
        statement(load, 0, 0, ARCH),
        statement(jeq, 1, 0, Cpu::AUDIT_ARCH),
        statement(ret, 0, 0, libc::SECCOMP_RET_TRAP),
        statement(load, 0, 0, HIGH),
        statement(jeq, 0, 3, (CODE >> 32) as u32),
        statement(load, 0, 0, LOW),
        statement(jge, 0, 1, CODE as u32),
        statement(jge, 0, 1, (CODE + Cpu::WINDOW) as u32),
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

/// Reads a request's fields in order.
struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take(&mut self, n: usize) -> &[u8] {
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        head
    }
    fn u8(&mut self) -> u8 {
        self.take(1)[0]
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take(8).try_into().unwrap())
    }
    fn u128(&mut self) -> u128 {
        u128::from_le_bytes(self.take(16).try_into().unwrap())
    }
}

fn read_record(from: &mut impl Read) -> Option<Vec<u8>> {
    let mut length = [0u8; 4];
    from.read_exact(&mut length).ok()?;
    let mut record = vec![0u8; u32::from_le_bytes(length) as usize];
    from.read_exact(&mut record).ok()?;
    Some(record)
}

fn write_record(to: &mut impl Write, record: &[u8]) {
    to.write_all(&(record.len() as u32).to_le_bytes()).unwrap();
    to.write_all(record).unwrap();
    to.flush().unwrap();
}

fn failed(message: &str) -> Vec<u8> {
    let mut result = vec![RESULT_FAILED];
    result.extend_from_slice(message.as_bytes());
    result
}

/// The case child: sets up the areas, the handlers and the filter once, then runs the cases it is sent.
fn serve_cases(mut input: std::fs::File, mut output: std::fs::File) -> ! {
    let fields = Cpu::FIELDS.len();
    // The runner's image and heap must be where the harness keeps cases from reaching.
    let image = serve_cases as *const () as u64;
    let data = addr_of!(GUEST) as u64;
    let heap = unsafe { libc::sbrk(0) } as u64;
    for at in [image, data, heap] {
        assert!(
            Cpu::RUNNER.contains(&at),
            "the runner's own memory at {at:#x} is outside {:#x?}",
            Cpu::RUNNER
        );
    }
    map(DATA, DATA_BYTES, libc::PROT_READ | libc::PROT_WRITE);
    let area = CODE - Cpu::CODE_SPAN;
    map(area, 2 * Cpu::CODE_SPAN, libc::PROT_READ | libc::PROT_WRITE);
    if Cpu::FILLER != 0 {
        unsafe {
            core::ptr::write_bytes(area as *mut u8, Cpu::FILLER, 2 * Cpu::CODE_SPAN as usize)
        };
    }
    protect(area, 2 * Cpu::CODE_SPAN, libc::PROT_READ | libc::PROT_EXEC);
    alternate_stack();
    handle(libc::SIGUSR1, enter, 0);
    for signal in [
        libc::SIGILL,
        libc::SIGSEGV,
        libc::SIGBUS,
        libc::SIGTRAP,
        libc::SIGSYS,
        libc::SIGFPE,
        libc::SIGALRM,
    ] {
        handle(signal, land, 0);
    }
    trap_test_system_calls();

    let mut expected = vec![0u8; DATA_BYTES as usize];
    while let Some(request) = read_record(&mut input) {
        let mut request = Reader(&request[1..]);
        let length = request.u8() as usize;
        let code = request.take(length).to_vec();
        let seed = request.u64();
        fill(seed, &mut expected);
        let at = request.u32();
        let count = request.u8() as usize;
        place(&(at, request.take(count).to_vec()), &mut expected);
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
        unsafe {
            core::ptr::write_bytes(CODE as *mut u8, Cpu::FILLER, 16);
            core::ptr::copy_nonoverlapping(code.as_ptr(), CODE as *mut u8, code.len());
        }
        protect(CODE, PAGE, libc::PROT_READ | libc::PROT_EXEC);
        Cpu::synchronise(CODE, 16);

        unsafe {
            let guest = &mut *addr_of_mut!(GUEST);
            for value in &mut guest[..fields] {
                *value = request.u128();
            }
            (&mut *addr_of_mut!(CODE_BYTES))[..length].copy_from_slice(&code);
            CODE_LENGTH = length;
            STEPS_LEFT = Cpu::limit(&code, &guest[..fields]);
            *addr_of_mut!(SIGNAL) = [0; 3];
            timer(CASE_MICROSECONDS);
            libc::raise(libc::SIGUSR1);
            timer(0);
        }

        if let Some(refused) = unsafe { REFUSED } {
            let mut result = vec![RESULT_REFUSED];
            result.extend_from_slice(refused.as_bytes());
            write_record(&mut output, &result);
            continue;
        }
        let data = unsafe { core::slice::from_raw_parts(DATA as *const u8, DATA_BYTES as usize) };
        let changed: Vec<(u32, u8)> = data
            .iter()
            .zip(&expected)
            .enumerate()
            .filter(|(_, (now, before))| now != before)
            .map(|(i, (now, _))| (i as u32, *now))
            .collect();
        let mut result = vec![RESULT_RAN];
        let signal = unsafe { *addr_of!(SIGNAL) };
        result.extend_from_slice(&(signal[0] as u32).to_le_bytes());
        result.extend_from_slice(&(signal[1] as u32).to_le_bytes());
        result.extend_from_slice(&signal[2].to_le_bytes());
        for value in unsafe { &(&*addr_of!(LANDED))[..fields] } {
            result.extend_from_slice(&value.to_le_bytes());
        }
        result.extend_from_slice(&(changed.len() as u32).to_le_bytes());
        for (offset, value) in changed {
            result.extend_from_slice(&offset.to_le_bytes());
            result.push(value);
        }
        write_record(&mut output, &result);
    }
    unsafe { libc::_exit(0) }
}

/// A pipe's two ends as files: reading, writing.
fn pipe() -> (std::fs::File, std::fs::File) {
    use std::os::fd::FromRawFd;
    let mut fds = [0; 2];
    check(
        unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) },
        "pipe",
    );
    unsafe {
        (
            std::fs::File::from_raw_fd(fds[0]),
            std::fs::File::from_raw_fd(fds[1]),
        )
    }
}

/// The child serving cases: its pid, where to send it cases and where its results come back.
struct CaseChild {
    pid: libc::pid_t,
    input: std::fs::File,
    output: std::fs::File,
}

fn start_case_child() -> CaseChild {
    let (child_reads, to_child) = pipe();
    let (from_child, child_writes) = pipe();
    match unsafe { libc::fork() } {
        0 => {
            drop((to_child, from_child));
            serve_cases(child_reads, child_writes)
        }
        -1 => panic!("fork: {}", std::io::Error::last_os_error()),
        pid => CaseChild {
            pid,
            input: to_child,
            output: from_child,
        },
    }
}

/// A wait status as a shell gives it: the exit status, or `SIGNALLED` and the signal.
fn status_of(status: libc::c_int) -> i32 {
    if libc::WIFSIGNALED(status) {
        SIGNALLED + libc::WTERMSIG(status)
    } else {
        libc::WEXITSTATUS(status)
    }
}

fn wait(pid: libc::pid_t) -> i32 {
    let mut status = 0;
    assert_eq!(unsafe { libc::waitpid(pid, &mut status, 0) }, pid);
    status_of(status)
}

/// A blob's stop: the signal and its `si_code`, the count, then pc and the general registers, which the
/// stop handler writes into a page the runner shares with the blob's child (no file descriptor, which the
/// blob could see). Only plain stores and raw system calls, since the blob's thread pointer is its own.
static mut STOP_RECORD: *mut u64 = core::ptr::null_mut();

extern "C" fn blob_stopped(
    signal: libc::c_int,
    info: *mut libc::siginfo_t,
    context: *mut libc::c_void,
) {
    unsafe {
        let mut registers = [0u64; MAX_STOP_REGISTERS];
        let count = Cpu::blob_stop(context as *const libc::ucontext_t, &mut registers);
        let record = core::slice::from_raw_parts_mut(STOP_RECORD, 2 + MAX_STOP_REGISTERS);
        record[0] = signal as u64 | ((*info).si_code as u64) << 32;
        record[1] = count as u64;
        record[2..2 + count].copy_from_slice(&registers[..count]);
        // SA_RESETHAND has made the action the default one, and the signal pends until this returns.
        libc::syscall(libc::SYS_kill, libc::syscall(libc::SYS_getpid), signal);
    }
}

extern "C" fn blob_enter(_: libc::c_int, _: *mut libc::siginfo_t, context: *mut libc::c_void) {
    unsafe {
        let guest = &*addr_of!(GUEST);
        Cpu::blob_start(
            context as *mut libc::ucontext_t,
            guest[0] as u64,
            guest[1] as u64,
        )
    };
}

/// A file in memory holding `bytes`, at its start, made with `memfd_create`'s `flags`.
fn memory_file(name: &core::ffi::CStr, bytes: &[u8], flags: libc::c_uint) -> libc::c_int {
    let fd = unsafe { libc::memfd_create(name.as_ptr(), flags) };
    assert!(fd >= 0, "memfd_create: {}", std::io::Error::last_os_error());
    let mut file = unsafe { <std::fs::File as std::os::fd::FromRawFd>::from_raw_fd(fd) };
    file.write_all(bytes).unwrap();
    unsafe { libc::lseek(fd, 0, libc::SEEK_SET) };
    core::mem::forget(file);
    fd
}

/// The blob, in a child process of its own. Never returns in the child.
struct BlobRequest<'a> {
    load: u64,
    stack_top: u64,
    stack_bytes: u64,
    headers: Option<(u64, u64)>,
    auxiliary: Vec<(u64, u64)>,
    bytes: &'a [u8],
}

/// In a child: make `input` and `out` its standard input, output and error, and close every other file, so
/// it sees only those, as a program Linux starts.
fn standard_files(input: libc::c_int, out: [libc::c_int; 2]) {
    unsafe {
        libc::dup2(input, 0);
        libc::dup2(out[0], 1);
        libc::dup2(out[1], 2);
        libc::syscall(libc::SYS_close_range, 3, u32::MAX, 0);
    }
}

fn run_blob_child(blob: &BlobRequest, input: libc::c_int, out: [libc::c_int; 2]) -> ! {
    standard_files(input, out);
    let length = (blob.bytes.len() as u64).next_multiple_of(PAGE);
    // One mapping for code and data, as a flat blob needs.
    map(
        blob.load,
        length,
        libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
    );
    unsafe {
        core::ptr::copy_nonoverlapping(blob.bytes.as_ptr(), blob.load as *mut u8, blob.bytes.len())
    };
    Cpu::synchronise(blob.load, length);
    map(
        blob.stack_top - blob.stack_bytes,
        blob.stack_bytes,
        libc::PROT_READ | libc::PROT_WRITE,
    );
    let (sp, stack) = initial_stack(blob.stack_top, blob.headers, &blob.auxiliary);
    unsafe { core::ptr::copy_nonoverlapping(stack.as_ptr(), sp as *mut u8, stack.len()) };
    alternate_stack();
    for signal in [
        libc::SIGTRAP,
        libc::SIGILL,
        libc::SIGSEGV,
        libc::SIGBUS,
        libc::SIGFPE,
    ] {
        handle(signal, blob_stopped, libc::SA_RESETHAND);
    }
    handle(libc::SIGUSR1, blob_enter, 0);
    unsafe {
        let guest = &mut *addr_of_mut!(GUEST);
        guest[0] = blob.load.into();
        guest[1] = sp.into();
        // A program that does not end in this time ends as if killed by SIGALRM (random programs may loop).
        libc::alarm(BLOB_SECONDS);
        libc::raise(libc::SIGUSR1);
    }
    unreachable!("the blob returned without exiting");
}

/// Run a blob (once, or `runs` times timed) and give its result record.
fn blob(request: &[u8]) -> Vec<u8> {
    let mut request = Reader(request);
    let (load, stack_top, stack_bytes) = (request.u64(), request.u64(), request.u64());
    let (headers_at, headers_count) = (request.u64(), request.u64());
    let auxiliary = (0..request.u32())
        .map(|_| (request.u64(), request.u64()))
        .collect();
    let runs = request.u32().max(1);
    let stdin_length = request.u32() as usize;
    let stdin = request.take(stdin_length).to_vec();
    let blob = BlobRequest {
        load,
        stack_top,
        stack_bytes,
        headers: (headers_count > 0).then_some((headers_at, headers_count)),
        auxiliary,
        bytes: request.0,
    };
    let mut best = f64::INFINITY;
    let mut last = None;
    for _ in 0..runs {
        let page = unsafe {
            libc::mmap(
                core::ptr::null_mut(),
                PAGE as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        assert_ne!(page, libc::MAP_FAILED, "a shared page");
        unsafe { STOP_RECORD = page as *mut u64 };
        let result = run_child(&stdin, |input, out| run_blob_child(&blob, input, out));
        let (status, seconds, [stdout, stderr]) = match result {
            Ok(result) => result,
            Err(e) => {
                unsafe { libc::munmap(page, PAGE as usize) };
                return failed(&e);
            }
        };
        best = best.min(seconds);
        let record = unsafe { core::slice::from_raw_parts(page as *const u64, 2) };
        let length = if record[0] == 0 {
            0
        } else {
            8 * (2 + record[1] as usize)
        };
        let stop = unsafe { core::slice::from_raw_parts(page as *const u8, length) }.to_vec();
        unsafe { libc::munmap(page, PAGE as usize) };
        last = Some((status, [stdout, stderr, stop]));
    }
    let (status, output) = last.unwrap();
    ran(status, best, output)
}

/// The result record of a blob or a program: its status, its time, its output and its stop record.
fn ran(status: i32, seconds: f64, [stdout, stderr, stop]: [Vec<u8>; 3]) -> Vec<u8> {
    let mut result = vec![RESULT_RAN];
    result.extend_from_slice(&status.to_le_bytes());
    result.extend_from_slice(&seconds.to_le_bytes());
    for bytes in [&stdout, &stderr, &stop] {
        result.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        result.extend_from_slice(bytes);
    }
    result
}

/// Run `child` (given its standard input and output files) in a process of its own, with `stdin` as its
/// input; gives its status as a shell does, the time from just before the process is made until it has
/// ended, and its output and error.
fn run_child(
    stdin: &[u8],
    child: impl FnOnce(libc::c_int, [libc::c_int; 2]),
) -> Result<(i32, f64, [Vec<u8>; 2]), String> {
    use std::os::fd::AsRawFd;
    let input = memory_file(c"stdin", stdin, 0);
    // Output goes to pipes, as a program's output does when its caller reads it: a write from a buffer that
    // faults part way fails on a pipe and is cut short on a file.
    let [(stdout, stdout_end), (stderr, stderr_end)] = [pipe(), pipe()];
    let readers = [stdout, stderr].map(|mut from| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            from.read_to_end(&mut bytes).unwrap();
            bytes
        })
    });
    let out = [stdout_end.as_raw_fd(), stderr_end.as_raw_fd()];
    let start = std::time::Instant::now();
    let pid = unsafe { libc::fork() };
    match pid {
        0 => {
            child(input, out);
            unreachable!("the child returned")
        }
        -1 => {
            unsafe { libc::close(input) };
            return Err(format!("fork: {}", std::io::Error::last_os_error()));
        }
        _ => {}
    }
    drop((stdout_end, stderr_end));
    let status = wait(pid);
    let seconds = start.elapsed().as_secs_f64();
    unsafe { libc::close(input) };
    let [stdout, stderr] = readers.map(|reader| reader.join().unwrap());
    Ok((status, seconds, [stdout, stderr]))
}

/// Run an ordinary static executable as Linux starts a program, with a blob's name and no other arguments,
/// no environment and empty input, for at most `BLOB_SECONDS` (then SIGALRM ends it, as a blob); its result
/// is a blob's with no stop record.
fn program(executable: &[u8]) -> Vec<u8> {
    let name = core::ffi::CStr::from_bytes_with_nul(BLOB_NAME).unwrap();
    let arguments = [name.as_ptr(), core::ptr::null()];
    let environment = [core::ptr::null()];
    let result = run_child(&[], |input, out| {
        standard_files(input, out);
        // Closed as the program starts, so it sees no file of its own.
        let file = memory_file(name, executable, libc::MFD_CLOEXEC);
        unsafe {
            // A program starts with no signal blocked and SIGPIPE's default action (Rust's runtime ignores
            // it in the runner, and exec keeps what is ignored); the alarm carries over exec.
            let mut none: libc::sigset_t = core::mem::zeroed();
            libc::sigemptyset(&mut none);
            libc::sigprocmask(libc::SIG_SETMASK, &none, core::ptr::null_mut());
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
            libc::alarm(BLOB_SECONDS);
            libc::fexecve(file, arguments.as_ptr(), environment.as_ptr());
        }
        panic!("fexecve: {}", std::io::Error::last_os_error())
    });
    match result {
        Ok((status, seconds, [stdout, stderr])) => {
            ran(status, seconds, [stdout, stderr, Vec::new()])
        }
        Err(e) => failed(&e),
    }
}

/// The CPU's model, as `/proc/cpuinfo` names it.
fn model() -> String {
    let info = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let field = |name: &str| {
        info.lines()
            .find(|line| line.split(':').next().is_some_and(|n| n.trim() == name))
            .and_then(|line| line.split_once(':'))
            .map(|(_, value)| value.trim().to_string())
    };
    field("model name").unwrap_or_else(|| {
        let part = |name| field(name).unwrap_or_default();
        format!(
            "implementer {} part {} variant {} revision {}",
            part("CPU implementer"),
            part("CPU part"),
            part("CPU variant"),
            part("CPU revision")
        )
    })
}

fn hello() -> Vec<u8> {
    let mut record = (Cpu::FIELDS.len() as u32).to_le_bytes().to_vec();
    for name in Cpu::FIELDS
        .iter()
        .map(|f| f.as_bytes())
        .chain([model().as_bytes()])
    {
        record.push(name.len() as u8);
        record.extend_from_slice(name);
    }
    record
}

/// How long the server waits for a request before it ends: a connection that died without closing (a dropped
/// network) never sends one nor ends its input, and the runner and its case child would stay for hours.
const IDLE_MILLISECONDS: i32 = 15 * 60 * 1000;

/// Whether a request starts within `IDLE_MILLISECONDS`, or the input ends.
fn request_arrives(input: &std::io::BufReader<std::fs::File>) -> bool {
    use std::os::fd::AsRawFd;
    if !input.buffer().is_empty() {
        return true;
    }
    let mut poll = libc::pollfd {
        fd: input.get_ref().as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    loop {
        match unsafe { libc::poll(&mut poll, 1, IDLE_MILLISECONDS) } {
            0 => return false,
            n if n > 0 => return true,
            _ if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted => {}
            _ => return true,
        }
    }
}

fn serve() {
    use std::os::fd::FromRawFd;
    // Standard input as a file of our own, so its buffer can be seen before waiting on it.
    let mut input =
        std::io::BufReader::new(unsafe { std::fs::File::from_raw_fd(libc::STDIN_FILENO) });
    let mut output = std::io::stdout().lock();
    write_record(&mut output, &hello());
    SERVER.store(
        unsafe { libc::getpid() },
        core::sync::atomic::Ordering::Relaxed,
    );
    let mut child: Option<CaseChild> = None;
    while request_arrives(&input)
        && let Some(request) = read_record(&mut input)
    {
        let result = match request.first() {
            Some(&RECORD_CASE) => {
                let serving = child.get_or_insert_with(start_case_child);
                write_record(&mut serving.input, &request);
                match read_record(&mut serving.output) {
                    Some(result) => result,
                    None => {
                        let gone = child.take().unwrap();
                        drop((gone.input, gone.output));
                        failed(&format!(
                            "the case ended the runner's child: status {}",
                            wait(gone.pid)
                        ))
                    }
                }
            }
            Some(&RECORD_BLOB) => blob(&request[1..]),
            Some(&RECORD_PROGRAM) => program(&request[1..]),
            _ => failed("an unknown request"),
        };
        write_record(&mut output, &result);
    }
}

/// The serving process once its hello is sent; 0 before.
static SERVER: core::sync::atomic::AtomicI32 = core::sync::atomic::AtomicI32::new(0);

fn main() {
    // A failure of the runner itself ends with status 101 after this message on stderr and, from the serving
    // process, as the request's result, so the harness reports it as the runner's failure and not a lost
    // connection. Records are written whole and flushed, so none is cut short here.
    std::panic::set_hook(Box::new(|info| {
        let message = format!("{}: {info}", env!("CARGO_PKG_NAME"));
        eprintln!("{message}");
        if unsafe { libc::getpid() } == SERVER.load(core::sync::atomic::Ordering::Relaxed) {
            let mut record = ((message.len() + 1) as u32).to_le_bytes().to_vec();
            record.push(RESULT_RUNNER_FAILED);
            record.extend_from_slice(message.as_bytes());
            let mut at = 0;
            while at < record.len() {
                let wrote =
                    unsafe { libc::write(1, record[at..].as_ptr().cast(), record.len() - at) };
                if wrote <= 0 {
                    break;
                }
                at += wrote as usize;
            }
        }
        unsafe { libc::_exit(101) };
    }));
    match std::env::args().nth(1).as_deref() {
        Some("serve") => serve(),
        _ => panic!("usage: wcontainers-reference serve"),
    }
}
