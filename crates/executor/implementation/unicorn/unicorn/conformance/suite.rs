//! The tests every implementation of the unicorn trait runs, written against the trait alone.
//!
//! Each expected value comes from the documented behavior of the instructions run (Arm's A64 descriptions, linked at each
//! encoding) or of the mapping rule, never from an engine's own answers.

use std::ops::Range;

use wcontainers_unicorn::{
    Arch, Arm64CpuModel, HookType, MemType, Mode, Prot, RegisterARM64, RegisterARM64CP, Requests,
    TlbEntry, TlbType, Unicorn, uc_error,
};
use wcontainers_volatile_bytes::VolatileBytes;

/// The page of unicorn's interface.
pub const PAGE: u64 = 4096;
/// Where each test's code is mapped.
const CODE: u64 = 0x1_0000;
/// Where each test's data is mapped, the page after the code.
const DATA: u64 = CODE + PAGE;
/// An address far past every mapping of a test, inside the 44 bits of physical address of the Cortex-A72, unicorn's
/// default model, so that it is an address unicorn names in its errors and not one beyond the processor's reach.
const FAR: u64 = 0x0800_0000_0000;
/// A value the tests store and load.
const WORD: u64 = 0x1122_3344_5566_7788;
/// A run with no limit on instructions or time.
const NO_LIMIT: usize = 0;
const NO_TIMEOUT: u64 = 0;
/// A run of one instruction.
const ONE: usize = 1;

/// [STR (immediate)](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STR--immediate---Store-register--immediate--)
/// of X1 to the address in X0, unsigned offset 0.
const STORE_X1_AT_X0: u32 = 0xF900_0001;
/// [LDR (immediate)](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDR--immediate---Load-register--immediate--)
/// of X1 from the address in X0, unsigned offset 0.
const LOAD_X1_FROM_X0: u32 = 0xF940_0001;
/// [BRK](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/BRK--Breakpoint-instruction-) #0.
const BREAKPOINT: u32 = 0xD420_0000;
/// [NOP](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/NOP--No-operation-).
const NOP: u32 = 0xD503_201F;
/// [LDADD](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDADD--LDADDA--LDADDAL--LDADDL--Atomic-add-on-word-or-doubleword-in-memory-)
/// X1, X2, [X0]: X2 takes the doubleword at X0, which becomes its sum with X1; FEAT_LSE, which the Cortex-A72 lacks.
const ATOMIC_ADD_X1_INTO_X0_OLD_IN_X2: u32 = 0xF821_0002;
/// QEMU's exception number for an undefined instruction (EXCP_UDEF) and a breakpoint (EXCP_BKPT), as the interrupt hook
/// reports them ([cpu.h](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h)).
const EXCEPTION_UNDEFINED: u32 = 1;
const EXCEPTION_BREAKPOINT: u32 = 7;
/// [TPIDR_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/TPIDR-EL0--EL0-Read-Write-Software-Thread-ID-Register)
/// as op0, op1, CRn, CRm and op2.
const THREAD_ID: RegisterARM64CP = RegisterARM64CP::new().op0(3).op1(3).crn(13).crm(0).op2(2);

/// [MOVZ](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MOVZ--Move-wide-with-zero-) of the
/// 16-bit value into X1.
const fn move_into_x1(value: u16) -> u32 {
    0xD280_0001 | (value as u32) << 5
}

/// What the tests' hooks saw.
#[derive(Default)]
pub struct Seen {
    pub exceptions: Vec<u32>,
    pub blocks: Vec<(u64, u32)>,
    pub invalid: Vec<u64>,
    /// The page of addresses the TLB hook gives another physical page, and that page.
    pub redirect: Option<(u64, u64)>,
    /// Whether the interrupt hook asks its engine to drop the translations of the code page.
    pub forget_on_exception: bool,
}

fn engine<U: Unicorn<Seen>>() -> U {
    U::new(Arch::ARM64, Mode::ARM, Seen::default()).expect("an engine")
}

fn new_memory<U: Unicorn<Seen>>(pages: u64) -> U::HostMemory {
    U::new_host_memory(pages * PAGE).expect("host memory")
}

