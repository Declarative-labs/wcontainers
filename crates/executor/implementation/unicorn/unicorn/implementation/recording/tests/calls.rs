use wcontainers_unicorn::{Arch, Mode, Prot, Unicorn};
use wcontainers_unicorn_direct::DirectUnicorn;
use wcontainers_unicorn_recording::{Call, RecordingUnicorn};

/// The page of unicorn's interface.
const PAGE: u64 = 4096;
/// Where the test maps its memory.
const START: u64 = 0x1_0000;

/// Each call that changes the memory, the translations or runs the engine is recorded with its arguments, in order,
/// and a take forgets them.
#[test]
fn calls_are_recorded_in_order_and_taken_once() {
    let memory = DirectUnicorn::<()>::new_host_memory(2 * PAGE).expect("host memory");
    let mut engine =
        RecordingUnicorn::<DirectUnicorn<()>>::new(Arch::ARM64, Mode::ARM, ()).expect("an engine");
    let read_write = Prot(Prot::READ.0 | Prot::WRITE.0);
    engine
        .mem_map_host_memory(START, 2 * PAGE, read_write, &memory, 0)
        .expect("two pages mapped");
    engine
        .mem_protect(START, PAGE, Prot::READ)
        .expect("the first page closed to writes");
    engine
        .mem_unmap(START + PAGE, PAGE)
        .expect("the second page unmapped");
    engine
        .ctl_remove_cache(START, START + PAGE)
        .expect("translations dropped");
    engine.ctl_flush_tb().expect("every translation dropped");
    // Zeros are no instruction, so the run fails; it is recorded all the same.
    let _ = engine.emu_start(START, u64::MAX, 0, 1);
    assert_eq!(
        engine.take_calls(),
        vec![
            Call::MemMapHostMemory {
                address: START,
                size: 2 * PAGE,
                perms: read_write.0,
                offset: 0,
            },
            Call::MemProtect {
                address: START,
                size: PAGE,
                perms: Prot::READ.0,
            },
            Call::MemUnmap {
                address: START + PAGE,
                size: PAGE,
            },
            Call::CtlRemoveCache {
                address: START,
                end: START + PAGE,
            },
            Call::CtlFlushTb,
            Call::EmuStart {
                begin: START,
                count: 1,
            },
        ]
    );
    assert_eq!(engine.take_calls(), Vec::new());
    let regions: Vec<(u64, u64, u32)> = engine
        .mem_regions()
        .expect("the regions")
        .iter()
        .map(|region| (region.begin, region.end, region.perms))
        .collect();
    assert_eq!(regions, vec![(START, START + PAGE - 1, Prot::READ.0)]);
}
