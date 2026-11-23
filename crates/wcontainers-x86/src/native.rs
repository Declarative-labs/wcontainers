//! How the native output carries each x86-64 instruction over to x86-64 host code (docs/x86-native-output.md
//! section 3.1): the route of every decoded form, per way of running.

use std::ops::Range;

use iced_x86::{MemorySize, Mnemonic, OpKind, Register};
use wcontainers_guest_cpu::position::{AddressPlace, Base, DirectExit, Index, Operand, Segment};
use wcontainers_guest_cpu::{HostCode, InsideCall, Native, NativeClass, Route, Stopping};

use crate::answer::{
    DescriptorRead, FloatStatus, Place, Question, Reciprocal, SegmentName, VectorEncoding,
};
use crate::claims::{CLAIM_KERNEL, CLAIM_NEVER};
use crate::decode::{Instruction, claim, told_feature};
use crate::guest::{Control, Hook};
use crate::meaning::{ENTER_LEVELS, is_string, written};
use crate::operand::{RSP, W32, W64, general};
use crate::segments::{is_far, operand_bytes};
use crate::x87::{Code, Operation};
use crate::{Offer, SegmentRegister, USER_END, X86_64, carry};

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
        let memory = ins.mnemonic() != Mnemonic::Lea
            && (0..ins.op_count()).any(|n| ins.op_kind(n) == OpKind::Memory);
        match operand(i) {
            Some(operand) if memory => Stopping::Access(operand),
            _ => Stopping::Plain,
        }
    }

    fn movable() -> Range<u64> {
        BRANCH_REACH..USER_END - BRANCH_REACH
    }

    fn write_place(code: &mut [u8], place: &AddressPlace) {
        carry::write_place(code, place);
    }
}

