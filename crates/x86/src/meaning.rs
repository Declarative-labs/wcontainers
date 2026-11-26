//! The meaning of each instruction as WebAssembly, from Intel's manual and what the reference CPU does.

use crate::claims::{CLAIM_KERNEL, CLAIM_NEVER};
use crate::decode::{Instruction, claim, told_feature};
use crate::flags::{Flag, Flags, KEPT_BY_SHIFT, Unmoved};
use crate::guest::{Control, Hook, Reason, Trap};
use crate::operand::*;
use crate::strings::{BULK_STRING_BYTES, StringKind};
use crate::{ALIGNMENT_CHECK, ALWAYS, ARITHMETIC_FLAGS, DF, TF, X86_64};
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
    match claim(ins.code()) {
        // At CPL 3 every kernel-only instruction is general protection, but rsm, which exists only in
        // system management mode and is #UD outside it.
        CLAIM_KERNEL => {
            e.trap(if ins.mnemonic() == Mnemonic::Rsm {
                Trap::Undefined
            } else {
                Trap::GeneralProtection
            });
            return;
        }
        CLAIM_NEVER => {
            e.trap(Trap::Unsupported);
            return;
        }
        // As on a CPU without the feature.
        c if told_feature(c).is_some_and(|at| !e.offer().tells(at)) => {
            e.warn(Reason::NotToldFeatureReached);
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
        return matches!(
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
    if crate::simd::written(i)
        || crate::vex::written(i)
        || crate::wide::written(i)
        || crate::fused::written(i)
        || crate::half::written(i)
        || crate::masked::written(i)
        || crate::float::written(i)
        || crate::fpu::written(m)
        || crate::text::written(m)
        || crate::descriptors::written(m)
        || crate::segments::written(i)
        || m == Maskmovdqu
    {
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
                    | Btr | Btc | Jrcxz | Jecxz | Pushf | Pushfq | Leave | Bsf | Bsr | Tzcnt
                    | Lzcnt | Popcnt | Bswap | Movbe | Xadd | Cmpxchg | Cmpxchg8b | Cmpxchg16b
                    | Shld | Shrd | Adcx | Adox | Loop | Loope | Loopne | Lahf | Sahf | Clflush
                    | Rdfsbase | Rdgsbase | Wrfsbase | Wrgsbase | Popf | Popfq | Rdtsc
                    | Rdtscp | Cpuid | Xlatb | Movnti | Lfence | Mfence | Sfence | Crc32
                    | Enter | Andn | Bextr | Blsi | Blsmsk | Blsr | Emms | Bzhi | Mulx | Pdep
                    | Pext | Rorx | Sarx | Shlx | Shrx | Xgetbv | Xsetbv
            ))
        // Far transfers through memory: a selector with the offset.
        && !(matches!(m, Call | Jmp) && is_memory(i, 0) && memory_bytes(i) != SLOT)
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
    if crate::vex::written(i) {
        crate::vex::emit(e, i, i.iced.mnemonic());
        e.next();
        return true;
    }
    if crate::wide::written(i) {
        crate::wide::emit(e, i, i.iced.mnemonic());
        e.next();
        return true;
    }
    if crate::fused::written(i) {
        crate::fused::emit(e, i, i.iced.mnemonic());
        e.next();
        return true;
    }
    if crate::half::written(i) {
        crate::half::emit(e, i, i.iced.mnemonic());
        e.next();
        return true;
    }
    if crate::masked::written(i) {
        crate::masked::emit(e, i);
        e.next();
        return true;
    }
    legacy_meaning(e, i)
}

/// The meaning of an instruction that is not VEX-encoded, or of a lane's legacy form.
pub(crate) fn legacy_meaning<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) -> bool {
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
    if crate::float::written(i) {
        crate::float::emit(e, i, m);
        e.next();
        return true;
    }
    if crate::fpu::written(m) {
        crate::fpu::emit(e, i, m);
        e.next();
        return true;
    }
    if crate::descriptors::written(m) {
        crate::descriptors::emit(e, i, m);
        e.next();
        return true;
    }
    if crate::segments::written(i) {
        if crate::segments::emit(e, i) {
            e.next();
        }
        return true;
    }
    if crate::text::written(m) {
        crate::text::emit(e, i, m);
        e.next();
        return true;
    }
    match m {
        Add | Adc | Sub | Sbb | Cmp | And | Or | Xor | Test => arithmetic(e, i, m),
        Inc | Dec | Neg | Not => unary(e, i, m),
        // One thread runs at a time, or x86's order is kept by the accesses themselves, so these fences order nothing
        // more; `mfence` is a fence of its own only where stores stay plain (`operand::store_load_fence`).
        Lfence | Sfence => {}
        Mfence => crate::operand::store_load_fence(e),
        Mov | Movzx | Movnti => {
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
        Xchg if locked(e, i) => {
            let mem = checked(e, i).expect("a memory operand");
            let n = u32::from(i.iced.op_kind(0) != OpKind::Memory);
            let bytes = u32::from(width(i, n) / W8);
            locked_start(e);
            let old = read(e, i, n, Some(mem));
            let new = read(e, i, 1 - n, None);
            locked_write(e, (mem, bytes), old, new);
            write(e, i, 1 - n, None, old);
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
                Cbw => W16,
                Cwde => W32,
                _ => W64,
            };
            let value = e.local(ValType::I64);
            get_general(e, accumulator(w / 2));
            e.sink().local_set(value);
            sign_extend(e, value, w / 2);
            set_general(e, accumulator(w), value);
        }
        Cwd | Cdq | Cqo => {
            let w = match m {
                Cwd => W16,
                Cdq => W32,
                _ => W64,
            };
            let value = e.local(ValType::I64);
            get_general(e, accumulator(w));
            e.sink().local_set(value);
            sign_extend(e, value, w);
            e.sink()
                .local_get(value)
                .i64_const((W64 - 1).into())
                .i64_shr_s()
                .local_set(value);
            set_general(e, register(RDX, w), value);
        }
        Push => push(e, i),
        Pushf | Pushfq => {
            // rflags as user code sees it: the six flags, the bits kept in the block, and those always set.
            let word = word_with(e, &[]);
            let value = e.local(ValType::I64);
            e.get_control(Control::Rflags);
            let mut s = e.sink();
            s.local_get(word)
                .i64_extend_i32_u()
                .i64_or()
                .i64_const(crate::ALWAYS as i64)
                .i64_or()
                .local_set(value);
            push_value(e, i, None, value);
        }
        Pop => pop(e, i),
        Popf | Popfq => pop_flags(e, i),
        Rdtsc | Rdtscp => time_stamp(e, m == Rdtscp),
        Cpuid => cpuid(e),
        Crc32 => crc32(e, i),
        // al = [rbx + al].
        Xlatb => {
            let at = checked(e, i).expect("a memory operand");
            let value = e.local(ValType::I64);
            load(e, at, memory_bytes(i));
            e.sink().local_set(value);
            set_general(e, register(RAX, W8), value);
        }
        Leave => {
            // rsp = rbp, then pop rbp (bp with 66), the read checked before anything changes.
            let bytes = stack_bytes(i);
            let from = e.local(ValType::I64);
            e.get(RBP);
            e.sink().local_set(from);
            e.check(from, bytes, Access::Read, 0);
            let value = e.local(ValType::I64);
            load(e, from, bytes);
            e.sink().local_set(value);
            e.sink().local_get(from).i64_const(bytes.into()).i64_add();
            e.set(RSP);
            set_general(e, register(RBP, (bytes * u8::BITS) as u8), value);
        }
        Call => call(e, i),
        Enter => enter(e, i),
        Maskmovdqu => masked_store(e, i),
        Emms => {
            crate::fpu::pending(e);
            crate::simd::mmx_transition(e, true);
        }
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
        Bsf | Bsr | Tzcnt | Lzcnt | Popcnt => bit_count(e, i, m),
        Bswap | Movbe => byte_reverse(e, i, m),
        Andn | Bextr | Blsi | Blsmsk | Blsr => bit_manipulation(e, i, m),
        Bzhi => zero_high_bits(e, i),
        Xgetbv => extended_control(e),
        // Writing an extended control register is the kernel's.
        Xsetbv => {
            e.trap(Trap::GeneralProtection);
            return true;
        }
        Mulx => multiply_flagless(e, i),
        Pdep | Pext => deposit_or_extract(e, i, m),
        Rorx | Sarx | Shlx | Shrx => shift_flagless(e, i, m),
        Xadd => exchange_add(e, i),
        Cmpxchg => compare_exchange(e, i),
        Cmpxchg8b | Cmpxchg16b => compare_exchange_pair(e, i),
        Shld | Shrd => double_shift(e, i, m),
        Adcx => add_with(e, i, Flag::C),
        Adox => add_with(e, i, Flag::O),
        Loop | Loope | Loopne => loop_(e, i, m),
        Lahf => load_flags(e),
        Rdfsbase | Rdgsbase => {
            let value = e.local(ValType::I64);
            e.get(if m == Rdfsbase { FS } else { GS });
            e.sink().local_set(value);
            write(e, i, 0, None, value);
        }
        // A base that is not a canonical address gives general protection.
        Wrfsbase | Wrgsbase => {
            let value = read(e, i, 0, None);
            canonical_target(e, value);
            e.sink().local_get(value);
            e.set(if m == Wrfsbase { FS } else { GS });
        }
        Sahf => store_flags(e),
        // Only its access is seen: a line written back to memory changes nothing a program can tell.
        Clflush => {
            checked(e, i);
        }
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
        // Nothing reads the carry they set.
        Clc | Stc | Cmc if !e.flags_live() => {}
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
            let each = each_with(e, &[(Flag::C, carry)]);
            e.set_flags(Flags::Each(each));
        }
        Jrcxz | Jecxz => {
            // A jump when rcx (ecx with an address-size prefix) is zero; no flag is read.
            get_general(e, register(RCX, if m == Jrcxz { W64 } else { W32 }));
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
        OpKind::Register => general(ins.op_register(n)).map_or(W64, |g| g.width),
        OpKind::Memory => (memory_bytes(i) * u8::BITS) as u8,
        _ if n > 0 => width(i, 0),
        _ => W64,
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
pub(crate) fn checked<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) -> Option<u32> {
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
pub(crate) fn read<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    n: u32,
    mem: Option<u32>,
) -> u32 {
    let ins = &i.iced;
    let w = width(i, n);
    let value = e.local(ValType::I64);
    match ins.op_kind(n) {
        OpKind::Register => {
            get_general(e, general(ins.op_register(n)).expect("a general register"))
        }
        OpKind::Memory => load(e, mem.expect("checked"), u32::from(w / W8)),
        _ => {
            e.sink().i64_const(ins.immediate(n) as i64 & mask(w));
        }
    }
    e.sink().local_set(value);
    value
}

/// Write the i64 in `value` to operand `n`.
pub(crate) fn write<E: Emitter<X86_64> + ?Sized>(
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
        _ => store(e, mem.expect("checked"), u32::from(width(i, n) / W8), value),
    }
}

