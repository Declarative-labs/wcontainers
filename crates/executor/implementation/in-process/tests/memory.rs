//! Form I's guest memories: their bytes, permissions, mappings and copies, as borrowed and as their windows show them.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::ops::Range;
use std::os::unix::fs::FileExt;
use std::sync::Arc;

use wcontainers_abi::Permissions;
use wcontainers_arm64::Way;
use wcontainers_arm64::way::{DISPLACED_SLOTS, Kept, OUTSIDE, SLOT_BITS};
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, PageTable,
};
use wcontainers_executor_in_process::memory::{
    GUEST_PAGE, InProcessMemory, InProcessPageTable, InProcessPool, WINDOW,
};
use wcontainers_volatile_bytes::VolatileBytes;

/// A page of data, far from the window's edges.
const DATA: u64 = 0x10_0000;
/// How far a mapping or copy moves its pages.
const SHIFT: u64 = 0x100_0000;
/// The window's last page.
const LAST: u64 = WINDOW.end - GUEST_PAGE;
/// Bytes that cross from one page into the next.
const CROSSING: Range<u64> = DATA + GUEST_PAGE / 2..DATA + GUEST_PAGE * 3 / 2;
/// How `/proc/self/maps` writes a page of the window that allows nothing, read only, and read and write.
const NOTHING: &str = "---s";
const READ_ONLY: &str = "r--s";
const READ_WRITE: &str = "rw-s";
/// How it writes the reserved bytes past the window, which are never mapped.
const RESERVED: &str = "---p";

fn pool() -> Arc<InProcessPool> {
    InProcessPool::new().expect("a host with the guest's pages")
}

fn memory(pool: &Arc<InProcessPool>) -> (InProcessMemory, InProcessPageTable) {
    pool.create_memory().expect("a free slot")
}

fn page(at: u64) -> Range<u64> {
    at..at + GUEST_PAGE
}

/// Bytes that differ from each other and from zero, from `seed`.
fn pattern(seed: u8, length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| seed.wrapping_add(index as u8) | 1)
        .collect()
}

fn read(memory: &InProcessMemory, addresses: Range<u64>) -> Vec<u8> {
    let mut bytes = vec![0; (addresses.end - addresses.start) as usize];
    let borrowed = memory.borrow(addresses).expect("inside the window");
    assert_eq!(borrowed.read(&mut bytes), bytes.len(), "the bytes read");
    bytes
}

fn write(memory: &InProcessMemory, at: u64, bytes: &[u8]) {
    let borrowed = memory
        .borrow(at..at + bytes.len() as u64)
        .expect("inside the window");
    assert_eq!(borrowed.write(bytes), bytes.len(), "the bytes written");
}

/// The bytes the memory's window shows at the addresses, read through `/proc/self/mem`, which reads a page whatever
/// its protection.
fn window_bytes(memory: &InProcessMemory, addresses: Range<u64>) -> Vec<u8> {
    let process = std::fs::File::open("/proc/self/mem").expect("the process's own memory");
    let mut bytes = vec![0; (addresses.end - addresses.start) as usize];
    process
        .read_exact_at(&mut bytes, memory.window_base() + addresses.start)
        .expect("the window is mapped");
    bytes
}

/// How `/proc/self/maps` writes the protection of the page at the host address.
fn host_protection(address: u64) -> Option<String> {
    let maps = std::fs::read_to_string("/proc/self/maps").expect("the process's own maps");
    maps.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let (start, end) = fields.next()?.split_once('-')?;
        let parse = |text: &str| u64::from_str_radix(text, 16).ok();
        (parse(start)?..parse(end)?)
            .contains(&address)
            .then(|| fields.next().unwrap_or_default().to_owned())
    })
}

fn window_protection(memory: &InProcessMemory, at: u64) -> String {
    host_protection(memory.window_base() + at).expect("the window is mapped")
}

