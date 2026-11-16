//! The small vDSO every program gets: a shared object with one function, `__kernel_rt_sigreturn`,
//! through which a handler without its own restorer returns. It is laid out with `object`'s ELF
//! structures, as Linux's arm64 vDSO names its symbol (version `LINUX_2.6.39`, soname
//! `linux-vdso.so.1`), and mapped below the data pages Linux puts before it (`[vvar]`).

use alloc::vec::Vec;

use object::LittleEndian as LE;
use object::elf;
use object::endian::{U16, U32, U64};
use object::pod::bytes_of;

/// Linux maps 4 pages of data (`[vvar]`) and then 2 pages of code (`[vdso]`) on arm64 (measured).
pub(crate) const VVAR_PAGES: u64 = 4;
pub(crate) const CODE_PAGES: u64 = 2;

const SONAME: &[u8] = b"linux-vdso.so.1";
const SYMBOL: &[u8] = b"__kernel_rt_sigreturn";
const VERSION: &[u8] = b"LINUX_2.6.39";

/// The code: a `nop` before the function, as Linux's has for unwinders, then `mov x8, #139` and
/// `svc #0`.
const CODE: [u32; 3] = [0xd503_201f, 0xd280_1168, 0xd400_0001];

/// Where things sit in the image.
const PROGRAM_HEADERS: usize = 64;
const HASH: usize = PROGRAM_HEADERS + 2 * 56;
const DYNSYM: usize = 200;
const DYNSTR: usize = DYNSYM + 2 * 24;
const VERSYM: usize = 300;
const VERDEF: usize = 304;
const TEXT: usize = 368;
const DYNAMIC: usize = 384;
const DYNAMIC_ENTRIES: usize = 10;
const SIZE: usize = DYNAMIC + DYNAMIC_ENTRIES * 16;

/// The offset of `__kernel_rt_sigreturn` from where the code pages start.
pub(crate) const SIGTRAMP: u64 = TEXT as u64 + 4;

