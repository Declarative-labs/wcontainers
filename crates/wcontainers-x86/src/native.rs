//! How the native output carries each x86-64 instruction over to x86-64 host code (docs/x86-native-output.md
//! section 3.1): the route of every decoded form, per way of running.

use iced_x86::{MemorySize, Mnemonic, OpKind, Register};
use wcontainers_guest_cpu::Route;

use crate::Offer;
use crate::claims::{CLAIM_KERNEL, CLAIM_NEVER};
use crate::decode::{Instruction, claim, told_feature};
use crate::meaning::{is_string, written};

/// How translated code reaches guest memory, the thread area and the inside; part of the cache key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Way {
    /// Inside a virtual machine whose floor owns the system call and the interrupt gates.
    Machine,
    /// Each guest process as a host process.
    Process,
    /// The kernel in the same host process, guest memory a window checked by this form.
    InProcess(Check),
}

/// The form of a guest access in the in-process way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Check {
    /// The address taken modulo 2^32 inside the window.
    Wrapping,
    /// The meaning's fault for an address outside the guest's memory.
    Exact,
}

impl Way {
    /// The segment register whose base is the thread area.
    pub fn area_segment(self) -> Register {
        match self {
            Way::Machine | Way::Process => Register::GS,
            Way::InProcess(_) => Register::FS,
        }
    }
}

/// What the probe at start found of the host, for the forms whose host execution may differ from the meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HostMatches {
    /// The host takes 5-level linear addresses, so it accepts FS bases the guest's 4-level rule refuses.
    pub five_level_paging: bool,
    /// The host leaves the meaning's partial writes behind a fault (`enter` with a level, `fstp m80`, `fbstp`).
    pub partial_writes: bool,
    /// The host gives the meaning's results where Intel leaves them undefined.
    pub undefined_results: bool,
}

/// The route of one decoded instruction under `way`, for a process told of `offer`'s features.
pub fn route(way: Way, i: &Instruction, host: &HostMatches, offer: &Offer) -> Route {
    use Mnemonic::*;
    let ins = &i.iced;
    let m = ins.mnemonic();
    let refused = i.too_long
        || ins.is_invalid()
        || matches!(claim(ins.code()), CLAIM_KERNEL | CLAIM_NEVER)
        || told_feature(claim(ins.code())).is_some_and(|at| !offer.tells(at))
        || !written(i);
    if refused || matches!(m, Int3 | Int1 | Int | Into | Ud0 | Ud1 | Ud2 | Sysenter) {
        return Route::Trap;
    }
    if matches!(
        m,
        Nop | Reservednop | Prefetchnta | Prefetcht0 | Prefetcht1 | Prefetcht2 | Prefetchw
    ) {
        return Route::Nothing;
    }
    if answered(way, i, host) {
        return Route::Kernel;
    }
    if matches!(
        m,
        Ldmxcsr
            | Stmxcsr
            | Vldmxcsr
            | Vstmxcsr
            | Fxsave
            | Fxsave64
            | Fxrstor
            | Fxrstor64
            | Fnstenv
            | Fldenv
            | Fnsave
            | Frstor
    ) {
        return Route::FloatStatus;
    }
    if transfers(i) {
        return Route::Transfer;
    }
    let area = way.area_segment();
    let on_area = |segment: Register| match way {
        Way::InProcess(_) => matches!(segment, Register::FS | Register::GS),
        Way::Machine | Way::Process => segment == area,
    };
    let base_of_area = match m {
        Rdgsbase => true,
        Rdfsbase => area == Register::FS,
        _ => false,
    };
    // `lea` forms an offset alone, whatever segment it names.
    if base_of_area || m != Lea && has_memory_operand(i) && on_area(ins.memory_segment()) {
        return Route::Area;
    }
    if ins.is_ip_rel_memory_operand() {
        return Route::Rebase;
    }
    Route::Copy
}

