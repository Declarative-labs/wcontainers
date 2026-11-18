//! Where code begins in an ELF image as it is in guest memory: the starts a range of code is translated
//! from at first use, besides the kernel's own.

use std::ops::Range;

use object::LittleEndian as LE;
use object::elf;
use object::pod::{Pod, from_bytes, slice_from_bytes};

/// How many starts each source gave, for the record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sources {
    pub entry: u64,
    pub init_fini: u64,
    pub symbols: u64,
    pub unwind: u64,
    pub relocations: u64,
    pub arrays: u64,
    pub data_words: u64,
}

/// Reads guest memory; false where it cannot.
pub trait Guest {
    fn read(&self, address: u64, into: &mut [u8]) -> bool;
    fn readable(&self, address: u64) -> bool;
}

const PAGE: u64 = wcontainers_abi::PAGE_SIZE;

/// The most pages below a range searched for its image's header.
const SEARCH_PAGES: u64 = 1 << 14;

/// The starts the ELF image whose executable segment is exactly `range` gives, as offsets into the
/// range, with how many came from each source. None when no such image is found.
pub fn image_starts(memory: &impl Guest, range: Range<u64>) -> Option<(Vec<u64>, Sources)> {
    let header_at = find_header(memory, range.start)?;
    let header: elf::FileHeader64<LE> = read(memory, header_at)?;
    if header.e_ident.class != elf::ELFCLASS64 || header.e_ident.data != elf::ELFDATA2LSB {
        return None;
    }
    let count = usize::from(header.e_phnum.get(LE));
    let phdrs: Vec<elf::ProgramHeader64<LE>> =
        read_slice(memory, header_at + header.e_phoff.get(LE), count)?;
    let loads: Vec<&elf::ProgramHeader64<LE>> = phdrs
        .iter()
        .filter(|p| p.p_type.get(LE) == elf::PT_LOAD)
        .collect();
    let first = loads.iter().find(|p| p.p_offset.get(LE) == 0)?;
    let base = header_at.wrapping_sub(first.p_vaddr.get(LE) & !(PAGE - 1));
    let image = Image {
        base,
        code: range.clone(),
    };
    // Accepted only if its one executable segment maps exactly onto the range.
    let mut executable = loads
        .iter()
        .filter(|p| p.p_flags.get(LE).0 & elf::PF_X.0 != 0);
    let text = executable.next()?;
    let text_start = base.wrapping_add(text.p_vaddr.get(LE)) & !(PAGE - 1);
    let text_end = base
        .wrapping_add(text.p_vaddr.get(LE) + text.p_memsz.get(LE))
        .next_multiple_of(PAGE);
    if executable.next().is_some() || text_start != range.start || text_end != range.end {
        return None;
    }
    let mut found = Found::default();
    found.add(
        &image,
        image.base.wrapping_add(header.e_entry.get(LE)),
        Source::Entry,
    );
    let span = loads.iter().map(|p| p.p_vaddr.get(LE)).min().unwrap_or(0)
        ..loads
            .iter()
            .map(|p| p.p_vaddr.get(LE) + p.p_memsz.get(LE))
            .max()
            .unwrap_or(0);
    if let Some(dynamic) = phdrs.iter().find(|p| p.p_type.get(LE) == elf::PT_DYNAMIC) {
        dynamic_starts(memory, &image, dynamic, span.clone(), &mut found);
    }
    if let Some(hdr) = phdrs
        .iter()
        .find(|p| p.p_type.get(LE) == elf::PT_GNU_EH_FRAME)
    {
        unwind_starts(memory, &image, hdr, &mut found);
    }
    if header.e_type.get(LE) == elf::ET_EXEC {
        for segment in loads
            .iter()
            .filter(|p| p.p_flags.get(LE).0 & elf::PF_X.0 == 0)
        {
            let at = segment.p_vaddr.get(LE);
            let words: Vec<u64> =
                read_slice(memory, at, (segment.p_filesz.get(LE) / 8) as usize).unwrap_or_default();
            for word in words {
                found.add(&image, word, Source::DataWord);
            }
        }
    }
    found.starts.sort_unstable();
    found.starts.dedup();
    Some((found.starts, found.sources))
}

