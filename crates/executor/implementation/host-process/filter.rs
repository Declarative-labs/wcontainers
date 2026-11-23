//! The seccomp filter a stub installs on itself before its first turn: its own host calls only, arguments bounded.
//!
//! Written as classic BPF here because it compares two arguments with each other (a mapping's file offset with its
//! address), which no filter-building crate offers.

use core::mem::offset_of;

use libc::{
    BPF_ABS, BPF_ADD, BPF_ALU, BPF_IMM, BPF_JA, BPF_JEQ, BPF_JGE, BPF_JGT, BPF_JMP, BPF_JSET,
    BPF_K, BPF_LD, BPF_MEM, BPF_MISC, BPF_RET, BPF_ST, BPF_TAX, BPF_W, BPF_X, SECCOMP_RET_ALLOW,
    SECCOMP_RET_KILL_PROCESS, seccomp_data, sock_filter,
};
use linux_raw_sys::general as linux;

use crate::layout::{CODE_AREA, CODE_AREA_SIZE, GUEST_START, OUR_AREA};
use crate::stub::{BLOCK_FD, MEMORY_FD, SOCKET_FD, THREAD_FLAGS};

#[cfg(target_arch = "aarch64")]
const ARCHITECTURE: u32 = linux_raw_sys::ptrace::AUDIT_ARCH_AARCH64;
#[cfg(target_arch = "x86_64")]
const ARCHITECTURE: u32 = linux_raw_sys::ptrace::AUDIT_ARCH_X86_64;

/// The guest area's end, the first address a stub may not map or protect.
const GUEST_END: usize = OUR_AREA;
const _: () = assert!(GUEST_END as u32 == 0 && GUEST_START >> 32 == 0);

/// A place in the program a jump goes to.
#[derive(Clone, Copy)]
enum Target {
    Next,
    Label(usize),
}

enum Op {
    Statement(u32, u32),
    Jump(u32, u32, Target, Target),
    Always(Target),
}

/// A program being written, with labels placed later than the jumps to them.
struct Program {
    ops: Vec<Op>,
    labels: Vec<Option<usize>>,
}

impl Program {
    fn label(&mut self) -> usize {
        self.labels.push(None);
        self.labels.len() - 1
    }

    fn place(&mut self, label: usize) {
        self.labels[label] = Some(self.ops.len());
    }

    fn load(&mut self, offset: usize) {
        self.ops
            .push(Op::Statement(BPF_LD | BPF_W | BPF_ABS, offset as u32));
    }

    fn load_low(&mut self, argument: usize) {
        self.load(offset_of!(seccomp_data, args) + argument * size_of::<u64>());
    }

    fn load_high(&mut self, argument: usize) {
        self.load(offset_of!(seccomp_data, args) + argument * size_of::<u64>() + size_of::<u32>());
    }

    fn statement(&mut self, code: u32, k: u32) {
        self.ops.push(Op::Statement(code, k));
    }

    fn jump(&mut self, code: u32, k: u32, then: Target, otherwise: Target) {
        self.ops.push(Op::Jump(BPF_JMP | code, k, then, otherwise));
    }

    fn always(&mut self, to: usize) {
        self.ops.push(Op::Always(Target::Label(to)));
    }

    fn finish(self) -> Vec<sock_filter> {
        let at = |target: Target, from: usize| match target {
            Target::Next => 0,
            Target::Label(label) => {
                let to = self.labels[label].expect("every label is placed");
                to - (from + 1)
            }
        };
        let short = |offset: usize| u8::try_from(offset).expect("a jump the filter can make");
        self.ops
            .iter()
            .enumerate()
            .map(|(from, op)| match *op {
                Op::Statement(code, k) => sock_filter {
                    code: code as u16,
                    jt: 0,
                    jf: 0,
                    k,
                },
                Op::Jump(code, k, then, otherwise) => sock_filter {
                    code: code as u16,
                    jt: short(at(then, from)),
                    jf: short(at(otherwise, from)),
                    k,
                },
                Op::Always(to) => sock_filter {
                    code: (BPF_JMP | BPF_JA) as u16,
                    jt: 0,
                    jf: 0,
                    k: at(to, from) as u32,
                },
            })
            .collect()
    }

