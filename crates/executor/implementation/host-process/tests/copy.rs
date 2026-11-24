//! A memory space's copy: at the same addresses its private pages are lent, as a fork lends them, and each side's
//! later stores stay its own; a copy within one space reads the bytes as they were before it.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::pin::pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll, Waker};

use wcontainers_abi::{Access, Permissions, SystemCallRequest};
use wcontainers_arm64::encode::{self, form};
use wcontainers_executor::memory_space::MemorySpace;
use wcontainers_executor::{
    Executor, InterruptResponse, MemoryFaultResponse, SystemCallResponse, TrapHandler,
};
use wcontainers_executor_host_process::executor::HostProcess;
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

fn run<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
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
fn storing_space(executor: &HostProcess) -> <HostProcess as Executor>::GuestMemory {
    let memory = executor.create_guest_memory().expect("a memory");
    let words = [
        encode::load_store(form::STR, 1, 0, 0, WORD).expect("str"),
        encode::exception(form::SVC, 0),
    ];
    let code: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    let bytes = memory.borrow(CODE..CODE + code.len() as u64).expect("code");
    assert_eq!(bytes.write(&code), code.len());
    memory
        .set_permissions(CODE..CODE + PAGE, Permissions::READ | Permissions::EXECUTE)
        .expect("code permissions");
    write_word(&memory, DATA, BEFORE);
    memory
        .set_permissions(DATA..DATA + PAGE, Permissions::READ | Permissions::WRITE)
        .expect("data permissions");
    memory
}

/// Runs the space's code once, storing `value` at `DATA`.
fn store(executor: &mut HostProcess, memory: &<HostProcess as Executor>::GuestMemory, value: u64) {
    let mut start = Processor {
        program_counter: CODE,
        ..ZERO_PROCESSOR
    };
    start.general_purpose_registers[0] = DATA;
    start.general_purpose_registers[1] = value;
    let mut handler = EndAtTheCall;
    let (execution, _) = executor.execute(start, memory, &mut handler);
    let stopped = run(execution);
    assert_eq!(
        stopped.program_counter,
        CODE + size_of::<u32>() as u64,
        "the run ends at its system call: {stopped:?}"
    );
}

#[test]
fn a_copy_at_the_same_addresses_lends_its_pages_and_each_side_s_stores_stay_its_own() {
    let mut executor = executor();
    let source = storing_space(&executor);
    let copy = executor.create_guest_memory().expect("a memory");
    copy.copy_from(CODE..DATA + PAGE, &source, CODE)
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

#[test]
fn a_copy_within_one_space_reads_its_bytes_as_they_were_before_it() {
    let executor = executor();
    let memory = executor.create_guest_memory().expect("a memory");
    for (page, value) in [(0, 10), (1, 11), (2, 12)] {
        write_word(&memory, DATA + page * PAGE, value);
    }
    memory
        .copy_from(DATA..DATA + 3 * PAGE, &memory, DATA)
        .expect("a copy onto itself");
    for (page, value) in [(0, 10), (1, 11), (2, 12)] {
        assert_eq!(
            read_word(&memory, DATA + page * PAGE),
            value,
            "page {page} unchanged"
        );
    }
    memory
        .copy_from(DATA + PAGE..DATA + 4 * PAGE, &memory, DATA)
        .expect("a copy one page up");
    for (page, value) in [(0, 10), (1, 10), (2, 11), (3, 12)] {
        assert_eq!(
            read_word(&memory, DATA + page * PAGE),
            value,
            "page {page} after the copy up"
        );
    }
    memory
        .copy_from(DATA..DATA + 3 * PAGE, &memory, DATA + PAGE)
        .expect("a copy one page down");
    for (page, value) in [(0, 10), (1, 11), (2, 12), (3, 12)] {
        assert_eq!(
            read_word(&memory, DATA + page * PAGE),
            value,
            "page {page} after the copy down"
        );
    }
}