fn bytes<U: Unicorn<Seen>>(memory: &U::HostMemory, offsets: Range<u64>) -> U::Bytes {
    U::host_memory_bytes(memory, offsets).expect("offsets inside the memory")
}

fn read_word<U: Unicorn<Seen>>(memory: &U::HostMemory, at: u64) -> u64 {
    let mut word = [u8::MIN; size_of::<u64>()];
    assert_eq!(
        bytes::<U>(memory, at..at + size_of::<u64>() as u64).read(&mut word),
        word.len()
    );
    u64::from_le_bytes(word)
}

fn write_word<U: Unicorn<Seen>>(memory: &U::HostMemory, at: u64, value: u64) {
    let written = bytes::<U>(memory, at..at + size_of::<u64>() as u64).write(&value.to_le_bytes());
    assert_eq!(written, size_of::<u64>());
}

/// Returns the bytes of the memory at the offsets.
fn read_all<U: Unicorn<Seen>>(memory: &U::HostMemory, offsets: Range<u64>) -> Vec<u8> {
    let mut read = vec![u8::MIN; usize::try_from(offsets.end - offsets.start).expect("a length")];
    bytes::<U>(memory, offsets).read(&mut read);
    read
}

/// Writes the instructions at the memory's start.
fn write_code<U: Unicorn<Seen>>(memory: &U::HostMemory, instructions: &[u32]) {
    let code: Vec<u8> = instructions
        .iter()
        .flat_map(|instruction| instruction.to_le_bytes())
        .collect();
    assert_eq!(
        bytes::<U>(memory, 0..code.len() as u64).write(&code),
        code.len()
    );
}

/// Maps the memory's first page as code, readable and executable, at [`CODE`].
fn map_code<U: Unicorn<Seen>>(engine: &mut U, memory: &U::HostMemory) {
    engine
        .mem_map_host_memory(CODE, PAGE, Prot(Prot::READ.0 | Prot::EXEC.0), memory, 0)
        .expect("the code page mapped");
}

fn read_write() -> Prot {
    Prot(Prot::READ.0 | Prot::WRITE.0)
}

fn set(engine: &mut impl Unicorn<Seen>, register: RegisterARM64, value: u64) {
    engine
        .reg_write(register as i32, value)
        .expect("a register written");
}

fn get(engine: &impl Unicorn<Seen>, register: RegisterARM64) -> u64 {
    engine.reg_read(register as i32).expect("a register read")
}

/// Runs one instruction from the start of the code page.
fn run_one(engine: &mut impl Unicorn<Seen>) -> Result<(), uc_error> {
    engine.emu_start(CODE, u64::MAX, NO_TIMEOUT, ONE)
}

/// Adds an interrupt hook that records each exception and stops the run, and drops the code page's translations when
/// the data asks it.
fn stop_at_exceptions(engine: &mut impl Unicorn<Seen>) {
    engine
        .add_intr_hook(|seen: &mut Seen, requests: &mut Requests, exception| {
            seen.exceptions.push(exception);
            requests.stop = true;
            if seen.forget_on_exception {
                requests.forget_code.push(CODE..CODE + PAGE);
            }
        })
        .expect("the interrupt hook");
}

/// Host memory is whole pages of zeros, and no view reaches past its end.
pub fn host_memory_is_zeroed_whole_pages<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(2);
    assert_eq!(
        read_all::<U>(&memory, 0..2 * PAGE),
        vec![u8::MIN; 2 * PAGE as usize]
    );
    assert!(U::host_memory_bytes(&memory, 0..2 * PAGE + 1).is_none());
    let reversed = Range {
        start: PAGE,
        end: PAGE - 1,
    };
    assert!(U::host_memory_bytes(&memory, reversed).is_none());
    assert_eq!(
        U::host_memory_bytes(&memory, PAGE..2 * PAGE).map(|view| view.len()),
        Some(PAGE)
    );
    assert_eq!(U::new_host_memory(0).err(), Some(uc_error::ARG));
    assert_eq!(U::new_host_memory(PAGE + 1).err(), Some(uc_error::ARG));
}