/// The address of the ELF header on the nearest readable page at or below `start` that holds one.
fn find_header(memory: &impl Guest, start: u64) -> Option<u64> {
    let mut page = start & !(PAGE - 1);
    for _ in 0..SEARCH_PAGES {
        if !memory.readable(page) {
            return None;
        }
        let mut magic = [0u8; 4];
        if memory.read(page, &mut magic) && magic == elf::ELFMAG {
            return Some(page);
        }
        page = page.checked_sub(PAGE)?;
    }
    None
}

struct Image {
    /// The load offset: where the image's address 0 sits.
    base: u64,
    /// The executable segment in memory.
    code: Range<u64>,
}

#[derive(Clone, Copy)]
enum Source {
    Entry,
    InitFini,
    Symbol,
    Unwind,
    Relocation,
    Array,
    DataWord,
}

#[derive(Default)]
struct Found {
    starts: Vec<u64>,
    sources: Sources,
}

impl Found {
    /// Adds a guest address as a start if it is in the code.
    fn add(&mut self, image: &Image, address: u64, source: Source) {
        if !image.code.contains(&address) || !address.is_multiple_of(4) {
            return;
        }
        self.starts.push(address - image.code.start);
        let s = &mut self.sources;
        *match source {
            Source::Entry => &mut s.entry,
            Source::InitFini => &mut s.init_fini,
            Source::Symbol => &mut s.symbols,
            Source::Unwind => &mut s.unwind,
            Source::Relocation => &mut s.relocations,
            Source::Array => &mut s.arrays,
            Source::DataWord => &mut s.data_words,
        } += 1;
    }
}

fn read<T: Pod>(memory: &impl Guest, address: u64) -> Option<T> {
    let mut bytes = vec![0u8; size_of::<T>()];
    memory.read(address, &mut bytes).then_some(())?;
    from_bytes::<T>(&bytes).ok().map(|(value, _)| *value)
}

fn read_slice<T: Pod>(memory: &impl Guest, address: u64, count: usize) -> Option<Vec<T>> {
    let mut bytes = vec![0u8; count.checked_mul(size_of::<T>())?];
    memory.read(address, &mut bytes).then_some(())?;
    slice_from_bytes::<T>(&bytes, count)
        .ok()
        .map(|(values, _)| values.to_vec())
}

/// Starts from the dynamic table: init and fini, their arrays, the dynamic symbols and the relative
/// relocations.
fn dynamic_starts(
    memory: &impl Guest,
    image: &Image,
    dynamic: &elf::ProgramHeader64<LE>,
    span: Range<u64>,
    found: &mut Found,
) {
    let entries = (dynamic.p_memsz.get(LE) / size_of::<elf::Dyn64<LE>>() as u64) as usize;
    let Some(table) = read_slice::<elf::Dyn64<LE>>(
        memory,
        image.base.wrapping_add(dynamic.p_vaddr.get(LE)),
        entries,
    ) else {
        return;
    };
    let value = |tag: elf::DynamicTag| {
        table
            .iter()
            .find(|d| d.d_tag.get(LE) == tag)
            .map(|d| d.d_val.get(LE))
    };
    // A table entry holds an address relative to the image, or, once the loader has run, an absolute one.
    let at = |offset: u64| {
        if span.contains(&offset) {
            image.base.wrapping_add(offset)
        } else {
            offset
        }
    };
    for tag in [elf::DT_INIT, elf::DT_FINI] {
        if let Some(offset) = value(tag) {
            found.add(image, at(offset), Source::InitFini);
        }
    }
    for (array, size) in [
        (elf::DT_INIT_ARRAY, elf::DT_INIT_ARRAYSZ),
        (elf::DT_FINI_ARRAY, elf::DT_FINI_ARRAYSZ),
        (elf::DT_PREINIT_ARRAY, elf::DT_PREINIT_ARRAYSZ),
    ] {
        if let (Some(offset), Some(size)) = (value(array), value(size)) {
            let words: Vec<u64> =
                read_slice(memory, at(offset), (size / 8) as usize).unwrap_or_default();
            for word in words {
                found.add(image, at(word), Source::Array);
            }
        }
    }
    if let (Some(symbols), Some(count)) = (value(elf::DT_SYMTAB), symbol_count(memory, &value, &at))
    {
        let symbols: Vec<elf::Sym64<LE>> =
            read_slice(memory, at(symbols), count).unwrap_or_default();
        for symbol in symbols {
            if symbol.st_type() == elf::STT_FUNC && symbol.st_shndx.get(LE) != elf::SHN_UNDEF {
                found.add(
                    image,
                    image.base.wrapping_add(symbol.st_value.get(LE)),
                    Source::Symbol,
                );
            }
        }
    }
    if let (Some(rela), Some(size)) = (value(elf::DT_RELA), value(elf::DT_RELASZ)) {
        let count = (size / size_of::<elf::Rela64<LE>>() as u64) as usize;
        let relocations: Vec<elf::Rela64<LE>> =
            read_slice(memory, at(rela), count).unwrap_or_default();
        for relocation in relocations {
            if (relocation.r_info.get(LE) & 0xffff_ffff) as u32 == elf::R_AARCH64_RELATIVE.0 {
                let addend = relocation.r_addend.get(LE) as u64;
                found.add(image, image.base.wrapping_add(addend), Source::Relocation);
            }
        }
    }
    if let (Some(relr), Some(size)) = (value(elf::DT_RELR), value(elf::DT_RELRSZ)) {
        let entries: Vec<u64> =
            read_slice(memory, at(relr), (size / 8) as usize).unwrap_or_default();
        let words: Vec<u64> = relr_places(&entries)
            .filter_map(|place| read::<u64>(memory, image.base.wrapping_add(place)))
            .collect();
        // One reading for the whole image: relocated when more words, less the load offset, fall in
        // the image's own span than do as they are.
        let relocated = words
            .iter()
            .filter(|w| span.contains(&w.wrapping_sub(image.base)))
            .count();
        let as_they_are = words.iter().filter(|w| span.contains(w)).count();
        let base = if relocated > as_they_are {
            0
        } else {
            image.base
        };
        for word in words {
            found.add(image, base.wrapping_add(word), Source::Relocation);
        }
    }
}

