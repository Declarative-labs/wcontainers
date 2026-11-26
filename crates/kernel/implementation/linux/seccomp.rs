//! Seccomp: Docker's default profile, which the reference applies before Linux sees a call, and the
//! filters a program installs itself, which are classic BPF programs checked and run as Linux does.
//!
//! What the profile refuses was measured on the reference (Docker 29.9.3) by making every call with
//! and without the profile, and the argument rules by making the filtered calls with chosen arguments.
//! Its arguments are compared as full 64 bit values, as seccomp does.

use alloc::boxed::Box;

use crate::Errno;
use crate::linux::general::*;
use crate::linux::net::{AF_MAX, AF_VSOCK};
use crate::linux::ptrace::*;

/// The highest call number the profile knows; it answers anything above as a kernel without the
/// call would, so that programs fall back.
const LAST_KNOWN: u32 = __NR_futex_requeue;

/// Calls the profile refuses whatever their arguments.
const REFUSED: &[u32] = &[
    __NR_lookup_dcookie,
    __NR_umount2,
    __NR_mount,
    __NR_pivot_root,
    __NR_nfsservctl,
    __NR_quotactl,
    __NR_unshare,
    __NR_kexec_load,
    __NR_delete_module,
    __NR_clock_settime,
    __NR_settimeofday,
    __NR_add_key,
    __NR_request_key,
    __NR_keyctl,
    __NR_swapon,
    __NR_mbind,
    __NR_get_mempolicy,
    __NR_set_mempolicy,
    __NR_migrate_pages,
    __NR_move_pages,
    __NR_perf_event_open,
    __NR_open_by_handle_at,
    __NR_setns,
    __NR_kcmp,
    __NR_bpf,
    __NR_userfaultfd,
    __NR_kexec_file_load,
    __NR_io_uring_setup,
    __NR_io_uring_enter,
    __NR_io_uring_register,
    __NR_open_tree,
    __NR_fsconfig,
    __NR_pidfd_getfd,
    __NR_process_madvise,
    __NR_mount_setattr,
    __NR_quotactl_fd,
    __NR_set_mempolicy_home_node,
    // The profile's sections for powers a default container lacks.
    __NR_vhangup,
    __NR_acct,
    __NR_init_module,
    __NR_finit_module,
    __NR_syslog,
    __NR_reboot,
    __NR_sethostname,
    __NR_setdomainname,
    __NR_swapoff,
    __NR_fanotify_init,
    __NR_move_mount,
    __NR_fsopen,
    __NR_fsmount,
    __NR_fspick,
];

/// The personalities the profile lets through; the last only asks.
const PERSONALITIES: [u64; 5] = [0, 8, 0x20000, 0x20008, 0xffff_ffff];

/// The `clone` flags that make namespaces, which need powers a default container lacks.
const NAMESPACE_FLAGS: u64 = (CLONE_NEWNS
    | CLONE_NEWCGROUP
    | CLONE_NEWUTS
    | CLONE_NEWIPC
    | CLONE_NEWUSER
    | CLONE_NEWPID
    | CLONE_NEWNET) as u64;

/// A thread's seccomp mode: filtering once Docker's profile or a filter of its own applies.
pub(crate) fn mode(docker_profile: bool, filters: usize) -> u32 {
    if docker_profile || filters > 0 {
        SECCOMP_MODE_FILTER
    } else {
        SECCOMP_MODE_DISABLED
    }
}

/// What the profile answers in place of Linux, if it stops the call. `known` says whether Linux has a
/// call with this number.
#[allow(non_upper_case_globals, reason = "Linux's names for call numbers")]
pub(crate) fn docker_default(number: u32, arguments: &[u64; 6], known: bool) -> Option<Errno> {
    if number > LAST_KNOWN || number == __NR_clone3 {
        return Some(Errno::ENOSYS);
    }
    let refused = match number {
        _ if !known => true,
        __NR_personality => !PERSONALITIES.contains(&arguments[0]),
        __NR_socket => arguments[0] == u64::from(AF_VSOCK) || arguments[0] >= u64::from(AF_MAX),
        __NR_clone => arguments[0] & NAMESPACE_FLAGS != 0,
        _ => REFUSED.contains(&number),
    };
    refused.then_some(Errno::EPERM)
}