/// A store of the guest lands in the host memory the address shows, little-endian.
pub fn a_guest_store_reaches_the_host_memory<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(2);
    write_code::<U>(&memory, &[STORE_X1_AT_X0]);
    let mut engine = engine::<U>();
    map_code(&mut engine, &memory);
    engine
        .mem_map_host_memory(DATA, PAGE, read_write(), &memory, PAGE)
        .expect("the data page mapped");
    set(&mut engine, RegisterARM64::X0, DATA + 8);
    set(&mut engine, RegisterARM64::X1, WORD);
    run_one(&mut engine).expect("the store ran");
    assert_eq!(read_word::<U>(&memory, PAGE + 8), WORD);
    assert_eq!(
        read_all::<U>(&memory, PAGE + 8..PAGE + 16),
        vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]
    );
    assert_eq!(get(&engine, RegisterARM64::PC), CODE + 4);
}

/// A load of the guest reads what our side wrote into the host memory.
pub fn a_guest_loads_what_the_host_wrote<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(2);
    write_code::<U>(&memory, &[LOAD_X1_FROM_X0]);
    write_word::<U>(&memory, PAGE + 16, WORD);
    let mut engine = engine::<U>();
    map_code(&mut engine, &memory);
    engine
        .mem_map_host_memory(DATA, PAGE, Prot::READ, &memory, PAGE)
        .expect("the data page mapped");
    set(&mut engine, RegisterARM64::X0, DATA + 16);
    run_one(&mut engine).expect("the load ran");
    assert_eq!(get(&engine, RegisterARM64::X1), WORD);
}

/// A mapping shows the memory from its offset on.
pub fn a_mapping_shows_the_memory_from_its_offset<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(3);
    write_code::<U>(&memory, &[LOAD_X1_FROM_X0]);
    write_word::<U>(&memory, 2 * PAGE + 24, WORD);
    let mut engine = engine::<U>();
    map_code(&mut engine, &memory);
    engine
        .mem_map_host_memory(DATA, PAGE, Prot::READ, &memory, 2 * PAGE)
        .expect("the data page mapped");
    set(&mut engine, RegisterARM64::X0, DATA + 24);
    run_one(&mut engine).expect("the load ran");
    assert_eq!(get(&engine, RegisterARM64::X1), WORD);
}

/// A mapping of bytes past the memory's end or from an offset that is no whole page is refused and maps nothing.
pub fn mappings_outside_the_memory_are_refused<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(2);
    let mut engine = engine::<U>();
    for (size, offset) in [
        (2 * PAGE, PAGE),
        (PAGE, 2 * PAGE),
        (PAGE, 1),
        (PAGE, u64::MAX),
    ] {
        assert_eq!(
            engine.mem_map_host_memory(DATA, size, read_write(), &memory, offset),
            Err(uc_error::ARG),
            "size {size} from offset {offset}"
        );
    }
    assert!(engine.mem_regions().expect("the regions").is_empty());
}

/// The regions are what was mapped, each with its last address and permissions.
pub fn regions_are_what_was_mapped<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(3);
    let mut engine = engine::<U>();
    engine
        .mem_map_host_memory(DATA, 2 * PAGE, Prot::READ, &memory, PAGE)
        .expect("two pages mapped");
    let regions: Vec<(u64, u64, u32)> = engine
        .mem_regions()
        .expect("the regions")
        .iter()
        .map(|region| (region.begin, region.end, region.perms))
        .collect();
    assert_eq!(regions, vec![(DATA, DATA + 2 * PAGE - 1, Prot::READ.0)]);
}

