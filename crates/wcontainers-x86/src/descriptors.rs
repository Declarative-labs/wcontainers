//! The descriptor-table instructions user mode may run (`lar`, `lsl`, `verr`, `verw`, `sldt`, `str`, `smsw`,
//! `sgdt`, `sidt`), answered from Linux's x86-64 tables as user mode sees them on the reference.

use crate::X86_64;
use crate::decode::Instruction;
use crate::flags::{Flag, Flags};
use crate::meaning::{checked, each_with, read, write};
use crate::operand::*;
use iced_x86::Mnemonic;
use wasm_encoder::{BlockType, ValType};
use wcontainers_guest_cpu::Emitter;

/// A selector's requested privilege level, which no user segment depends on. Its bit choosing the local table
/// is compared, so a local selector names nothing.
const SELECTOR_PRIVILEGE: u16 = 0b11;
/// A selector's lowest index bit: the bits below it are the privilege level and the local table's bit.
pub const SELECTOR_INDEX: u16 = 1 << 3;
/// The kernel's 64-bit code segment, which user mode may not name.
pub const KERNEL_CS: u16 = 0x10;

/// Linux's user segments (arch/x86/include/asm/segment.h): 32-bit code, data, 64-bit code, and the per-CPU
/// segment whose limit holds the processor and node numbers.
pub const USER32_CS: u16 = 0x23;
pub const USER_DS: u16 = 0x2b;
pub const USER_CS: u16 = 0x33;
pub const CPU_NUMBER: u16 = 0x7b;
/// The task register's selector (GDT entry 8); Linux gives a process no local table.
pub const TASK_STATE: u16 = 0x40;
const NO_LOCAL_TABLE: u16 = 0;
/// CR0 as Linux runs it: PE, MP, ET, NE, WP, AM and PG.
const CR0: u32 = 0x8005_0033;
/// What `sgdt` and `sidt` store, by design: the answers Linux gives where it emulates them for user mode
/// (`UMIP_DUMMY_GDT_BASE`, `UMIP_DUMMY_IDT_BASE` and a zero limit in arch/x86/kernel/umip.c).
const DUMMY_GDT_BASE: u64 = 0xffff_ffff_fffe_0000;
const DUMMY_IDT_BASE: u64 = 0xffff_ffff_ffff_0000;
const DUMMY_TABLE_LIMIT: u16 = 0;
/// The limit of a segment that spans the address space, in bytes.
const WHOLE_LIMIT: u32 = u32::MAX;
/// The per-CPU segment's limit by design: processor 0 on node 0, since a guest thread does not know where the
/// host runs it.
const CPU_NUMBER_LIMIT: u32 = 0;

/// A segment user mode can name, with the access rights `lar` gives and the limit `lsl` gives.
struct Segment {
    selector: u16,
    rights: u32,
    limit: u32,
    writable: bool,
}

/// Every one is readable; only the data segment is writable.
const SEGMENTS: [Segment; 4] = [
    Segment {
        selector: USER32_CS,
        rights: 0x00cf_fb00,
        limit: WHOLE_LIMIT,
        writable: false,
    },
    Segment {
        selector: USER_DS,
        rights: 0x00cf_f300,
        limit: WHOLE_LIMIT,
        writable: true,
    },
    Segment {
        selector: USER_CS,
        rights: 0x00af_fb00,
        limit: WHOLE_LIMIT,
        writable: false,
    },
    Segment {
        selector: CPU_NUMBER,
        rights: 0x0040_f500,
        limit: CPU_NUMBER_LIMIT,
        writable: false,
    },
];

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