#[test]
fn a_new_memory_reads_as_zeros_and_its_window_allows_nothing() {
    let pool = pool();
    let (memory, _table) = memory(&pool);
    assert!(memory.slot() >= 1, "slot 0 is no guest's");
    assert_eq!(memory.window_base(), u64::from(memory.slot()) << SLOT_BITS);
    for at in [WINDOW.start, DATA, LAST] {
        assert_eq!(read(&memory, page(at)), vec![0; GUEST_PAGE as usize]);
        assert_eq!(
            window_bytes(&memory, page(at)),
            vec![0; GUEST_PAGE as usize]
        );
        assert_eq!(window_protection(&memory, at), NOTHING);
    }
    assert_eq!(window_protection(&memory, OUTSIDE), RESERVED);
    // The slot holds the bytes below the window that translated instructions reach (a stack step of up to 60 KiB and
    // the probe of the page below it, docs/native-without-virtual-machine.md section 3.2 rule c), and above it up to
    // the next slot's.
    for below in [GUEST_PAGE, REACH_BELOW] {
        assert_eq!(
            host_protection(memory.window_base() - below).as_deref(),
            Some(RESERVED),
            "{below:#x} below the window"
        );
    }
    assert_eq!(
        window_protection(&memory, (1 << SLOT_BITS) - REACH_BELOW - GUEST_PAGE),
        RESERVED
    );
}

/// The bytes below a window its slot holds never mapped.
const REACH_BELOW: u64 = 64 << 10;

#[test]
fn each_window_s_slot_table_and_displaced_code_s_are_held_inaccessible() {
    let pool = pool();
    let (memory, _table) = memory(&pool);
    let way = Way::InProcess {
        slot: memory.slot(),
        displaced: false,
        kept: Kept::USUAL,
    };
    for at in [WINDOW.start, DATA, LAST, OUTSIDE] {
        assert_eq!(
            host_protection(way.slot(at)).as_deref(),
            Some(RESERVED),
            "the slot of {at:#x}"
        );
    }
    let displaced = Way::InProcess {
        slot: memory.slot(),
        displaced: true,
        kept: Kept::USUAL,
    };
    assert_eq!(displaced.slot_table(), DISPLACED_SLOTS);
    for at in [WINDOW.start, LAST] {
        assert_eq!(
            host_protection(displaced.slot(at)).as_deref(),
            Some(RESERVED)
        );
    }
}

#[test]
fn borrowed_bytes_are_the_bytes_the_window_shows() {
    let pool = pool();
    let (memory, _table) = memory(&pool);
    let crossing = pattern(1, (CROSSING.end - CROSSING.start) as usize);
    write(&memory, CROSSING.start, &crossing);
    assert_eq!(window_bytes(&memory, CROSSING), crossing);
    assert_eq!(read(&memory, CROSSING), crossing);
    let last = pattern(2, GUEST_PAGE as usize);
    write(&memory, LAST, &last);
    assert_eq!(window_bytes(&memory, page(LAST)), last);
    let before = DATA..CROSSING.start;
    assert_eq!(
        read(&memory, before),
        vec![0; (CROSSING.start - DATA) as usize]
    );
}

#[test]
fn permissions_give_the_window_pages_what_the_guest_may_read_and_write() {
    let pool = pool();
    let (memory, mut table) = memory(&pool);
    let cases = [
        (Permissions::READ, READ_ONLY),
        (Permissions::READ | Permissions::WRITE, READ_WRITE),
        (Permissions::READ | Permissions::EXECUTE, READ_ONLY),
        (Permissions::EXECUTE, NOTHING),
        (Permissions::empty(), NOTHING),
    ];
    for (allowed, shown) in cases {
        assert_eq!(table.set_permissions(CROSSING, allowed), Ok(()));
        assert_eq!(window_protection(&memory, DATA), shown, "{allowed:?}");
        assert_eq!(
            window_protection(&memory, DATA + GUEST_PAGE),
            shown,
            "{allowed:?}"
        );
        assert_eq!(window_protection(&memory, DATA - GUEST_PAGE), NOTHING);
        assert_eq!(window_protection(&memory, DATA + 2 * GUEST_PAGE), NOTHING);
    }
    write(&memory, DATA, &pattern(3, GUEST_PAGE as usize));
    assert_eq!(
        read(&memory, page(DATA)),
        pattern(3, GUEST_PAGE as usize),
        "borrowed bytes of a page that allows nothing"
    );
}

