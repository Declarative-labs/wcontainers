//! The meaning of each instruction as WebAssembly, from Intel's manual and what the reference CPU does.

use crate::claims::{GROUP_AVX, GROUP_KERNEL, GROUP_NEVER};
use crate::decode::{Instruction, claim_group};
use crate::flags::{Flag, Flags};
use crate::guest::{Control, Reason, Trap};
use crate::operand::*;
use crate::{ALWAYS, DF, X86_64};
use iced_x86::{Mnemonic, OpKind};
use wasm_encoder::{BlockType, ValType};
use wcontainers_guest_cpu::{Access, Emitter, Target};

pub fn emit<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    if i.too_long {
        e.trap(Trap::GeneralProtection);
        return;
    }
    let ins = &i.iced;
    if ins.is_invalid() {
        e.trap(Trap::Undefined);
        return;
    }
    match claim_group(ins.code()) {
        // At CPL 3 every kernel-only instruction is general protection, but rsm, which exists only in
        // system management mode and is #UD outside it.
        GROUP_KERNEL => {
            e.trap(if ins.mnemonic() == Mnemonic::Rsm {
                Trap::Undefined
            } else {
                Trap::GeneralProtection
            });
            return;
        }
        GROUP_NEVER => {
            e.trap(Trap::Unsupported);
            return;
        }
        GROUP_AVX if !e.offer().avx => {
            e.warn(Reason::AvxNotOffered);
            e.trap(Trap::Unsupported);
            return;
        }
        _ => {}
    }
    if !meaning(e, i) {
        e.trap(Trap::Unsupported);
    }
}

/// Whether the instruction's meaning is written; when it is not, nothing has been emitted.
pub fn written(i: &Instruction) -> bool {
    use Mnemonic::*;
    let ins = &i.iced;
    let m = ins.mnemonic();
    if is_string(i) {
        return !ins.has_rep_prefix()
            && !ins.has_repne_prefix()
            && matches!(
                m,
                Movsb
                    | Movsw
                    | Movsd
                    | Movsq
                    | Stosb
                    | Stosw
                    | Stosd
                    | Stosq
                    | Lodsb
                    | Lodsw
                    | Lodsd
                    | Lodsq
                    | Cmpsb
                    | Cmpsw
                    | Cmpsd
                    | Cmpsq
                    | Scasb
                    | Scasw
                    | Scasd
                    | Scasq
            );
    }
    if crate::simd::written(i) {
        return true;
    }
    // Every operand of these is a general register, memory or an immediate.
    let general_only = (0..ins.op_count()).all(|n| match ins.op_kind(n) {
        OpKind::Register => general(ins.op_register(n)).is_some(),
        _ => true,
    });
    general_only
        && (condition(m).is_some()
            || matches!(
                m,
                Add | Adc | Sub | Sbb | Cmp | And | Or | Xor | Test | Inc | Dec | Neg | Not | Mov
                    | Movzx | Movsx | Movsxd | Lea | Xchg | Cbw | Cwde | Cdqe | Cwd | Cdq | Cqo
                    | Push | Pop | Call | Ret | Jmp | Shl | Sal | Shr | Sar | Rol | Ror | Rcl
                    | Rcr | Mul | Imul | Div | Idiv | Syscall | Nop | Pause | Reservednop | Int3
                    | Int1 | Int | Sysenter | Ud0 | Ud1 | Ud2 | Clc | Stc | Cmc | Cld | Std | Bt | Bts
                    | Btr | Btc | Jrcxz | Jecxz
            ))
        // Far transfers through memory: a selector with the offset.
        && !(matches!(m, Call | Jmp) && is_memory(i, 0) && memory_bytes(i) != 8)
        && !matches!(ins.op_kind(0), OpKind::FarBranch16 | OpKind::FarBranch32)
}

/// A string instruction: its operands are rsi and rdi.
pub fn is_string(i: &Instruction) -> bool {
    (0..i.iced.op_count()).any(|n| {
        matches!(
            i.iced.op_kind(n),
            OpKind::MemorySegSI
                | OpKind::MemorySegESI
                | OpKind::MemorySegRSI
                | OpKind::MemoryESDI
                | OpKind::MemoryESEDI
                | OpKind::MemoryESRDI
        )
    })
}

