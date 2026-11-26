//! A page table's copy: at the same addresses its private pages are lent, as a fork lends them, and each side's later
//! stores stay its own.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::sync::{Arc, OnceLock};

use wcontainers_abi::{Access, Permissions, SystemCallRequest};
use wcontainers_arm64::encode::{self, form};
use wcontainers_executor::memory_space::{MemorySpace, PageTable};
use wcontainers_executor::{
    Execution, ExecutionStop, Executor, InterruptResponse, MemoryFaultResponse, SystemCallResponse,
    TrapHandler,
};
use wcontainers_executor_host_process::executor::{
    HostProcess, HostProcessMemory, HostProcessPageTable,
};
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_processor_architecture_arm64::{Processor, ZERO_PROCESSOR};
use wcontainers_volatile_bytes::VolatileBytes;

static HOST: OnceLock<Arc<HostProcessHost>> = OnceLock::new();

fn executor() -> HostProcess {
    HostProcess::new(Arc::clone(HOST.get_or_init(|| {
        Arc::new(HostProcessHost::new().expect("the host is refused"))
    })))
}

const CODE: u64 = 0x40_0000;
const DATA: u64 = 0x50_0000;
const PAGE: u64 = 4096;
const WORD: u64 = size_of::<u64>() as u64;
const BEFORE: u64 = 0x1111_2222_3333_4444;

/// Ends the run at its system call, the only trap the guest makes.
struct EndAtTheCall;

impl TrapHandler for EndAtTheCall {
    fn handle_system_call(&mut self, _: SystemCallRequest) -> SystemCallResponse {
        SystemCallResponse::Terminate
    }

    fn handle_memory_fault(&mut self, address: u64, access: Access) -> MemoryFaultResponse {
        panic!("the guest's store is allowed, yet faulted at {address:#x} ({access:?})")
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        InterruptResponse::Continue
    }
}

/// The processor an execution ended with: form P never waits.
fn ended<Wait>(stop: ExecutionStop<Processor<()>, Wait>) -> Processor<()> {
    match stop {
        ExecutionStop::Terminated(processor) => processor,
        ExecutionStop::Waiting { .. } => panic!("form P waited"),
    }
}

fn write_word(memory: &impl MemorySpace, address: u64, value: u64) {
    let bytes = memory.borrow(address..address + WORD).expect("a word");
    assert_eq!(bytes.write(&value.to_le_bytes()), WORD as usize);
}

fn read_word(memory: &impl MemorySpace, address: u64) -> u64 {
    let mut word = [0; WORD as usize];
    let bytes = memory.borrow(address..address + WORD).expect("a word");
    assert_eq!(bytes.read(&mut word), WORD as usize);
    u64::from_le_bytes(word)
}

/// A space whose code stores x1 at x0 and makes a system call, and whose data page holds `BEFORE`, writable.
fn storing_space(executor: &HostProcess) -> (HostProcessMemory, HostProcessPageTable) {
    let (memory, mut table) = executor.create_guest_memory().expect("a memory");
    let words = [
        encode::load_store(form::STR, 1, 0, 0, WORD).expect("str"),
        encode::exception(form::SVC, 0),
    ];
    let code: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    let bytes = memory.borrow(CODE..CODE + code.len() as u64).expect("code");
    assert_eq!(bytes.write(&code), code.len());
    table
        .set_permissions(CODE..CODE + PAGE, Permissions::READ | Permissions::EXECUTE)
        .expect("code permissions");
    write_word(&memory, DATA, BEFORE);
    table
        .set_permissions(DATA..DATA + PAGE, Permissions::READ | Permissions::WRITE)
        .expect("data permissions");
    (memory, table)
}

/// Runs the space's code once, storing `value` at `DATA`.
fn store(executor: &mut HostProcess, memory: &HostProcessMemory, value: u64) {
    let mut start = Processor {
        program_counter: CODE,
        ..ZERO_PROCESSOR
    };
    start.general_purpose_registers[0] = DATA;
    start.general_purpose_registers[1] = value;
    let (execution, _) = executor.prepare_execution(start, memory);
    let stopped = ended(execution.run(&mut EndAtTheCall));
    assert_eq!(
        stopped.program_counter,
        CODE + 2 * size_of::<u32>() as u64,
        "the run ends after its system call: {stopped:?}"
    );
}

#[test]
fn a_copy_at_the_same_addresses_lends_its_pages_and_each_side_s_stores_stay_its_own() {
    let mut executor = executor();
    let (source, source_table) = storing_space(&executor);
    let (copy, mut copy_table) = executor.create_guest_memory().expect("a memory");
    copy_table
        .copy_from(CODE..DATA + PAGE, &source_table, CODE)
        .expect("the copy");
    let before = executor.counters();
    assert_eq!(read_word(&copy, DATA), BEFORE, "the copy's bytes");

    store(&mut executor, &source, 1);
    let after_source = executor.counters();
    assert!(
        after_source.faults_resolved > before.faults_resolved
            && after_source.pages_copied > before.pages_copied,
        "the source's store found its page lent and copied it for the copy: {before:?} then {after_source:?}"
    );
    assert_eq!(read_word(&source, DATA), 1, "the source's store");
    assert_eq!(read_word(&copy, DATA), BEFORE, "the copy, as at the copy");

    store(&mut executor, &copy, 2);
    assert_eq!(read_word(&copy, DATA), 2, "the copy's store");
    assert_eq!(
        read_word(&source, DATA),
        1,
        "the source, unchanged by the copy's store"
    );
}

/// A copy refused for want of the source's host mappings leaves the copy reading none of the source's later stores:
/// the source loses write before any page is lent, or nothing is lent.
#[test]
fn a_refused_copy_lends_no_page_the_source_still_writes() {
    let limit = HOST
        .get_or_init(|| Arc::new(HostProcessHost::new().expect("the host is refused")))
        .probe()
        .mapping_limit();
    // gVisor gives 2^31 - 1, which no test reaches in reasonable time (`WCHP_MAPPING_LIMIT_MAX`).
    let reachable = std::env::var("WCHP_MAPPING_LIMIT_MAX")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1 << 22);
    if limit > reachable {
        eprintln!("not run: the host's limit of {limit} mappings is past {reachable}");
        return;
    }
    let mut executor = executor();
    let (source, mut source_table) = storing_space(&executor);
    store(&mut executor, &source, 1);
    // Pages of alternating permissions take the source's host mappings until one more change is refused, which the
    // memory space's `set_permissions` has no error for: it panics, and nothing is changed.
    let far = 0x1000_0000;
    let refused = (0..).find(|index: &u64| {
        let allowed = if index.is_multiple_of(2) {
            Permissions::READ | Permissions::WRITE
        } else {
            Permissions::READ
        };
        let at = far + index * PAGE;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            source_table.set_permissions(at..at + PAGE, allowed)
        }))
        .is_err()
    });
    assert!(refused.is_some());
    let (copy, mut copy_table) = executor.create_guest_memory().expect("a memory");
    assert!(
        copy_table
            .copy_from(CODE..DATA + PAGE, &source_table, CODE)
            .is_err(),
        "the copy is refused"
    );
    store(&mut executor, &source, 7);
    assert_eq!(read_word(&source, DATA), 7, "the source's store");
    assert_ne!(
        read_word(&copy, DATA),
        7,
        "the copy reads a store the source made after it"
    );
}