/// The length of Docker's profile as a BPF program, which counts towards the instructions one call
/// may run through (measured: the limit is reached 1032 instructions early).
pub(crate) const DOCKER_PROFILE_LENGTH: usize = 1032;

/// Linux's `MAX_INSNS_PER_PATH`: the most instructions the filters of a thread may hold, each counted
/// with four more for its own cost.
pub(crate) const MAX_INSTRUCTIONS_PER_PATH: usize = 32768;

/// What a filter answers, in the order Linux ranks the actions: lower wins (`ACTION_ONLY`).
pub(crate) fn rank(answer: u32) -> i32 {
    (answer & SECCOMP_RET_ACTION_FULL) as i32
}

/// The words of `struct seccomp_data` a filter loads: the call number, the architecture, the address
/// after the call instruction and the six arguments, each in two halves, low first.
pub(crate) type Data = [u32; 16];

/// The `seccomp_data` word of the instruction pointer's low half.
const POINTER_WORD: u32 = 2;

/// Words of `seccomp_data`, as `BPF_LD | BPF_W | BPF_LEN` reports its size.
const DATA_SIZE: u32 = core::mem::size_of::<seccomp_data>() as u32;

/// A program checked as Linux's `bpf_check_classic`, `check_load_and_stores` and
/// `seccomp_check_filter` do, with the length loads replaced by the size of the data, as Linux does.
pub(crate) struct Filter {
    program: Box<[sock_filter]>,
    /// Whether it loads the instruction pointer, which the call must then read from the registers.
    pub reads_pointer: bool,
}

const LD_W_ABS: u16 = (BPF_LD | BPF_W | BPF_ABS) as u16;
const LD_W_LEN: u16 = (BPF_LD | BPF_W | BPF_LEN) as u16;
const LDX_W_LEN: u16 = (BPF_LDX | BPF_W | BPF_LEN) as u16;
const LD_IMM: u16 = (BPF_LD | BPF_IMM) as u16;
const LDX_IMM: u16 = (BPF_LDX | BPF_IMM) as u16;
const LD_MEM: u16 = (BPF_LD | BPF_MEM) as u16;
const LDX_MEM: u16 = (BPF_LDX | BPF_MEM) as u16;
const ST: u16 = BPF_ST as u16;
const STX: u16 = BPF_STX as u16;
const TAX: u16 = (BPF_MISC | BPF_TAX) as u16;
const TXA: u16 = (BPF_MISC | BPF_TXA) as u16;
const RET_K: u16 = (BPF_RET | BPF_K) as u16;
const RET_A: u16 = (BPF_RET | BPF_A) as u16;
const JA: u16 = (BPF_JMP | BPF_JA) as u16;
const NEG: u16 = (BPF_ALU | BPF_NEG) as u16;

const fn alu(operation: u32, source: u32) -> u16 {
    (BPF_ALU | operation | source) as u16
}

const fn jump(test: u32, source: u32) -> u16 {
    (BPF_JMP | test | source) as u16
}

/// The arithmetic codes, each with a constant or X; MOD is classic BPF but not seccomp's.
const ARITHMETIC: [u32; 9] = [
    BPF_ADD, BPF_SUB, BPF_MUL, BPF_DIV, BPF_OR, BPF_AND, BPF_XOR, BPF_LSH, BPF_RSH,
];

/// The conditional jumps, each with a constant or X.
const TESTS: [u32; 4] = [BPF_JEQ, BPF_JGT, BPF_JGE, BPF_JSET];

fn is_conditional(code: u16) -> bool {
    TESTS
        .iter()
        .any(|&test| code == jump(test, BPF_K) || code == jump(test, BPF_X))
}

/// Linux's `chk_code_allowed`: the codes classic BPF has.
fn classic_code(code: u16) -> bool {
    const LOADS: [u32; 14] = [
        BPF_LD | BPF_W | BPF_ABS,
        BPF_LD | BPF_H | BPF_ABS,
        BPF_LD | BPF_B | BPF_ABS,
        BPF_LD | BPF_W | BPF_LEN,
        BPF_LD | BPF_W | BPF_IND,
        BPF_LD | BPF_H | BPF_IND,
        BPF_LD | BPF_B | BPF_IND,
        BPF_LD | BPF_IMM,
        BPF_LD | BPF_MEM,
        BPF_LDX | BPF_W | BPF_LEN,
        BPF_LDX | BPF_B | BPF_MSH,
        BPF_LDX | BPF_IMM,
        BPF_LDX | BPF_MEM,
        BPF_ST,
    ];
    LOADS.iter().any(|&load| code == load as u16)
        || [STX, TAX, TXA, RET_K, RET_A, JA, NEG].contains(&code)
        || is_conditional(code)
        || ARITHMETIC
            .iter()
            .chain(&[BPF_MOD])
            .any(|&op| code == alu(op, BPF_K) || code == alu(op, BPF_X))
}