fn meaning<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) -> bool {
    use Mnemonic::*;
    if !written(i) {
        return false;
    }
    let ins = &i.iced;
    let m = ins.mnemonic();
    if is_string(i) {
        string(e, i, m);
        e.next();
        return true;
    }
    if crate::simd::written(i) {
        crate::simd::emit(e, i, m);
        e.next();
        return true;
    }
    match m {
        Add | Adc | Sub | Sbb | Cmp | And | Or | Xor | Test => arithmetic(e, i, m),
        Inc | Dec | Neg | Not => unary(e, i, m),
        Mov | Movzx => {
            let mem = checked(e, i);
            let value = read(e, i, 1, mem);
            write(e, i, 0, mem, value);
        }
        Movsx | Movsxd => {
            let mem = checked(e, i);
            let value = read(e, i, 1, mem);
            sign_extend(e, value, width(i, 1));
            write(e, i, 0, mem, value);
        }
        Lea => {
            let at = linear_free_address(e, i);
            write(e, i, 0, None, at);
        }
        Xchg => {
            let mem = checked(e, i);
            let a = read(e, i, 0, mem);
            let b = read(e, i, 1, mem);
            write(e, i, 0, mem, b);
            write(e, i, 1, mem, a);
        }
        Cbw | Cwde | Cdqe => {
            let w = match m {
                Cbw => 16,
                Cwde => 32,
                _ => 64,
            };
            let value = e.local(ValType::I64);
            get_general(e, accumulator(w / 2));
            e.sink().local_set(value);
            sign_extend(e, value, w / 2);
            set_general(e, accumulator(w), value);
        }
        Cwd | Cdq | Cqo => {
            let w = match m {
                Cwd => 16,
                Cdq => 32,
                _ => 64,
            };
            let value = e.local(ValType::I64);
            get_general(e, accumulator(w));
            e.sink().local_set(value);
            sign_extend(e, value, w);
            e.sink()
                .local_get(value)
                .i64_const(63)
                .i64_shr_s()
                .local_set(value);
            set_general(e, register(RDX, w), value);
        }
        Push => push(e, i),
        Pop => pop(e, i),
        Call => call(e, i),
        Ret => {
            ret(e, i);
            return true;
        }
        Jmp => {
            jmp(e, i);
            return true;
        }
        Shl | Sal | Shr | Sar | Rol | Ror | Rcl | Rcr => shift(e, i, m),
        Mul | Imul => multiply(e, i, m),
        Bt | Bts | Btr | Btc => bit_test(e, i, m),
        Div | Idiv => divide(e, i, m == Idiv),
        Syscall => syscall(e, i),
        Nop | Pause | Reservednop => {}
        Int3 => {
            e.trap(Trap::Int3);
            return true;
        }
        Int1 => {
            e.trap(Trap::Int1);
            return true;
        }
        Int => {
            int(e, ins.immediate8());
            return true;
        }
        Sysenter => {
            // The other way into Linux's 32-bit system calls: a kernel without them leaves its selector
            // null, and the instruction gives general protection.
            e.warn(Reason::ThirtyTwoBitSystemCall);
            e.trap(Trap::GeneralProtection);
            return true;
        }
        Ud0 | Ud1 | Ud2 => {
            e.trap(Trap::Undefined);
            return true;
        }
        Clc | Stc | Cmc => {
            let carry = e.local(ValType::I32);
            match m {
                Clc => {
                    e.sink().i32_const(0);
                }
                Stc => {
                    e.sink().i32_const(1);
                }
                _ => {
                    e.flag(Flag::C);
                    e.sink().i32_eqz();
                }
            }
            e.sink().local_set(carry);
            let word = word_with(e, &[(Flag::C, carry)]);
            e.set_flags(Flags::Packed(word));
        }
        Jrcxz | Jecxz => {
            // A jump when rcx (ecx with an address-size prefix) is zero; no flag is read.
            get_general(e, register(RCX, if m == Jrcxz { 64 } else { 32 }));
            e.sink().i64_eqz();
            e.jump_if(Target::Relative(ins.near_branch64() as i64));
        }
        Cld | Std => {
            e.get_control(Control::Rflags);
            let mut s = e.sink();
            s.i64_const(!(DF as i64)).i64_and();
            if m == Std {
                s.i64_const(DF as i64).i64_or();
            }
            e.set_control(Control::Rflags);
        }
        _ => {
            if let Some(cond) = jcc(m) {
                e.condition(cond);
                e.jump_if(Target::Relative(ins.near_branch64() as i64));
            } else if let Some(cond) = setcc(m) {
                let mem = checked(e, i);
                let value = e.local(ValType::I64);
                e.condition(cond);
                e.sink().i64_extend_i32_u().local_set(value);
                write(e, i, 0, mem, value);
            } else if let Some(cond) = cmovcc(m) {
                let mem = checked(e, i);
                let value = read(e, i, 1, mem);
                let old = read(e, i, 0, None);
                e.sink().local_get(value).local_get(old);
                e.condition(cond);
                e.sink().select().local_set(value);
                write(e, i, 0, None, value);
            } else {
                unreachable!("{m:?} is written");
            }
        }
    }
    e.next();
    true
}

/// The width in bits of operand `n`: a register's, the memory operand's, else operand 0's.
pub fn width(i: &Instruction, n: u32) -> u8 {
    let ins = &i.iced;
    match ins.op_kind(n) {
        OpKind::Register => general(ins.op_register(n)).map_or(64, |g| g.width),
        OpKind::Memory => (memory_bytes(i) * 8) as u8,
        _ if n > 0 => width(i, 0),
        _ => 64,
    }
}

fn register(number: u8, width: u8) -> General {
    General {
        number,
        width,
        high: false,
    }
}

fn accumulator(width: u8) -> General {
    register(RAX, width)
}

/// Compute and check the explicit memory operand's address, before anything changes.
fn checked<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) -> Option<u32> {
    let kind = memory_use(i)?;
    let at = address(e, i);
    e.check(at, memory_bytes(i), check_kind(kind), 0);
    Some(at)
}

/// The effective address of `lea`: no segment base.
fn linear_free_address<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) -> u32 {
    let unsegmented = Instruction {
        iced: {
            let mut ins = i.iced;
            ins.set_segment_prefix(iced_x86::Register::None);
            ins
        },
        ..*i
    };
    address(e, &unsegmented)
}

/// Operand `n`'s value, zero-extended from its width, into a fresh i64 local.
fn read<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, n: u32, mem: Option<u32>) -> u32 {
    let ins = &i.iced;
    let w = width(i, n);
    let value = e.local(ValType::I64);
    match ins.op_kind(n) {
        OpKind::Register => {
            get_general(e, general(ins.op_register(n)).expect("a general register"))
        }
        OpKind::Memory => load(e, mem.expect("checked"), u32::from(w / 8)),
        _ => {
            e.sink().i64_const(ins.immediate(n) as i64 & mask(w));
        }
    }
    e.sink().local_set(value);
    value
}

/// Write the i64 in `value` to operand `n`.
fn write<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    n: u32,
    mem: Option<u32>,
    value: u32,
) {
    let ins = &i.iced;
    match ins.op_kind(n) {
        OpKind::Register => set_general(
            e,
            general(ins.op_register(n)).expect("a general register"),
            value,
        ),
        _ => store(e, mem.expect("checked"), u32::from(width(i, n) / 8), value),
    }
}

/// Sign-extend the i64 in `value` from `width` bits, in place.
fn sign_extend<E: Emitter<X86_64> + ?Sized>(e: &mut E, value: u32, width: u8) {
    if width < 64 {
        let shift = i64::from(64 - width);
        e.sink()
            .local_get(value)
            .i64_const(shift)
            .i64_shl()
            .i64_const(shift)
            .i64_shr_s()
            .local_set(value);
    }
}

/// Mask the i64 in `value` to `width` bits, in place.
fn truncate<E: Emitter<X86_64> + ?Sized>(e: &mut E, value: u32, width: u8) {
    if width < 64 {
        e.sink()
            .local_get(value)
            .i64_const(mask(width))
            .i64_and()
            .local_set(value);
    }
}

/// The flags word with these flags from locals and the rest as they are, into a fresh i32 local.
fn word_with<E: Emitter<X86_64> + ?Sized>(e: &mut E, new: &[(Flag, u32)]) -> u32 {
    let word = e.local(ValType::I32);
    for (k, flag) in Flag::ALL.into_iter().enumerate() {
        match new.iter().find(|(f, _)| *f == flag) {
            Some(&(_, local)) => {
                e.sink().local_get(local);
            }
            None => e.flag(flag),
        }
        let mut s = e.sink();
        s.i32_const(flag.bit() as i32).i32_shl();
        if k > 0 {
            s.i32_or();
        }
    }
    e.sink().local_set(word);
    word
}

