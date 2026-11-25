//! Linux's x86-64 descriptor tables as user mode sees them on the reference, which `lar`, `lsl`, `verr`, `verw`,
//! `sldt`, `str`, `smsw`, `sgdt` and `sidt` answer from.

/// A selector's requested privilege level, which no user segment depends on. Its bit choosing the local table
/// is compared, so a local selector names nothing.
pub const SELECTOR_PRIVILEGE: u16 = 0b11;
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
pub(crate) const NO_LOCAL_TABLE: u16 = 0;
/// CR0 as Linux runs it: PE, MP, ET, NE, WP, AM and PG.
pub(crate) const CR0: u32 = 0x8005_0033;
/// What `sgdt` and `sidt` store, by design: the answers Linux gives where it emulates them for user mode
/// (`UMIP_DUMMY_GDT_BASE`, `UMIP_DUMMY_IDT_BASE` and a zero limit in arch/x86/kernel/umip.c).
pub(crate) const DUMMY_GDT_BASE: u64 = 0xffff_ffff_fffe_0000;
pub(crate) const DUMMY_IDT_BASE: u64 = 0xffff_ffff_ffff_0000;
pub(crate) const DUMMY_TABLE_LIMIT: u16 = 0;
/// The limit of a segment that spans the address space, in bytes.
const WHOLE_LIMIT: u32 = u32::MAX;
/// The per-CPU segment's limit by design: processor 0 on node 0, since a guest thread does not know where the
/// host runs it.
const CPU_NUMBER_LIMIT: u32 = 0;

/// A segment user mode can name, with the access rights `lar` gives and the limit `lsl` gives.
pub(crate) struct Segment {
    pub(crate) selector: u16,
    pub(crate) rights: u32,
    pub(crate) limit: u32,
    pub(crate) writable: bool,
}

/// Every one is readable; only the data segment is writable.
pub(crate) const SEGMENTS: [Segment; 4] = [
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