/// Sign-extend the i64 in `value` from `width` bits, in place.
fn sign_extend<E: Emitter<X86_64> + ?Sized>(e: &mut E, value: u32, width: u8) {
    if width < W64 {
        let shift = i64::from(W64 - width);
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
    if width < W64 {
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

/// Each flag in an i32 local: these from their locals, the rest read as they are.
pub(crate) fn each_with<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    new: &[(Flag, u32)],
) -> [u32; Flag::ALL.len()] {
    Flag::ALL.map(|flag| match new.iter().find(|(f, _)| *f == flag) {
        Some(&(_, local)) => local,
        None => {
            let local = e.local(ValType::I32);
            e.flag(flag);
            e.sink().local_set(local);
            local
        }
    })
}

fn arithmetic<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    use Mnemonic::*;
    let w = width(i, 0);
    let mem = checked(e, i);
    let lock = locked(e, i) && !matches!(m, Cmp | Test);
    if lock {
        locked_start(e);
    }
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
    if lock {
        let at = mem.expect("a memory operand");
        locked_write(e, (at, u32::from(w / W8)), a, result);
    } else if !matches!(m, Cmp | Test) {
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
    let lock = locked(e, i);
    if lock {
        locked_start(e);
    }
    let value = read(e, i, 0, mem);
    let result = e.local(ValType::I64);
    let put = |e: &mut E| match (lock, mem) {
        (true, Some(at)) => locked_write(e, (at, u32::from(w / W8)), value, result),
        _ => write(e, i, 0, mem, result),
    };
    match m {
        Inc => e.sink().local_get(value).i64_const(1).i64_add(),
        Dec => e.sink().local_get(value).i64_const(1).i64_sub(),
        Neg => e.sink().i64_const(0).local_get(value).i64_sub(),
        _ => e.sink().local_get(value).i64_const(-1).i64_xor(),
    };
    e.sink().local_set(result);
    truncate(e, result, w);
    if !e.flags_live() {
        put(e);
        return;
    }
    let kept = matches!(m, Inc | Dec).then(|| {
        let c = e.local(ValType::I32);
        e.flag(Flag::C);
        e.sink().local_set(c);
        c
    });
    put(e);
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
pub(crate) fn canonical_target<E: Emitter<X86_64> + ?Sized>(e: &mut E, local: u32) {
    not_canonical(e, local);
    e.sink().if_(BlockType::Empty);
    e.trap(Trap::GeneralProtection);
    e.sink().end();
}

fn push<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let mem = checked(e, i);
    let value = read(e, i, 0, mem);
    push_value(e, i, mem, value);
}

/// Push the i64 in `value`, the instruction's operand (read from memory at `mem`) or what it makes.
pub(crate) fn push_value<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    mem: Option<u32>,
    value: u32,
) {
    let bytes = stack_bytes(i);
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

/// `enter` takes its nesting level modulo this.
pub(crate) const ENTER_LEVELS: u8 = 32;

/// `enter size, level`: push rbp, copy `level - 1` frame pointers down from the old frame and push the new
/// frame's address, then rbp is the new frame and rsp moves down by `size`. Each access is checked just before
/// it is made, and the registers change only at the end.
fn enter<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let ins = &i.iced;
    let bytes = stack_bytes(i);
    let size = ins.immediate16();
    let level = ins.immediate8_2nd() % ENTER_LEVELS;
    let (at, frame_pointer, value, frame) = (
        e.local(ValType::I64),
        e.local(ValType::I64),
        e.local(ValType::I64),
        e.local(ValType::I64),
    );
    e.get(RSP);
    e.sink().local_set(frame);
    e.get(RBP);
    e.sink().local_set(frame_pointer);
    // Each push writes below the last; `which` 0 is the push of rbp, as `Guest::accesses` describes it.
    let push = |e: &mut E, value: u32, which: u8| {
        e.sink()
            .local_get(frame)
            .i64_const(-i64::from(bytes))
            .i64_add()
            .local_tee(frame)
            .local_set(at);
        e.check(at, bytes, Access::Write, which);
        store(e, at, bytes, value);
    };
    push(e, frame_pointer, 0);
    let pushed = e.local(ValType::I64);
    e.sink().local_get(frame).local_set(pushed);
    if level > 0 {
        for _ in 1..level {
            e.sink()
                .local_get(frame_pointer)
                .i64_const(-i64::from(bytes))
                .i64_add()
                .local_tee(frame_pointer)
                .local_set(at);
            e.check(at, bytes, Access::Read, 1);
            load(e, at, bytes);
            e.sink().local_set(value);
            push(e, value, 1);
        }
        push(e, pushed, 1);
    }
    // After the pushes the CPU checks a write at the final stack pointer (Intel's manual: "a write using the
    // final value of the stack pointer").
    e.sink()
        .local_get(frame)
        .i64_const(-i64::from(size))
        .i64_add()
        .local_set(at);
    e.check(at, bytes, Access::Write, 1);
    // A word-sized `enter` writes bp alone, over what the copies left in rbp.
    e.sink().local_get(frame_pointer);
    e.set(RBP);
    set_general(e, register(RBP, (bytes * u8::BITS) as u8), pushed);
    e.sink()
        .local_get(frame)
        .i64_const(-i64::from(size))
        .i64_add();
    e.set(RSP);
}

/// `maskmovdqu`: the bytes of the first register whose byte in the second has its top bit set, stored at
/// [rdi]. The CPU checks the 16 bytes whatever the mask, the upper 8 first; each half's address wraps by
/// itself under an address-size prefix, so the halves are stored apart.
fn masked_store<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let ins = &i.iced;
    let half = crate::XMM_BYTES / 2;
    let [low, high] = [0, half].map(|by| {
        let at = register_address(e, i, RDI, by.into());
        if let Some(base) = segment(i) {
            e.sink().local_get(at);
            e.get(base);
            e.sink().i64_add().local_set(at);
        }
        at
    });
    e.check(high, half, Access::Write, 0);
    e.check(low, half, Access::Write, 1);
    let (merged, value) = (e.local(ValType::V128), e.local(ValType::I64));
    e.get_vector(xmm(ins.op_register(1)).expect("an xmm register"));
    e.sink().v128_const(0);
    load(e, low, half);
    e.sink().i64x2_replace_lane(0);
    load(e, high, half);
    e.sink().i64x2_replace_lane(1);
    e.get_vector(xmm(ins.op_register(2)).expect("an xmm register"));
    e.sink()
        .i32_const((u8::BITS - 1) as i32)
        .i8x16_shr_s()
        .v128_bitselect()
        .local_set(merged);
    for (lane, at) in [low, high].into_iter().enumerate() {
        e.sink()
            .local_get(merged)
            .i64x2_extract_lane(lane as u8)
            .local_set(value);
        store(e, at, half, value);
    }
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
        -i64::from(SLOT),
    );
    e.check(at, SLOT, Access::Write, u8::from(mem.is_some()));
    let back = e.local(ValType::I64);
    e.this_address(ins.len() as i64);
    e.sink().local_set(back);
    store(e, at, SLOT, back);
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
    e.check(at, SLOT, Access::Read, 0);
    let target = e.local(ValType::I64);
    load(e, at, SLOT);
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

impl StringKind {
    pub fn of(m: Mnemonic) -> Self {
        use Mnemonic::*;
        match m {
            Movsb | Movsw | Movsd | Movsq => StringKind::Move,
            Stosb | Stosw | Stosd | Stosq => StringKind::Store,
            Lodsb | Lodsw | Lodsd | Lodsq => StringKind::Load,
            Cmpsb | Cmpsw | Cmpsd | Cmpsq => StringKind::Compare,
            _ => StringKind::Scan,
        }
    }
}

/// Movs, stos, lods, cmps and scas: one element, or with a repeat prefix as many as rcx says.
fn string<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    let kind = StringKind::of(m);
    if i.iced.has_rep_prefix() || i.iced.has_repne_prefix() {
        repeated_string(e, i, kind);
        return;
    }
    let (source_at, destination_at) = string_addresses(e, i, kind);
    if let Some(flags) = string_element(e, i, kind, source_at, destination_at) {
        e.set_flags(flags);
    }
    string_advance(e, i, kind, None, None);
}

/// The addresses of an element: rsi with its segment base, and rdi.
fn string_addresses<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    kind: StringKind,
) -> (Option<u32>, Option<u32>) {
    let source_at = kind.source().then(|| {
        let at = register_address(e, i, RSI, 0);
        if let Some(base) = segment(i) {
            e.get(base);
            e.sink().local_get(at).i64_add().local_set(at);
        }
        at
    });
    let destination_at = kind.destination().then(|| register_address(e, i, RDI, 0));
    (source_at, destination_at)
}

/// One element's checks and work, and for cmps and scas the flags it sets; rsi, rdi and rcx are left as
/// they are.
fn string_element<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    kind: StringKind,
    source_at: Option<u32>,
    destination_at: Option<u32>,
) -> Option<Flags> {
    let bytes = element_bytes(i);
    let w = (bytes * u8::BITS) as u8;
    let destination_access = if matches!(kind, StringKind::Move | StringKind::Store) {
        Access::Write
    } else {
        Access::Read
    };
    // The CPU's order: cmps reads [rdi] before [rsi]; movs reads [rsi] before writing [rdi].
    let mut checks = [
        source_at.map(|at| (at, Access::Read)),
        destination_at.map(|at| (at, destination_access)),
    ];
    if kind == StringKind::Compare {
        checks.reverse();
    }
    for (which, (at, access)) in checks.into_iter().flatten().enumerate() {
        e.check(at, bytes, access, which as u8);
    }
    let acc = accumulator(w);
    match kind {
        StringKind::Move => {
            let value = e.local(ValType::I64);
            load(e, source_at.unwrap(), bytes);
            e.sink().local_set(value);
            store(e, destination_at.unwrap(), bytes, value);
        }
        StringKind::Store => {
            let value = e.local(ValType::I64);
            get_general(e, acc);
            e.sink().local_set(value);
            store(e, destination_at.unwrap(), bytes, value);
        }
        StringKind::Load => {
            let value = e.local(ValType::I64);
            load(e, source_at.unwrap(), bytes);
            e.sink().local_set(value);
            set_general(e, acc, value);
        }
        StringKind::Compare | StringKind::Scan => {
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
            return Some(Flags::Sub {
                width: w,
                a,
                b,
                borrow: None,
                result,
                kept_carry: None,
            });
        }
    }
    None
}