fn arithmetic<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    use Mnemonic::*;
    let w = width(i, 0);
    let mem = checked(e, i);
    let a = read(e, i, 0, mem);
    let b = read(e, i, 1, mem);
    let carry = matches!(m, Adc | Sbb).then(|| {
        let c = e.local(ValType::I32);
        e.flag(Flag::C);
        e.sink().local_set(c);
        c
    });
    let result = e.local(ValType::I64);
    {
        let mut s = e.sink();
        s.local_get(a).local_get(b);
        match m {
            Add | Adc => s.i64_add(),
            Sub | Sbb | Cmp => s.i64_sub(),
            And | Test => s.i64_and(),
            Or => s.i64_or(),
            _ => s.i64_xor(),
        };
        if let Some(c) = carry {
            s.local_get(c).i64_extend_i32_u();
            if m == Adc {
                s.i64_add();
            } else {
                s.i64_sub();
            }
        }
        s.local_set(result);
    }
    truncate(e, result, w);
    if !matches!(m, Cmp | Test) {
        write(e, i, 0, mem, result);
    }
    e.set_flags(match m {
        Add | Adc => Flags::Add {
            width: w,
            a,
            b,
            carry,
            result,
            kept_carry: None,
        },
        Sub | Sbb | Cmp => Flags::Sub {
            width: w,
            a,
            b,
            borrow: carry,
            result,
            kept_carry: None,
        },
        _ => Flags::Logic { width: w, result },
    });
}

fn unary<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    use Mnemonic::*;
    let w = width(i, 0);
    let mem = checked(e, i);
    let value = read(e, i, 0, mem);
    let result = e.local(ValType::I64);
    match m {
        Inc => e.sink().local_get(value).i64_const(1).i64_add(),
        Dec => e.sink().local_get(value).i64_const(1).i64_sub(),
        Neg => e.sink().i64_const(0).local_get(value).i64_sub(),
        _ => e.sink().local_get(value).i64_const(-1).i64_xor(),
    };
    e.sink().local_set(result);
    truncate(e, result, w);
    let kept = matches!(m, Inc | Dec).then(|| {
        let c = e.local(ValType::I32);
        e.flag(Flag::C);
        e.sink().local_set(c);
        c
    });
    write(e, i, 0, mem, result);
    let constant = |e: &mut E, k: i64| {
        let local = e.local(ValType::I64);
        e.sink().i64_const(k).local_set(local);
        local
    };
    match m {
        Inc => {
            let one = constant(e, 1);
            e.set_flags(Flags::Add {
                width: w,
                a: value,
                b: one,
                carry: None,
                result,
                kept_carry: kept,
            });
        }
        Dec => {
            let one = constant(e, 1);
            e.set_flags(Flags::Sub {
                width: w,
                a: value,
                b: one,
                borrow: None,
                result,
                kept_carry: kept,
            });
        }
        Neg => {
            let zero = constant(e, 0);
            e.set_flags(Flags::Sub {
                width: w,
                a: zero,
                b: value,
                borrow: None,
                result,
                kept_carry: None,
            });
        }
        _ => {}
    }
}

/// Push i32 1 when the i64 in `local` is not a canonical address (bits 63 to 47 not all equal).
fn not_canonical<E: Emitter<X86_64> + ?Sized>(e: &mut E, local: u32) {
    e.sink()
        .local_get(local)
        .i64_const(crate::CANONICAL_SHIFT.into())
        .i64_shl()
        .i64_const(crate::CANONICAL_SHIFT.into())
        .i64_shr_s()
        .local_get(local)
        .i64_ne();
}

/// Stop with general protection, as the CPU does for a branch to it, when the address in `local` is not
/// canonical.
fn canonical_target<E: Emitter<X86_64> + ?Sized>(e: &mut E, local: u32) {
    not_canonical(e, local);
    e.sink().if_(BlockType::Empty);
    e.trap(Trap::GeneralProtection);
    e.sink().end();
}

fn push<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let bytes = stack_bytes(i);
    let mem = checked(e, i);
    let value = read(e, i, 0, mem);
    // The stack's addresses are 64 bits whatever the operand's size is.
    let stack = Instruction {
        address32: false,
        ..*i
    };
    let at = register_address(e, &stack, RSP, -i64::from(bytes));
    e.check(at, bytes, Access::Write, u8::from(mem.is_some()));
    store(e, at, bytes, value);
    // The stack pointer is not wrapped by an address-size prefix.
    e.get(RSP);
    e.sink().i64_const(-i64::from(bytes)).i64_add();
    e.set(RSP);
}

fn pop<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let bytes = stack_bytes(i);
    let from = e.local(ValType::I64);
    e.get(RSP);
    e.sink().local_set(from);
    e.check(from, bytes, Access::Read, 0);
    let value = e.local(ValType::I64);
    load(e, from, bytes);
    e.sink().local_set(value);
    let mem = if is_memory(i, 0) {
        // The destination's address is worked out with the stack pointer already moved, before an
        // address-size prefix wraps it.
        let mut moved = *i;
        if general(i.iced.memory_base()).is_some_and(|g| g.number == RSP) {
            moved.iced.set_memory_displacement64(
                i.iced
                    .memory_displacement64()
                    .wrapping_add(u64::from(bytes)),
            );
        }
        let at = address(e, &moved);
        e.check(at, bytes, Access::Write, 1);
        Some(at)
    } else {
        None
    };
    e.sink().local_get(from).i64_const(bytes.into()).i64_add();
    e.set(RSP);
    write(e, i, 0, mem, value);
}

fn call<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let ins = &i.iced;
    let through = !matches!(ins.op_kind(0), OpKind::NearBranch64);
    let (target, mem) = if through {
        let mem = checked(e, i);
        (Some(read(e, i, 0, mem)), mem)
    } else {
        (None, None)
    };
    let at = register_address(
        e,
        &Instruction {
            address32: false,
            ..*i
        },
        RSP,
        -8,
    );
    e.check(at, 8, Access::Write, u8::from(mem.is_some()));
    let back = e.local(ValType::I64);
    e.this_address(ins.len() as i64);
    e.sink().local_set(back);
    store(e, at, 8, back);
    if let Some(target) = target {
        canonical_target(e, target);
    }
    e.sink().local_get(at);
    e.set(RSP);
    e.call(match target {
        Some(target) => Target::Address(target),
        None => Target::Relative(ins.near_branch64() as i64),
    });
}

fn ret<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let at = e.local(ValType::I64);
    e.get(RSP);
    e.sink().local_set(at);
    e.check(at, 8, Access::Read, 0);
    let target = e.local(ValType::I64);
    load(e, at, 8);
    e.sink().local_set(target);
    canonical_target(e, target);
    let extra = if i.iced.op_count() > 0 {
        i.iced.immediate16()
    } else {
        0
    };
    if extra != 0 {
        e.get(RSP);
        e.sink().i64_const(extra.into()).i64_add();
        e.set(RSP);
    }
    e.ret(target);
}