/// The vDSO's bytes.
pub(crate) fn image() -> Vec<u8> {
    let mut out = alloc::vec![0u8; SIZE];
    let mut put = |at: usize, bytes: &[u8]| out[at..at + bytes.len()].copy_from_slice(bytes);
    let header = elf::FileHeader64::<LE> {
        e_ident: elf::Ident {
            magic: elf::ELFMAG,
            class: elf::ELFCLASS64,
            data: elf::ELFDATA2LSB,
            version: elf::EV_CURRENT,
            os_abi: elf::ELFOSABI_NONE,
            abi_version: 0,
            padding: [0; 7],
        },
        e_type: U16::new(LE, elf::ET_DYN),
        e_machine: U16::new(LE, elf::EM_AARCH64),
        e_version: U32::new(LE, 1),
        e_entry: U64::new(LE, 0),
        e_phoff: U64::new(LE, PROGRAM_HEADERS as u64),
        e_shoff: U64::new(LE, 0),
        e_flags: U32::new(LE, Default::default()),
        e_ehsize: U16::new(LE, 64),
        e_phentsize: U16::new(LE, 56),
        e_phnum: U16::new(LE, 2),
        e_shentsize: U16::new(LE, 64),
        e_shnum: U16::new(LE, 0),
        e_shstrndx: U16::new(LE, elf::SHN_UNDEF),
    };
    put(0, bytes_of(&header));
    let segment = |p_type, p_flags, at: usize, size: usize, align: u64| elf::ProgramHeader64::<LE> {
        p_type: U32::new(LE, p_type),
        p_flags: U32::new(LE, p_flags),
        p_offset: U64::new(LE, at as u64),
        p_vaddr: U64::new(LE, at as u64),
        p_paddr: U64::new(LE, at as u64),
        p_filesz: U64::new(LE, size as u64),
        p_memsz: U64::new(LE, size as u64),
        p_align: U64::new(LE, align),
    };
    put(
        PROGRAM_HEADERS,
        bytes_of(&segment(elf::PT_LOAD, elf::PF_R | elf::PF_X, 0, SIZE, 16)),
    );
    put(
        PROGRAM_HEADERS + 56,
        bytes_of(&segment(
            elf::PT_DYNAMIC,
            elf::PF_R,
            DYNAMIC,
            DYNAMIC_ENTRIES * 16,
            8,
        )),
    );
    // The strings: the soname, the function, the version.
    let soname_at = 1;
    let symbol_at = soname_at + SONAME.len() + 1;
    let version_at = symbol_at + SYMBOL.len() + 1;
    let strings_size = version_at + VERSION.len() + 1;
    put(DYNSTR + soname_at, SONAME);
    put(DYNSTR + symbol_at, SYMBOL);
    put(DYNSTR + version_at, VERSION);
    // A hash table of one bucket over the null symbol and the function.
    for (index, word) in [1u32, 2, 1, 0, 0].iter().enumerate() {
        put(HASH + 4 * index, &word.to_le_bytes());
    }
    let symbol = elf::Sym64::<LE> {
        st_name: U32::new(LE, symbol_at as u32),
        st_info: elf::SymbolInfo::new(elf::STB_GLOBAL, elf::STT_FUNC),
        st_other: elf::STV_DEFAULT.into(),
        st_shndx: U16::new(LE, elf::SHN_ABS),
        st_value: U64::new(LE, SIGTRAMP),
        st_size: U64::new(LE, 8),
    };
    put(DYNSYM + 24, bytes_of(&symbol));
    // Version 1 is the object itself, version 2 the function's.
    put(VERSYM + 2, &2u16.to_le_bytes());
    let definition = |flags, index: u16, name: &[u8], next: u32| elf::Verdef::<LE> {
        vd_version: U16::new(LE, 1),
        vd_flags: U16::new(LE, flags),
        vd_ndx: U16::new(LE, elf::VersionIndex(index)),
        vd_cnt: U16::new(LE, 1),
        vd_hash: U32::new(LE, elf::hash(name)),
        vd_aux: U32::new(LE, 20),
        vd_next: U32::new(LE, next),
    };
    let auxiliary = |name: usize| elf::Verdaux::<LE> {
        vda_name: U32::new(LE, name as u32),
        vda_next: U32::new(LE, 0),
    };
    put(
        VERDEF,
        bytes_of(&definition(elf::VER_FLG_BASE, 1, SONAME, 28)),
    );
    put(VERDEF + 20, bytes_of(&auxiliary(soname_at)));
    put(
        VERDEF + 28,
        bytes_of(&definition(elf::VersionFlags(0), 2, VERSION, 0)),
    );
    put(VERDEF + 48, bytes_of(&auxiliary(version_at)));
    for (index, word) in CODE.iter().enumerate() {
        put(TEXT + 4 * index, &word.to_le_bytes());
    }
    let entries = [
        (elf::DT_HASH, HASH as u64),
        (elf::DT_STRTAB, DYNSTR as u64),
        (elf::DT_SYMTAB, DYNSYM as u64),
        (elf::DT_STRSZ, strings_size as u64),
        (elf::DT_SYMENT, 24),
        (elf::DT_SONAME, soname_at as u64),
        (elf::DT_VERSYM, VERSYM as u64),
        (elf::DT_VERDEF, VERDEF as u64),
        (elf::DT_VERDEFNUM, 2),
        (elf::DT_NULL, 0),
    ];
    for (index, (tag, value)) in entries.into_iter().enumerate() {
        let entry = elf::Dyn64::<LE> {
            d_tag: object::endian::I64::new(LE, tag),
            d_val: U64::new(LE, value),
        };
        put(DYNAMIC + 16 * index, bytes_of(&entry));
    }
    debug_assert!(DYNSTR + strings_size <= VERSYM && VERDEF + 56 <= TEXT);
    out
}