/// A store to a mapping that refuses writes stops the run and changes no byte.
pub fn a_store_to_a_mapping_refusing_writes_changes_nothing<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(2);
    write_code::<U>(&memory, &[STORE_X1_AT_X0]);
    let mut engine = engine::<U>();
    map_code(&mut engine, &memory);
    engine
        .mem_map_host_memory(DATA, PAGE, Prot::READ, &memory, PAGE)
        .expect("the data page mapped");
    set(&mut engine, RegisterARM64::X0, DATA);
    set(&mut engine, RegisterARM64::X1, WORD);
    assert_eq!(run_one(&mut engine), Err(uc_error::WRITE_PROT));
    assert_eq!(
        read_all::<U>(&memory, PAGE..2 * PAGE),
        vec![u8::MIN; PAGE as usize]
    );
}

/// After `mem_protect` takes writes away, a store is refused and changes no byte.
pub fn protect_refuses_later_stores<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(2);
    write_code::<U>(&memory, &[STORE_X1_AT_X0]);
    let mut engine = engine::<U>();
    map_code(&mut engine, &memory);
    engine
        .mem_map_host_memory(DATA, PAGE, read_write(), &memory, PAGE)
        .expect("the data page mapped");
    engine
        .mem_protect(DATA, PAGE, Prot::READ)
        .expect("the data page closed to writes");
    set(&mut engine, RegisterARM64::X0, DATA);
    set(&mut engine, RegisterARM64::X1, WORD);
    assert_eq!(run_one(&mut engine), Err(uc_error::WRITE_PROT));
    assert_eq!(
        read_all::<U>(&memory, PAGE..2 * PAGE),
        vec![u8::MIN; PAGE as usize]
    );
}

/// After `mem_unmap` the guest reaches no byte of the memory there.
pub fn unmapped_addresses_reach_no_host_memory<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(3);
    write_code::<U>(&memory, &[STORE_X1_AT_X0]);
    let mut engine = engine::<U>();
    map_code(&mut engine, &memory);
    engine
        .mem_map_host_memory(DATA, 2 * PAGE, read_write(), &memory, PAGE)
        .expect("the data pages mapped");
    engine
        .mem_unmap(DATA, PAGE)
        .expect("the first data page unmapped");
    set(&mut engine, RegisterARM64::X0, DATA);
    set(&mut engine, RegisterARM64::X1, WORD);
    assert_eq!(run_one(&mut engine), Err(uc_error::WRITE_UNMAPPED));
    assert_eq!(
        read_all::<U>(&memory, PAGE..2 * PAGE),
        vec![u8::MIN; PAGE as usize]
    );
    // The page after it stays mapped, from the memory's third page.
    set(&mut engine, RegisterARM64::X0, DATA + PAGE);
    run_one(&mut engine).expect("the store to the page still mapped ran");
    assert_eq!(read_word::<U>(&memory, 2 * PAGE), WORD);
}

/// Two engines that map one host memory see each other's stores.
pub fn two_engines_share_one_host_memory<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(3);
    write_code::<U>(&memory, &[STORE_X1_AT_X0]);
    let loader = new_memory::<U>(1);
    write_code::<U>(&loader, &[LOAD_X1_FROM_X0]);
    let mut storing = engine::<U>();
    map_code(&mut storing, &memory);
    storing
        .mem_map_host_memory(DATA, PAGE, read_write(), &memory, 2 * PAGE)
        .expect("the data page mapped writable");
    let mut loading = engine::<U>();
    map_code(&mut loading, &loader);
    loading
        .mem_map_host_memory(FAR, PAGE, Prot::READ, &memory, 2 * PAGE)
        .expect("the same page mapped readable");
    set(&mut storing, RegisterARM64::X0, DATA + 32);
    set(&mut storing, RegisterARM64::X1, WORD);
    run_one(&mut storing).expect("the store ran");
    set(&mut loading, RegisterARM64::X0, FAR + 32);
    run_one(&mut loading).expect("the load ran");
    assert_eq!(get(&loading, RegisterARM64::X1), WORD);
}