fn jmp<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let ins = &i.iced;
    if matches!(ins.op_kind(0), OpKind::NearBranch64) {
        e.jump(Target::Relative(ins.near_branch64() as i64));
        return;
    }
    let mem = checked(e, i);
    let target = read(e, i, 0, mem);
    canonical_target(e, target);
    e.jump(Target::Address(target));
}

fn int<E: Emitter<X86_64> + ?Sized>(e: &mut E, vector: u8) {
    match vector {
        crate::BREAKPOINT_VECTOR => e.trap(Trap::Int3),
        crate::OVERFLOW_VECTOR => e.trap(Trap::Overflow),
        crate::LINUX_32_BIT_SYSTEM_CALL => {
            // Linux's 32-bit system call gate: this phase has no 32-bit system calls, as a kernel built
            // without 32-bit emulation, where the gate is the kernel's own.
            e.warn(Reason::ThirtyTwoBitSystemCall);
            e.trap(Trap::GeneralProtection);
        }
        _ => e.trap(Trap::GeneralProtection),
    }
}

/// Push rflags as user code sees it.
fn rflags<E: Emitter<X86_64> + ?Sized>(e: &mut E) {
    e.get_control(Control::Rflags);
    for flag in Flag::ALL {
        e.flag(flag);
        e.sink()
            .i64_extend_i32_u()
            .i64_const(flag.bit().into())
            .i64_shl()
            .i64_or();
    }
    e.sink().i64_const(ALWAYS as i64).i64_or();
}

fn syscall<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    // The CPU's part: rcx gets the next instruction's address and r11 rflags.
    e.this_address(i.iced.len() as i64);
    e.set(RCX);
    rflags(e);
    e.set(R11);
    e.syscall();
}

/// Movs, stos, lods, cmps and scas, one element.
fn string<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    use Mnemonic::*;
    let bytes = element_bytes(i);
    let w = (bytes * 8) as u8;
    let source = matches!(
        m,
        Movsb
            | Movsw
            | Movsd
            | Movsq
            | Lodsb
            | Lodsw
            | Lodsd
            | Lodsq
            | Cmpsb
            | Cmpsw
            | Cmpsd
            | Cmpsq
    );
    let destination = !matches!(m, Lodsb | Lodsw | Lodsd | Lodsq);
    let source_at = source.then(|| {
        let at = register_address(e, i, RSI, 0);
        if let Some(base) = segment(i) {
            e.get(base);
            e.sink().local_get(at).i64_add().local_set(at);
        }
        at
    });
    let destination_at = destination.then(|| register_address(e, i, RDI, 0));
    let writes = matches!(
        m,
        Movsb | Movsw | Movsd | Movsq | Stosb | Stosw | Stosd | Stosq
    );
    let destination_access = if writes { Access::Write } else { Access::Read };
    // The CPU's order: cmps reads [rdi] before [rsi]; movs reads [rsi] before writing [rdi].
    let mut checks = [
        source_at.map(|at| (at, Access::Read)),
        destination_at.map(|at| (at, destination_access)),
    ];
    if matches!(m, Cmpsb | Cmpsw | Cmpsd | Cmpsq) {
        checks.reverse();
    }
    for (which, (at, access)) in checks.into_iter().flatten().enumerate() {
        e.check(at, bytes, access, which as u8);
    }
    let acc = accumulator(w);
    match m {
        Movsb | Movsw | Movsd | Movsq => {
            let value = e.local(ValType::I64);
            load(e, source_at.unwrap(), bytes);
            e.sink().local_set(value);
            store(e, destination_at.unwrap(), bytes, value);
        }
        Stosb | Stosw | Stosd | Stosq => {
            let value = e.local(ValType::I64);
            get_general(e, acc);
            e.sink().local_set(value);
            store(e, destination_at.unwrap(), bytes, value);
        }
        Lodsb | Lodsw | Lodsd | Lodsq => {
            let value = e.local(ValType::I64);
            load(e, source_at.unwrap(), bytes);
            e.sink().local_set(value);
            set_general(e, acc, value);
        }
        _ => {
            let (a, b) = (e.local(ValType::I64), e.local(ValType::I64));
            match source_at {
                Some(at) => load(e, at, bytes),
                None => get_general(e, acc),
            }
            e.sink().local_set(a);
            load(e, destination_at.unwrap(), bytes);
            e.sink().local_set(b);
            let result = e.local(ValType::I64);
            e.sink()
                .local_get(a)
                .local_get(b)
                .i64_sub()
                .local_set(result);
            truncate(e, result, w);
            e.set_flags(Flags::Sub {
                width: w,
                a,
                b,
                borrow: None,
                result,
                kept_carry: None,
            });
        }
    }
    // Each register used moves by an element, down when DF is set.
    let step = e.local(ValType::I64);
    e.get_control(Control::Rflags);
    e.sink()
        .i64_const(crate::DF.trailing_zeros().into())
        .i64_shr_u()
        .i64_const(1)
        .i64_and()
        .i64_const(-2 * i64::from(bytes))
        .i64_mul()
        .i64_const(bytes.into())
        .i64_add()
        .local_set(step);
    for (used, register) in [(source, RSI), (destination, RDI)] {
        if used {
            let moved = e.local(ValType::I64);
            e.get(register);
            e.sink().local_get(step).i64_add().local_set(moved);
            let g = General {
                number: register,
                width: if i.address32 { 32 } else { 64 },
                high: false,
            };
            set_general(e, g, moved);
        }
    }
}