/// How many dynamic symbols there are: `DT_HASH`'s chain count, or the end of `DT_GNU_HASH`'s chains.
fn symbol_count(
    memory: &impl Guest,
    value: &impl Fn(elf::DynamicTag) -> Option<u64>,
    at: &impl Fn(u64) -> u64,
) -> Option<usize> {
    if let Some(hash) = value(elf::DT_HASH) {
        let header: [u32; 2] = read(memory, at(hash))?;
        return Some(header[1] as usize);
    }
    let gnu = at(value(elf::DT_GNU_HASH)?);
    let [buckets, offset, blooms, _]: [u32; 4] = read(memory, gnu)?;
    let buckets_at = gnu + 16 + 8 * u64::from(blooms);
    let bucket: Vec<u32> = read_slice(memory, buckets_at, buckets as usize)?;
    let mut last = *bucket.iter().max()?;
    if last < offset {
        return Some(offset as usize);
    }
    let chains = buckets_at + 4 * u64::from(buckets);
    loop {
        let entry: u32 = read(memory, chains + 4 * u64::from(last - offset))?;
        if entry & 1 == 1 {
            return Some(last as usize + 1);
        }
        last += 1;
    }
}

/// The places a `RELR` table names, as image offsets.
fn relr_places(entries: &[u64]) -> impl Iterator<Item = u64> + '_ {
    let mut next = 0u64;
    entries.iter().flat_map(move |&entry| {
        let places: Vec<u64> = if entry & 1 == 0 {
            next = entry + 8;
            vec![entry]
        } else {
            let start = next;
            next += 63 * 8;
            (1..64)
                .filter(|bit| entry >> bit & 1 == 1)
                .map(|bit| start + (bit - 1) * 8)
                .collect()
        };
        places
    })
}

/// Starts from the unwind table index: the first address of every function it covers.
fn unwind_starts(
    memory: &impl Guest,
    image: &Image,
    hdr: &elf::ProgramHeader64<LE>,
    found: &mut Found,
) {
    let address = image.base.wrapping_add(hdr.p_vaddr.get(LE));
    let mut bytes = vec![0u8; hdr.p_memsz.get(LE) as usize];
    if !memory.read(address, &mut bytes) {
        return;
    }
    let bases = gimli::BaseAddresses::default().set_eh_frame_hdr(address);
    let Ok(parsed) = gimli::EhFrameHdr::new(&bytes, gimli::LittleEndian).parse(&bases, 8) else {
        return;
    };
    let Some(table) = parsed.table() else {
        return;
    };
    let mut entries = table.iter(&bases);
    while let Ok(Some((start, _))) = entries.next() {
        if let gimli::Pointer::Direct(start) = start {
            found.add(image, start, Source::Unwind);
        }
    }
}