/// A view keeps its memory after the engine that mapped it is dropped.
pub fn a_view_outlives_its_engine<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(2);
    write_code::<U>(&memory, &[STORE_X1_AT_X0]);
    let view = bytes::<U>(&memory, PAGE..PAGE + 8);
    let mut engine = engine::<U>();
    map_code(&mut engine, &memory);
    engine
        .mem_map_host_memory(DATA, PAGE, read_write(), &memory, PAGE)
        .expect("the data page mapped");
    set(&mut engine, RegisterARM64::X0, DATA);
    set(&mut engine, RegisterARM64::X1, WORD);
    run_one(&mut engine).expect("the store ran");
    drop(engine);
    drop(memory);
    let mut word = [u8::MIN; size_of::<u64>()];
    assert_eq!(view.read(&mut word), word.len());
    assert_eq!(u64::from_le_bytes(word), WORD);
}

/// THE HOSTILE-GUEST TEST OF THE MAPPING RULE: a guest that stores everywhere it can name changes only bytes that a
/// mapping allowing writes shows.
pub fn a_guest_writes_only_bytes_a_writable_mapping_shows<U: Unicorn<Seen>>() {
    // Code from page 0, read-only data from page 2, writable data from page 1, page 3 never mapped.
    let memory = new_memory::<U>(4);
    write_code::<U>(&memory, &[STORE_X1_AT_X0]);
    let code = read_all::<U>(&memory, 0..PAGE);
    let mut engine = engine::<U>();
    map_code(&mut engine, &memory);
    engine
        .mem_map_host_memory(DATA, PAGE, read_write(), &memory, PAGE)
        .expect("the writable page mapped");
    engine
        .mem_map_host_memory(DATA + PAGE, PAGE, Prot::READ, &memory, 2 * PAGE)
        .expect("the read-only page mapped");
    set(&mut engine, RegisterARM64::X1, WORD);
    let refused = [
        (CODE, uc_error::WRITE_PROT),
        (CODE + PAGE - 4, uc_error::WRITE_PROT),
        (DATA + PAGE, uc_error::WRITE_PROT),
        (DATA + 2 * PAGE - 8, uc_error::WRITE_PROT),
        (DATA + 2 * PAGE, uc_error::WRITE_UNMAPPED),
        (DATA + 3 * PAGE, uc_error::WRITE_UNMAPPED),
        (CODE - 8, uc_error::WRITE_UNMAPPED),
        (FAR, uc_error::WRITE_UNMAPPED),
    ];
    for (address, error) in refused {
        set(&mut engine, RegisterARM64::X0, address);
        assert_eq!(run_one(&mut engine), Err(error), "a store at {address:#x}");
    }
    // The one store allowed comes last, so that a hostile engine writes over the writable page before it.
    set(&mut engine, RegisterARM64::X0, DATA + PAGE - 8);
    run_one(&mut engine).expect("the store to the writable page ran");
    assert_eq!(read_all::<U>(&memory, 0..PAGE), code);
    assert_eq!(read_word::<U>(&memory, 2 * PAGE - 8), WORD);
    assert_eq!(
        read_all::<U>(&memory, 2 * PAGE..4 * PAGE),
        vec![u8::MIN; 2 * PAGE as usize]
    );
}

/// The interrupt hook sees the exception's number in the data, and its stop request ends the run.
pub fn an_interrupt_hook_sees_the_exception_and_stops_the_run<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(1);
    write_code::<U>(&memory, &[BREAKPOINT, move_into_x1(1)]);
    let mut engine = engine::<U>();
    map_code(&mut engine, &memory);
    stop_at_exceptions(&mut engine);
    engine
        .emu_start(CODE, u64::MAX, NO_TIMEOUT, NO_LIMIT)
        .expect("the run stopped by its hook");
    assert_eq!(engine.get_data().exceptions, vec![EXCEPTION_BREAKPOINT]);
    assert_eq!(get(&engine, RegisterARM64::X1), 0);
}