/// Shifts and rotates. Their flags are worked out at once into a packed word, kept as they were when the
/// masked count is zero.
fn shift<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    use Mnemonic::*;
    let w = width(i, 0);
    let wide = i64::from(w);
    let mem = checked(e, i);
    let v = read(e, i, 0, mem);
    let count = e.local(ValType::I64);
    match i.iced.op_kind(1) {
        OpKind::Register => {
            e.get(RCX);
        }
        _ => {
            e.sink().i64_const(i.iced.immediate(1) as i64);
        }
    }
    e.sink()
        .i64_const(if w == 64 { 63 } else { 31 })
        .i64_and()
        .local_set(count);
    let (result, carry, overflow) = (
        e.local(ValType::I64),
        e.local(ValType::I32),
        e.local(ValType::I32),
    );
    // The value rotated, for rotates: the count modulo the width (or the width and CF).
    let rotated = e.local(ValType::I64);
    let old_carry = e.local(ValType::I32);
    e.flag(Flag::C);
    e.sink().local_set(old_carry);
    let m_all = mask(w);
    {
        let mut s = e.sink();
        match m {
            Shl | Sal => {
                s.local_get(v).local_get(count).i64_shl().local_set(result);
                // The last bit shifted out.
                s.local_get(v)
                    .local_get(count)
                    .i64_const(1)
                    .i64_sub()
                    .i64_shl()
                    .i64_const(wide - 1)
                    .i64_shr_u()
                    .i32_wrap_i64()
                    .i32_const(1)
                    .i32_and()
                    .local_set(carry);
                if w < 64 {
                    // A count above the width shifts every bit out (for 8 and 16 bits).
                    s.local_get(carry)
                        .i32_const(0)
                        .local_get(count)
                        .i64_const(wide)
                        .i64_le_u()
                        .select()
                        .local_set(carry);
                }
            }
            Shr | Sar => {
                s.local_get(v);
                if m == Sar && w < 64 {
                    s.i64_const(64 - wide)
                        .i64_shl()
                        .i64_const(64 - wide)
                        .i64_shr_s();
                }
                s.local_tee(rotated).local_get(count);
                if m == Sar {
                    s.i64_shr_s();
                } else {
                    s.i64_shr_u();
                }
                s.local_set(result);
                s.local_get(rotated).local_get(count).i64_const(1).i64_sub();
                if m == Sar {
                    s.i64_shr_s();
                } else {
                    s.i64_shr_u();
                }
                s.i32_wrap_i64().i32_const(1).i32_and().local_set(carry);
                if w < 64 {
                    s.local_get(carry)
                        .i32_const(0)
                        .local_get(count)
                        .i64_const(wide)
                        .i64_le_u()
                        .select()
                        .local_set(carry);
                    if m == Sar {
                        // A count above the width leaves the sign in every bit and in CF.
                        s.local_get(carry)
                            .local_get(v)
                            .i64_const(wide - 1)
                            .i64_shr_u()
                            .i32_wrap_i64()
                            .i32_const(1)
                            .i32_and()
                            .local_get(count)
                            .i64_const(wide)
                            .i64_le_u()
                            .select()
                            .local_set(carry);
                    }
                }
            }
            Rol | Ror => {
                // The count modulo the width.
                s.local_get(count)
                    .i64_const(wide - 1)
                    .i64_and()
                    .local_set(rotated);
                if w == 64 {
                    s.local_get(v).local_get(rotated);
                    if m == Rol {
                        s.i64_rotl();
                    } else {
                        s.i64_rotr();
                    }
                } else if w == 32 {
                    s.local_get(v)
                        .i32_wrap_i64()
                        .local_get(rotated)
                        .i32_wrap_i64();
                    if m == Rol {
                        s.i32_rotl();
                    } else {
                        s.i32_rotr();
                    }
                    s.i64_extend_i32_u();
                } else {
                    let left = m == Rol;
                    s.local_get(v).local_get(rotated);
                    if left {
                        s.i64_shl();
                    } else {
                        s.i64_shr_u();
                    }
                    s.local_get(v).i64_const(wide).local_get(rotated).i64_sub();
                    if left {
                        s.i64_shr_u();
                    } else {
                        s.i64_shl();
                    }
                    s.i64_or();
                }
                s.local_set(result);
                // CF: ROL the result's low bit, ROR its top bit.
                s.local_get(result);
                if m == Ror {
                    s.i64_const(wide - 1).i64_shr_u();
                }
                s.i32_wrap_i64().i32_const(1).i32_and().local_set(carry);
            }
            _ => {
                // Rcl, Rcr: through CF, the count modulo the width plus one at 8 and 16 bits.
                if w < 32 {
                    s.local_get(count)
                        .i64_const(wide + 1)
                        .i64_rem_u()
                        .local_set(rotated);
                } else {
                    s.local_get(count).local_set(rotated);
                }
                if w < 64 {
                    // CF above the value, rotated within width + 1 bits.
                    let full = (1i64 << (w + 1)) - 1;
                    let joined = |s: &mut wasm_encoder::InstructionSink<'_>| {
                        s.local_get(old_carry)
                            .i64_extend_i32_u()
                            .i64_const(wide)
                            .i64_shl()
                            .local_get(v)
                            .i64_or();
                    };
                    joined(&mut s);
                    s.local_get(rotated);
                    if m == Rcl {
                        s.i64_shl();
                    } else {
                        s.i64_shr_u();
                    }
                    joined(&mut s);
                    s.i64_const(wide + 1).local_get(rotated).i64_sub();
                    if m == Rcl {
                        s.i64_shr_u();
                    } else {
                        s.i64_shl();
                    }
                    s.i64_or().i64_const(full).i64_and().local_set(result);
                    s.local_get(result)
                        .i64_const(wide)
                        .i64_shr_u()
                        .i32_wrap_i64()
                        .local_set(carry);
                } else if m == Rcl {
                    // (v << c) | (CF << (c - 1)) | (v >> (65 - c)), and CF the bit 64 - c.
                    s.local_get(v)
                        .local_get(rotated)
                        .i64_shl()
                        .local_get(old_carry)
                        .i64_extend_i32_u()
                        .local_get(rotated)
                        .i64_const(1)
                        .i64_sub()
                        .i64_shl()
                        .i64_or()
                        .local_get(v)
                        .i64_const(1)
                        .i64_shr_u()
                        .i64_const(64)
                        .local_get(rotated)
                        .i64_sub()
                        .i64_shr_u()
                        .i64_or()
                        .local_set(result);
                    s.local_get(v)
                        .i64_const(64)
                        .local_get(rotated)
                        .i64_sub()
                        .i64_shr_u()
                        .i32_wrap_i64()
                        .i32_const(1)
                        .i32_and()
                        .local_set(carry);
                } else {
                    // (v >> c) | (CF << (64 - c)) | (v << (65 - c)), and CF the bit c - 1.
                    s.local_get(v)
                        .local_get(rotated)
                        .i64_shr_u()
                        .local_get(old_carry)
                        .i64_extend_i32_u()
                        .i64_const(64)
                        .local_get(rotated)
                        .i64_sub()
                        .i64_shl()
                        .i64_or()
                        .local_get(v)
                        .i64_const(1)
                        .i64_shl()
                        .i64_const(64)
                        .local_get(rotated)
                        .i64_sub()
                        .i64_shl()
                        .i64_or()
                        .local_set(result);
                    s.local_get(v)
                        .local_get(rotated)
                        .i64_const(1)
                        .i64_sub()
                        .i64_shr_u()
                        .i32_wrap_i64()
                        .i32_const(1)
                        .i32_and()
                        .local_set(carry);
                }
            }
        }
        if w < 64 {
            s.local_get(result)
                .i64_const(m_all)
                .i64_and()
                .local_set(result);
        }
        // OF is what a count of one would give, for every count (the reference CPU's answer where Intel
        // leaves it undefined): from the value's top bit and the bit below it, its low bit, or CF.
        let top = |s: &mut wasm_encoder::InstructionSink<'_>, bit: i64| {
            s.local_get(v)
                .i64_const(bit)
                .i64_shr_u()
                .i32_wrap_i64()
                .i32_const(1)
                .i32_and();
        };
        match m {
            Sar => {
                s.i32_const(0);
            }
            Shr => top(&mut s, wide - 1),
            Ror => {
                top(&mut s, wide - 1);
                top(&mut s, 0);
                s.i32_xor();
            }
            Rcr => {
                top(&mut s, wide - 1);
                s.local_get(old_carry).i32_xor();
            }
            _ => {
                top(&mut s, wide - 1);
                top(&mut s, wide - 2);
                s.i32_xor();
            }
        }
        s.local_set(overflow);
    }
    if matches!(m, Rcl | Rcr) {
        e.sink()
            .local_get(v)
            .local_get(result)
            .local_get(rotated)
            .i64_eqz()
            .select()
            .local_set(result);
    }
    write(e, i, 0, mem, result);
    let rotate = matches!(m, Rol | Ror | Rcl | Rcr);
    let mut new = vec![(Flag::C, carry)];
    // The reference CPU keeps OF for `rol` and `ror` of a register by an immediate above one, and gives a
    // count of one's OF otherwise.
    let immediate = match i.iced.op_kind(1) {
        OpKind::Register => None,
        _ => Some(i.iced.immediate(1) & if w == 64 { 63 } else { 31 }),
    };
    if !(matches!(m, Rol | Ror) && mem.is_none() && immediate.is_some_and(|k| k > 1)) {
        new.push((Flag::O, overflow));
    }
    if !rotate {
        let (p, a, z, sign) = (
            e.local(ValType::I32),
            e.local(ValType::I32),
            e.local(ValType::I32),
            e.local(ValType::I32),
        );
        let logic = Flags::Logic { width: w, result };
        for (flag, local) in [(Flag::P, p), (Flag::Z, z), (Flag::S, sign)] {
            crate::flags::flag_value(&mut e.sink(), logic, flag);
            e.sink().local_set(local);
        }
        e.sink().i32_const(0).local_set(a);
        new.extend([(Flag::P, p), (Flag::A, a), (Flag::Z, z), (Flag::S, sign)]);
    }
    let changed = word_with(e, &new);
    let kept = word_with(e, &[]);
    // Nothing changes when the masked count is zero, or for rcl and rcr a multiple of the width plus one.
    let moved = if matches!(m, Rcl | Rcr) {
        rotated
    } else {
        count
    };
    e.sink()
        .local_get(kept)
        .local_get(changed)
        .local_get(moved)
        .i64_eqz()
        .select()
        .local_set(changed);
    e.set_flags(Flags::Packed(changed));
}

