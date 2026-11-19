//! x86-64's part of the runner. A case is stopped after one instruction by the trap flag set in the frame
//! the runner returns through; an `int3` filler stops one whose trap is held back (`mov ss`). Only the
//! handlers' own frames and raw system calls are used while the case's thread pointers are set, since the
//! runner's own thread pointer is FS.

use super::{CODE, MAX_STOP_REGISTERS, Machine};
use core::ptr::{addr_of, addr_of_mut};

pub struct Cpu;

const TF: u64 = 0x100;
/// NT and ID: `rt_sigreturn` takes these from the caller's rflags, not the frame's.
const KEEP_BITS: u64 = 0x20_4000;
/// Alignment check: a handler starts with the case's AC, under which the runner's own unaligned accesses
/// would fault.
const AC: u64 = 0x4_0000;
const TRAP_TRACE: i32 = 2;
const AUDIT_ARCH_X86_64: u32 = 0xc000_003e;
const FP_XSTATE_MAGIC1: u32 = 0x4650_5853;
const ARCH_SET_GS: i64 = 0x1001;
const ARCH_SET_FS: i64 = 0x1002;
const ARCH_GET_FS: i64 = 0x1003;
const ARCH_GET_GS: i64 = 0x1004;

// The fields: the general registers in encoding order, rip, rflags, the FS and GS bases, MXCSR, the x87
// control, status and abridged tag words, R0 to R7 (physical, 80 bits each), xmm0 to xmm15 and the upper
// halves of ymm0 to ymm15.
const RCX: usize = 1;
const RIP: usize = 16;
const RFLAGS: usize = 17;
const FS: usize = 18;
const GS: usize = 19;
const MXCSR: usize = 20;
const FCW: usize = 21;
const FSW: usize = 22;
const FTW: usize = 23;
const R0: usize = 24;
const XMM: usize = 32;
const YMMH: usize = 48;
/// The frame's `gregs` index of each general register, in field order.
const GREGS: [usize; 16] = [
    libc::REG_RAX as usize,
    libc::REG_RCX as usize,
    libc::REG_RDX as usize,
    libc::REG_RBX as usize,
    libc::REG_RSP as usize,
    libc::REG_RBP as usize,
    libc::REG_RSI as usize,
    libc::REG_RDI as usize,
    libc::REG_R8 as usize,
    libc::REG_R9 as usize,
    libc::REG_R10 as usize,
    libc::REG_R11 as usize,
    libc::REG_R12 as usize,
    libc::REG_R13 as usize,
    libc::REG_R14 as usize,
    libc::REG_R15 as usize,
];

/// The runner's own frame, its XSAVE area and its thread pointers, while a case runs.
static mut HOST_GREGS: [libc::greg_t; 23] = [0; 23];
const HOST_FP_CAPACITY: usize = 8192;
static mut HOST_FP: [u8; HOST_FP_CAPACITY] = [0; HOST_FP_CAPACITY];
static mut HOST_FP_BYTES: usize = 0;
static mut HOST_FS: u64 = 0;
static mut HOST_GS: u64 = 0;

fn arch_prctl(code: i64, value: u64) -> i64 {
    let result: i64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") libc::SYS_arch_prctl => result,
            in("rdi") code,
            in("rsi") value,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        )
    };
    result
}

fn get_base(code: i64) -> u64 {
    let mut value = 0u64;
    arch_prctl(code, &raw mut value as u64);
    value
}

/// Whether the instruction loads rflags from memory, so may write TF.
fn loads_rflags(code: &[u8]) -> bool {
    use iced_x86::{Decoder, DecoderOptions, Mnemonic};
    let instruction = Decoder::with_ip(64, code, CODE, DecoderOptions::NONE).decode();
    matches!(
        instruction.mnemonic(),
        Mnemonic::Popf | Mnemonic::Popfq | Mnemonic::Iret | Mnemonic::Iretd | Mnemonic::Iretq
    )
}

/// Where the upper halves of the ymm registers are in an XSAVE area.
fn ymm_offset() -> usize {
    core::arch::x86_64::__cpuid_count(0xd, 2).ebx as usize
}

