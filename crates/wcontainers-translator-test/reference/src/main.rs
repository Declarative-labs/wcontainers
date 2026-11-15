//! The native reference runner, for real ARM64 Linux.
//!
//! `wcontainers-reference cases` reads layer 1 cases on stdin and writes results on stdout. Each case runs one
//! instruction with every register exact: a signal handler rewrites its own frame to the guest's registers
//! and returns into the instruction; wherever control then lands raises the next signal, whose frame holds
//! the registers after it. System calls made from the instruction are caught by seccomp and never run.
//!
//! `wcontainers-reference blob <file> <load address> <stack top>` runs a flat blob from its first byte with
//! all registers zero, for real; its output and exit status are the program's.

#![allow(clippy::missing_safety_doc)]

use core::ptr::{addr_of, addr_of_mut};
use std::io::{Read, Write};

include!("../../src/layout.rs");

const PAGE: u64 = 4096;
const NZCV: u64 = 0xf000_0000;

/// The registers the guest starts with: x0 to x30, sp, pc, pstate, thread pointer.
static mut GUEST: [u64; 35] = [0; 35];
/// The registers where control landed, in the same order.
static mut LANDED: [u64; 35] = [0; 35];
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

/// Entry: keep the host's frame, put the guest's registers in its place.
extern "C" fn enter(_: libc::c_int, _: *mut libc::siginfo_t, context: *mut libc::c_void) {
    unsafe {
        let uc = &mut *(context as *mut libc::ucontext_t);
        (*addr_of_mut!(HOST)).write(core::ptr::read(&uc.uc_mcontext));
        HOST_THREAD_POINTER = thread_pointer();
        let guest = &*addr_of!(GUEST);
        uc.uc_mcontext.regs.copy_from_slice(&guest[..31]);
        uc.uc_mcontext.sp = guest[31];
        uc.uc_mcontext.pc = guest[32];
        uc.uc_mcontext.pstate = (uc.uc_mcontext.pstate & !NZCV) | (guest[33] & NZCV);
        set_thread_pointer(guest[34]);
    }
}

/// Exit: record where control landed, then put the host's frame back.
extern "C" fn land(signal: libc::c_int, info: *mut libc::siginfo_t, context: *mut libc::c_void) {
    unsafe {
        let guest_thread_pointer = thread_pointer();
        set_thread_pointer(HOST_THREAD_POINTER);
        let uc = &mut *(context as *mut libc::ucontext_t);
        let landed = &mut *addr_of_mut!(LANDED);
        landed[..31].copy_from_slice(&uc.uc_mcontext.regs);
        landed[31] = uc.uc_mcontext.sp;
        landed[32] = uc.uc_mcontext.pc;
        landed[33] = uc.uc_mcontext.pstate & NZCV;
        landed[34] = guest_thread_pointer;
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
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
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
        let seed = word(&request, 4);
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
            for (i, value) in guest.iter_mut().take(32).enumerate() {
                *value = word(&request, 12 + 8 * i);
            }
            guest[32] = CODE;
            guest[33] = word(&request, 12 + 8 * 32);
            guest[34] = word(&request, 12 + 8 * 33);
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
        let mut result = Vec::with_capacity(RESULT_HEADER_BYTES + 5 * changed.len());
        let signal = unsafe { *addr_of!(SIGNAL) };
        result.extend_from_slice(&(signal[0] as u32).to_le_bytes());
        result.extend_from_slice(&(signal[1] as u32).to_le_bytes());
        result.extend_from_slice(&signal[2].to_le_bytes());
        for value in unsafe { *addr_of!(LANDED) } {
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

fn blob(path: &str, load: u64, stack_top: u64) {
    let bytes = std::fs::read(path).expect("read the blob");
    let length = (bytes.len() as u64).next_multiple_of(PAGE);
    map(load, length, libc::PROT_READ | libc::PROT_WRITE);
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), load as *mut u8, bytes.len()) };
    protect(load, length, libc::PROT_READ | libc::PROT_EXEC);
    let mut address = load;
    while address < load + length {
        synchronise(address);
        address += 64;
    }
    const STACK: u64 = 1 << 20;
    map(stack_top - STACK, STACK, libc::PROT_READ | libc::PROT_WRITE);
    handle(libc::SIGUSR1, enter);
    unsafe {
        let guest = &mut *addr_of_mut!(GUEST);
        *guest = [0; 35];
        guest[31] = stack_top;
        guest[32] = load;
        libc::raise(libc::SIGUSR1);
    }
    unreachable!("the blob returned without exiting");
}

fn number(text: &str) -> u64 {
    u64::from_str_radix(text.trim_start_matches("0x"), 16).expect("a hexadecimal number")
}

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    match arguments.get(1).map(String::as_str) {
        Some("cases") => cases(),
        Some("blob") => blob(&arguments[2], number(&arguments[3]), number(&arguments[4])),
        _ => panic!("usage: wcontainers-reference cases | blob <file> <load> <stack top>"),
    }
}