/// Move rsi and rdi past one element, or past the count of them in the i64 local `elements`, down when DF
/// is set; with `ran`, an i32 local, only when it is not zero.
fn string_advance<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    kind: StringKind,
    elements: Option<u32>,
    ran: Option<u32>,
) {
    let bytes = element_bytes(i);
    let step = e.local(ValType::I64);
    e.sink()
        .i64_const(-i64::from(bytes))
        .i64_const(bytes.into());
    e.get_control(Control::Rflags);
    let mut s = e.sink();
    s.i64_const(DF as i64)
        .i64_and()
        .i64_const(0)
        .i64_ne()
        .select();
    if let Some(n) = elements {
        s.local_get(n).i64_mul();
    }
    s.local_set(step);
    for (used, number) in [(kind.source(), RSI), (kind.destination(), RDI)] {
        if used {
            let moved = e.local(ValType::I64);
            e.get(number);
            e.sink().local_get(step).i64_add().local_set(moved);
            match ran {
                // A 32-bit register left as it is, upper half and all, when no element ran.
                Some(ran) if i.address32 => {
                    e.sink().local_get(moved);
                    e.sink().i64_const(mask(W32));
                    e.sink().i64_and();
                    e.get(number);
                    e.sink().local_get(ran).select().local_set(moved);
                    set_general(e, register(number, W64), moved);
                }
                _ => set_general(
                    e,
                    register(number, if i.address32 { W32 } else { W64 }),
                    moved,
                ),
            }
        }
    }
}

/// A string instruction with a repeat prefix. Each pass ends on an element boundary with exact rcx, rsi
/// and rdi and jumps back to the instruction while elements remain, so the stop check runs between passes
/// and a stop can land between any two elements.
fn repeated_string<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, kind: StringKind) {
    let count_register = register(RCX, if i.address32 { W32 } else { W64 });
    if i.address32 {
        // With 32-bit addresses the reference writes ecx before anything else, and for movs and stos esi
        // and edi too, clearing their upper halves even when no element is done.
        let bulk = matches!(kind, StringKind::Move | StringKind::Store);
        for (used, number) in [(true, RCX), (bulk && kind.source(), RSI), (bulk, RDI)] {
            if used {
                let g = register(number, W32);
                let value = e.local(ValType::I64);
                get_general(e, g);
                e.sink().local_set(value);
                set_general(e, g, value);
            }
        }
    }
    if matches!(kind, StringKind::Compare | StringKind::Scan) {
        repeated_compare(e, i, kind, count_register);
        return;
    }
    // A count of zero does no pass and falls through, so the next instruction is reached from one place.
    let ran = e.local(ValType::I32);
    get_general(e, count_register);
    e.sink()
        .i64_eqz()
        .i32_eqz()
        .local_tee(ran)
        .if_(BlockType::Empty);
    let elements = e.local(ValType::I64);
    if kind != StringKind::Load && !i.address32 {
        bulk_or_element(e, i, kind, elements);
    } else {
        e.warn(if kind == StringKind::Load {
            Reason::StringLoaded
        } else {
            Reason::StringAddress32
        });
        let (source_at, destination_at) = string_addresses(e, i, kind);
        string_element(e, i, kind, source_at, destination_at);
        e.sink().i64_const(1).local_set(elements);
    }
    e.sink().else_().i64_const(0).local_set(elements).end();
    let left = e.local(ValType::I64);
    get_general(e, count_register);
    e.sink().local_get(elements).i64_sub().local_set(left);
    set_general(e, count_register, left);
    string_advance(e, i, kind, Some(elements), Some(ran));
    e.sink().local_get(left).i64_const(0).i64_ne();
    e.jump_if(Target::Relative(0));
}

/// `repe` and `repne` `cmps` and `scas`, one element a pass. The flags change only on the pass that ends
/// the instruction: the reference CPU shows the flags from before it at a fault or a trap-flag stop part
/// way, and a count of zero keeps them.
fn repeated_compare<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    kind: StringKind,
    count_register: General,
) {
    e.warn(Reason::StringCompared);
    let old = each_with(e, &[]);
    let ran = e.local(ValType::I32);
    get_general(e, count_register);
    e.sink()
        .i64_eqz()
        .i32_eqz()
        .local_tee(ran)
        .if_(BlockType::Empty);
    let (source_at, destination_at) = string_addresses(e, i, kind);
    let flags = string_element(e, i, kind, source_at, destination_at).expect("a compare");
    e.sink().end();
    let Flags::Sub { result, .. } = flags else {
        unreachable!("a compare's flags")
    };
    let (elements, left) = (e.local(ValType::I64), e.local(ValType::I64));
    get_general(e, count_register);
    e.sink()
        .local_get(ran)
        .i64_extend_i32_u()
        .local_tee(elements)
        .i64_sub()
        .local_set(left);
    set_general(e, count_register, left);
    string_advance(e, i, kind, Some(elements), Some(ran));
    // It ends when the count runs out, or when repe finds a difference or repne an equal pair.
    let ends = e.local(ValType::I32);
    {
        let mut s = e.sink();
        s.local_get(left).i64_eqz().local_get(result).i64_eqz();
        if !i.iced.has_repne_prefix() {
            s.i32_eqz();
        }
        s.i32_or().local_get(ran).i32_and().local_set(ends);
    }
    let each = Flag::ALL.map(|_| e.local(ValType::I32));
    for (k, flag) in Flag::ALL.into_iter().enumerate() {
        crate::flags::flag_value(&mut e.sink(), flags, flag);
        e.sink()
            .local_get(old[k])
            .local_get(ends)
            .select()
            .local_set(each[k]);
    }
    e.set_flags(Flags::Each(each));
    e.sink().local_get(ran).local_get(ends).i32_eqz().i32_and();
    e.jump_if(Target::Relative(0));
}

/// The bulk pass of `rep movs` and `rep stos` when it gives what elements one at a time would (DF and TF
/// clear, no copy onto a destination starting inside its source, every byte of both ranges allowed), else
/// one element counted with the reason. Leaves the elements done in the i64 local `elements`.
fn bulk_or_element<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    kind: StringKind,
    elements: u32,
) {
    let bytes = element_bytes(i);
    let (source_at, destination_at) = string_addresses(e, i, kind);
    let destination = destination_at.expect("a destination");
    let most = (BULK_STRING_BYTES / u64::from(bytes)) as i64;
    let count = e.local(ValType::I64);
    get_general(e, register(RCX, W64));
    let length = e.local(ValType::I64);
    e.sink()
        .local_tee(count)
        .i64_const(most)
        .local_get(count)
        .i64_const(most)
        .i64_lt_u()
        .select()
        .local_tee(elements)
        .i64_const(bytes.into())
        .i64_mul()
        .local_set(length);
    let rflags = e.local(ValType::I64);
    e.get_control(Control::Rflags);
    e.sink().local_set(rflags);
    // A forward copy onto a destination 1 to length - 1 bytes above its source repeats what it wrote.
    let overlap = e.local(ValType::I32);
    match source_at {
        Some(source) => e
            .sink()
            .local_get(destination)
            .local_get(source)
            .i64_sub()
            .i64_const(1)
            .i64_sub()
            .local_get(length)
            .i64_const(1)
            .i64_sub()
            .i64_lt_u()
            .local_set(overlap),
        None => e.sink().i32_const(0).local_set(overlap),
    };
    let readable = source_at.map(|at| e.check_range(at, length, Access::Read));
    let writable = e.check_range(destination, length, Access::Write);
    {
        let mut s = e.sink();
        s.local_get(rflags)
            .i64_const((DF | TF) as i64)
            .i64_and()
            .i64_eqz()
            .local_get(overlap)
            .i32_eqz()
            .i32_and()
            .local_get(writable)
            .i32_and();
        if let Some(readable) = readable {
            s.local_get(readable).i32_and();
        }
        s.if_(BlockType::Empty);
    }
    bulk_fence(e);
    match source_at {
        Some(source) => {
            e.sink()
                .local_get(destination)
                .local_get(source)
                .local_get(length)
                .memory_copy(GUEST_MEMORY, GUEST_MEMORY);
        }
        None => bulk_store(e, i, destination, length),
    }
    bulk_fence(e);
    e.sink().else_();
    for (bit, reason) in [
        (DF, Reason::StringDirectionDown),
        (TF, Reason::StringTrapFlag),
    ] {
        e.sink()
            .local_get(rflags)
            .i64_const(bit as i64)
            .i64_and()
            .i64_const(0)
            .i64_ne()
            .if_(BlockType::Empty);
        e.warn(reason);
        e.sink().else_();
    }
    e.sink().local_get(overlap).if_(BlockType::Empty);
    e.warn(Reason::StringOverlap);
    e.sink().else_();
    e.warn(Reason::StringRangeNotAllowed);
    e.sink().end().end().end();
    string_element(e, i, kind, source_at, destination_at);
    e.sink().i64_const(1).local_set(elements).end();
}