/// How the instruction's memory operand forms its address, where its base and index are general registers or rip
/// (a vector index has no such form), for an instruction decoded at address 0.
fn operand(i: &Instruction) -> Option<Operand> {
    let ins = &i.iced;
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
    let base = register(ins.memory_base())?;
    let index = register(ins.memory_index())?;
    let offset = if i.address32 && base != Base::Pc {
        i64::from(ins.memory_displacement32() as i32)
    } else {
        ins.memory_displacement64() as i64
    };
    Some(Operand {
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

/// Operand `n` of the instruction as an answer reaches it.
fn place(i: &Instruction, n: u32) -> Option<Place> {
    let ins = &i.iced;
    match ins.op_kind(n) {
        OpKind::Memory => operand(i).map(Place::Memory),
        OpKind::Register => {
            let r = ins.op_register(n);
            if r.is_xmm() || r.is_ymm() {
                return Some(Place::Vector(r.number() as u8));
            }
            general(r).filter(|g| !g.high).map(|g| Place::General {
                number: g.number,
                bits: g.width,
            })
        }
        _ => None,
    }
}

/// What the inside's `answer` is asked for an instruction of the Kernel route, where it answers it.
pub fn question(i: &Instruction) -> Option<Question> {
    use Mnemonic::*;
    let ins = &i.iced;
    let m = ins.mnemonic();
    let vector = |n: u32| match place(i, n)? {
        Place::Vector(v) => Some(v),
        _ => None,
    };
    let general_destination = || match place(i, 0)? {
        Place::General { number, bits } => Some((number, bits)),
        _ => None,
    };
    Some(match m {
        Cpuid => Question::Cpuid,
        Xgetbv => Question::Xgetbv,
        Rcpps | Rcpss | Rsqrtps | Rsqrtss | Vrcpps | Vrcpss | Vrsqrtps | Vrsqrtss => {
            let scalar = matches!(m, Rcpss | Rsqrtss | Vrcpss | Vrsqrtss);
            let encoded = matches!(m, Vrcpps | Vrcpss | Vrsqrtps | Vrsqrtss);
            let destination = vector(0)?;
            let (merged, source) = if encoded && scalar {
                (vector(1)?, place(i, 2)?)
            } else {
                (destination, place(i, 1)?)
            };
            Question::Reciprocal(Reciprocal {
                root: matches!(m, Rsqrtps | Rsqrtss | Vrsqrtps | Vrsqrtss),
                scalar,
                encoding: match (encoded, ins.op_register(0).is_ymm()) {
                    (false, _) => VectorEncoding::Legacy,
                    (true, false) => VectorEncoding::Vex128,
                    (true, true) => VectorEncoding::Vex256,
                },
                destination,
                merged,
                source,
            })
        }
        Popf | Popfq => Question::PopFlags {
            bytes: crate::operand::stack_bytes(i) as u8,
        },
        Sldt | Str | Smsw | Sgdt | Sidt | Verr | Verw => Question::Descriptor {
            read: match m {
                Sldt => DescriptorRead::LocalTable,
                Str => DescriptorRead::TaskRegister,
                Smsw => DescriptorRead::MachineStatus,
                Sgdt => DescriptorRead::GlobalTable,
                Sidt => DescriptorRead::InterruptTable,
                Verr => DescriptorRead::Readable,
                _ => DescriptorRead::Writable,
            },
            operand: place(i, 0)?,
        },
        Lar | Lsl => {
            let (number, bits) = general_destination()?;
            Question::Descriptor {
                read: if m == Lar {
                    DescriptorRead::Rights { number, bits }
                } else {
                    DescriptorRead::Limit { number, bits }
                },
                operand: place(i, 1)?,
            }
        }
        Mov if segment(ins.op_register(1)).is_some() => Question::SegmentRead {
            from: segment(ins.op_register(1))?,
            to: place(i, 0)?,
        },
        Mov if segment(ins.op_register(0)).is_some() => Question::SegmentWrite {
            to: segment(ins.op_register(0))?,
            from: place(i, 1)?,
        },
        Push => Question::SegmentPush {
            from: segment(ins.op_register(0))?,
            bytes: crate::operand::stack_bytes(i) as u8,
        },
        Pop => Question::SegmentPop {
            to: segment(ins.op_register(0))?,
            bytes: crate::operand::stack_bytes(i) as u8,
        },
        Lfs | Lgs | Lss => {
            let (number, bits) = general_destination()?;
            Question::FarPointer {
                to: match m {
                    Lfs => SegmentName::Kept(SegmentRegister::Fs),
                    Lgs => SegmentName::Kept(SegmentRegister::Gs),
                    _ => SegmentName::Stack,
                },
                number,
                offset_bytes: bits / u8::BITS as u8,
                at: operand(i)?,
            }
        }
        Rdtsc | Rdtscp => Question::TimeStamp {
            processor: m == Rdtscp,
        },
        Wrfsbase | Wrgsbase => Question::WriteBase {
            gs: m == Wrgsbase,
            from: place(i, 0)?,
        },
        Retf => Question::FarReturn {
            bytes: operand_bytes(i) as u8,
            release: if ins.op_count() > 0 {
                ins.immediate16()
            } else {
                0
            },
        },
        Iret | Iretd | Iretq => Question::InterruptReturn {
            bytes: operand_bytes(i) as u8,
        },
        Xsave | Xsave64 | Xrstor | Xrstor64 => Question::XState {
            restore: matches!(m, Xrstor | Xrstor64),
            wide: matches!(m, Xsave64 | Xrstor64),
            at: operand(i)?,
        },
        Call | Jmp if is_far(i) => Question::FarBranch {
            call: m == Call,
            bytes: operand_bytes(i) as u8,
            at: operand(i)?,
            length: ins.len() as u8,
        },
        Enter => Question::Enter {
            size: ins.immediate16(),
            level: ins.immediate8_2nd() % ENTER_LEVELS,
            bytes: crate::operand::stack_bytes(i) as u8,
        },
        Fst | Fstp | Fist | Fistp | Fisttp | Fbstp => Question::X87Store {
            code: crate::fpu::store_code(i)?,
            at: operand(i)?,
            bytes: crate::operand::memory_bytes(i) as u8,
        },
        _ => crate::fpu::transcendental_code(i).map(Question::X87)?,
    })
}

/// What the inside's second call is told of an instruction of the Float status route.
pub fn float_status(i: &Instruction) -> Option<FloatStatus> {
    use Mnemonic::*;
    let m = i.iced.mnemonic();
    Some(match m {
        Ldmxcsr | Vldmxcsr => FloatStatus::LoadMxcsr,
        Stmxcsr | Vstmxcsr => FloatStatus::StoreMxcsr,
        Fxsave | Fxsave64 | Fxrstor | Fxrstor64 => FloatStatus::Image {
            restore: matches!(m, Fxrstor | Fxrstor64),
            wide: matches!(m, Fxsave64 | Fxrstor64),
            at: operand(i)?,
        },
        Fnstenv | Fnsave | Fldenv | Frstor => FloatStatus::Environment {
            restore: matches!(m, Fldenv | Frstor),
            registers: matches!(m, Fnsave | Frstor),
            wide: crate::x87::Environment::of(i).wide(),
            at: operand(i)?,
        },
        _ => return None,
    })
}

/// The segment register `r` is, where it is one.
fn segment(r: Register) -> Option<SegmentName> {
    Some(match r {
        Register::CS => SegmentName::Code,
        Register::SS => SegmentName::Stack,
        Register::DS => SegmentName::Kept(SegmentRegister::Ds),
        Register::ES => SegmentName::Kept(SegmentRegister::Es),
        Register::FS => SegmentName::Kept(SegmentRegister::Fs),
        Register::GS => SegmentName::Kept(SegmentRegister::Gs),
        _ => return None,
    })
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