/// The legacy FXSAVE area's bytes.
const LEGACY_FP_BYTES: usize = size_of::<libc::user_fpregs_struct>();

/// MXCSR's place in it.
const MXCSR_AT: usize = core::mem::offset_of!(libc::user_fpregs_struct, mxcsr);

/// Linux's `struct _fpx_sw_bytes`, in the last 48 bytes of the legacy area of a signal frame
/// (arch/x86/include/uapi/asm/sigcontext.h).
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct FpxSoftwareBytes {
    magic1: u32,
    /// The frame's whole FP area, the XSAVE area and its trailing magic.
    extended_size: u32,
    xfeatures: u64,
    xstate_size: u32,
    padding: [u32; 7],
}

/// The frame's XSAVE area and its size: the legacy 512 bytes, or the whole area when the frame holds the
/// extended state.
unsafe fn fp_area(uc: *const libc::ucontext_t) -> (*mut u8, usize, bool) {
    unsafe {
        // musl's `mcontext_t` keeps `fpregs`, the pointer to the area, just after the general registers.
        let fp = (&raw const (*uc).uc_mcontext.gregs)
            .add(1)
            .cast::<*mut u8>()
            .read();
        let software = (fp.add(LEGACY_FP_BYTES - size_of::<FpxSoftwareBytes>())
            as *const FpxSoftwareBytes)
            .read_unaligned();
        let extended = software.magic1 == FP_XSTATE_MAGIC1;
        let size = if extended {
            software.extended_size as usize
        } else {
            LEGACY_FP_BYTES
        };
        assert!(
            size <= HOST_FP_CAPACITY,
            "the frame's FP area is {size} bytes, more than the runner keeps"
        );
        (fp, size, extended)
    }
}

unsafe fn put(at: *mut u8, bytes: &[u8]) {
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), at, bytes.len()) };
}

unsafe fn get<const N: usize>(at: *const u8) -> [u8; N] {
    unsafe { (at as *const [u8; N]).read_unaligned() }
}

/// The 80-bit value in the low bits of a field, as 16 bytes of an XSAVE slot.
fn slot(value: u128) -> [u8; 16] {
    (value & ((1 << 80) - 1)).to_le_bytes()
}