/// `rep stos` over a checked range: one fill when the value is one byte repeated (every `stosb`, and zero),
/// else the value repeated across a vector stored 16 bytes at a time, and the last elements one at a time.
fn bulk_store<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, at: u32, length: u32) {
    let bytes = element_bytes(i);
    let w = (bytes * u8::BITS) as u8;
    let value = e.local(ValType::I64);
    get_general(e, accumulator(w));
    e.sink().local_set(value);
    let repeated = (u64::MAX / u64::from(u8::MAX)) as i64 & mask(w);
    let fill = |e: &mut E| {
        e.sink()
            .local_get(at)
            .local_get(value)
            .i32_wrap_i64()
            .local_get(length)
            .memory_fill(GUEST_MEMORY);
    };
    if w == W8 {
        fill(e);
        return;
    }
    e.sink()
        .local_get(value)
        .i64_const(u8::MAX.into())
        .i64_and()
        .i64_const(repeated)
        .i64_mul()
        .local_get(value)
        .i64_eq()
        .if_(BlockType::Empty);
    fill(e);
    e.sink().else_();
    let vector = crate::XMM_BYTES;
    let pattern = e.local(ValType::V128);
    let (vectors_end, end) = (e.local(ValType::I64), e.local(ValType::I64));
    {
        let mut s = e.sink();
        s.local_get(value);
        if w == W64 {
            s.i64x2_splat();
        } else {
            s.i32_wrap_i64();
            if w == W32 {
                s.i32x4_splat();
            } else {
                s.i16x8_splat();
            }
        }
        s.local_set(pattern)
            .local_get(at)
            .local_get(length)
            .i64_add()
            .local_tee(end)
            .local_get(length)
            .i64_const(i64::from(vector) - 1)
            .i64_and()
            .i64_sub()
            .local_set(vectors_end);
    }
    // Each loop runs while `at` is below its end, storing `step` bytes a turn.
    let stores = |e: &mut E, limit: u32, step: u32, each: &dyn Fn(&mut E)| {
        e.sink()
            .block(BlockType::Empty)
            .loop_(BlockType::Empty)
            .local_get(at)
            .local_get(limit)
            .i64_ge_u()
            .br_if(1);
        each(e);
        e.sink()
            .local_get(at)
            .i64_const(step.into())
            .i64_add()
            .local_set(at)
            .br(0)
            .end()
            .end();
    };
    stores(e, vectors_end, vector, &|e: &mut E| {
        store_vector_at(e, at, 0, pattern)
    });
    stores(e, end, bytes, &|e: &mut E| store(e, at, bytes, value));
    e.sink().end();
}

/// The bits of a shift's count the CPU uses at width `w`: six at 64 bits, else five.
fn count_mask(w: u8) -> u64 {
    let used = if w == u64::BITS as u8 {
        u64::BITS
    } else {
        u32::BITS
    };
    u64::from(used) - 1
}