/// Whether the inside answers the instruction as its meaning does (section 3.5).
fn answered(way: Way, i: &Instruction, host: &HostMatches) -> bool {
    use Mnemonic::*;
    let ins = &i.iced;
    let m = ins.mnemonic();
    let in_process = matches!(way, Way::InProcess(_));
    let segment_register = (0..ins.op_count())
        .any(|n| ins.op_kind(n) == OpKind::Register && ins.op_register(n).is_segment_register());
    let far = matches!(ins.op_kind(0), OpKind::FarBranch16 | OpKind::FarBranch32)
        || matches!(m, Call | Jmp)
            && matches!(
                ins.memory_size(),
                MemorySize::SegPtr16 | MemorySize::SegPtr32 | MemorySize::SegPtr64
            );
    // Their address is implicit in rsi, rdi or rbx, so it cannot be rebuilt on the guest's segment base.
    let implicit = is_string(i) || matches!(m, Xlatb | Maskmovq | Maskmovdqu | Vmaskmovdqu);
    let overridden = ins.segment_prefix() != Register::None;
    let implicit_on_area = implicit
        && if in_process {
            overridden
        } else {
            overridden && ins.memory_segment() == way.area_segment()
        };
    let gather = matches!(
        m,
        Vgatherdpd
            | Vgatherdps
            | Vgatherqpd
            | Vgatherqps
            | Vpgatherdd
            | Vpgatherdq
            | Vpgatherqd
            | Vpgatherqq
    );
    let partial = m == Enter && ins.immediate8_2nd() != 0
        || m == Fstp && ins.memory_size() == MemorySize::Float80
        || m == Fbstp;
    segment_register
        || far
        || implicit_on_area
        || matches!(
            m,
            Cpuid
                | Rdtsc
                | Rdtscp
                | Rdpid
                | Xgetbv
                | Popf
                | Popfq
                | Iret
                | Iretd
                | Iretq
                | Retf
                | Lss
                | Lfs
                | Lgs
                | Sgdt
                | Sidt
                | Sldt
                | Str
                | Smsw
                | Lar
                | Lsl
                | Verr
                | Verw
                | Rcpps
                | Rcpss
                | Rsqrtps
                | Rsqrtss
                | Vrcpps
                | Vrcpss
                | Vrsqrtps
                | Vrsqrtss
                | Xsave
                | Xsave64
                | Xrstor
                | Xrstor64
                | Wrgsbase
        )
        || crate::x87::transcendental(m)
        || m == Syscall && way != Way::Machine
        || m == Wrfsbase && (in_process || way == Way::Process && host.five_level_paging)
        || in_process && is_string(i) && i.address32
        || way == Way::InProcess(Check::Exact) && gather
        || partial && !host.partial_writes
        || undefined_result(m) && !host.undefined_results
}

/// A branch, call, return or jump within the code segment.
fn transfers(i: &Instruction) -> bool {
    use Mnemonic::*;
    let ins = &i.iced;
    ins.is_jcc_short_or_near()
        || matches!(
            ins.mnemonic(),
            Call | Jmp | Ret | Loop | Loope | Loopne | Jrcxz | Jecxz
        )
}

fn has_memory_operand(i: &Instruction) -> bool {
    (0..i.iced.op_count()).any(|n| {
        matches!(
            i.iced.op_kind(n),
            OpKind::Memory
                | OpKind::MemorySegSI
                | OpKind::MemorySegESI
                | OpKind::MemorySegRSI
                | OpKind::MemorySegDI
                | OpKind::MemorySegEDI
                | OpKind::MemorySegRDI
        )
    })
}

/// The forms whose results or flags Intel leaves undefined and the meaning fixes to the reference CPU's.
fn undefined_result(m: Mnemonic) -> bool {
    use Mnemonic::*;
    matches!(
        m,
        Mul | Imul
            | Div
            | Idiv
            | Shl
            | Sal
            | Shr
            | Sar
            | Rol
            | Ror
            | Rcl
            | Rcr
            | Shld
            | Shrd
            | And
            | Or
            | Xor
            | Test
            | Bt
            | Bts
            | Btr
            | Btc
            | Bsf
            | Bsr
            | Andn
            | Bextr
            | Blsi
            | Blsmsk
            | Blsr
            | Bzhi
            | Lzcnt
            | Tzcnt
    )
}
