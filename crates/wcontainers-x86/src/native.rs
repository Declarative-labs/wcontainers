//! How the native output carries each x86-64 instruction over to x86-64 host code (docs/x86-native-output.md
//! section 3.1): the route of every decoded form, per way of running.

use std::ops::Range;

use iced_x86::{MemorySize, Mnemonic, OpKind, Register};
use wcontainers_guest_cpu::position::{AddressPlace, Base, DirectExit, Index, Operand, Segment};
use wcontainers_guest_cpu::{HostCode, InsideCall, Native, NativeClass, Route, Stopping};

use crate::answer::Question;
use crate::claims::{CLAIM_KERNEL, CLAIM_NEVER};
use crate::decode::{Instruction, claim, told_feature};
use crate::guest::{Control, Hook};
use crate::meaning::{is_string, written};
use crate::operand::{RSP, W32, W64, general};
use crate::x87::{Code, Operation};
use crate::{Offer, USER_END, X86_64, carry};

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

/// What a translation is made for besides the way: the host's matches and the features the process was told of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Target {
    pub host: HostMatches,
    pub offer: Offer,
}

/// How far a rel32 branch of the guest reaches either way.
const BRANCH_REACH: u64 = 1 << (u32::BITS - 1);

impl Native for X86_64 {
    type Way = Way;
    type Target = Target;

    fn route(way: &Way, i: &Instruction, target: &Target) -> Route {
        route(*way, i, &target.host, &target.offer)
    }

    fn hook_class(hook: Hook, first: Option<i64>) -> NativeClass {
        let answered = |code: i64| {
            matches!(
                Code::decode(code as i32).operation,
                Operation::Sine
                    | Operation::Cosine
                    | Operation::SineCosine
                    | Operation::Tangent
                    | Operation::ExponentMinusOne
                    | Operation::ArcTangent
                    | Operation::Logarithm
                    | Operation::LogarithmPlusOne
            )
        };
        match hook {
            Hook::Clock => NativeClass::Routed(Route::Kernel),
            Hook::X87 if first.is_some_and(answered) => NativeClass::Routed(Route::Kernel),
            Hook::Float | Hook::X87 => NativeClass::Hardware,
        }
    }

    fn control_class(control: Control, write: bool) -> NativeClass {
        match control {
            Control::Rflags if !write => NativeClass::Hardware,
            Control::Rflags | Control::Selectors | Control::StepHeld => {
                NativeClass::Routed(Route::Kernel)
            }
            Control::Mxcsr | Control::MxcsrTracking if write => {
                NativeClass::Routed(Route::FloatStatus)
            }
            Control::Mxcsr
            | Control::MxcsrTracking
            | Control::FloatException
            | Control::X87Words
            | Control::X87StatusTag
            | Control::X87Header
            | Control::X87High(_)
            | Control::FloatOperand(_) => NativeClass::Hardware,
            // Only the saves read the x87 pointers; the meaning's write at each x87 instruction is the run's
            // store into the register area natively (section 3.1).
            Control::X87InstructionPointer | Control::X87DataPointer if !write => {
                NativeClass::Routed(Route::FloatStatus)
            }
            Control::X87InstructionPointer | Control::X87DataPointer => NativeClass::Bookkeeping,
        }
    }

    /// A loop's homes: what is settled for its turns is where the withheld registers it names are held.
    type LoopInvariant = carry::Homes;

    fn loop_invariant(part: &[(u64, Instruction)]) -> carry::Homes {
        carry::Homes::of(part)
    }

    fn test_loop_invariant(way: &Way, code: &mut HostCode, homes: &carry::Homes) -> Option<usize> {
        carry::enter_homes(*way, code, homes)
    }

    fn carry(
        way: &Way,
        target: &Target,
        _: &[(u64, Instruction)],
        i: &Instruction,
        bytes: &[u8],
        address: u64,
        homes: &carry::Homes,
        code: &mut HostCode,
    ) {
        let route = route(*way, i, &target.host, &target.offer);
        carry::carry(*way, route, i, bytes, address, homes, code);
    }

    fn count(code: &mut HostCode, events: u32) {
        carry::count(code, events);
    }

    fn call(way: &Way, code: &mut HostCode, call: InsideCall) {
        carry::call(*way, code, call);
    }

    fn exit(way: &Way, code: &mut HostCode, target: u64) {
        carry::exit(*way, code, target, None, &carry::Homes::default());
    }

    fn link(code: &mut HostCode, exit: &DirectExit, to: usize) -> bool {
        carry::link(code, exit, to)
    }

    fn stopping(i: &Instruction, _: u64) -> Stopping {
        let ins = &i.iced;
        if i.too_long || ins.is_invalid() || !written(i) {
            return Stopping::Illegal;
        }
        if ins.mnemonic() == Mnemonic::Syscall {
            return Stopping::Syscall;
        }
        let width = if i.address32 { W32 } else { W64 };
        let register = |r: Register| match r {
            Register::None => Some(Base::Zero),
            Register::RIP | Register::EIP => Some(Base::Pc),
            r => general(r)
                .filter(|g| g.width == width)
                .map(|g| match g.number {
                    RSP => Base::StackPointer,
                    n => Base::Register(n),
                }),
        };
        let memory = ins.mnemonic() != Mnemonic::Lea
            && (0..ins.op_count()).any(|n| ins.op_kind(n) == OpKind::Memory);
        let (Some(base), Some(index)) = (register(ins.memory_base()), register(ins.memory_index()))
        else {
            return Stopping::Plain;
        };
        if !memory {
            return Stopping::Plain;
        }
        let offset = if i.address32 && base != Base::Pc {
            i64::from(ins.memory_displacement32() as i32)
        } else {
            ins.memory_displacement64() as i64
        };
        Stopping::Access(Operand {
            base,
            offset,
            index: (index != Base::Zero).then(|| Index {
                register: index,
                bits: width,
                signed: false,
                shift: ins.memory_index_scale().trailing_zeros() as u8,
            }),
            segment: match ins.memory_segment() {
                Register::FS => Some(Segment::Fs),
                Register::GS => Some(Segment::Gs),
                _ => None,
            },
            address_bits: width,
        })
    }

    fn movable() -> Range<u64> {
        BRANCH_REACH..USER_END - BRANCH_REACH
    }

    fn write_place(code: &mut [u8], place: &AddressPlace) {
        carry::write_place(code, place);
    }
}

/// What the inside's `answer` is asked for an instruction of the Kernel route, where it answers it.
pub fn question(i: &Instruction) -> Option<Question> {
    match i.iced.mnemonic() {
        Mnemonic::Cpuid => Some(Question::Cpuid),
        Mnemonic::Xgetbv => Some(Question::Xgetbv),
        _ => crate::fpu::transcendental_code(i).map(Question::X87),
    }
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
    // A vector index cannot take part in the 32-bit `lea` the area's operand is rebuilt with.
    let index = ins.memory_index();
    let vector_index_on_area = (index.is_xmm() || index.is_ymm())
        && !in_process
        && ins.memory_segment() == way.area_segment()
        && i.address32;
    // Under 0x67 a bit test's register offset is added to its operand's address modulo 2^32 too, which the
    // 64-bit sum on the guest's base no longer wraps.
    let bit_offset_on_area = matches!(m, Bt | Bts | Btr | Btc)
        && ins.op1_kind() == OpKind::Register
        && ins.op0_kind() == OpKind::Memory
        && !in_process
        && ins.memory_segment() == way.area_segment()
        && i.address32;
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
        || vector_index_on_area
        || bit_offset_on_area
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
pub(crate) fn undefined_result(m: Mnemonic) -> bool {
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