/// Shifts and rotates. Their flags stay as they were when the masked count is zero.
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
        .i64_const(count_mask(w) as i64)
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
    if matches!(m, Rcl | Rcr) {
        e.flag(Flag::C);
        e.sink().local_set(old_carry);
    }
    let m_all = mask(w);
    let live = e.flags_live();
    {
        let mut s = e.sink();
        match m {
            Shl | Sal => {
                s.local_get(v).local_get(count).i64_shl().local_set(result);
            }
            Shr | Sar => {
                s.local_get(v);
                if m == Sar && w < W64 {
                    s.i64_const(i64::from(W64) - wide)
                        .i64_shl()
                        .i64_const(i64::from(W64) - wide)
                        .i64_shr_s();
                }
                s.local_tee(rotated).local_get(count);
                if m == Sar {
                    s.i64_shr_s();
                } else {
                    s.i64_shr_u();
                }
                s.local_set(result);
            }
            _ => {}
        }
        match m {
            Shl | Sal if live => {
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
                if w < W64 {
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
            Shl | Sal | Shr | Sar if !live => {}
            Shr | Sar => {
                s.local_get(rotated).local_get(count).i64_const(1).i64_sub();
                if m == Sar {
                    s.i64_shr_s();
                } else {
                    s.i64_shr_u();
                }
                s.i32_wrap_i64().i32_const(1).i32_and().local_set(carry);
                if w < W64 {
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
                if w == W64 {
                    s.local_get(v).local_get(rotated);
                    if m == Rol {
                        s.i64_rotl();
                    } else {
                        s.i64_rotr();
                    }
                } else if w == W32 {
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
                if live {
                    // CF: ROL the result's low bit, ROR its top bit.
                    s.local_get(result);
                    if m == Ror {
                        s.i64_const(wide - 1).i64_shr_u();
                    }
                    s.i32_wrap_i64().i32_const(1).i32_and().local_set(carry);
                }
            }
            _ => {
                // Rcl, Rcr: through CF, the count modulo the width plus one at 8 and 16 bits.
                if w < W32 {
                    s.local_get(count)
                        .i64_const(wide + 1)
                        .i64_rem_u()
                        .local_set(rotated);
                } else {
                    s.local_get(count).local_set(rotated);
                }
                if w < W64 {
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
                    if live {
                        s.local_get(result)
                            .i64_const(wide)
                            .i64_shr_u()
                            .i32_wrap_i64()
                            .local_set(carry);
                    }
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
                        .i64_const(W64.into())
                        .local_get(rotated)
                        .i64_sub()
                        .i64_shr_u()
                        .i64_or()
                        .local_set(result);
                    if live {
                        s.local_get(v)
                            .i64_const(W64.into())
                            .local_get(rotated)
                            .i64_sub()
                            .i64_shr_u()
                            .i32_wrap_i64()
                            .i32_const(1)
                            .i32_and()
                            .local_set(carry);
                    }
                } else {
                    // (v >> c) | (CF << (64 - c)) | (v << (65 - c)), and CF the bit c - 1.
                    s.local_get(v)
                        .local_get(rotated)
                        .i64_shr_u()
                        .local_get(old_carry)
                        .i64_extend_i32_u()
                        .i64_const(W64.into())
                        .local_get(rotated)
                        .i64_sub()
                        .i64_shl()
                        .i64_or()
                        .local_get(v)
                        .i64_const(1)
                        .i64_shl()
                        .i64_const(W64.into())
                        .local_get(rotated)
                        .i64_sub()
                        .i64_shl()
                        .i64_or()
                        .local_set(result);
                    if live {
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
        }
        if w < W64 {
            s.local_get(result)
                .i64_const(m_all)
                .i64_and()
                .local_set(result);
        }
        if live {
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
    if !live {
        return;
    }
    let rotate = matches!(m, Rol | Ror | Rcl | Rcr);
    let immediate = match i.iced.op_kind(1) {
        OpKind::Register => None,
        _ => Some(i.iced.immediate(1) & count_mask(w)),
    };
    // Nothing changes when the masked count is zero, or for rcl and rcr a multiple of the width plus one.
    let (moved, cycle) = if matches!(m, Rcl | Rcr) && w < u32::BITS as u8 {
        (rotated, u64::from(w) + 1)
    } else {
        (count, u64::MAX)
    };
    let stays = immediate.is_none_or(|k| k % cycle == 0);
    // The reference CPU keeps OF for `rol` and `ror` of a register by an immediate above one, and gives a
    // count of one's OF otherwise.
    if matches!(m, Rol | Ror) && mem.is_none() && immediate.is_some_and(|k| k > 1) {
        e.flag(Flag::O);
        e.sink().local_set(overflow);
    }
    if stays {
        kept_when_unmoved(e, moved, carry, overflow);
    }
    let flags = if rotate {
        Flags::Each(each_with(e, &[(Flag::C, carry), (Flag::O, overflow)]))
    } else {
        Flags::Shift {
            width: w,
            result,
            carry,
            overflow,
            unmoved: stays.then(|| unmoved(e, count)),
        }
    };
    e.set_flags(flags);
}

/// CF and OF in their i32 locals become the flags as they are where the i64 in `moved` is zero.
fn kept_when_unmoved<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    moved: u32,
    carry: u32,
    overflow: u32,
) {
    for (flag, new) in [(Flag::C, carry), (Flag::O, overflow)] {
        e.flag(flag);
        e.sink()
            .local_get(new)
            .local_get(moved)
            .i64_eqz()
            .select()
            .local_set(new);
    }
}

/// What a shift by the i64 in `count` keeps for a count of zero: the flags of `KEPT_BY_SHIFT` as they are.
fn unmoved<E: Emitter<X86_64> + ?Sized>(e: &mut E, count: u32) -> Unmoved {
    let kept = KEPT_BY_SHIFT.map(|_| e.local(ValType::I32));
    for (flag, local) in KEPT_BY_SHIFT.into_iter().zip(kept) {
        e.flag(flag);
        e.sink().local_set(local);
    }
    Unmoved { count, kept }
}

/// Push the high 64 bits of the unsigned 128-bit product of the i64 locals `a` and `b`.
fn high_product<E: Emitter<X86_64> + ?Sized>(e: &mut E, a: u32, b: u32, signed: bool) {
    let [a_low, a_high, b_low, b_high, t, middle] = [0; 6].map(|_| e.local(ValType::I64));
    let mut s = e.sink();
    for (from, low, high) in [(a, a_low, a_high), (b, b_low, b_high)] {
        s.local_get(from)
            .i64_const(u32::MAX.into())
            .i64_and()
            .local_set(low)
            .local_get(from)
            .i64_const(W32.into())
            .i64_shr_u()
            .local_set(high);
    }
    s.local_get(a_low).local_get(b_low).i64_mul().local_set(t);
    // a_high * b_low plus the carry out of the low product cannot overflow.
    s.local_get(a_high)
        .local_get(b_low)
        .i64_mul()
        .local_get(t)
        .i64_const(W32.into())
        .i64_shr_u()
        .i64_add()
        .local_set(middle);
    s.local_get(a_low)
        .local_get(b_high)
        .i64_mul()
        .local_get(middle)
        .i64_const(u32::MAX.into())
        .i64_and()
        .i64_add()
        .local_set(t);
    s.local_get(a_high)
        .local_get(b_high)
        .i64_mul()
        .local_get(middle)
        .i64_const(W32.into())
        .i64_shr_u()
        .i64_add()
        .local_get(t)
        .i64_const(W32.into())
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
    let live = e.flags_live();
    if w == W64 {
        e.sink().local_get(a).local_get(b).i64_mul().local_set(low);
        // The high half: a result of the one-operand forms, else only what OF and CF are worked out from.
        if one_operand || live {
            high_product(e, a, b, signed);
            e.sink().local_set(high);
        }
        if live {
            let mut s = e.sink();
            s.local_get(high);
            if signed {
                s.local_get(low).i64_const((W64 - 1).into()).i64_shr_s();
            } else {
                s.i64_const(0);
            }
            s.i64_ne().local_set(overflow);
        }
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
        if one_operand || live {
            e.sink()
                .local_get(product)
                .i64_const(w.into())
                .i64_shr_s()
                .i64_const(mask(w))
                .i64_and()
                .local_set(high);
        }
        // The product does not fit the width: as unsigned, any high bit; as signed, the low half
        // sign-extended differs from it.
        if live {
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
    }
    if one_operand {
        if w == W8 {
            e.sink()
                .local_get(high)
                .i64_const(W8.into())
                .i64_shl()
                .local_get(low)
                .i64_or()
                .local_set(high);
            set_general(e, accumulator(W16), high);
        } else {
            set_general(e, accumulator(w), low);
            set_general(e, register(RDX, w), high);
        }
    } else {
        write(e, i, 0, None, low);
    }
    if live {
        e.set_flags(Flags::Multiply {
            width: w,
            result: low,
            overflow,
        });
    }
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
    if w == W64 {
        long_divide(e, divisor, signed, quotient, remainder);
        set_general(e, accumulator(W64), quotient);
        set_general(e, register(RDX, W64), remainder);
        return;
    }
    // The dividend is twice the width, which fits an i64.
    let dividend = e.local(ValType::I64);
    let (low, high) = if w == W8 {
        (
            accumulator(W8),
            General {
                high: true,
                ..accumulator(W8)
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
            s.local_get(low).i64_const((W64 - 1).into()).i64_shr_s();
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
    const DIGIT: i64 = 1 << W32;
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
        .i64_const(W32.into())
        .i64_shr_u()
        .local_set(v_high);
    s.local_get(v)
        .i64_const(u32::MAX.into())
        .i64_and()
        .local_set(v_low);
    // The dividend shifted left too; the bits of `low` that move up are none when the shift is zero.
    s.local_get(high)
        .local_get(shift)
        .i64_shl()
        .i64_const(0)
        .local_get(low)
        .i64_const(W64.into())
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
        .i64_const(W32.into())
        .i64_shr_u()
        .local_set(low_high);
    s.local_get(below)
        .i64_const(u32::MAX.into())
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
            .i64_const(W32.into())
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
        .i64_const(W32.into())
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
        .i64_const(W32.into())
        .i64_shl()
        .local_get(q0)
        .i64_or()
        .local_set(quotient);
    s.local_get(middle)
        .i64_const(W32.into())
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
                .i64_const(i64::from((w / W8).trailing_zeros()))
                .i64_shl()
                .i64_add()
                .local_set(at);
            if i.address32 {
                e.sink()
                    .local_get(at)
                    .i64_const(u32::MAX.into())
                    .i64_and()
                    .local_set(at);
            }
        }
        if let Some(base) = segment(i) {
            e.get(base);
            e.sink().local_get(at).i64_add().local_set(at);
        }
        e.check(at, u32::from(w / W8), check_kind(kind), 0);
        at
    });
    let lock = locked(e, i) && m != Mnemonic::Bt;
    if lock {
        locked_start(e);
    }
    let value = read(e, i, 0, mem);
    let bit = e.local(ValType::I64);
    e.sink()
        .local_get(offset)
        .i64_const(i64::from(w - 1))
        .i64_and()
        .local_set(bit);
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
        match (lock, mem) {
            (true, Some(at)) => locked_write(e, (at, u32::from(w / W8)), value, result),
            _ => write(e, i, 0, mem, result),
        }
    }
    if e.flags_live() {
        let carry = e.local(ValType::I32);
        e.sink()
            .local_get(value)
            .local_get(bit)
            .i64_shr_u()
            .i32_wrap_i64()
            .i32_const(1)
            .i32_and()
            .local_set(carry);
        let each = each_with(e, &[(Flag::C, carry)]);
        e.set_flags(Flags::Each(each));
    }
}

/// Every other byte of a word: the bytes a byte swap moves by one byte.
const ALTERNATE_BYTES: u64 = u64::MAX / u16::MAX as u64 * u8::MAX as u64;
/// Every other 16 bits of a word.
const ALTERNATE_HALVES: u64 = u64::MAX / u32::MAX as u64 * u16::MAX as u64;

/// Reverse the bytes of the low `width` bits of the i64 in `value`, in place.
fn byte_swap<E: Emitter<X86_64> + ?Sized>(e: &mut E, value: u32, width: u8) {
    let mut s = e.sink();
    s.local_get(value)
        .i64_const(W32.into())
        .i64_rotl()
        .local_set(value);
    for (moved, by) in [(ALTERNATE_HALVES, W16), (ALTERNATE_BYTES, W8)] {
        s.local_get(value)
            .i64_const(moved as i64)
            .i64_and()
            .i64_const(by.into())
            .i64_shl()
            .local_get(value)
            .i64_const(by.into())
            .i64_shr_u()
            .i64_const(moved as i64)
            .i64_and()
            .i64_or()
            .local_set(value);
    }
    if width < W64 {
        s.local_get(value)
            .i64_const((W64 - width).into())
            .i64_shr_u()
            .local_set(value);
    }
}

/// Write the i64 in `value` to a general register where the i32 in `when` is not zero, and leave the whole
/// register as it was where it is zero.
fn set_general_when<E: Emitter<X86_64> + ?Sized>(e: &mut E, g: General, value: u32, when: u32) {
    let whole = General {
        width: if g.width == W32 { W64 } else { g.width },
        ..g
    };
    let chosen = e.local(ValType::I64);
    e.sink().local_get(value);
    if g.width == W32 {
        e.sink().i64_const(mask(W32)).i64_and();
    }
    get_general(e, whole);
    e.sink().local_get(when).select().local_set(chosen);
    set_general(e, whole, chosen);
}

/// The destination register of an instruction, which must be one.
fn destination(i: &Instruction) -> General {
    general(i.iced.op_register(0)).expect("a general register")
}

/// `bswap` and `movbe`: the operand's bytes reversed. `bswap` of 16 bits gives zero on the reference CPU.
fn byte_reverse<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    let w = width(i, 0);
    let mem = checked(e, i);
    let value = if m == Mnemonic::Bswap {
        let value = e.local(ValType::I64);
        if w > W16 {
            get_general(e, destination(i));
        } else {
            e.sink().i64_const(0);
        }
        e.sink().local_set(value);
        value
    } else {
        read(e, i, 1, mem)
    };
    if w > W16 || m != Mnemonic::Bswap {
        byte_swap(e, value, w);
    }
    write(e, i, 0, mem, value);
}

/// `bsf`, `bsr`, `tzcnt`, `lzcnt` and `popcnt`. Where the source is zero, `bsf` and `bsr` leave the whole
/// destination as it was. Their flags are the reference CPU's: `bsf` and `bsr` ZF for a zero source and
/// PF from the index (from zero for a zero source), `tzcnt` and `lzcnt` CF for a zero source and ZF for a
/// zero count, `popcnt` ZF for a zero source; every other flag clear.
fn bit_count<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    use Mnemonic::*;
    let w = width(i, 0);
    let wide = i64::from(w);
    let mem = checked(e, i);
    let source = read(e, i, 1, mem);
    let result = e.local(ValType::I64);
    {
        let mut s = e.sink();
        match m {
            Bsf => s.local_get(source).i64_ctz(),
            Bsr => s
                .i64_const((W64 - 1).into())
                .local_get(source)
                .i64_clz()
                .i64_sub(),
            Tzcnt if w < W64 => s
                .local_get(source)
                .i64_const(1)
                .i64_const(wide)
                .i64_shl()
                .i64_or()
                .i64_ctz(),
            Tzcnt => s.local_get(source).i64_ctz(),
            Lzcnt => s
                .local_get(source)
                .i64_clz()
                .i64_const(i64::from(W64 - w))
                .i64_sub(),
            _ => s.local_get(source).i64_popcnt(),
        };
        s.local_set(result);
    }
    let scan = matches!(m, Bsf | Bsr);
    if scan {
        let found = e.local(ValType::I32);
        e.sink()
            .local_get(source)
            .i64_const(0)
            .i64_ne()
            .local_set(found);
        set_general_when(e, destination(i), result, found);
        // The index PF is worked out from: zero for a zero source.
        e.sink()
            .local_get(result)
            .i64_const(0)
            .local_get(found)
            .select()
            .local_set(result);
    } else {
        write(e, i, 0, None, result);
    }
    if !e.flags_live() {
        return;
    }
    let word = e.local(ValType::I32);
    let at = |s: &mut wasm_encoder::InstructionSink<'_>, flag: Flag| {
        s.i32_const(flag.bit() as i32).i32_shl();
    };
    let mut s = e.sink();
    s.local_get(source).i64_eqz();
    match m {
        Bsf | Bsr => {
            at(&mut s, Flag::Z);
            crate::flags::parity(&mut s, result);
            at(&mut s, Flag::P);
            s.i32_or();
        }
        Tzcnt | Lzcnt => {
            at(&mut s, Flag::C);
            s.local_get(result).i64_eqz();
            at(&mut s, Flag::Z);
            s.i32_or();
        }
        _ => at(&mut s, Flag::Z),
    }
    s.local_set(word);
    e.set_flags(Flags::Packed(word));
}

/// BMI1's `andn`, `bextr`, `blsi`, `blsmsk` and `blsr`. Flags as the reference CPU leaves them: SF from the
/// result (clear for `bextr`), ZF from the result (clear for `blsmsk`, whose result is never zero), CF from
/// whether the source is zero for the three `bls`, and OF, AF and PF clear.
fn bit_manipulation<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    use Mnemonic::*;
    let w = width(i, 0);
    let mem = checked(e, i);
    let result = e.local(ValType::I64);
    let source = match m {
        Andn => {
            let inverted = read(e, i, 1, mem);
            let source = read(e, i, 2, mem);
            e.sink()
                .local_get(inverted)
                .i64_const(mask(w))
                .i64_xor()
                .local_get(source)
                .i64_and()
                .local_set(result);
            source
        }
        // The start is the control's low byte and the length the next; bits from past the operand are zero.
        Bextr => {
            let source = read(e, i, 1, mem);
            let control = read(e, i, 2, None);
            let (start, length) = (e.local(ValType::I64), e.local(ValType::I64));
            let field = i64::from(u8::MAX);
            let wide = i64::from(W64);
            let mut s = e.sink();
            s.local_get(control)
                .i64_const(field)
                .i64_and()
                .local_set(start)
                .local_get(control)
                .i64_const(u8::BITS.into())
                .i64_shr_u()
                .i64_const(field)
                .i64_and()
                .local_set(length);
            s.local_get(source)
                .local_get(start)
                .i64_shr_u()
                .i64_const(0)
                .local_get(start)
                .i64_const(wide)
                .i64_lt_u()
                .select()
                .i64_const(1)
                .local_get(length)
                .i64_shl()
                .i64_const(1)
                .i64_sub()
                .i64_const(-1)
                .local_get(length)
                .i64_const(wide)
                .i64_lt_u()
                .select()
                .i64_and()
                .local_set(result);
            source
        }
        _ => {
            let source = read(e, i, 1, mem);
            let mut s = e.sink();
            match m {
                Blsi => s
                    .i64_const(0)
                    .local_get(source)
                    .i64_sub()
                    .local_get(source)
                    .i64_and(),
                Blsmsk => s
                    .local_get(source)
                    .i64_const(1)
                    .i64_sub()
                    .local_get(source)
                    .i64_xor(),
                _ => s
                    .local_get(source)
                    .i64_const(1)
                    .i64_sub()
                    .local_get(source)
                    .i64_and(),
            };
            s.local_set(result);
            source
        }
    };
    truncate(e, result, w);
    write(e, i, 0, None, result);
    if !e.flags_live() {
        return;
    }
    let word = e.local(ValType::I32);
    let mut s = e.sink();
    let at = |s: &mut wasm_encoder::InstructionSink<'_>, flag: Flag| {
        s.i32_const(flag.bit() as i32).i32_shl();
    };
    match m {
        Blsmsk => {
            s.i32_const(0);
        }
        _ => {
            s.local_get(result).i64_eqz();
            at(&mut s, Flag::Z);
        }
    }
    if m != Bextr {
        s.local_get(result)
            .i64_const(i64::from(w - 1))
            .i64_shr_u()
            .i32_wrap_i64();
        at(&mut s, Flag::S);
        s.i32_or();
    }
    match m {
        Blsi => {
            s.local_get(source).i64_eqz().i32_eqz();
            at(&mut s, Flag::C);
            s.i32_or();
        }
        Blsmsk | Blsr => {
            s.local_get(source).i64_eqz();
            at(&mut s, Flag::C);
            s.i32_or();
        }
        _ => {}
    }
    s.local_set(word);
    e.set_flags(Flags::Packed(word));
}

/// BMI2's `bzhi`: the source with the bits from the index (the third operand's low byte) up cleared, CF set where
/// the index is past the operand (the source then kept), ZF and SF from the result, OF, AF and PF clear.
fn zero_high_bits<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let w = width(i, 0);
    let mem = checked(e, i);
    let source = read(e, i, 1, mem);
    let index = read(e, i, 2, None);
    let (result, past) = (e.local(ValType::I64), e.local(ValType::I32));
    let mut s = e.sink();
    s.local_get(index)
        .i64_const(u8::MAX.into())
        .i64_and()
        .local_tee(index)
        .i64_const(w.into())
        .i64_ge_u()
        .local_set(past);
    s.local_get(source)
        .local_get(source)
        .i64_const(1)
        .local_get(index)
        .i64_shl()
        .i64_const(1)
        .i64_sub()
        .i64_and()
        .local_get(past)
        .select()
        .local_set(result);
    write(e, i, 0, None, result);
    if !e.flags_live() {
        return;
    }
    let word = e.local(ValType::I32);
    let mut s = e.sink();
    s.local_get(result)
        .i64_eqz()
        .i32_const(Flag::Z.bit() as i32)
        .i32_shl()
        .local_get(result)
        .i64_const(i64::from(w - 1))
        .i64_shr_u()
        .i32_wrap_i64()
        .i32_const(1)
        .i32_and()
        .i32_const(Flag::S.bit() as i32)
        .i32_shl()
        .i32_or()
        .local_get(past)
        .i32_const(Flag::C.bit() as i32)
        .i32_shl()
        .i32_or()
        .local_set(word);
    e.set_flags(Flags::Packed(word));
}

/// BMI2's `mulx`: the unsigned product of rdx or edx and the source, its low half to the second operand and
/// then its high half to the first (which wins where they are one register), no flags changed.
fn multiply_flagless<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let w = width(i, 0);
    let mem = checked(e, i);
    let source = read(e, i, 2, mem);
    let multiplier = e.local(ValType::I64);
    get_general(e, register(RDX, w));
    e.sink().local_set(multiplier);
    let (low, high) = (e.local(ValType::I64), e.local(ValType::I64));
    if w == W64 {
        e.sink()
            .local_get(multiplier)
            .local_get(source)
            .i64_mul()
            .local_set(low);
        high_product(e, multiplier, source, false);
        e.sink().local_set(high);
    } else {
        e.sink()
            .local_get(multiplier)
            .local_get(source)
            .i64_mul()
            .local_tee(low)
            .i64_const(w.into())
            .i64_shr_u()
            .local_set(high);
        truncate(e, low, w);
    }
    write(e, i, 1, None, low);
    write(e, i, 0, None, high);
}