/// A removed hook is called no more.
pub fn a_removed_hook_is_not_called<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(1);
    write_code::<U>(&memory, &[NOP, BREAKPOINT]);
    let mut engine = engine::<U>();
    map_code(&mut engine, &memory);
    let hook = engine
        .add_block_hook(1, 0, |seen: &mut Seen, _: &mut Requests, address, size| {
            seen.blocks.push((address, size));
        })
        .expect("the block hook");
    stop_at_exceptions(&mut engine);
    engine
        .emu_start(CODE, u64::MAX, NO_TIMEOUT, NO_LIMIT)
        .expect("the run");
    // The block runs to the breakpoint, which ends it: two instructions of 4 bytes.
    assert_eq!(engine.get_data().blocks, vec![(CODE, 8)]);
    engine.remove_hook(hook).expect("the hook removed");
    engine
        .emu_start(CODE, u64::MAX, NO_TIMEOUT, NO_LIMIT)
        .expect("the run");
    assert_eq!(engine.get_data().blocks, vec![(CODE, 8)]);
    assert_eq!(
        engine.get_data().exceptions,
        vec![EXCEPTION_BREAKPOINT, EXCEPTION_BREAKPOINT]
    );
}

/// A memory hook on invalid stores sees the address, and its refusal ends the run with the store's error.
pub fn a_memory_hook_sees_invalid_stores<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(1);
    write_code::<U>(&memory, &[STORE_X1_AT_X0]);
    let mut engine = engine::<U>();
    map_code(&mut engine, &memory);
    engine
        .add_mem_hook(
            HookType::MEM_WRITE_UNMAPPED,
            1,
            0,
            |seen: &mut Seen, _: &mut Requests, kind, address, size, _| {
                assert_eq!((kind, size), (MemType::WRITE_UNMAPPED, size_of::<u64>()));
                seen.invalid.push(address);
                false
            },
        )
        .expect("the memory hook");
    set(&mut engine, RegisterARM64::X0, FAR + 16);
    assert_eq!(run_one(&mut engine), Err(uc_error::WRITE_UNMAPPED));
    assert_eq!(engine.get_data().invalid, vec![FAR + 16]);
}

/// Code our side rewrites runs once its translations are dropped, by `ctl_remove_cache`, by `ctl_flush_tb` or by a
/// hook's request, and the old translation runs until then.
pub fn rewritten_code_runs_once_its_translations_are_dropped<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(1);
    let mut engine = engine::<U>();
    map_code(&mut engine, &memory);
    stop_at_exceptions(&mut engine);
    let run_and_read = |engine: &mut U| {
        engine
            .emu_start(CODE, u64::MAX, NO_TIMEOUT, NO_LIMIT)
            .expect("the run to the breakpoint");
        get(engine, RegisterARM64::X1)
    };
    write_code::<U>(&memory, &[move_into_x1(1), BREAKPOINT]);
    assert_eq!(run_and_read(&mut engine), 1);
    write_code::<U>(&memory, &[move_into_x1(2), BREAKPOINT]);
    assert_eq!(run_and_read(&mut engine), 1, "the old translation runs");
    engine
        .ctl_remove_cache(CODE, CODE + PAGE)
        .expect("the translations dropped");
    assert_eq!(run_and_read(&mut engine), 2);
    write_code::<U>(&memory, &[move_into_x1(3), BREAKPOINT]);
    engine.ctl_flush_tb().expect("every translation dropped");
    assert_eq!(run_and_read(&mut engine), 3);
    // The hook drops the translations at the breakpoint, before the code changes.
    engine.get_data_mut().forget_on_exception = true;
    assert_eq!(run_and_read(&mut engine), 3);
    write_code::<U>(&memory, &[move_into_x1(4), BREAKPOINT]);
    assert_eq!(run_and_read(&mut engine), 4);
}

