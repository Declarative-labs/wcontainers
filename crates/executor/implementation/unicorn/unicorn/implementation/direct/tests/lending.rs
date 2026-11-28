use wcontainers_unicorn::{Arch, Mode, Prot, Unicorn};
use wcontainers_unicorn_direct::DirectUnicorn;
use wcontainers_volatile_bytes::VolatileBytes;

/// The page of unicorn's interface.
const PAGE: u64 = 4096;
/// Where the test maps its memory.
const START: u64 = 0x1_0000;

type Direct = DirectUnicorn<()>;

/// An engine holds a host memory once for each range of addresses that maps it, from the mapping until no address of
/// the range maps it, and a view holds it while it lives (the first point of the mapping rule).
#[test]
fn an_engine_holds_its_memory_exactly_while_addresses_map_it() {
    let memory = Direct::new_host_memory(4 * PAGE).expect("host memory");
    assert_eq!(memory.holders(), 1);
    let view = Direct::host_memory_bytes(&memory, 0..PAGE).expect("a view");
    assert_eq!(memory.holders(), 2);
    drop(view);
    let mut engine = Direct::new(Arch::ARM64, Mode::ARM, ()).expect("an engine");
    engine
        .mem_map_host_memory(START, 3 * PAGE, Prot::READ, &memory, 0)
        .expect("three pages mapped");
    engine
        .mem_map_host_memory(START + 3 * PAGE, PAGE, Prot::READ, &memory, 3 * PAGE)
        .expect("the fourth page mapped after them");
    assert_eq!(memory.holders(), 3);
    // Unmapping the second page leaves two ranges of the first mapping, each holding the memory.
    engine
        .mem_unmap(START + PAGE, PAGE)
        .expect("the second page unmapped");
    assert_eq!(memory.holders(), 4);
    // Unmapping the page right after the first range leaves that range and the fourth page's.
    engine
        .mem_unmap(START + 2 * PAGE, PAGE)
        .expect("the third page unmapped");
    assert_eq!(memory.holders(), 3);
    engine
        .mem_unmap(START, PAGE)
        .expect("the first page unmapped");
    assert_eq!(memory.holders(), 2);
    // A refused unmap of addresses nothing maps holds on to what it held.
    assert!(engine.mem_unmap(START, PAGE).is_err());
    assert_eq!(memory.holders(), 2);
    drop(engine);
    assert_eq!(memory.holders(), 1);
}

/// A view reads to a sink and writes from a source all its bytes at once.
#[test]
fn a_view_reads_to_a_sink_and_writes_from_a_source() {
    let memory = Direct::new_host_memory(PAGE).expect("host memory");
    let view = Direct::host_memory_bytes(&memory, 8..24).expect("a view");
    let written: Vec<u8> = (1..=16).collect();
    let mut source = written.as_slice();
    assert_eq!(view.write_from(&mut source), Ok(16));
    let mut sink = Vec::new();
    assert_eq!(view.read_to(&mut sink), Ok(16));
    assert_eq!(sink, written);
    let mut whole = vec![0; 32];
    Direct::host_memory_bytes(&memory, 0..32)
        .expect("a view")
        .read(&mut whole);
    assert_eq!(&whole[8..24], written.as_slice());
    assert_eq!(&whole[..8], &[0; 8]);
    assert_eq!(&whole[24..], &[0; 8]);
}