/// Push the high 64 bits of the unsigned 128-bit product of the i64 locals `a` and `b`.
fn high_product<E: Emitter<X86_64> + ?Sized>(e: &mut E, a: u32, b: u32, signed: bool) {
    let [a_low, a_high, b_low, b_high, t, middle] = [0; 6].map(|_| e.local(ValType::I64));
    let mut s = e.sink();
    for (from, low, high) in [(a, a_low, a_high), (b, b_low, b_high)] {
        s.local_get(from)
            .i64_const(0xffff_ffff)
            .i64_and()
            .local_set(low)
            .local_get(from)
            .i64_const(32)
            .i64_shr_u()
            .local_set(high);
    }
    s.local_get(a_low).local_get(b_low).i64_mul().local_set(t);
    // a_high * b_low plus the carry out of the low product cannot overflow.
    s.local_get(a_high)
        .local_get(b_low)
        .i64_mul()
        .local_get(t)
        .i64_const(32)
        .i64_shr_u()
        .i64_add()
        .local_set(middle);
    s.local_get(a_low)
        .local_get(b_high)
        .i64_mul()
        .local_get(middle)
        .i64_const(0xffff_ffff)
        .i64_and()
        .i64_add()
        .local_set(t);
    s.local_get(a_high)
        .local_get(b_high)
        .i64_mul()
        .local_get(middle)
        .i64_const(32)
        .i64_shr_u()
        .i64_add()
        .local_get(t)
        .i64_const(32)
        .i64_shr_u()
        .i64_add();
    if signed {
        // Less b where a is negative and a where b is negative.
        for (x, y) in [(a, b), (b, a)] {
            s.local_get(y)
                .i64_const(0)
                .local_get(x)
                .i64_const(0)
                .i64_lt_s()
                .select()
                .i64_sub();
        }
    }
}

fn multiply<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    let ins = &i.iced;
    let signed = m == Mnemonic::Imul;
    let mem = checked(e, i);
    let one_operand = ins.op_count() == 1;
    let w = width(i, 0);
    let (a, b) = if one_operand {
        let a = e.local(ValType::I64);
        get_general(e, accumulator(w));
        e.sink().local_set(a);
        (a, read(e, i, 0, mem))
    } else if ins.op_count() == 2 {
        (read(e, i, 0, None), read(e, i, 1, mem))
    } else {
        (read(e, i, 1, mem), read(e, i, 2, mem))
    };
    let (low, high, overflow) = (
        e.local(ValType::I64),
        e.local(ValType::I64),
        e.local(ValType::I32),
    );
    if w == 64 {
        e.sink().local_get(a).local_get(b).i64_mul().local_set(low);
        high_product(e, a, b, signed);
        e.sink().local_set(high);
        let mut s = e.sink();
        s.local_get(high);
        if signed {
            s.local_get(low).i64_const(63).i64_shr_s();
        } else {
            s.i64_const(0);
        }
        s.i64_ne().local_set(overflow);
    } else {
        if signed {
            sign_extend(e, a, w);
            sign_extend(e, b, w);
        }
        let product = e.local(ValType::I64);
        e.sink()
            .local_get(a)
            .local_get(b)
            .i64_mul()
            .local_tee(product)
            .i64_const(mask(w))
            .i64_and()
            .local_set(low);
        e.sink()
            .local_get(product)
            .i64_const(w.into())
            .i64_shr_s()
            .i64_const(mask(w))
            .i64_and()
            .local_set(high);
        // The product does not fit the width: as unsigned, any high bit; as signed, the low half
        // sign-extended differs from it.
        let extended = e.local(ValType::I64);
        e.sink().local_get(low).local_set(extended);
        if signed {
            sign_extend(e, extended, w);
        }
        let mut s = e.sink();
        if signed {
            s.local_get(extended).local_get(product).i64_ne();
        } else {
            s.local_get(high).i64_const(0).i64_ne();
        }
        s.local_set(overflow);
    }
    if one_operand {
        if w == 8 {
            e.sink()
                .local_get(high)
                .i64_const(8)
                .i64_shl()
                .local_get(low)
                .i64_or()
                .local_set(high);
            set_general(e, accumulator(16), high);
        } else {
            set_general(e, accumulator(w), low);
            set_general(e, register(RDX, w), high);
        }
    } else {
        write(e, i, 0, None, low);
    }
    e.set_flags(Flags::Multiply {
        width: w,
        result: low,
        overflow,
    });
}