#[test]
fn two_memories_have_windows_apart() {
    let pool = pool();
    let (first, _first_table) = memory(&pool);
    let (second, _second_table) = memory(&pool);
    assert_ne!(first.slot(), second.slot());
    let distance = first.window_base().abs_diff(second.window_base());
    assert!(distance >= 1 << SLOT_BITS, "windows {distance:#x} apart");
    for at in [DATA, LAST] {
        write(&first, at, &pattern(4, GUEST_PAGE as usize));
        assert_eq!(read(&second, page(at)), vec![0; GUEST_PAGE as usize]);
        assert_eq!(
            window_bytes(&second, page(at)),
            vec![0; GUEST_PAGE as usize]
        );
    }
}

#[test]
fn addresses_past_the_window_are_refused() {
    let pool = pool();
    let (memory, mut table) = memory(&pool);
    let (_other, other_table) = self::memory(&pool);
    let past = WINDOW.end..WINDOW.end + GUEST_PAGE;
    let reversed = Range {
        start: DATA + GUEST_PAGE,
        end: DATA,
    };
    for addresses in [past.clone(), LAST..past.end, reversed] {
        assert!(matches!(
            memory.borrow(addresses.clone()),
            Err(InvalidAddressError)
        ));
        assert_eq!(memory.zero(addresses.clone()), Err(InvalidAddressError));
        assert_eq!(
            table.set_permissions(addresses.clone(), Permissions::READ),
            Err(InvalidAddressError)
        );
        assert_eq!(
            table.map_from(addresses.clone(), &other_table, DATA),
            Err(MapMemoryError::InvalidAddress(InvalidAddressError))
        );
        assert_eq!(
            table.unmap_from(addresses.clone()),
            Err(InvalidAddressError)
        );
        assert_eq!(
            table.copy_from(addresses, &other_table, DATA),
            Err(CopyMemoryError::InvalidAddress(InvalidAddressError))
        );
    }
    for from in [past.start, LAST + 1, u64::MAX - GUEST_PAGE + 1] {
        assert_eq!(
            table.map_from(page(DATA), &other_table, from),
            Err(MapMemoryError::InvalidAddress(InvalidAddressError))
        );
        assert_eq!(
            table.copy_from(page(DATA), &other_table, from),
            Err(CopyMemoryError::InvalidAddress(InvalidAddressError))
        );
    }
    assert_eq!(
        table.map_from(CROSSING, &other_table, DATA),
        Err(MapMemoryError::InvalidAddress(InvalidAddressError))
    );
    assert_eq!(window_protection(&memory, LAST), NOTHING);
    assert_eq!(window_protection(&memory, OUTSIDE), RESERVED);
}

#[test]
fn zero_clears_only_its_addresses() {
    let pool = pool();
    let (memory, _table) = memory(&pool);
    let length = 4 * GUEST_PAGE;
    let written = pattern(5, length as usize);
    write(&memory, DATA, &written);
    assert_eq!(memory.zero(CROSSING), Ok(()));
    let mut expected = written;
    let cleared = (CROSSING.start - DATA) as usize..(CROSSING.end - DATA) as usize;
    expected[cleared].fill(0);
    assert_eq!(read(&memory, DATA..DATA + length), expected);
    assert_eq!(window_bytes(&memory, DATA..DATA + length), expected);
}