/// The codes seccomp takes (Linux's `seccomp_check_filter`).
fn seccomp_code(code: u16) -> bool {
    [
        LD_W_ABS, LD_W_LEN, LDX_W_LEN, RET_K, RET_A, NEG, LD_IMM, LDX_IMM, TAX, TXA, LD_MEM,
        LDX_MEM, ST, STX, JA,
    ]
    .contains(&code)
        || is_conditional(code)
        || ARITHMETIC
            .iter()
            .any(|&op| code == alu(op, BPF_K) || code == alu(op, BPF_X))
}

impl Filter {
    /// Checks a program as Linux does when it is installed; every refusal is EINVAL.
    pub(crate) fn new(mut program: Box<[sock_filter]>) -> Result<Self, Errno> {
        let length = program.len();
        // `bpf_check_classic`.
        for (pc, instruction) in program.iter().enumerate() {
            let code = instruction.code;
            let k = instruction.k;
            let valid = classic_code(code)
                && match code {
                    _ if code == alu(BPF_DIV, BPF_K) || code == alu(BPF_MOD, BPF_K) => k != 0,
                    _ if code == alu(BPF_LSH, BPF_K) || code == alu(BPF_RSH, BPF_K) => k < 32,
                    LD_MEM | LDX_MEM | ST | STX => k < BPF_MEMWORDS,
                    JA => (k as usize) < length - pc - 1,
                    _ if is_conditional(code) => {
                        pc + usize::from(instruction.jt) + 1 < length
                            && pc + usize::from(instruction.jf) + 1 < length
                    }
                    _ => true,
                };
            if !valid {
                return Err(Errno::EINVAL);
            }
        }
        if ![RET_K, RET_A].contains(&program[length - 1].code) {
            return Err(Errno::EINVAL);
        }
        // `check_load_and_stores`: scratch memory is read only where every path has written it.
        let mut masks = alloc::vec![u16::MAX; length];
        let mut valid: u16 = 0;
        for (pc, instruction) in program.iter().enumerate() {
            valid &= masks[pc];
            let code = instruction.code;
            match code {
                ST | STX => valid |= 1 << instruction.k,
                LD_MEM | LDX_MEM if valid & (1 << instruction.k) == 0 => {
                    return Err(Errno::EINVAL);
                }
                JA => {
                    masks[pc + 1 + instruction.k as usize] &= valid;
                    valid = u16::MAX;
                }
                _ if is_conditional(code) => {
                    masks[pc + 1 + usize::from(instruction.jt)] &= valid;
                    masks[pc + 1 + usize::from(instruction.jf)] &= valid;
                    valid = u16::MAX;
                }
                _ => {}
            }
        }
        // `seccomp_check_filter`: only whole aligned words of the data, and length loads become the
        // data's size.
        let mut reads_pointer = false;
        for instruction in program.iter_mut() {
            match instruction.code {
                LD_W_ABS => {
                    if instruction.k >= DATA_SIZE || instruction.k & 3 != 0 {
                        return Err(Errno::EINVAL);
                    }
                    reads_pointer |= matches!(instruction.k / 4, POINTER_WORD | 3);
                }
                LD_W_LEN => (instruction.code, instruction.k) = (LD_IMM, DATA_SIZE),
                LDX_W_LEN => (instruction.code, instruction.k) = (LDX_IMM, DATA_SIZE),
                code if seccomp_code(code) => {}
                _ => return Err(Errno::EINVAL),
            }
        }
        Ok(Self {
            program,
            reads_pointer,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.program.len()
    }

    /// Runs the program on one call's data and gives its answer. A and X start at zero, arithmetic is
    /// on 32 bits, shifts by X take its low five bits, and a division by an X of zero ends the program
    /// with 0, as Linux's translation of classic BPF does.
    pub(crate) fn run(&self, data: &Data) -> u32 {
        let (mut a, mut x) = (0u32, 0u32);
        let mut memory = [0u32; BPF_MEMWORDS as usize];
        let mut pc = 0;
        loop {
            let instruction = self.program[pc];
            let k = instruction.k;
            pc += 1;
            match instruction.code {
                LD_W_ABS => a = data[(k / 4) as usize],
                LD_IMM => a = k,
                LDX_IMM => x = k,
                LD_MEM => a = memory[k as usize],
                LDX_MEM => x = memory[k as usize],
                ST => memory[k as usize] = a,
                STX => memory[k as usize] = x,
                TAX => x = a,
                TXA => a = x,
                RET_K => return k,
                RET_A => return a,
                JA => pc += k as usize,
                NEG => a = a.wrapping_neg(),
                code if code & 7 == BPF_JMP as u16 => {
                    let operand = if code & BPF_X as u16 != 0 { x } else { k };
                    let taken = match u32::from(code) & 0xf0 {
                        BPF_JEQ => a == operand,
                        BPF_JGT => a > operand,
                        BPF_JGE => a >= operand,
                        _ => a & operand != 0,
                    };
                    pc += usize::from(if taken {
                        instruction.jt
                    } else {
                        instruction.jf
                    });
                }
                code => {
                    let operand = if code & BPF_X as u16 != 0 { x } else { k };
                    a = match u32::from(code) & 0xf0 {
                        BPF_ADD => a.wrapping_add(operand),
                        BPF_SUB => a.wrapping_sub(operand),
                        BPF_MUL => a.wrapping_mul(operand),
                        BPF_DIV if operand == 0 => return 0,
                        BPF_DIV => a / operand,
                        BPF_OR => a | operand,
                        BPF_AND => a & operand,
                        BPF_XOR => a ^ operand,
                        BPF_LSH => a << (operand & 31),
                        _ => a >> (operand & 31),
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    /// The calls Docker's profile refuses with EPERM when called with zero arguments, measured on the
    /// reference by stacking a filter that answers SECCOMP_RET_TRACE for everything: the profile's
    /// ERRNO outranks it, while a call the profile lets through ends as TRACE does with no tracer.
    const MEASURED: &[u32] = &[
        18, 39, 40, 41, 42, 58, 60, 89, 97, 104, 105, 106, 112, 116, 142, 161, 162, 170, 217, 218,
        219, 224, 225, 235, 236, 237, 238, 239, 241, 244, 245, 246, 247, 248, 249, 250, 251, 252,
        253, 254, 255, 256, 257, 258, 259, 262, 265, 268, 272, 273, 280, 282, 294, 295, 296, 297,
        298, 299, 300, 301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312, 313, 314, 315,
        316, 317, 318, 319, 320, 321, 322, 323, 324, 325, 326, 327, 328, 329, 330, 331, 332, 333,
        334, 335, 336, 337, 338, 339, 340, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 351,
        352, 353, 354, 355, 356, 357, 358, 359, 360, 361, 362, 363, 364, 365, 366, 367, 368, 369,
        370, 371, 372, 373, 374, 375, 376, 377, 378, 379, 380, 381, 382, 383, 384, 385, 386, 387,
        388, 389, 390, 391, 392, 393, 394, 395, 396, 397, 398, 399, 400, 401, 402, 403, 404, 405,
        406, 407, 408, 409, 410, 411, 412, 413, 414, 415, 416, 417, 418, 419, 420, 421, 422, 423,
        425, 426, 427, 428, 429, 430, 431, 432, 433, 438, 440, 442, 443, 450,
    ];

    #[test]
    fn profile_refuses_what_the_reference_refuses() {
        let mut wrong = alloc::vec::Vec::new();
        for number in 0..1100u32 {
            if number == crate::linux::general::__NR_write
                || number == crate::linux::general::__NR_exit_group
            {
                continue;
            }
            let known = crate::syscall::name(number.into()).is_some();
            let ours = super::docker_default(number, &[0; 6], known) == Some(crate::Errno::EPERM);
            if ours != MEASURED.contains(&number) {
                wrong.push(number);
            }
        }
        assert!(wrong.is_empty(), "differ from the reference: {wrong:?}");
    }
}