/// Stop with a divide error when the i32 on the stack is not zero.
fn divide_error_if<E: Emitter<X86_64> + ?Sized>(e: &mut E) {
    e.sink().if_(BlockType::Empty);
    e.trap(Trap::DivideError);
    e.sink().end();
}

fn divide<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, signed: bool) {
    let mem = checked(e, i);
    let w = width(i, 0);
    let divisor = read(e, i, 0, mem);
    let (quotient, remainder) = (e.local(ValType::I64), e.local(ValType::I64));
    if w == 64 {
        long_divide(e, divisor, signed, quotient, remainder);
        set_general(e, accumulator(64), quotient);
        set_general(e, register(RDX, 64), remainder);
        return;
    }
    // The dividend is twice the width, which fits an i64.
    let dividend = e.local(ValType::I64);
    let (low, high) = if w == 8 {
        (
            accumulator(8),
            General {
                high: true,
                ..accumulator(8)
            },
        )
    } else {
        (accumulator(w), register(RDX, w))
    };
    get_general(e, high);
    e.sink().i64_const(w.into()).i64_shl();
    get_general(e, low);
    e.sink().i64_or().local_set(dividend);
    e.sink().local_get(divisor).i64_eqz();
    divide_error_if(e);
    if signed {
        sign_extend(e, divisor, w);
        sign_extend(e, dividend, 2 * w);
        // i64::MIN / -1 traps in WebAssembly; its quotient never fits.
        e.sink()
            .local_get(dividend)
            .i64_const(i64::MIN)
            .i64_eq()
            .local_get(divisor)
            .i64_const(-1)
            .i64_eq()
            .i32_and();
        divide_error_if(e);
        e.sink()
            .local_get(dividend)
            .local_get(divisor)
            .i64_div_s()
            .local_set(quotient);
        e.sink()
            .local_get(dividend)
            .local_get(divisor)
            .i64_rem_s()
            .local_set(remainder);
        // The quotient must fit the width, signed.
        let limit = 1i64 << (w - 1);
        e.sink()
            .local_get(quotient)
            .i64_const(limit)
            .i64_add()
            .i64_const(2 * limit)
            .i64_ge_u();
    } else {
        e.sink()
            .local_get(dividend)
            .local_get(divisor)
            .i64_div_u()
            .local_set(quotient);
        e.sink()
            .local_get(dividend)
            .local_get(divisor)
            .i64_rem_u()
            .local_set(remainder);
        e.sink().local_get(quotient).i64_const(1i64 << w).i64_ge_u();
    }
    divide_error_if(e);
    truncate(e, quotient, w);
    truncate(e, remainder, w);
    set_general(e, low, quotient);
    set_general(e, high, remainder);
}

/// rdx:rax divided by the 64-bit divisor, with a divide error when the divisor is zero or the quotient does
/// not fit. The usual case, rdx the extension of rax, is one WebAssembly division; otherwise a 128-by-64
/// long division in two 32-bit digit steps (Knuth's algorithm D).
fn long_divide<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    divisor: u32,
    signed: bool,
    quotient: u32,
    remainder: u32,
) {
    let (low, high) = (e.local(ValType::I64), e.local(ValType::I64));
    e.get(RAX);
    e.sink().local_set(low);
    e.get(RDX);
    e.sink().local_set(high);
    e.sink().local_get(divisor).i64_eqz();
    divide_error_if(e);
    // Whether rdx is rax's extension.
    {
        let mut s = e.sink();
        s.local_get(high);
        if signed {
            s.local_get(low).i64_const(63).i64_shr_s();
        } else {
            s.i64_const(0);
        }
        s.i64_eq().if_(BlockType::Empty);
    }
    if signed {
        e.sink()
            .local_get(low)
            .i64_const(i64::MIN)
            .i64_eq()
            .local_get(divisor)
            .i64_const(-1)
            .i64_eq()
            .i32_and();
        divide_error_if(e);
    }
    {
        let mut s = e.sink();
        s.local_get(low).local_get(divisor);
        if signed {
            s.i64_div_s();
        } else {
            s.i64_div_u();
        }
        s.local_set(quotient).local_get(low).local_get(divisor);
        if signed {
            s.i64_rem_s();
        } else {
            s.i64_rem_u();
        }
        s.local_set(remainder).else_();
    }
    // The magnitudes, and the signs they came with.
    let (negative_dividend, negative_divisor, magnitude) = (
        e.local(ValType::I32),
        e.local(ValType::I32),
        e.local(ValType::I64),
    );
    {
        let mut s = e.sink();
        s.local_get(divisor).local_set(magnitude);
        if signed {
            s.local_get(high)
                .i64_const(0)
                .i64_lt_s()
                .local_set(negative_dividend);
            s.local_get(divisor)
                .i64_const(0)
                .i64_lt_s()
                .local_tee(negative_divisor)
                .if_(BlockType::Empty)
                .i64_const(0)
                .local_get(divisor)
                .i64_sub()
                .local_set(magnitude)
                .end();
            // Negate the 128-bit dividend: ~x + 1.
            s.local_get(negative_dividend)
                .if_(BlockType::Empty)
                .local_get(high)
                .i64_const(-1)
                .i64_xor()
                .local_get(low)
                .i64_eqz()
                .i64_extend_i32_u()
                .i64_add()
                .local_set(high)
                .i64_const(0)
                .local_get(low)
                .i64_sub()
                .local_set(low)
                .end();
        }
    }
    // The quotient fits 64 bits only when the high half is below the divisor.
    e.sink().local_get(high).local_get(magnitude).i64_ge_u();
    divide_error_if(e);
    divide_128(e, (high, low), magnitude, quotient, remainder);
    if signed {
        // The quotient's sign is the signs' difference and must fit; the remainder takes the dividend's.
        let negative = e.local(ValType::I32);
        e.sink()
            .local_get(negative_dividend)
            .local_get(negative_divisor)
            .i32_xor()
            .local_set(negative);
        e.sink()
            .local_get(quotient)
            .i64_const(i64::MIN)
            .i64_gt_u()
            .local_get(quotient)
            .i64_const(i64::MAX)
            .i64_gt_u()
            .local_get(negative)
            .select();
        divide_error_if(e);
        e.sink()
            .i64_const(0)
            .local_get(quotient)
            .i64_sub()
            .local_get(quotient)
            .local_get(negative)
            .select()
            .local_set(quotient);
        e.sink()
            .i64_const(0)
            .local_get(remainder)
            .i64_sub()
            .local_get(remainder)
            .local_get(negative_dividend)
            .select()
            .local_set(remainder);
    }
    e.sink().end();
}