#[test]
fn a_mapping_shares_bytes_both_ways_in_both_windows_and_keeps_permissions() {
    let pool = pool();
    let (first, mut first_table) = memory(&pool);
    let (second, mut second_table) = memory(&pool);
    let shared = pattern(6, 2 * GUEST_PAGE as usize);
    write(&first, DATA, &shared);
    let both = DATA..DATA + 2 * GUEST_PAGE;
    assert_eq!(
        first_table.set_permissions(page(DATA), Permissions::READ | Permissions::WRITE),
        Ok(())
    );
    let target = DATA + SHIFT..DATA + SHIFT + 2 * GUEST_PAGE;
    assert_eq!(
        second_table.map_from(target.clone(), &first_table, DATA),
        Ok(())
    );
    assert_eq!(read(&second, target.clone()), shared);
    assert_eq!(window_bytes(&second, target.clone()), shared);
    assert_eq!(window_bytes(&first, both.clone()), shared);
    assert_eq!(window_protection(&first, DATA), READ_WRITE);
    assert_eq!(window_protection(&first, DATA + GUEST_PAGE), NOTHING);
    assert_eq!(window_protection(&second, target.start), NOTHING);

    let from_second = pattern(7, shared.len());
    write(&second, target.start, &from_second);
    assert_eq!(read(&first, both.clone()), from_second);
    assert_eq!(window_bytes(&first, both.clone()), from_second);
    let from_first = pattern(8, shared.len());
    write(&first, DATA, &from_first);
    assert_eq!(window_bytes(&second, target.clone()), from_first);

    let (third, mut third_table) = memory(&pool);
    assert_eq!(
        third_table.map_from(target.clone(), &second_table, target.start),
        Ok(())
    );
    write(&third, target.start, &shared);
    assert_eq!(read(&first, both), shared, "a mapping of a mapping");
}

#[test]
fn an_unmapping_zeroes_this_side_only() {
    let pool = pool();
    let (first, first_table) = memory(&pool);
    let (second, mut second_table) = memory(&pool);
    let kept = pattern(9, GUEST_PAGE as usize);
    write(&first, DATA, &kept);
    let target = page(DATA + SHIFT);
    assert_eq!(
        second_table.map_from(target.clone(), &first_table, DATA),
        Ok(())
    );
    assert_eq!(
        second_table.set_permissions(target.clone(), Permissions::READ),
        Ok(())
    );
    assert_eq!(second_table.unmap_from(target.clone()), Ok(()));
    let zeros = vec![0; GUEST_PAGE as usize];
    assert_eq!(read(&second, target.clone()), zeros);
    assert_eq!(window_bytes(&second, target.clone()), zeros);
    assert_eq!(window_protection(&second, target.start), READ_ONLY);
    assert_eq!(read(&first, page(DATA)), kept);
    write(&second, target.start, &pattern(10, GUEST_PAGE as usize));
    assert_eq!(read(&first, page(DATA)), kept);
    assert_eq!(window_bytes(&first, page(DATA)), kept);
}

#[test]
fn a_copy_carries_bytes_permissions_and_mappings() {
    let pool = pool();
    let (source, mut source_table) = memory(&pool);
    let (copy, mut copy_table) = memory(&pool);
    let (mapped, mapped_table) = memory(&pool);
    let data = pattern(11, GUEST_PAGE as usize);
    write(&source, DATA, &data);
    let elsewhere = DATA + 4 * GUEST_PAGE;
    assert_eq!(
        source_table.set_permissions(page(DATA), Permissions::READ | Permissions::WRITE),
        Ok(())
    );
    assert_eq!(
        source_table.map_from(page(elsewhere), &mapped_table, DATA),
        Ok(())
    );
    assert_eq!(
        source_table.set_permissions(page(elsewhere), Permissions::READ),
        Ok(())
    );
    write(
        &copy,
        DATA + SHIFT + GUEST_PAGE,
        &pattern(12, GUEST_PAGE as usize),
    );

    let copied = DATA..elsewhere + GUEST_PAGE;
    assert_eq!(
        copy_table.copy_from(
            copied.start + SHIFT..copied.end + SHIFT,
            &source_table,
            DATA
        ),
        Ok(())
    );
    assert_eq!(read(&copy, page(DATA + SHIFT)), data);
    assert_eq!(window_bytes(&copy, page(DATA + SHIFT)), data);
    assert_eq!(
        read(&copy, page(DATA + SHIFT + GUEST_PAGE)),
        vec![0; GUEST_PAGE as usize],
        "a hole of the source copied over bytes"
    );
    assert_eq!(window_protection(&copy, DATA + SHIFT), READ_WRITE);
    assert_eq!(window_protection(&copy, DATA + SHIFT + GUEST_PAGE), NOTHING);
    assert_eq!(window_protection(&copy, elsewhere + SHIFT), READ_ONLY);

    let through = pattern(13, GUEST_PAGE as usize);
    write(&mapped, DATA, &through);
    assert_eq!(read(&copy, page(elsewhere + SHIFT)), through);
    assert_eq!(window_bytes(&copy, page(elsewhere + SHIFT)), through);

    write(&copy, DATA + SHIFT, &pattern(14, GUEST_PAGE as usize));
    assert_eq!(
        read(&source, page(DATA)),
        data,
        "the source after a write to the copy"
    );
    write(&source, DATA, &pattern(15, GUEST_PAGE as usize));
    assert_eq!(
        read(&copy, page(DATA + SHIFT)),
        pattern(14, GUEST_PAGE as usize),
        "the copy after a write to the source"
    );
}