    /// Goes on only where `[address, address + length)` lies inside the guest area; otherwise to `refuse`.
    fn guest_range(&mut self, address: usize, length: usize, refuse: usize) {
        let end_high = (GUEST_END >> 32) as u32;
        let (refuse, next) = (Target::Label(refuse), Target::Next);
        let low_checked = self.label();
        let carry = self.label();
        let no_carry = self.label();
        let summed = self.label();
        let end_low = self.label();
        let inside = self.label();
        self.load_high(address);
        self.jump(BPF_JGE | BPF_K, end_high, refuse, next);
        self.jump(BPF_JGT | BPF_K, 0, Target::Label(low_checked), next);
        self.load_low(address);
        self.jump(BPF_JGE | BPF_K, GUEST_START as u32, next, refuse);
        self.place(low_checked);
        self.load_high(length);
        self.jump(BPF_JGE | BPF_K, end_high, refuse, next);
        // The end's low word, and its carry: the sum is below the address's low word exactly when it carried.
        self.load_low(address);
        self.statement(BPF_MISC | BPF_TAX, 0);
        self.load_low(length);
        self.statement(BPF_ALU | BPF_ADD | BPF_X, 0);
        self.statement(BPF_ST, 0);
        self.jump(
            BPF_JGE | BPF_X,
            0,
            Target::Label(no_carry),
            Target::Label(carry),
        );
        self.place(carry);
        self.statement(BPF_LD | BPF_IMM, 1);
        self.statement(BPF_ST, 1);
        self.always(summed);
        self.place(no_carry);
        self.statement(BPF_LD | BPF_IMM, 0);
        self.statement(BPF_ST, 1);
        self.place(summed);
        self.load_high(address);
        self.statement(BPF_MISC | BPF_TAX, 0);
        self.load_high(length);
        self.statement(BPF_ALU | BPF_ADD | BPF_X, 0);
        self.statement(BPF_MISC | BPF_TAX, 0);
        self.statement(BPF_LD | BPF_MEM, 1);
        self.statement(BPF_ALU | BPF_ADD | BPF_X, 0);
        self.jump(BPF_JGT | BPF_K, end_high, refuse, next);
        self.jump(
            BPF_JEQ | BPF_K,
            end_high,
            Target::Label(end_low),
            Target::Label(inside),
        );
        self.place(end_low);
        self.statement(BPF_LD | BPF_MEM, 0);
        self.jump(BPF_JEQ | BPF_K, 0, Target::Label(inside), refuse);
        self.place(inside);
    }

    /// Goes on only where a 64-bit argument equals `value`.
    fn argument_is(&mut self, argument: usize, value: u64, refuse: usize) {
        self.load_low(argument);
        self.jump(
            BPF_JEQ | BPF_K,
            value as u32,
            Target::Next,
            Target::Label(refuse),
        );
        self.load_high(argument);
        self.jump(
            BPF_JEQ | BPF_K,
            (value >> 32) as u32,
            Target::Next,
            Target::Label(refuse),
        );
    }

    /// Goes on only where a permissions argument has no execute bit.
    fn not_executable(&mut self, argument: usize, refuse: usize) {
        self.load_low(argument);
        self.jump(
            BPF_JSET | BPF_K,
            linux::PROT_EXEC,
            Target::Label(refuse),
            Target::Next,
        );
        self.load_high(argument);
        self.jump(BPF_JEQ | BPF_K, 0, Target::Next, Target::Label(refuse));
    }
}