/// BMI2's `pdep` and `pext`, one pass for each set bit of the mask (the third operand), counted as software:
/// `pdep` puts the source's low bits, in order, where the mask has its bits; `pext` gathers the source's bits
/// under the mask into the low bits. No flags changed.
fn deposit_or_extract<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    e.warn(Reason::BitDepositOrExtractInSoftware);
    let mem = checked(e, i);
    let source = read(e, i, 1, None);
    let mask_bits = read(e, i, 2, mem);
    let (result, lowest, next) = (
        e.local(ValType::I64),
        e.local(ValType::I64),
        e.local(ValType::I64),
    );
    // `next` is the bit of the packed side the pass is at; `lowest` the mask's lowest bit left.
    let (from, to) = if m == Mnemonic::Pdep {
        (next, lowest)
    } else {
        (lowest, next)
    };
    let mut s = e.sink();
    s.i64_const(0)
        .local_set(result)
        .i64_const(1)
        .local_set(next);
    s.block(BlockType::Empty).loop_(BlockType::Empty);
    s.local_get(mask_bits).i64_eqz().br_if(1);
    s.i64_const(0)
        .local_get(mask_bits)
        .i64_sub()
        .local_get(mask_bits)
        .i64_and()
        .local_set(lowest);
    s.local_get(result)
        .local_get(to)
        .i64_const(0)
        .local_get(source)
        .local_get(from)
        .i64_and()
        .i64_eqz()
        .i32_eqz()
        .select()
        .i64_or()
        .local_set(result);
    s.local_get(mask_bits)
        .local_get(lowest)
        .i64_xor()
        .local_set(mask_bits)
        .local_get(next)
        .i64_const(1)
        .i64_shl()
        .local_set(next)
        .br(0);
    s.end().end();
    write(e, i, 0, None, result);
}

/// BMI2's `rorx`, `sarx`, `shlx` and `shrx`: a rotate by an immediate or a shift by a register, the count
/// masked to the width as the shifts mask it, no flags changed.
fn shift_flagless<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    use Mnemonic::*;
    let w = width(i, 0);
    let mem = checked(e, i);
    let source = read(e, i, 1, mem);
    let count = if m == Rorx {
        let count = e.local(ValType::I64);
        e.sink()
            .i64_const(i.iced.immediate8().into())
            .local_set(count);
        count
    } else {
        read(e, i, 2, None)
    };
    let result = e.local(ValType::I64);
    let mut s = e.sink();
    s.local_get(count)
        .i64_const(i64::from(w - 1))
        .i64_and()
        .local_set(count);
    if w == W64 {
        s.local_get(source).local_get(count);
        match m {
            Rorx => s.i64_rotr(),
            Sarx => s.i64_shr_s(),
            Shlx => s.i64_shl(),
            _ => s.i64_shr_u(),
        };
    } else {
        s.local_get(source)
            .i32_wrap_i64()
            .local_get(count)
            .i32_wrap_i64();
        match m {
            Rorx => s.i32_rotr(),
            Sarx => s.i32_shr_s(),
            Shlx => s.i32_shl(),
            _ => s.i32_shr_u(),
        };
        s.i64_extend_i32_u();
    }
    s.local_set(result);
    write(e, i, 0, None, result);
}

/// `xadd`: the destination gets the sum and the source the destination's old value, flags as `add`.
fn exchange_add<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let w = width(i, 0);
    let mem = checked(e, i);
    let lock = locked(e, i);
    if lock {
        locked_start(e);
    }
    let a = read(e, i, 0, mem);
    let b = read(e, i, 1, None);
    let result = e.local(ValType::I64);
    e.sink()
        .local_get(a)
        .local_get(b)
        .i64_add()
        .local_set(result);
    truncate(e, result, w);
    if lock {
        let at = mem.expect("a memory operand");
        locked_write(e, (at, u32::from(w / W8)), a, result);
        write(e, i, 1, None, a);
    } else {
        write(e, i, 1, None, a);
        write(e, i, 0, mem, result);
    }
    e.set_flags(Flags::Add {
        width: w,
        a,
        b,
        carry: None,
        result,
        kept_carry: None,
    });
}

/// `cmpxchg`: flags as `cmp` of the accumulator with the destination. Where they are equal the destination
/// gets the source, else the accumulator gets the destination; a register not chosen is not written at all,
/// and a memory destination is always written.
fn compare_exchange<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let w = width(i, 0);
    let mem = checked(e, i);
    let lock = locked(e, i);
    if lock {
        locked_start(e);
    }
    let dest = read(e, i, 0, mem);
    let source = read(e, i, 1, None);
    let acc = e.local(ValType::I64);
    get_general(e, accumulator(w));
    e.sink().local_set(acc);
    let (equal, differ) = (e.local(ValType::I32), e.local(ValType::I32));
    e.sink()
        .local_get(acc)
        .local_get(dest)
        .i64_eq()
        .local_tee(equal)
        .i32_eqz()
        .local_set(differ);
    match mem {
        Some(at) => {
            let stored = e.local(ValType::I64);
            e.sink()
                .local_get(source)
                .local_get(dest)
                .local_get(equal)
                .select()
                .local_set(stored);
            if lock {
                locked_write(e, (at, u32::from(w / W8)), dest, stored);
            } else {
                store(e, at, u32::from(w / W8), stored);
            }
        }
        None => set_general_when(e, destination(i), source, equal),
    }
    set_general_when(e, accumulator(w), dest, differ);
    let result = e.local(ValType::I64);
    e.sink()
        .local_get(acc)
        .local_get(dest)
        .i64_sub()
        .local_set(result);
    truncate(e, result, w);
    e.set_flags(Flags::Sub {
        width: w,
        a: acc,
        b: dest,
        borrow: None,
        result,
        kept_carry: None,
    });
}