/// Bytes that span the memory's own file and a block shown from another are read and written whole, through every
/// form of borrowed bytes, and the borrowed bytes' length is the addresses'.
#[test]
fn bytes_across_the_own_file_and_a_shown_block_are_read_and_written_whole() {
    let pool = pool();
    let (first, first_table) = memory(&pool);
    let (second, mut second_table) = memory(&pool);
    let middle = DATA + GUEST_PAGE;
    assert_eq!(
        second_table.map_from(page(middle), &first_table, middle),
        Ok(())
    );
    let three = DATA..DATA + 3 * GUEST_PAGE;
    let bytes = pattern(10, 3 * GUEST_PAGE as usize);
    write(&second, DATA, &bytes);
    assert_eq!(read(&second, three.clone()), bytes);
    assert_eq!(window_bytes(&second, three.clone()), bytes);
    assert_eq!(
        read(&first, page(middle)),
        bytes[GUEST_PAGE as usize..2 * GUEST_PAGE as usize]
    );
    let borrowed = second.borrow(three.clone()).expect("inside the window");
    assert_eq!(borrowed.len(), 3 * GUEST_PAGE);
    let mut sink = Vec::new();
    assert_eq!(borrowed.read_to(&mut sink), Ok(bytes.len()));
    assert_eq!(sink, bytes);
    let again = pattern(11, bytes.len());
    let mut source: &[u8] = &again;
    assert_eq!(borrowed.write_from(&mut source), Ok(again.len()));
    assert_eq!(window_bytes(&second, three), again);
    let inner = second
        .borrow(DATA + 1..DATA + 9)
        .expect("inside the window");
    assert_eq!(inner.len(), 8);
}

/// Two blocks shown side by side whose file offsets run on are still two: each page shows its own block's bytes.
#[test]
fn two_blocks_shown_side_by_side_stay_apart() {
    let pool = pool();
    let (first, first_table) = memory(&pool);
    let (third, third_table) = memory(&pool);
    let (_fourth, mut fourth_table) = memory(&pool);
    let (second, mut second_table) = memory(&pool);
    let first_bytes = pattern(12, GUEST_PAGE as usize);
    let third_bytes = pattern(13, 2 * GUEST_PAGE as usize);
    write(&first, DATA, &first_bytes);
    write(&third, DATA, &third_bytes);
    // The third's two pages become one block of two pages; the first's page a block of one.
    assert_eq!(
        fourth_table.map_from(DATA..DATA + 2 * GUEST_PAGE, &third_table, DATA),
        Ok(())
    );
    let at = DATA + SHIFT;
    assert_eq!(second_table.map_from(page(at), &first_table, DATA), Ok(()));
    assert_eq!(
        second_table.map_from(page(at + GUEST_PAGE), &third_table, DATA + GUEST_PAGE),
        Ok(())
    );
    let mut wanted = first_bytes;
    wanted.extend_from_slice(&third_bytes[GUEST_PAGE as usize..]);
    assert_eq!(read(&second, at..at + 2 * GUEST_PAGE), wanted);
    assert_eq!(window_bytes(&second, at..at + 2 * GUEST_PAGE), wanted);
}