/// The stub's filter: `futex`, `exit`, `exit_group`, `rt_sigprocmask` and `sigaltstack` as they come; `mmap` shared and
/// fixed, inside the guest area, never executable, of the memory's memfd at the offset equal to the address or of a
/// shared block's received memfd, and of the code area's received memfd execute-only over exactly the code area; `mprotect` inside the guest area, never executable; `munmap` inside the guest area;
/// `clone` of a thread of the stub only; `recvmsg` on its socket without waiting and `close` of a received block;
/// `sched_setaffinity` of the calling thread only; on ARM64 `rt_sigreturn`. Anything else kills the stub.
pub(crate) fn stub_filter() -> Vec<sock_filter> {
    let mut program = Program {
        ops: Vec::new(),
        labels: Vec::new(),
    };
    let kill = program.label();
    let allow = program.label();
    let mmap = program.label();
    let mprotect = program.label();
    let munmap = program.label();
    let clone = program.label();
    let recvmsg = program.label();
    let close = program.label();
    let place = program.label();
    program.load(offset_of!(seccomp_data, arch));
    program.jump(
        BPF_JEQ | BPF_K,
        ARCHITECTURE,
        Target::Next,
        Target::Label(kill),
    );
    program.load(offset_of!(seccomp_data, nr));
    for (number, to) in [
        (linux::__NR_futex, allow),
        (linux::__NR_exit, allow),
        (linux::__NR_exit_group, allow),
        (linux::__NR_rt_sigprocmask, allow),
        (linux::__NR_sigaltstack, allow),
        (linux::__NR_mmap, mmap),
        (linux::__NR_mprotect, mprotect),
        (linux::__NR_munmap, munmap),
        (linux::__NR_clone, clone),
        (linux::__NR_recvmsg, recvmsg),
        (linux::__NR_close, close),
        (linux::__NR_sched_setaffinity, place),
    ] {
        program.jump(BPF_JEQ | BPF_K, number, Target::Label(to), Target::Next);
    }
    // A handler whose frame holds no FP/SIMD registers returns through it to save them (gVisor on ARM64).
    #[cfg(target_arch = "aarch64")]
    program.jump(
        BPF_JEQ | BPF_K,
        linux::__NR_rt_sigreturn,
        Target::Label(allow),
        Target::Next,
    );
    program.always(kill);

    program.place(mmap);
    // The code area's memfd, received as a block, execute-only over exactly the code area.
    let guest_mmap = program.label();
    for (argument, value) in [
        (0, CODE_AREA as u64),
        (1, CODE_AREA_SIZE as u64),
        (2, u64::from(linux::PROT_EXEC)),
        (3, u64::from(linux::MAP_SHARED | linux::MAP_FIXED)),
        (4, u64::from(BLOCK_FD)),
        (5, 0),
    ] {
        program.argument_is(argument, value, guest_mmap);
    }
    program.always(allow);
    program.place(guest_mmap);
    program.guest_range(0, 1, kill);
    program.not_executable(2, kill);
    program.argument_is(3, u64::from(linux::MAP_SHARED | linux::MAP_FIXED), kill);
    // A block's memfd at any offset, the block being the embedder's to choose; the memory's at the address.
    program.load_low(4);
    program.jump(
        BPF_JEQ | BPF_K,
        BLOCK_FD,
        Target::Label(allow),
        Target::Next,
    );
    program.jump(
        BPF_JEQ | BPF_K,
        MEMORY_FD,
        Target::Next,
        Target::Label(kill),
    );
    for load in [Program::load_low, Program::load_high] {
        load(&mut program, 5);
        program.statement(BPF_MISC | BPF_TAX, 0);
        load(&mut program, 0);
        program.jump(BPF_JEQ | BPF_X, 0, Target::Next, Target::Label(kill));
    }
    program.always(allow);

    program.place(mprotect);
    program.guest_range(0, 1, kill);
    program.not_executable(2, kill);
    program.always(allow);

    program.place(munmap);
    program.guest_range(0, 1, kill);
    program.always(allow);

    program.place(clone);
    program.argument_is(0, u64::from(THREAD_FLAGS), kill);
    program.always(allow);

    program.place(recvmsg);
    program.argument_is(0, u64::from(SOCKET_FD), kill);
    program.argument_is(2, u64::from(linux_raw_sys::net::MSG_DONTWAIT), kill);
    program.always(allow);

    program.place(close);
    program.argument_is(0, u64::from(BLOCK_FD), kill);
    program.always(allow);

    program.place(place);
    program.argument_is(0, 0, kill);
    program.argument_is(1, size_of::<libc::cpu_set_t>() as u64, kill);
    program.always(allow);

    // Last, since a filter's jumps go only forward.
    program.place(kill);
    program.statement(BPF_RET | BPF_K, SECCOMP_RET_KILL_PROCESS);
    program.place(allow);
    program.statement(BPF_RET | BPF_K, SECCOMP_RET_ALLOW);
    program.finish()
}
