use wcontainers_unicorn::{Arch, Mode, Prot, Unicorn};
use wcontainers_unicorn_direct::DirectUnicorn;
use wcontainers_unicorn_hostile::HostileUnicorn;
use wcontainers_volatile_bytes::VolatileBytes;

/// The page of unicorn's interface.
const PAGE: u64 = 4096;
/// Where the test maps its code and its data.
const CODE: u64 = 0x1_0000;
const DATA: u64 = 0x2_0000;
/// [BRK](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/BRK--Breakpoint-instruction-) #0.
const BREAKPOINT: u32 = 0xD420_0000;

type Hostile = HostileUnicorn<(), DirectUnicorn<()>>;

/// The bytes the hostile engine writes, from [xorshift64](https://www.jstatsoft.org/article/view/v008i14)'s 13, 7 and
/// 17 from its seed, computed here apart from it.
fn reference(count: usize) -> Vec<u8> {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    (0..count)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state.to_le_bytes()[0]
        })
        .collect()
}

fn read(memory: &<Hostile as Unicorn<()>>::HostMemory, page: u64) -> Vec<u8> {
    let mut bytes = vec![0; PAGE as usize];
    Hostile::host_memory_bytes(memory, page * PAGE..(page + 1) * PAGE)
        .expect("a page of the memory")
        .read(&mut bytes);
    bytes
}

/// Before each run the hostile guest writes its bytes over every page a mapping lets it write, in the order of their
/// addresses, and over nothing else: not a page made read-only inside a writable mapping, and not a page unmapped.
#[test]
fn the_guest_writes_every_writable_byte_before_each_run_and_nothing_else() {
    let code = Hostile::new_host_memory(PAGE).expect("code memory");
    Hostile::host_memory_bytes(&code, 0..4)
        .expect("the first instruction")
        .write(&BREAKPOINT.to_le_bytes());
    let data = Hostile::new_host_memory(3 * PAGE).expect("data memory");
    let mut engine = Hostile::new(Arch::ARM64, Mode::ARM, ()).expect("an engine");
    engine
        .mem_map_host_memory(CODE, PAGE, Prot(Prot::READ.0 | Prot::EXEC.0), &code, 0)
        .expect("the code mapped");
    engine
        .mem_map_host_memory(DATA, 3 * PAGE, Prot(Prot::READ.0 | Prot::WRITE.0), &data, 0)
        .expect("the data mapped");
    engine
        .mem_protect(DATA + PAGE, PAGE, Prot::READ)
        .expect("the middle page closed to writes");
    // The breakpoint stops the run with an exception unicorn reports, which this test does not look at.
    let _ = engine.emu_start(CODE, u64::MAX, 0, 1);
    let written = reference(3 * PAGE as usize);
    let page = PAGE as usize;
    assert_eq!(engine.written(), 2 * PAGE);
    assert_eq!(read(&data, 0), written[..page]);
    assert_eq!(read(&data, 1), vec![0; page]);
    assert_eq!(read(&data, 2), written[page..2 * page]);
    assert_eq!(read(&code, 0)[..4], BREAKPOINT.to_le_bytes());
    engine
        .mem_unmap(DATA + 2 * PAGE, PAGE)
        .expect("the last page unmapped");
    let _ = engine.emu_start(CODE, u64::MAX, 0, 1);
    assert_eq!(engine.written(), 3 * PAGE);
    assert_eq!(read(&data, 0), written[2 * page..]);
    assert_eq!(read(&data, 1), vec![0; page]);
    assert_eq!(read(&data, 2), written[page..2 * page]);
}