/// A block shown over pages of which only some allow anything shows on every page, those between included.
#[test]
fn a_block_shows_on_every_page_whatever_their_permissions() {
    let pool = pool();
    let (first, first_table) = memory(&pool);
    let (second, mut second_table) = memory(&pool);
    let bytes = pattern(14, 3 * GUEST_PAGE as usize);
    write(&first, DATA, &bytes);
    let at = DATA + SHIFT;
    for allowed in [at, at + 2 * GUEST_PAGE] {
        assert_eq!(
            second_table.set_permissions(page(allowed), Permissions::READ | Permissions::WRITE),
            Ok(())
        );
    }
    assert_eq!(
        second_table.map_from(at..at + 3 * GUEST_PAGE, &first_table, DATA),
        Ok(())
    );
    assert_eq!(window_bytes(&second, at..at + 3 * GUEST_PAGE), bytes);
    assert_eq!(window_protection(&second, at + GUEST_PAGE), NOTHING);
    assert_eq!(window_protection(&second, at), READ_WRITE);
}

/// The seals every memory file of this process holds, by its name.
fn seals_of_files_named(name: &str) -> Vec<rustix::fs::SealFlags> {
    std::fs::read_dir("/proc/self/fd")
        .expect("this process's files")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let target = std::fs::read_link(&path).ok()?;
            target.to_str()?.contains(name).then_some(path)
        })
        .filter_map(|path| {
            let file = std::fs::File::open(path).ok()?;
            rustix::fs::fcntl_get_seals(&file).ok()
        })
        .collect()
}

/// A memory's own file and every shared block cannot change size, and their seals cannot be changed: no access inside
/// a window is ever past a file's end.
#[test]
fn every_memory_file_cannot_change_size() {
    use rustix::fs::SealFlags;
    let pool = pool();
    let (first, first_table) = memory(&pool);
    let (_second, mut second_table) = memory(&pool);
    write(&first, DATA, &pattern(15, GUEST_PAGE as usize));
    assert_eq!(
        second_table.map_from(page(DATA), &first_table, DATA),
        Ok(())
    );
    let wanted = SealFlags::SHRINK | SealFlags::GROW | SealFlags::SEAL;
    for name in [
        "wcontainers in-process memory",
        "wcontainers in-process shared block",
    ] {
        let seals = seals_of_files_named(name);
        assert!(!seals.is_empty(), "no file named {name}");
        assert!(
            seals.iter().all(|seals| seals.contains(wanted)),
            "{name}: {seals:?}"
        );
    }
}

/// A copy of addresses that begin with a hole, whose source holds data past their end, copies exactly them: the hole
/// stays zeros, the data lands at its own place, and nothing past the end is copied.
#[test]
fn a_copy_takes_exactly_its_addresses_holes_and_data_alike() {
    let pool = pool();
    let (source, source_table) = memory(&pool);
    let (copy, mut copy_table) = memory(&pool);
    let data = pattern(16, 2 * GUEST_PAGE as usize);
    write(&source, DATA + GUEST_PAGE, &data);
    let to = DATA + SHIFT;
    assert_eq!(
        copy_table.copy_from(to..to + 2 * GUEST_PAGE, &source_table, DATA),
        Ok(())
    );
    let mut wanted = vec![0; GUEST_PAGE as usize];
    wanted.extend_from_slice(&data[..GUEST_PAGE as usize]);
    wanted.extend(vec![0; GUEST_PAGE as usize]);
    assert_eq!(read(&copy, to..to + 3 * GUEST_PAGE), wanted);
}