/// `cmpxchg8b` and `cmpxchg16b`: rdx:rax (each half 32 bits for 8 bytes) against memory; equal, memory gets
/// rcx:rbx, else rdx:rax gets memory, which is written either way. ZF is whether they were equal; no other
/// flag changes. `cmpxchg16b` needs its operand aligned to 16 bytes.
fn compare_exchange_pair<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let bytes = memory_bytes(i);
    let half = bytes / 2;
    let w = (half * u8::BITS) as u8;
    let at = address(e, i);
    if bytes == u128::BITS / u8::BITS {
        crate::simd::aligned16(e, at);
    }
    e.check(at, bytes, Access::Write, 0);
    let high_at = e.local(ValType::I64);
    e.sink()
        .local_get(at)
        .i64_const(half.into())
        .i64_add()
        .local_set(high_at);
    let [low, high, rax, rdx, rbx, rcx] = core::array::from_fn(|_| e.local(ValType::I64));
    // Locked in an ordered form, `cmpxchg8b` is one 8-byte compare and exchange; `cmpxchg16b`, which no
    // WebAssembly atomic makes, holds the process's other threads first.
    let lock = locked(e, i);
    let whole = (lock && bytes == u64::BITS / u8::BITS).then(|| {
        let whole = e.local(ValType::I64);
        locked_start(e);
        load(e, at, bytes);
        e.sink()
            .local_tee(whole)
            .i64_const(mask(W32))
            .i64_and()
            .local_set(low)
            .local_get(whole)
            .i64_const(u32::BITS.into())
            .i64_shr_u()
            .local_set(high);
        whole
    });
    if whole.is_none() {
        if lock {
            e.warn(Reason::LockedAccessNeedsGroupStop);
        }
        for (local, from) in [(low, at), (high, high_at)] {
            load(e, from, half);
            e.sink().local_set(local);
        }
    }
    for (local, number) in [(rax, RAX), (rdx, RDX), (rbx, RBX), (rcx, RCX)] {
        get_general(e, register(number, w));
        e.sink().local_set(local);
    }
    let (equal, differ) = (e.local(ValType::I32), e.local(ValType::I32));
    e.sink()
        .local_get(low)
        .local_get(rax)
        .i64_eq()
        .local_get(high)
        .local_get(rdx)
        .i64_eq()
        .i32_and()
        .local_tee(equal)
        .i32_eqz()
        .local_set(differ);
    match whole {
        Some(whole) => {
            let stored = e.local(ValType::I64);
            e.sink()
                .local_get(rcx)
                .i64_const(u32::BITS.into())
                .i64_shl()
                .local_get(rbx)
                .i64_or()
                .local_get(whole)
                .local_get(equal)
                .select()
                .local_set(stored);
            locked_write(e, (at, bytes), whole, stored);
        }
        None => {
            for (memory, new, from) in [(low, rbx, at), (high, rcx, high_at)] {
                let stored = e.local(ValType::I64);
                e.sink()
                    .local_get(new)
                    .local_get(memory)
                    .local_get(equal)
                    .select()
                    .local_set(stored);
                store(e, from, half, stored);
            }
        }
    }
    set_general_when(e, register(RAX, w), low, differ);
    set_general_when(e, register(RDX, w), high, differ);
    if e.flags_live() {
        let each = each_with(e, &[(Flag::Z, equal)]);
        e.set_flags(Flags::Each(each));
    }
}

/// `shld` and `shrd`: the destination shifted with the source's bits shifted in, by the count masked as for
/// the shifts. At 16 bits a count above 16 shifts the destination in again after the source, as the
/// reference CPU does. Flags as the shifts: CF the last bit shifted out, OF what a count of one gives, AF
/// clear, all six as they were for a count of zero.
fn double_shift<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    let left = m == Mnemonic::Shld;
    let w = width(i, 0);
    let wide = i64::from(w);
    let mem = checked(e, i);
    let d = read(e, i, 0, mem);
    let s = read(e, i, 1, None);
    let count = e.local(ValType::I64);
    let immediate = match i.iced.op_kind(2) {
        OpKind::Register => {
            e.get(RCX);
            None
        }
        _ => {
            e.sink().i64_const(i.iced.immediate(2) as i64);
            Some(i.iced.immediate(2) & count_mask(w))
        }
    };
    e.sink()
        .i64_const(count_mask(w) as i64)
        .i64_and()
        .local_set(count);
    let (result, carry, overflow) = (
        e.local(ValType::I64),
        e.local(ValType::I32),
        e.local(ValType::I32),
    );
    let live = e.flags_live();
    let bit = |s: &mut wasm_encoder::InstructionSink<'_>| {
        s.i32_wrap_i64().i32_const(1).i32_and();
    };
    if w == W64 {
        let mut x = e.sink();
        // The source by 64 less the count, which a zero count would make a shift by 64: the result is then
        // the destination's, chosen after.
        x.local_get(d).local_get(count);
        if left {
            x.i64_shl();
        } else {
            x.i64_shr_u();
        }
        x.local_get(s).i64_const(wide).local_get(count).i64_sub();
        if left {
            x.i64_shr_u();
        } else {
            x.i64_shl();
        }
        // The masked count fits an i32: not zero, the shifted bits.
        x.i64_or()
            .local_get(d)
            .local_get(count)
            .i32_wrap_i64()
            .select()
            .local_set(result);
        if live {
            x.local_get(d);
            if left {
                x.i64_const(wide).local_get(count).i64_sub().i64_shr_u();
            } else {
                x.local_get(count).i64_const(1).i64_sub().i64_shr_u();
            }
            bit(&mut x);
            x.local_set(carry);
        }
    } else {
        // Both operands joined into one word: the destination above the source at 32 bits; at 16 bits the
        // destination, the source and the destination again, from the top.
        let joined = e.local(ValType::I64);
        let bits = i64::from(if w == W32 { W64 } else { W64 - W16 });
        let mut x = e.sink();
        if w == W32 {
            let (high, low) = if left { (d, s) } else { (s, d) };
            x.local_get(high)
                .i64_const(wide)
                .i64_shl()
                .local_get(low)
                .i64_or();
        } else {
            x.local_get(d)
                .i64_const(2 * wide)
                .i64_shl()
                .local_get(s)
                .i64_const(wide)
                .i64_shl()
                .i64_or()
                .local_get(d)
                .i64_or();
        }
        x.local_set(joined).local_get(joined);
        if left {
            x.i64_const(bits - wide).local_get(count).i64_sub();
        } else {
            x.local_get(count);
        }
        x.i64_shr_u().i64_const(mask(w)).i64_and().local_set(result);
        if live {
            x.local_get(joined);
            if left {
                x.i64_const(bits).local_get(count).i64_sub();
            } else {
                x.local_get(count).i64_const(1).i64_sub();
            }
            x.i64_shr_u();
            bit(&mut x);
            x.local_set(carry);
        }
    }
    write(e, i, 0, mem, result);
    if !live {
        return;
    }
    {
        // OF as a count of one gives it: the top bit against the next bit in, the destination's next or the
        // source's lowest.
        let mut x = e.sink();
        x.local_get(d).i64_const(wide - 1).i64_shr_u();
        if left {
            x.local_get(d).i64_const(wide - 2).i64_shr_u();
        } else {
            x.local_get(s);
        }
        x.i64_xor();
        bit(&mut x);
        x.local_set(overflow);
    }
    let stays = immediate.is_none_or(|k| k == 0);
    if stays {
        kept_when_unmoved(e, count, carry, overflow);
    }
    let unmoved = stays.then(|| unmoved(e, count));
    e.set_flags(Flags::Shift {
        width: w,
        result,
        carry,
        overflow,
        unmoved,
    });
}

/// `adcx` and `adox`: an unsigned add with CF (or OF) in and out, no other flag touched.
fn add_with<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, flag: Flag) {
    let w = width(i, 0);
    let mem = checked(e, i);
    let a = read(e, i, 0, mem);
    let b = read(e, i, 1, mem);
    let (carry_in, carry, result) = (
        e.local(ValType::I32),
        e.local(ValType::I32),
        e.local(ValType::I64),
    );
    e.flag(flag);
    let mut s = e.sink();
    s.local_set(carry_in)
        .local_get(a)
        .local_get(b)
        .i64_add()
        .local_get(carry_in)
        .i64_extend_i32_u()
        .i64_add()
        .local_set(result);
    if w == W64 {
        // Out where the sum is below an operand, or equal to it with a carry in.
        s.local_get(result)
            .local_get(a)
            .i64_lt_u()
            .local_get(result)
            .local_get(a)
            .i64_eq()
            .local_get(carry_in)
            .i32_and()
            .i32_or()
            .local_set(carry);
    } else {
        s.local_get(result)
            .i64_const(w.into())
            .i64_shr_u()
            .i32_wrap_i64()
            .local_set(carry);
    }
    truncate(e, result, w);
    write(e, i, 0, None, result);
    if e.flags_live() {
        let each = each_with(e, &[(flag, carry)]);
        e.set_flags(Flags::Each(each));
    }
}

/// `loop`, `loope` and `loopne`: rcx (ecx with an address-size prefix) less one, and a jump while it is not
/// zero (and ZF is set, or clear). No flag changes.
fn loop_<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    let g = register(RCX, if i.address32 { W32 } else { W64 });
    let count = e.local(ValType::I64);
    get_general(e, g);
    e.sink().i64_const(1).i64_sub().local_set(count);
    set_general(e, g, count);
    e.sink().local_get(count);
    if g.width < W64 {
        e.sink().i64_const(mask(g.width)).i64_and();
    }
    e.sink().i64_const(0).i64_ne();
    match m {
        Mnemonic::Loope => {
            e.flag(Flag::Z);
            e.sink().i32_and();
        }
        Mnemonic::Loopne => {
            e.flag(Flag::Z);
            e.sink().i32_eqz().i32_and();
        }
        _ => {}
    }
    e.jump_if(Target::Relative(i.iced.near_branch64() as i64));
}

