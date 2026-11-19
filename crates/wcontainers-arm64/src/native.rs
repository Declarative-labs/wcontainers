//! How the native output carries each ARM64 instruction over to ARM64 host code (docs/native.md section 3.4).

use wcontainers_guest_cpu::position::{Base, Index, Operand};
use wcontainers_guest_cpu::{HostCode, InsideCall, Native, NativeClass, Route, Stopping};

use crate::Arm64;
use crate::carry::{self, COUNT};
use crate::encode;
use crate::exact::Operation;
use crate::instruction::{Address, Instruction as I, Reg, SystemRegister};
use crate::meaning::{Control, Hook};

/// The operand the float hook is given for a read of the float status while it is not kept.
fn read_status() -> i64 {
    Operation::ReadStatus.encode(0).into()
}

/// The base register of an access that writes its address back.
fn written_back(address: &Address) -> Option<Reg> {
    match *address {
        Address::PreIndex { base, .. } | Address::PostIndex { base, .. } => Some(base),
        _ => None,
    }
}

/// Two register fields name the same general register (never the zero register, whose reads and writes
/// collide with nothing).
fn same(a: Reg, b: Reg) -> bool {
    matches!((a, b), (Reg::X(x), Reg::X(y)) if x == y)
}

/// A copied load or store: its access can cross a page unless it is of one byte.
fn access_route(bytes: u32, address: &Address) -> Route {
    if matches!(address, Address::Literal { .. }) {
        Route::Rebase
    } else if bytes > 1 {
        Route::Split
    } else {
        Route::Copy
    }
}

/// The bytes a structure load or store moves.
fn structure_bytes(structure: u8, registers: u8, element: u8, whole: bool, single: bool) -> u32 {
    let register_bytes = if whole { u128::BITS } else { u64::BITS } / u8::BITS;
    if single {
        u32::from(structure) * u32::from(element)
    } else {
        u32::from(registers) * register_bytes
    }
}

impl Native for Arm64 {
    fn route(i: &I) -> Route {
        match *i {
            I::AddSub { .. }
            | I::AddSubCarry { .. }
            | I::Logical { .. }
            | I::MoveWide { .. }
            | I::Bitfield { .. }
            | I::Extract { .. }
            | I::CondCompare { .. }
            | I::CondSelect { .. }
            | I::Unary { .. }
            | I::Binary { .. }
            | I::MulAdd { .. }
            | I::VectorImmediate { .. }
            | I::Simd { .. }
            | I::Permute { .. }
            | I::Table { .. }
            | I::DuplicateGeneral { .. }
            | I::DuplicateElement { .. }
            | I::ScalarElement { .. }
            | I::MoveToGeneral { .. }
            | I::MoveFromGeneral { .. }
            | I::InsertGeneral { .. }
            | I::InsertElement { .. }
            | I::ExtractVector { .. }
            | I::Crc32 { .. }
            | I::Float { .. }
            | I::FloatConvert { .. }
            | I::FloatBinary { .. }
            | I::FloatMulAdd { .. }
            | I::FloatCompare { .. }
            | I::FloatCondCompare { .. }
            | I::FloatSelect { .. }
            | I::FloatToInteger { .. }
            | I::IntegerToFloat { .. }
            | I::ZeroBlock { .. }
            | I::CleanCache { .. } => Route::Copy,
            I::PcRelative { .. } => Route::Rebase,
            I::Branch { .. }
            | I::BranchCond { .. }
            | I::CompareBranch { .. }
            | I::TestBranch { .. }
            | I::BranchRegister { .. } => Route::Transfer,
            I::Load {
                bytes, rt, address, ..
            }
            | I::Store {
                bytes, rt, address, ..
            } => {
                if written_back(&address).is_some_and(|base| same(base, rt)) {
                    Route::Collision
                } else {
                    access_route(bytes.into(), &address)
                }
            }
            I::LoadPair {
                bytes,
                rt,
                rt2,
                address,
                ..
            } => {
                let base = written_back(&address);
                // One register twice is open even when it is the zero register.
                if rt == rt2 || base.is_some_and(|b| same(b, rt) || same(b, rt2)) {
                    Route::Collision
                } else {
                    access_route(u32::from(bytes) * 2, &address)
                }
            }
            I::StorePair {
                bytes,
                rt,
                rt2,
                address,
            } => {
                if written_back(&address).is_some_and(|b| same(b, rt) || same(b, rt2)) {
                    Route::Collision
                } else {
                    access_route(u32::from(bytes) * 2, &address)
                }
            }
            I::LoadVector { bytes, address, .. } | I::StoreVector { bytes, address, .. } => {
                access_route(bytes.into(), &address)
            }
            I::LoadVectorPair { rt, rt2, .. } if rt == rt2 => Route::Collision,
            I::LoadVectorPair { bytes, address, .. }
            | I::StoreVectorPair { bytes, address, .. } => {
                access_route(u32::from(bytes) * 2, &address)
            }
            I::SimdMemory {
                structure,
                registers,
                element,
                whole,
                lane,
                replicate,
                ..
            } => {
                let single = lane.is_some() || replicate;
                let bytes = structure_bytes(structure, registers, element, whole, single);
                if bytes > 1 { Route::Split } else { Route::Copy }
            }
            I::Atomic { .. }
            | I::CompareSwap { .. }
            | I::LoadAcquire { .. }
            | I::StoreRelease { .. } => Route::Aligned,
            I::LoadExclusive { .. } | I::StoreExclusive { .. } | I::ClearExclusive => {
                Route::Exclusive
            }
            I::CodeChanged { .. } | I::Syscall => Route::Kernel,
            I::Breakpoint { .. } | I::Undefined | I::Unsupported => Route::Trap,
            I::Nothing => Route::Nothing,
            I::ReadSystem { register, .. } => match register {
                SystemRegister::Nzcv | SystemRegister::TpidrEl0 | SystemRegister::Fpcr => {
                    Route::Copy
                }
                SystemRegister::Fpsr => Route::FloatStatus,
                SystemRegister::Counter => Route::Kernel,
                SystemRegister::Fixed(_) => Route::Constant,
            },
            I::WriteSystem { register, .. } => match register {
                SystemRegister::Nzcv | SystemRegister::TpidrEl0 => Route::Copy,
                SystemRegister::Fpcr | SystemRegister::Fpsr => Route::FloatStatus,
                SystemRegister::Counter | SystemRegister::Fixed(_) => {
                    unreachable!("a register that cannot be written is never decoded as a write")
                }
            },
        }
    }

