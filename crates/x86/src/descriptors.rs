//! The descriptor-table instructions user mode may run (`lar`, `lsl`, `verr`, `verw`, `sldt`, `str`, `smsw`,
//! `sgdt`, `sidt`), answered from Linux's x86-64 tables as user mode sees them on the reference
//! (`crate::user_segments`).

use crate::X86_64;
use crate::decode::Instruction;
use crate::flags::{Flag, Flags};
use crate::meaning::{checked, each_with, read, write};
use crate::operand::*;
use crate::user_segments::{
    CR0, DUMMY_GDT_BASE, DUMMY_IDT_BASE, DUMMY_TABLE_LIMIT, NO_LOCAL_TABLE, SEGMENTS,
    SELECTOR_PRIVILEGE, TASK_STATE,
};
use iced_x86::Mnemonic;
use wasm_encoder::{BlockType, ValType};
use wcontainers_guest_cpu::Emitter;

pub fn written(m: Mnemonic) -> bool {
    use Mnemonic::*;
    matches!(m, Lar | Lsl | Verr | Verw | Sldt | Str | Smsw | Sgdt | Sidt)
}

/// Whether a real CPU under another kernel may answer otherwise: the per-CPU segment's limit, the tables'
/// addresses, and what a kernel that emulates these for user mode stores.
pub fn may_vary(m: Mnemonic) -> bool {
    use Mnemonic::*;
    matches!(m, Lsl | Sldt | Str | Smsw | Sgdt | Sidt)
}

pub fn emit<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    use Mnemonic::*;
    let mem = checked(e, i);
    match m {
        Sldt | Str | Smsw => {
            let value = e.local(ValType::I64);
            let stored = match m {
                Sldt => NO_LOCAL_TABLE.into(),
                Str => TASK_STATE.into(),
                _ => CR0,
            };
            e.sink().i64_const(stored.into()).local_set(value);
            write(e, i, 0, mem, value);
        }
        Sgdt | Sidt => {
            let at = mem.expect("a memory operand");
            let (limit, base) = (e.local(ValType::I64), e.local(ValType::I64));
            let table = if m == Sgdt {
                DUMMY_GDT_BASE
            } else {
                DUMMY_IDT_BASE
            };
            e.sink()
                .i64_const(DUMMY_TABLE_LIMIT.into())
                .local_set(limit)
                .i64_const(table as i64)
                .local_set(base);
            let limit_bytes = size_of_val(&DUMMY_TABLE_LIMIT) as u32;
            store(e, at, limit_bytes, limit);
            let base_at = e.local(ValType::I64);
            e.sink()
                .local_get(at)
                .i64_const(limit_bytes.into())
                .i64_add()
                .local_set(base_at);
            store(e, base_at, SLOT, base);
        }
        _ => {
            // The selector: the source's low 16 bits, the rest masked off below.
            let n = u32::from(matches!(m, Lar | Lsl));
            let selector = read(e, i, n, mem);
            let (found, value) = (e.local(ValType::I32), e.local(ValType::I64));
            let mut s = e.sink();
            s.i32_const(0).local_set(found);
            for segment in &SEGMENTS {
                if m == Verw && !segment.writable {
                    continue;
                }
                s.local_get(selector)
                    .i64_const((!SELECTOR_PRIVILEGE).into())
                    .i64_and()
                    .i64_const((segment.selector & !SELECTOR_PRIVILEGE).into())
                    .i64_eq()
                    .if_(BlockType::Empty)
                    .i32_const(1)
                    .local_set(found)
                    .i64_const(
                        if m == Lar {
                            segment.rights
                        } else {
                            segment.limit
                        }
                        .into(),
                    )
                    .local_set(value)
                    .end();
            }
            // The destination changes only when the selector names a segment.
            if matches!(m, Lar | Lsl) {
                e.sink().local_get(found).if_(BlockType::Empty);
                write(e, i, 0, None, value);
                e.sink().end();
            }
            let each = each_with(e, &[(Flag::Z, found)]);
            e.set_flags(Flags::Each(each));
        }
    }
}