/// The flags of `lahf` and `sahf`: the low byte of rflags but bit 1, which is always set.
const LOW_FLAGS: u64 = ARITHMETIC_FLAGS & u8::MAX as u64;

/// `lahf`: ah gets SF, ZF, AF, PF and CF at their rflags places, with bit 1 set.
fn load_flags<E: Emitter<X86_64> + ?Sized>(e: &mut E) {
    let word = word_with(e, &[]);
    let value = e.local(ValType::I64);
    e.sink()
        .local_get(word)
        .i64_extend_i32_u()
        .i64_const(LOW_FLAGS as i64)
        .i64_and()
        .i64_const((ALWAYS & u64::from(u8::MAX)) as i64)
        .i64_or()
        .local_set(value);
    set_general(
        e,
        General {
            number: RAX,
            width: W8,
            high: true,
        },
        value,
    );
}

/// CRC-32C's polynomial, bit-reflected (Intel SDM Vol. 2A, CRC32).
const CRC32C_POLYNOMIAL: i32 = 0x82f6_3b78_u32 as i32;

/// `crc32`: the CRC-32C of the source's bytes, low first, continued from the destination's low 32 bits, a bit
/// at a time; the destination's upper half cleared.
fn crc32<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let mem = checked(e, i);
    let value = read(e, i, 1, mem);
    let bits = u32::from(width(i, 1));
    let destination = general(i.iced.op_register(0)).expect("a general register");
    let crc = e.local(ValType::I32);
    get_general(e, register(destination.number, W32));
    e.sink().i32_wrap_i64().local_set(crc);
    for chunk in 0..bits.div_ceil(u32::BITS) {
        let mut s = e.sink();
        s.local_get(crc)
            .local_get(value)
            .i64_const((chunk * u32::BITS).into())
            .i64_shr_u()
            .i32_wrap_i64()
            .i32_xor()
            .local_set(crc);
        for _ in 0..bits.min(u32::BITS) {
            s.local_get(crc)
                .i32_const(1)
                .i32_shr_u()
                .i32_const(CRC32C_POLYNOMIAL)
                .i32_const(0)
                .local_get(crc)
                .i32_const(1)
                .i32_and()
                .i32_sub()
                .i32_and()
                .i32_xor()
                .local_set(crc);
        }
    }
    let result = e.local(ValType::I64);
    e.sink().local_get(crc).i64_extend_i32_u().local_set(result);
    set_general(e, register(destination.number, W64), result);
}

/// `xgetbv`: for ecx 0, XCR0 in edx:eax, the state components the kernel enables for what the program is told,
/// upper halves cleared; any other ecx names no register the CPU presented has (XGETBV1 is not offered),
/// general protection.
fn extended_control<E: Emitter<X86_64> + ?Sized>(e: &mut E) {
    let state = crate::cpuid::enabled_state(e.offer());
    get_general(e, register(RCX, W32));
    e.sink().i64_eqz().i32_eqz().if_(BlockType::Empty);
    e.trap(Trap::GeneralProtection);
    e.sink().end();
    let (low, high) = (e.local(ValType::I64), e.local(ValType::I64));
    e.sink()
        .i64_const(state.into())
        .local_set(low)
        .i64_const(0)
        .local_set(high);
    set_general(e, register(RAX, W64), low);
    set_general(e, register(RDX, W64), high);
}

/// `cpuid`: eax, ebx, ecx and edx from the table of [`crate::cpuid`] by the leaf in eax and the sub-leaf in
/// ecx, their upper halves cleared.
fn cpuid<E: Emitter<X86_64> + ?Sized>(e: &mut E) {
    use crate::cpuid::{EXTENDED, MAX_BASIC, MAX_EXTENDED};
    let offer = e.offer();
    let (leaf, sub_leaf) = (e.local(ValType::I32), e.local(ValType::I32));
    get_general(e, register(RCX, W32));
    e.sink().i32_wrap_i64().local_set(sub_leaf);
    get_general(e, register(RAX, W32));
    // Above the highest basic or extended leaf, the highest basic one.
    e.sink().i32_wrap_i64().local_set(leaf);
    e.sink()
        .i32_const(MAX_BASIC as i32)
        .local_get(leaf)
        .local_get(leaf)
        .i32_const(MAX_BASIC as i32)
        .i32_gt_u()
        .local_get(leaf)
        .i32_const(EXTENDED as i32)
        .i32_lt_u()
        .i32_and()
        .local_get(leaf)
        .i32_const(MAX_EXTENDED as i32)
        .i32_gt_u()
        .i32_or()
        .select()
        .local_set(leaf);
    if !offer.avx() {
        e.sink()
            .local_get(leaf)
            .i32_const(crate::cpuid::FEATURES_LEAF as i32)
            .i32_eq()
            .local_get(leaf)
            .i32_const(crate::cpuid::EXTENDED_FEATURES_LEAF as i32)
            .i32_eq()
            .local_get(sub_leaf)
            .i32_eqz()
            .i32_and()
            .i32_or()
            .if_(BlockType::Empty);
        e.warn(Reason::AvxAskedAndToldNo);
        e.sink().end();
    }
    let out = [(); crate::cpuid::OUTPUTS].map(|_| e.local(ValType::I32));
    let set = |e: &mut E, answer: crate::cpuid::Registers| {
        let mut s = e.sink();
        for (local, value) in out.iter().zip(answer) {
            s.i32_const(value as i32).local_set(*local);
        }
    };
    for l in crate::cpuid::leaves() {
        e.sink()
            .local_get(leaf)
            .i32_const(l as i32)
            .i32_eq()
            .if_(BlockType::Empty);
        let subs = crate::cpuid::sub_leaves(l);
        set(e, crate::cpuid::answer(offer, l, subs));
        if crate::cpuid::echoes_sub_leaf(l) {
            e.sink()
                .local_get(sub_leaf)
                .i32_const(u8::MAX.into())
                .i32_and()
                .local_set(out[crate::cpuid::ECX]);
        }
        for sub in 0..subs {
            e.sink()
                .local_get(sub_leaf)
                .i32_const(sub as i32)
                .i32_eq()
                .if_(BlockType::Empty);
            set(e, crate::cpuid::answer(offer, l, sub));
            e.sink().end();
        }
        e.sink().end();
    }
    for (number, local) in [RAX, RBX, RCX, RDX].into_iter().zip(out) {
        let value = e.local(ValType::I64);
        e.sink()
            .local_get(local)
            .i64_extend_i32_u()
            .local_set(value);
        set_general(e, register(number, W64), value);
    }
}

/// `rdtsc`: edx:eax the clock hook's nanoseconds, a 1 GHz counter; `rdtscp` also ecx 0, the processor
/// number, which a guest thread does not know.
fn time_stamp<E: Emitter<X86_64> + ?Sized>(e: &mut E, processor: bool) {
    let (time, low, high) = (
        e.local(ValType::I64),
        e.local(ValType::I64),
        e.local(ValType::I64),
    );
    e.hook(Hook::Clock);
    e.sink()
        .local_tee(time)
        .i64_const(mask(W32))
        .i64_and()
        .local_set(low)
        .local_get(time)
        .i64_const(W32.into())
        .i64_shr_u()
        .local_set(high);
    set_general(e, register(RAX, W64), low);
    set_general(e, register(RDX, W64), high);
    if processor {
        let zero = e.local(ValType::I64);
        e.sink().i64_const(0).local_set(zero);
        set_general(e, register(RCX, W64), zero);
    }
}

use crate::answer::POPPED_CONTROL;

/// `popf`: the six flags and TF, DF, NT and ID from the stack, of them only the low 16 bits with 66.
fn pop_flags<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let bytes = stack_bytes(i);
    let from = e.local(ValType::I64);
    e.get(RSP);
    e.sink().local_set(from);
    e.check(from, bytes, Access::Read, 0);
    let value = e.local(ValType::I64);
    load(e, from, bytes);
    e.sink().local_set(value);
    e.sink().local_get(from).i64_const(bytes.into()).i64_add();
    e.set(RSP);
    load_rflags(e, value, bytes);
}

/// Load the flags user mode may write from the i64 in `value`, of `bytes` bytes, as `popf` and `iret` do at
/// CPL 3.
pub(crate) fn load_rflags<E: Emitter<X86_64> + ?Sized>(e: &mut E, value: u32, bytes: u32) {
    let written = (POPPED_CONTROL as i64) & mask((bytes * u8::BITS) as u8);
    e.get_control(Control::Rflags);
    e.sink()
        .i64_const(!written)
        .i64_and()
        .local_get(value)
        .i64_const(written)
        .i64_and()
        .i64_or();
    e.set_control(Control::Rflags);
    for (bit, reason) in [
        (TF, Reason::TrapFlagSet),
        (ALIGNMENT_CHECK, Reason::AlignmentCheckNotProvided),
    ] {
        e.sink()
            .local_get(value)
            .i64_const(bit as i64)
            .i64_and()
            .i64_const(0)
            .i64_ne()
            .if_(BlockType::Empty);
        e.warn(reason);
        // Single stepping starts at the next stop check (`Guest::step`).
        if bit == TF {
            e.sink().i32_const(1);
            e.raise_stop();
        }
        e.sink().end();
    }
    let word = e.local(ValType::I32);
    e.sink()
        .local_get(value)
        .i32_wrap_i64()
        .i32_const(ARITHMETIC_FLAGS as i32)
        .i32_and()
        .local_set(word);
    e.set_flags(Flags::Packed(word));
}

/// `sahf`: SF, ZF, AF, PF and CF from ah; OF as it was.
fn store_flags<E: Emitter<X86_64> + ?Sized>(e: &mut E) {
    if !e.flags_live() {
        return;
    }
    let word = e.local(ValType::I32);
    e.get(RAX);
    e.sink()
        .i64_const(W8.into())
        .i64_shr_u()
        .i32_wrap_i64()
        .i32_const(LOW_FLAGS as i32)
        .i32_and();
    e.flag(Flag::O);
    e.sink()
        .i32_const(Flag::O.bit() as i32)
        .i32_shl()
        .i32_or()
        .local_set(word);
    e.set_flags(Flags::Packed(word));
}