/// The TLB hook decides the physical page of each page of addresses once the TLB is unicorn's virtual one, and a flush
/// of the TLB asks it again.
pub fn the_tlb_hook_translates_addresses<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(3);
    write_code::<U>(&memory, &[LOAD_X1_FROM_X0]);
    write_word::<U>(&memory, PAGE, WORD);
    write_word::<U>(&memory, 2 * PAGE, !WORD);
    let mut engine = engine::<U>();
    map_code(&mut engine, &memory);
    engine
        .mem_map_host_memory(DATA, 2 * PAGE, Prot::READ, &memory, PAGE)
        .expect("the data pages mapped");
    engine
        .ctl_set_tlb_type(TlbType::VIRTUAL)
        .expect("unicorn's virtual TLB");
    engine
        .add_tlb_hook(1, 0, |seen: &mut Seen, _: &mut Requests, page, _| {
            let physical = match seen.redirect {
                Some((from, to)) if from == page => to,
                _ => page,
            };
            Some(TlbEntry {
                paddr: physical,
                perms: Prot::ALL,
            })
        })
        .expect("the TLB hook");
    engine.get_data_mut().redirect = Some((FAR, DATA));
    set(&mut engine, RegisterARM64::X0, FAR);
    run_one(&mut engine).expect("the load through the hook's page");
    assert_eq!(get(&engine, RegisterARM64::X1), WORD);
    engine.get_data_mut().redirect = Some((FAR, DATA + PAGE));
    engine.ctl_flush_tlb().expect("the TLB flushed");
    run_one(&mut engine).expect("the load through the hook's new page");
    assert_eq!(get(&engine, RegisterARM64::X1), !WORD);
}

/// A count of one runs one instruction.
pub fn a_count_of_one_runs_one_instruction<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(1);
    write_code::<U>(&memory, &[NOP, move_into_x1(3)]);
    let mut engine = engine::<U>();
    map_code(&mut engine, &memory);
    run_one(&mut engine).expect("one instruction");
    assert_eq!(get(&engine, RegisterARM64::PC), CODE + 4);
    assert_eq!(get(&engine, RegisterARM64::X1), 0);
}

/// A restored context puts back the registers it saved.
pub fn a_restored_context_puts_back_the_registers<U: Unicorn<Seen>>() {
    let mut engine = engine::<U>();
    set(&mut engine, RegisterARM64::X1, WORD);
    let context = engine.context_init().expect("the context saved");
    set(&mut engine, RegisterARM64::X1, !WORD);
    engine
        .context_restore(&context)
        .expect("the context restored");
    assert_eq!(get(&engine, RegisterARM64::X1), WORD);
}

/// A vector register takes and gives back 16 bytes, and a system register written by its encoding reads back by name.
pub fn long_and_system_registers_read_back<U: Unicorn<Seen>>() {
    let mut engine = engine::<U>();
    let vector: Vec<u8> = (1..=16).collect();
    engine
        .reg_write_long(RegisterARM64::Q3 as i32, &vector)
        .expect("Q3 written");
    assert_eq!(
        engine
            .reg_read_long(RegisterARM64::Q3 as i32)
            .expect("Q3 read")
            .to_vec(),
        vector
    );
    engine
        .reg_write_arm64_coproc(&THREAD_ID.val(WORD))
        .expect("TPIDR_EL0 written by its encoding");
    assert_eq!(get(&engine, RegisterARM64::TPIDR_EL0), WORD);
    let mut read = THREAD_ID;
    engine
        .reg_read_arm64_coproc(&mut read)
        .expect("TPIDR_EL0 read by its encoding");
    assert_eq!(read.val, WORD);
}

/// The CPU model set decides the instructions: the Cortex-A72, unicorn's default, has no LSE atomics, the max model has.
pub fn the_cpu_model_decides_the_instructions<U: Unicorn<Seen>>() {
    let memory = new_memory::<U>(2);
    write_code::<U>(&memory, &[ATOMIC_ADD_X1_INTO_X0_OLD_IN_X2]);
    write_word::<U>(&memory, PAGE, 40);
    let run = |model: Option<Arm64CpuModel>| {
        let mut engine = engine::<U>();
        if let Some(model) = model {
            engine
                .ctl_set_cpu_model(model as i32)
                .expect("the CPU model set");
        }
        map_code(&mut engine, &memory);
        engine
            .mem_map_host_memory(DATA, PAGE, read_write(), &memory, PAGE)
            .expect("the data page mapped");
        stop_at_exceptions(&mut engine);
        set(&mut engine, RegisterARM64::X0, DATA);
        set(&mut engine, RegisterARM64::X1, 2);
        // A hostile engine may write over the writable page before the run, so the old value is read back after.
        let ran = run_one(&mut engine);
        (
            ran,
            engine.get_data().exceptions.clone(),
            get(&engine, RegisterARM64::X2),
        )
    };
    let (_, exceptions, _) = run(None);
    assert_eq!(exceptions, vec![EXCEPTION_UNDEFINED]);
    let (ran, exceptions, old) = run(Some(Arm64CpuModel::MAX));
    assert_eq!((ran, exceptions), (Ok(()), Vec::new()));
    assert_eq!(read_word::<U>(&memory, PAGE), old.wrapping_add(2));
}