/// The unsigned 128-bit `high:low` divided by `divisor`, with `high` below it, into `quotient` and
/// `remainder`: normalise so the divisor's top bit is set, then two digit steps of 32 bits, each estimating
/// a digit from the top digits and correcting it at most twice.
fn divide_128<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    (high, low): (u32, u32),
    divisor: u32,
    quotient: u32,
    remainder: u32,
) {
    const DIGIT: i64 = 1 << 32;
    let [
        shift,
        v,
        v_high,
        v_low,
        top,
        below,
        low_high,
        low_low,
        q1,
        q0,
        rhat,
        middle,
    ] = [0; 12].map(|_| e.local(ValType::I64));
    let mut s = e.sink();
    s.local_get(divisor).i64_clz().local_set(shift);
    s.local_get(divisor)
        .local_get(shift)
        .i64_shl()
        .local_tee(v)
        .i64_const(32)
        .i64_shr_u()
        .local_set(v_high);
    s.local_get(v)
        .i64_const(0xffff_ffff)
        .i64_and()
        .local_set(v_low);
    // The dividend shifted left too; the bits of `low` that move up are none when the shift is zero.
    s.local_get(high)
        .local_get(shift)
        .i64_shl()
        .i64_const(0)
        .local_get(low)
        .i64_const(64)
        .local_get(shift)
        .i64_sub()
        .i64_shr_u()
        .local_get(shift)
        .i64_eqz()
        .select()
        .i64_or()
        .local_set(top);
    s.local_get(low).local_get(shift).i64_shl().local_set(below);
    s.local_get(below)
        .i64_const(32)
        .i64_shr_u()
        .local_set(low_high);
    s.local_get(below)
        .i64_const(0xffff_ffff)
        .i64_and()
        .local_set(low_low);
    // One digit of the quotient of `from` (with the next digit `next`) by v, into `digit`.
    let step = |s: &mut wasm_encoder::InstructionSink<'_>, from: u32, next: u32, digit: u32| {
        s.local_get(from)
            .local_get(v_high)
            .i64_div_u()
            .local_set(digit);
        s.local_get(from)
            .local_get(digit)
            .local_get(v_high)
            .i64_mul()
            .i64_sub()
            .local_set(rhat);
        s.block(BlockType::Empty).loop_(BlockType::Empty);
        // Too large while digit >= 2^32 or digit * v_low > rhat * 2^32 + next.
        s.local_get(digit)
            .i64_const(DIGIT)
            .i64_ge_u()
            .local_get(digit)
            .local_get(v_low)
            .i64_mul()
            .local_get(rhat)
            .i64_const(32)
            .i64_shl()
            .local_get(next)
            .i64_or()
            .i64_gt_u()
            .i32_or()
            .i32_eqz()
            .br_if(1);
        s.local_get(digit).i64_const(1).i64_sub().local_set(digit);
        s.local_get(rhat)
            .local_get(v_high)
            .i64_add()
            .local_tee(rhat)
            .i64_const(DIGIT)
            .i64_lt_u()
            .br_if(0);
        s.end().end();
    };
    step(&mut s, top, low_high, q1);
    // What is left after the first digit, with the next digit brought down.
    s.local_get(top)
        .i64_const(32)
        .i64_shl()
        .local_get(low_high)
        .i64_or()
        .local_get(q1)
        .local_get(v)
        .i64_mul()
        .i64_sub()
        .local_set(middle);
    step(&mut s, middle, low_low, q0);
    s.local_get(q1)
        .i64_const(32)
        .i64_shl()
        .local_get(q0)
        .i64_or()
        .local_set(quotient);
    s.local_get(middle)
        .i64_const(32)
        .i64_shl()
        .local_get(low_low)
        .i64_or()
        .local_get(q0)
        .local_get(v)
        .i64_mul()
        .i64_sub()
        .local_get(shift)
        .i64_shr_u()
        .local_set(remainder);
}

/// `bt`, `bts`, `btr` and `btc`: CF is the bit; the others as the reference CPU leaves them. A register bit
/// offset with a memory operand reaches whole operands away, the offset read signed.
fn bit_test<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    let ins = &i.iced;
    let w = width(i, 0);
    let offset = read(e, i, 1, None);
    let mem = memory_use(i).map(|kind| {
        // The offset is added before the address is wrapped to its size and the segment base added.
        let at = linear_free_address(e, i);
        if ins.op_kind(1) == OpKind::Register {
            let extended = e.local(ValType::I64);
            e.sink().local_get(offset).local_set(extended);
            sign_extend(e, extended, w);
            e.sink()
                .local_get(at)
                .local_get(extended)
                .i64_const(i64::from(w.trailing_zeros()))
                .i64_shr_s()
                .i64_const(i64::from((w / 8).trailing_zeros()))
                .i64_shl()
                .i64_add()
                .local_set(at);
            if i.address32 {
                e.sink()
                    .local_get(at)
                    .i64_const(0xffff_ffff)
                    .i64_and()
                    .local_set(at);
            }
        }
        if let Some(base) = segment(i) {
            e.get(base);
            e.sink().local_get(at).i64_add().local_set(at);
        }
        e.check(at, u32::from(w / 8), check_kind(kind), 0);
        at
    });
    let value = read(e, i, 0, mem);
    let (bit, carry) = (e.local(ValType::I64), e.local(ValType::I32));
    e.sink()
        .local_get(offset)
        .i64_const(i64::from(w - 1))
        .i64_and()
        .local_set(bit);
    e.sink()
        .local_get(value)
        .local_get(bit)
        .i64_shr_u()
        .i32_wrap_i64()
        .i32_const(1)
        .i32_and()
        .local_set(carry);
    if m != Mnemonic::Bt {
        let result = e.local(ValType::I64);
        let mut s = e.sink();
        s.i64_const(1).local_get(bit).i64_shl();
        match m {
            Mnemonic::Bts => s.local_get(value).i64_or(),
            Mnemonic::Btr => s.i64_const(-1).i64_xor().local_get(value).i64_and(),
            _ => s.local_get(value).i64_xor(),
        };
        s.local_set(result);
        write(e, i, 0, mem, result);
    }
    let word = word_with(e, &[(Flag::C, carry)]);
    e.set_flags(Flags::Packed(word));
}
