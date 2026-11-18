//! ARM64's part of the runner. A case stops wherever control lands, on the `udf #0` filler or by its own
//! fault, so nothing is stepped.

use super::{MAX_STOP_REGISTERS, Machine};
use core::ptr::{addr_of, addr_of_mut};

pub struct Cpu;

const NZCV: u64 = 0xf000_0000;
/// `AUDIT_ARCH_AARCH64`.
const AUDIT_ARCH_AARCH64: u32 = 0xc000_00b7;

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
unsafe fn frame_vectors(uc: *mut libc::ucontext_t) -> *mut [u128; 32] {
    const FPSIMD_MAGIC: u32 = 0x4650_8001;
    unsafe {
        let record = (&raw mut (*uc).uc_mcontext as *mut u8).add(288);
        assert_eq!(
            (record as *const u32).read(),
            FPSIMD_MAGIC,
            "the signal frame starts with the FPSIMD record"
        );
        record.add(16) as *mut [u128; 32]
    }
}

/// FPSR and FPCR in a signal frame: right after the FPSIMD record's header.
unsafe fn frame_float(uc: *mut libc::ucontext_t) -> *mut [u32; 2] {
    unsafe { (frame_vectors(uc) as *mut u8).sub(8) as *mut [u32; 2] }
}

// The fields: x0 to x30, sp, pc, the flags (at their `pstate` bits), the thread pointer, v0 to v31, FPSR, FPCR.
const SP: usize = 31;
const PC: usize = 32;
const FLAGS: usize = 33;
const THREAD_POINTER: usize = 34;
const VECTORS: usize = 35;
const FPSR: usize = 67;

impl Machine for Cpu {
    const FILLER: u8 = aarch64::FILLER;
    const CODE_SPAN: u64 = aarch64::CODE_SPAN;
    /// The instruction and the address after a `svc` there.
    const WINDOW: u64 = 8;
    const AUDIT_ARCH: u32 = AUDIT_ARCH_AARCH64;
    const FIELDS: &'static [&'static str] = &[
        "x0",
        "x1",
        "x2",
        "x3",
        "x4",
        "x5",
        "x6",
        "x7",
        "x8",
        "x9",
        "x10",
        "x11",
        "x12",
        "x13",
        "x14",
        "x15",
        "x16",
        "x17",
        "x18",
        "x19",
        "x20",
        "x21",
        "x22",
        "x23",
        "x24",
        "x25",
        "x26",
        "x27",
        "x28",
        "x29",
        "x30",
        "sp",
        "pc",
        "nzcv",
        "tpidr_el0",
        "v0",
        "v1",
        "v2",
        "v3",
        "v4",
        "v5",
        "v6",
        "v7",
        "v8",
        "v9",
        "v10",
        "v11",
        "v12",
        "v13",
        "v14",
        "v15",
        "v16",
        "v17",
        "v18",
        "v19",
        "v20",
        "v21",
        "v22",
        "v23",
        "v24",
        "v25",
        "v26",
        "v27",
        "v28",
        "v29",
        "v30",
        "v31",
        "fpsr",
        "fpcr",
    ];

    unsafe fn enter(uc: *mut libc::ucontext_t, state: &[u128]) {
        unsafe {
            (*addr_of_mut!(HOST)).write(core::ptr::read(&(*uc).uc_mcontext));
            HOST_THREAD_POINTER = thread_pointer();
            let context = &mut (*uc).uc_mcontext;
            for (register, value) in context.regs.iter_mut().zip(state) {
                *register = *value as u64;
            }
            context.sp = state[SP] as u64;
            context.pc = super::CODE;
            context.pstate = (context.pstate & !NZCV) | (state[FLAGS] as u64 & NZCV);
            let mut vectors = [0u128; 32];
            vectors.copy_from_slice(&state[VECTORS..VECTORS + 32]);
            frame_vectors(uc).write_unaligned(vectors);
            frame_float(uc).write_unaligned([state[FPSR] as u32, state[FPSR + 1] as u32]);
            set_thread_pointer(state[THREAD_POINTER] as u64);
        }
    }

    unsafe fn leave(uc: *mut libc::ucontext_t, state: &mut [u128]) {
        unsafe {
            let guest_thread_pointer = thread_pointer();
            set_thread_pointer(HOST_THREAD_POINTER);
            let context = &(*uc).uc_mcontext;
            for (value, register) in state.iter_mut().zip(context.regs) {
                *value = register.into();
            }
            state[SP] = context.sp.into();
            state[PC] = context.pc.into();
            state[FLAGS] = (context.pstate & NZCV).into();
            state[THREAD_POINTER] = guest_thread_pointer.into();
            state[VECTORS..VECTORS + 32].copy_from_slice(&frame_vectors(uc).read_unaligned());
            let [fpsr, fpcr] = frame_float(uc).read_unaligned();
            state[FPSR] = fpsr.into();
            state[FPSR + 1] = fpcr.into();
            (*uc).uc_mcontext = (*addr_of!(HOST)).assume_init_read();
        }
    }

    unsafe fn unfinished(_: &[u8], _: *const libc::ucontext_t, _: *const libc::siginfo_t) -> bool {
        false
    }

    fn limit(_: &[u8], _: &[u128]) -> u64 {
        0
    }

    fn synchronise(address: u64, length: u64) {
        let mut line = address & !63;
        while line < address + length {
            unsafe {
                core::arch::asm!(
                    "dc cvau, {0}", "dsb ish", "ic ivau, {0}", "dsb ish", "isb",
                    in(reg) line,
                )
            };
            line += 64;
        }
    }

    /// Every register zero but sp and pc.
    unsafe fn blob_start(uc: *mut libc::ucontext_t, entry: u64, stack: u64) {
        unsafe {
            let context = &mut (*uc).uc_mcontext;
            context.regs = [0; 31];
            context.sp = stack;
            context.pc = entry;
            context.pstate &= !NZCV;
            frame_vectors(uc).write_unaligned([0; 32]);
            frame_float(uc).write_unaligned([0; 2]);
            set_thread_pointer(0);
        }
    }

    /// x0 to x30, sp and pc.
    unsafe fn blob_stop(uc: *const libc::ucontext_t, out: &mut [u64; MAX_STOP_REGISTERS]) -> usize {
        unsafe {
            let context = &(*uc).uc_mcontext;
            out[..31].copy_from_slice(&context.regs);
            out[31] = context.sp;
            out[32] = context.pc;
        }
        33
    }
}

use crate::aarch64;