    fn hook_class(hook: Hook, first: Option<i64>) -> NativeClass {
        match hook {
            Hook::Float if first == Some(read_status()) => NativeClass::Routed(Route::FloatStatus),
            Hook::Float => NativeClass::Hardware,
            Hook::Clock | Hook::CodeChanged => NativeClass::Routed(Route::Kernel),
        }
    }

    fn control_class(control: Control, write: bool) -> NativeClass {
        match control {
            Control::ThreadPointer | Control::Fpsr => NativeClass::Hardware,
            Control::Fpcr | Control::FloatTracking if !write => NativeClass::Hardware,
            Control::Fpcr | Control::FloatTracking => NativeClass::Routed(Route::FloatStatus),
            Control::ExclusiveBytes
            | Control::ExclusiveAddress
            | Control::ExclusiveLow
            | Control::ExclusiveHigh => NativeClass::Routed(Route::Exclusive),
        }
    }

    fn carry(i: &I, bytes: &[u8], address: u64, code: &mut HostCode) {
        let word = u32::from_le_bytes(bytes.try_into().expect("an instruction is one word"));
        carry::carry(i, word, address, code);
    }

    fn count(code: &mut HostCode, events: u32) {
        let add = encode::add_immediate(COUNT, COUNT, events.into()).expect("a run's events fit");
        code.count(&add.to_le_bytes(), events);
    }

    fn call(code: &mut HostCode, call: InsideCall) {
        code.push(&carry::call_word(call).to_le_bytes());
    }

    fn stopping(i: &I, _address: u64) -> Stopping {
        let register = |r: Reg| match r {
            Reg::X(n) => Base::Register(n),
            Reg::Sp => Base::StackPointer,
            Reg::Zero => Base::Zero,
        };
        if let Some(m) = i.memory_access() {
            let (base, offset, index) = match m.address {
                Address::Offset { base, offset } | Address::PreIndex { base, offset } => {
                    (register(base), offset, None)
                }
                Address::PostIndex { base, .. } => (register(base), 0, None),
                Address::Indexed {
                    base,
                    index,
                    extend,
                    shift,
                } => (
                    register(base),
                    0,
                    Some(Index {
                        register: register(index),
                        bits: extend.from_bits,
                        signed: extend.signed,
                        shift,
                    }),
                ),
                Address::Literal { offset } => (Base::Pc, offset, None),
            };
            return Stopping::Access(Operand {
                base,
                offset,
                index,
            });
        }
        match *i {
            I::Syscall => Stopping::Syscall,
            I::CodeChanged { .. } => Stopping::CodeChanged,
            I::ReadSystem {
                rt,
                register: SystemRegister::Counter,
            } => Stopping::Counter(match rt {
                Reg::X(n) => Some(n),
                _ => None,
            }),
            I::Breakpoint { comment } => Stopping::Breakpoint(comment),
            I::WriteSystem {
                register: SystemRegister::Fpsr,
                ..
            } => Stopping::FloatStatusWrite,
            _ if Self::route(i) == Route::Trap => Stopping::Illegal,
            _ => Stopping::Plain,
        }
    }
}