/// Runs every test of the suite on the implementation.
#[macro_export]
macro_rules! tests {
    ($unicorn:ty) => {
        #[test]
        fn host_memory_is_zeroed_whole_pages() {
            $crate::host_memory_is_zeroed_whole_pages::<$unicorn>();
        }

        #[test]
        fn a_guest_store_reaches_the_host_memory() {
            $crate::a_guest_store_reaches_the_host_memory::<$unicorn>();
        }

        #[test]
        fn a_guest_loads_what_the_host_wrote() {
            $crate::a_guest_loads_what_the_host_wrote::<$unicorn>();
        }

        #[test]
        fn a_mapping_shows_the_memory_from_its_offset() {
            $crate::a_mapping_shows_the_memory_from_its_offset::<$unicorn>();
        }

        #[test]
        fn mappings_outside_the_memory_are_refused() {
            $crate::mappings_outside_the_memory_are_refused::<$unicorn>();
        }

        #[test]
        fn regions_are_what_was_mapped() {
            $crate::regions_are_what_was_mapped::<$unicorn>();
        }

        #[test]
        fn a_store_to_a_mapping_refusing_writes_changes_nothing() {
            $crate::a_store_to_a_mapping_refusing_writes_changes_nothing::<$unicorn>();
        }

        #[test]
        fn protect_refuses_later_stores() {
            $crate::protect_refuses_later_stores::<$unicorn>();
        }

        #[test]
        fn unmapped_addresses_reach_no_host_memory() {
            $crate::unmapped_addresses_reach_no_host_memory::<$unicorn>();
        }

        #[test]
        fn two_engines_share_one_host_memory() {
            $crate::two_engines_share_one_host_memory::<$unicorn>();
        }

        #[test]
        fn a_view_outlives_its_engine() {
            $crate::a_view_outlives_its_engine::<$unicorn>();
        }

        #[test]
        fn a_guest_writes_only_bytes_a_writable_mapping_shows() {
            $crate::a_guest_writes_only_bytes_a_writable_mapping_shows::<$unicorn>();
        }

        #[test]
        fn an_interrupt_hook_sees_the_exception_and_stops_the_run() {
            $crate::an_interrupt_hook_sees_the_exception_and_stops_the_run::<$unicorn>();
        }

        #[test]
        fn a_removed_hook_is_not_called() {
            $crate::a_removed_hook_is_not_called::<$unicorn>();
        }

        #[test]
        fn a_memory_hook_sees_invalid_stores() {
            $crate::a_memory_hook_sees_invalid_stores::<$unicorn>();
        }

        #[test]
        fn rewritten_code_runs_once_its_translations_are_dropped() {
            $crate::rewritten_code_runs_once_its_translations_are_dropped::<$unicorn>();
        }

        #[test]
        fn the_tlb_hook_translates_addresses() {
            $crate::the_tlb_hook_translates_addresses::<$unicorn>();
        }

        #[test]
        fn a_count_of_one_runs_one_instruction() {
            $crate::a_count_of_one_runs_one_instruction::<$unicorn>();
        }

        #[test]
        fn a_restored_context_puts_back_the_registers() {
            $crate::a_restored_context_puts_back_the_registers::<$unicorn>();
        }

        #[test]
        fn long_and_system_registers_read_back() {
            $crate::long_and_system_registers_read_back::<$unicorn>();
        }

        #[test]
        fn the_cpu_model_decides_the_instructions() {
            $crate::the_cpu_model_decides_the_instructions::<$unicorn>();
        }
    };
}