impl Machine for Cpu {
    const FILLER: u8 = crate::x86_64::FILLER;
    const CODE_SPAN: u64 = crate::x86_64::CODE_SPAN;
    const RUNNER: core::ops::Range<u64> = crate::x86_64::RUNNER;
    /// The longest instruction and the `syscall` after it.
    const WINDOW: u64 = 16 + 2;
    const AUDIT_ARCH: u32 = AUDIT_ARCH_X86_64;
    const FIELDS: &'static [&'static str] = &[
        "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12",
        "r13", "r14", "r15", "rip", "rflags", "fs_base", "gs_base", "mxcsr", "fcw", "fsw", "ftw",
        "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "xmm0", "xmm1", "xmm2", "xmm3", "xmm4",
        "xmm5", "xmm6", "xmm7", "xmm8", "xmm9", "xmm10", "xmm11", "xmm12", "xmm13", "xmm14",
        "xmm15", "ymmh0", "ymmh1", "ymmh2", "ymmh3", "ymmh4", "ymmh5", "ymmh6", "ymmh7", "ymmh8",
        "ymmh9", "ymmh10", "ymmh11", "ymmh12", "ymmh13", "ymmh14", "ymmh15",
    ];

    /// Linux refuses a base at or above the top of user space (2^47 - 4096); nothing else changes then.
    unsafe fn enter(uc: *mut libc::ucontext_t, state: &[u128]) -> Result<(), &'static str> {
        unsafe {
            HOST_FS = get_base(ARCH_GET_FS);
            HOST_GS = get_base(ARCH_GET_GS);
            if arch_prctl(ARCH_SET_FS, state[FS] as u64) != 0 {
                return Err("arch_prctl refused the FS base");
            }
            if arch_prctl(ARCH_SET_GS, state[GS] as u64) != 0 {
                arch_prctl(ARCH_SET_FS, HOST_FS);
                return Err("arch_prctl refused the GS base");
            }
            let gregs = &mut (*uc).uc_mcontext.gregs;
            *addr_of_mut!(HOST_GREGS) = *gregs;
            for (field, &at) in GREGS.iter().enumerate() {
                gregs[at] = state[field] as i64;
            }
            gregs[libc::REG_RIP as usize] = CODE as i64;
            gregs[libc::REG_EFL as usize] = (state[RFLAGS] as u64 | TF) as i64;

            let (fp, size, extended) = fp_area(uc);
            HOST_FP_BYTES = size;
            core::ptr::copy_nonoverlapping(fp, addr_of_mut!(HOST_FP) as *mut u8, size);
            // Control, status and abridged tag words; the last-instruction pointers zero.
            core::ptr::write_bytes(fp, 0, 24);
            put(fp, &(state[FCW] as u16).to_le_bytes());
            put(fp.add(2), &(state[FSW] as u16).to_le_bytes());
            *fp.add(4) = state[FTW] as u8;
            put(fp.add(MXCSR_AT), &(state[MXCSR] as u32).to_le_bytes());
            // The frame holds the stack in order from ST(0); the state the physical registers.
            let top = (state[FSW] as usize >> 11) & 7;
            for i in 0..8 {
                put(fp.add(32 + 16 * i), &slot(state[R0 + (top + i) % 8]));
            }
            for i in 0..16 {
                put(fp.add(160 + 16 * i), &state[XMM + i].to_le_bytes());
            }
            if extended {
                let ymm = ymm_offset();
                for i in 0..16 {
                    put(fp.add(ymm + 16 * i), &state[YMMH + i].to_le_bytes());
                }
                let bv = u64::from_le_bytes(get(fp.add(512))) | 7;
                put(fp.add(512), &bv.to_le_bytes());
            }
            let keep = state[RFLAGS] as u64 & KEEP_BITS;
            core::arch::asm!("pushfq", "or qword ptr [rsp], {0}", "popfq", in(reg) keep);
        }
        Ok(())
    }

    /// TF in the frame is the runner's own unless the instruction loaded rflags (`popf`, `iret`) and
    /// finished, which a `TRAP_TRACE` stop shows; otherwise the case's starting TF is what it left.
    unsafe fn leave(
        code: &[u8],
        start: &[u128],
        uc: *mut libc::ucontext_t,
        info: *const libc::siginfo_t,
        state: &mut [u128],
    ) {
        unsafe {
            core::arch::asm!("pushfq", "and qword ptr [rsp], {0}", "popfq", in(reg) !(KEEP_BITS | AC));
            state[FS] = get_base(ARCH_GET_FS).into();
            state[GS] = get_base(ARCH_GET_GS).into();
            arch_prctl(ARCH_SET_FS, HOST_FS);
            arch_prctl(ARCH_SET_GS, HOST_GS);
            let gregs = &mut (*uc).uc_mcontext.gregs;
            for (field, &at) in GREGS.iter().enumerate() {
                state[field] = (gregs[at] as u64).into();
            }
            state[RIP] = (gregs[libc::REG_RIP as usize] as u64).into();
            let rflags = gregs[libc::REG_EFL as usize] as u64;
            let traced = (*info).si_signo == libc::SIGTRAP && (*info).si_code == TRAP_TRACE;
            let tf = if traced && loads_rflags(code) {
                rflags & TF
            } else {
                start[RFLAGS] as u64 & TF
            };
            state[RFLAGS] = (rflags & !TF | tf).into();
            *gregs = *addr_of!(HOST_GREGS);

            let (fp, _, extended) = fp_area(uc);
            // A component the CPU holds in its first state is not written: x87 reads as after `fninit`, the
            // vector registers as zero.
            let bv = if extended {
                u64::from_le_bytes(get(fp.add(512)))
            } else {
                3
            };
            state[MXCSR] = u32::from_le_bytes(get(fp.add(MXCSR_AT))).into();
            if bv & 1 != 0 {
                state[FCW] = u16::from_le_bytes(get(fp)).into();
                state[FSW] = u16::from_le_bytes(get(fp.add(2))).into();
                state[FTW] = (*fp.add(4)).into();
                let top = (state[FSW] as usize >> 11) & 7;
                for i in 0..8 {
                    let value = u128::from_le_bytes(get(fp.add(32 + 16 * i)));
                    state[R0 + (top + i) % 8] = value & ((1 << 80) - 1);
                }
            } else {
                state[FCW] = 0x37f;
                state[FSW] = 0;
                state[FTW] = 0;
                state[R0..R0 + 8].fill(0);
            }
            for i in 0..16 {
                state[XMM + i] = if bv & 2 != 0 {
                    u128::from_le_bytes(get(fp.add(160 + 16 * i)))
                } else {
                    0
                };
                state[YMMH + i] = if bv & 4 != 0 {
                    u128::from_le_bytes(get(fp.add(ymm_offset() + 16 * i)))
                } else {
                    0
                };
            }
            core::ptr::copy_nonoverlapping(addr_of!(HOST_FP) as *const u8, fp, HOST_FP_BYTES);
        }
    }

    /// A trap-flag stop with rip still at CODE: a rep string instruction one element on, or a gather after
    /// the elements before a faulting one. `limit` gives steps only to those.
    unsafe fn unfinished(
        _: &[u8],
        uc: *const libc::ucontext_t,
        info: *const libc::siginfo_t,
    ) -> bool {
        unsafe {
            (*info).si_code == TRAP_TRACE
                && (*uc).uc_mcontext.gregs[libc::REG_RIP as usize] as u64 == CODE
        }
    }

    /// A rep string instruction's count register (rcx, or ecx with an address-size prefix) and a gather's
    /// most elements, 8; nothing for any other instruction.
    fn limit(code: &[u8], state: &[u128]) -> u64 {
        use iced_x86::{Decoder, DecoderOptions, OpKind};
        let instruction = Decoder::with_ip(64, code, CODE, DecoderOptions::NONE).decode();
        if instruction.is_string_instruction()
            && (instruction.has_rep_prefix() || instruction.has_repne_prefix())
        {
            let count = state[RCX] as u64;
            let counts_ecx = instruction
                .op_kinds()
                .any(|k| matches!(k, OpKind::MemorySegESI | OpKind::MemoryESEDI));
            if counts_ecx {
                count & 0xffff_ffff
            } else {
                count
            }
        } else if instruction.is_vsib() {
            8
        } else {
            0
        }
    }

    fn synchronise(_: u64, _: u64) {}

    /// Every general register zero but rsp, rflags 0x202, MXCSR 0x1F80, the x87 control word 0x37F, the
    /// vector registers zero, and no thread pointers.
    unsafe fn blob_start(uc: *mut libc::ucontext_t, entry: u64, stack: u64) {
        unsafe {
            let gregs = &mut (*uc).uc_mcontext.gregs;
            for &at in &GREGS {
                gregs[at] = 0;
            }
            gregs[libc::REG_RSP as usize] = stack as i64;
            gregs[libc::REG_RIP as usize] = entry as i64;
            gregs[libc::REG_EFL as usize] = 0x202;
            let (fp, _, extended) = fp_area(uc);
            core::ptr::write_bytes(fp, 0, 24);
            put(fp, &0x37fu16.to_le_bytes());
            put(fp.add(MXCSR_AT), &0x1f80u32.to_le_bytes());
            core::ptr::write_bytes(fp.add(32), 0, 128 + 256);
            if extended {
                core::ptr::write_bytes(fp.add(ymm_offset()), 0, 256);
            }
            arch_prctl(ARCH_SET_FS, 0);
            arch_prctl(ARCH_SET_GS, 0);
        }
    }

    /// The general registers in field order, rip and rflags.
    unsafe fn blob_stop(uc: *const libc::ucontext_t, out: &mut [u64; MAX_STOP_REGISTERS]) -> usize {
        unsafe {
            let gregs = &(*uc).uc_mcontext.gregs;
            for (field, &at) in GREGS.iter().enumerate() {
                out[field] = gregs[at] as u64;
            }
            out[16] = gregs[libc::REG_RIP as usize] as u64;
            out[17] = gregs[libc::REG_EFL as usize] as u64;
        }
        18
    }
}
