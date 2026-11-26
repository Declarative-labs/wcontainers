#![cfg(target_arch = "aarch64")]
//! Changes of a running guest's space from another thread, a tagged store, and stops from other threads.

use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::Duration;

use wcontainers_abi::{Access, LOWEST_ADDRESS, PAGE_SIZE, Permissions};
use wcontainers_executor::memory_space::MemorySpace;
use wcontainers_executor::{
    ExecutionInterrupter, Executor, InterruptResponse, MemoryFaultResponse, SystemCallResponse,
    TrapHandler,
};
use wcontainers_executor_hardware_virtualization::single_step::SingleStepMachine;
use wcontainers_processor_architecture_arm64::{Processor, ZERO_PROCESSOR};
use wcontainers_volatile_bytes::VolatileBytes;

type Space = <SingleStepMachine as Executor>::GuestMemory;

const DATA: u64 = LOWEST_ADDRESS + PAGE_SIZE;
/// Where a third space's pages lie, away from the code and the data.
const ELSEWHERE: u64 = DATA + PAGE_SIZE * PAGE_SIZE;
/// The pages of the third space, which a frame let go of may land on.
const ELSEWHERE_PAGES: u64 = 64;
const STORED: u64 = 0x1122_3344_5566_7788;
const READ_WRITE: Permissions = Permissions::READ.union(Permissions::WRITE);
const READ_EXECUTE: Permissions = Permissions::READ.union(Permissions::EXECUTE);
/// How long the guest runs before another thread changes its space, and after, before it is interrupted.
const WHILE_RUNNING: Duration = Duration::from_millis(200);
/// The interrupts raised at a guest that never ends, one each [`WHILE_RUNNING`].
#[cfg(target_os = "linux")]
const INTERRUPTS: usize = 25;
/// The top byte of an address, which the guest's translations leave as a tag.
const TAG: u64 = 1 << (u64::BITS - u8::BITS);

/// `str x1, [x0]`.
const STORE: u32 = 0xf900_0001;
/// `brk #0`.
const BREAKPOINT: u32 = 0xd420_0000;
/// `str x1, [x0]` then `b` back to it: stores forever.
const STORE_FOREVER: [u32; 2] = [STORE, 0x17ff_ffff];
/// `sub x0, x0, #1`, `cbnz x0` back to it, then `brk #0`.
const COUNT_DOWN: [u32; 3] = [0xd100_0400, 0xb5ff_ffe0, BREAKPOINT];

struct ThreadWaker(thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        thread::park();
    }
}

/// Counts the traps, retries no fault and ends at the first interrupt.
#[derive(Default)]
struct Counter {
    faults: Vec<(u64, Access)>,
    interrupts: usize,
}

impl TrapHandler for Counter {
    fn handle_system_call(
        &mut self,
        _: wcontainers_abi::SystemCallRequest,
    ) -> SystemCallResponse {
        SystemCallResponse::Terminate
    }

    fn handle_memory_fault(&mut self, address: u64, access: Access) -> MemoryFaultResponse {
        self.faults.push((address, access));
        MemoryFaultResponse::Terminate
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        self.interrupts += 1;
        InterruptResponse::Terminate
    }
}

fn machine() -> SingleStepMachine {
    SingleStepMachine::new().expect("this host's hypervisor makes a machine")
}

fn space(machine: &SingleStepMachine) -> Space {
    machine.create_guest_memory().expect("a space")
}

fn to_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn place_code(space: &Space, words: &[u32]) {
    let bytes = to_bytes(words);
    let written = space
        .borrow(LOWEST_ADDRESS..LOWEST_ADDRESS + bytes.len() as u64)
        .expect("the code's addresses")
        .write(&bytes);
    assert_eq!(written, bytes.len());
    space
        .set_permissions(LOWEST_ADDRESS..LOWEST_ADDRESS + PAGE_SIZE, READ_EXECUTE)
        .expect("the code's page");
}

fn read_word(space: &Space, at: u64) -> u64 {
    let mut word = [u8::MIN; size_of::<u64>()];
    space
        .borrow(at..at + size_of::<u64>() as u64)
        .expect("the word's addresses")
        .read(&mut word);
    u64::from_le_bytes(word)
}

fn write_word(space: &Space, at: u64, word: u64) {
    space
        .borrow(at..at + size_of::<u64>() as u64)
        .expect("the word's addresses")
        .write(&word.to_le_bytes());
}

/// Returns a start that stores `STORED` at `DATA` forever.
fn storing_forever() -> Processor {
    let mut start = ZERO_PROCESSOR;
    start.program_counter = LOWEST_ADDRESS;
    start.general_purpose_registers[0] = DATA;
    start.general_purpose_registers[1] = STORED;
    start
}

/// Runs the guest until the other thread's work and its interrupt end it, and returns what the work returned.
fn run_beside<T: Send>(
    machine: &mut SingleStepMachine,
    start: Processor,
    space: &Space,
    handler: &mut Counter,
    work: impl FnOnce() -> T + Send,
) -> T {
    let (execution, interrupter) = machine.execute(start, space, handler);
    thread::scope(|scope| {
        let worker = scope.spawn(|| {
            thread::sleep(WHILE_RUNNING);
            let outcome = work();
            thread::sleep(WHILE_RUNNING);
            interrupter.interrupt();
            outcome
        });
        block_on(execution);
        worker.join().expect("the other thread's work")
    })
}

#[test]
fn removing_write_from_another_thread_faults_the_storing_guest() {
    let mut machine = machine();
    let space = space(&machine);
    place_code(&space, &STORE_FOREVER);
    space.set_permissions(DATA..DATA + PAGE_SIZE, READ_WRITE).unwrap();
    let mut handler = Counter::default();
    run_beside(&mut machine, storing_forever(), &space, &mut handler, || {
        space
            .set_permissions(DATA..DATA + PAGE_SIZE, Permissions::READ)
            .unwrap();
    });
    assert_eq!(handler.faults, [(DATA, Access::Write)]);
    assert_eq!(handler.interrupts, usize::MIN);
}

#[test]
fn unmap_from_another_thread_ends_the_guests_stores_into_the_other_space() {
    let mut machine = machine();
    let origin = space(&machine);
    let mapped = space(&machine);
    write_word(&origin, DATA, u64::MIN);
    mapped.map_from(DATA..DATA + PAGE_SIZE, &origin, DATA).unwrap();
    mapped.set_permissions(DATA..DATA + PAGE_SIZE, READ_WRITE).unwrap();
    place_code(&mapped, &STORE_FOREVER);
    let mut handler = Counter::default();
    let (stored, after) = run_beside(&mut machine, storing_forever(), &mapped, &mut handler, || {
        let stored = read_word(&origin, DATA);
        mapped.unmap_from(DATA..DATA + PAGE_SIZE).unwrap();
        write_word(&origin, DATA, u64::MIN);
        thread::sleep(WHILE_RUNNING);
        (stored, read_word(&origin, DATA))
    });
    assert_eq!(stored, STORED, "the guest stored through the mapping");
    assert_eq!(after, u64::MIN, "the guest stored into the other space after unmap_from returned");
}

#[test]
fn a_frame_let_go_of_while_the_guest_stores_never_reaches_a_third_space() {
    let mut machine = machine();
    let origin = space(&machine);
    let mapped = space(&machine);
    let third = space(&machine);
    write_word(&origin, DATA, u64::MIN);
    mapped.map_from(DATA..DATA + PAGE_SIZE, &origin, DATA).unwrap();
    mapped.set_permissions(DATA..DATA + PAGE_SIZE, READ_WRITE).unwrap();
    place_code(&mapped, &STORE_FOREVER);
    let mut handler = Counter::default();
    let pages = || (0..ELSEWHERE_PAGES).map(|page| ELSEWHERE + page * PAGE_SIZE);
    let reached: Vec<u64> = run_beside(&mut machine, storing_forever(), &mapped, &mut handler, || {
        mapped.unmap_from(DATA..DATA + PAGE_SIZE).unwrap();
        drop(origin);
        for page in pages() {
            write_word(&third, page, u64::MIN);
        }
        thread::sleep(WHILE_RUNNING);
        pages().filter(|&page| read_word(&third, page) == STORED).collect()
    });
    assert!(reached.is_empty(), "the guest stored into a third space's pages {reached:?}");
}

#[test]
fn a_tagged_store_to_a_page_that_allows_it_never_reaches_the_handler() {
    let mut machine = machine();
    let space = space(&machine);
    space.set_permissions(DATA..DATA + PAGE_SIZE, READ_WRITE).unwrap();
    place_code(&space, &[STORE, BREAKPOINT]);
    let mut start = storing_forever();
    start.general_purpose_registers[0] = TAG | DATA;
    let mut handler = Counter::default();
    let (execution, _) = machine.execute(start, &space, &mut handler);
    let stopped = block_on(execution);
    assert_eq!(handler.faults, []);
    assert_eq!(stopped.program_counter, LOWEST_ADDRESS + size_of::<u32>() as u64, "stopped at the breakpoint");
    assert_eq!(read_word(&space, DATA), STORED);
}

// Hypervisor.framework binds a virtual CPU to the thread that made it, so there the machine is not `Send`.
#[cfg(target_os = "linux")]
#[test]
fn a_machine_made_on_one_thread_is_interrupted_while_it_runs_on_another() {
    let machine = machine();
    let interrupts = thread::spawn(move || {
        let mut machine = machine;
        let space = space(&machine);
        place_code(&space, &COUNT_DOWN);
        let mut start = ZERO_PROCESSOR;
        start.program_counter = LOWEST_ADDRESS;
        start.general_purpose_registers[0] = u64::MAX;
        let mut handler = Counter::default();
        let (execution, interrupter) = machine.execute(start, &space, &mut handler);
        thread::scope(|scope| {
            scope.spawn(|| {
                for _ in 0..INTERRUPTS {
                    thread::sleep(WHILE_RUNNING);
                    interrupter.interrupt();
                }
            });
            block_on(execution);
        });
        handler.interrupts
    });
    assert_eq!(interrupts.join().expect("the moved machine's run"), 1);
}

#[test]
fn an_interrupt_after_the_end_leaves_the_next_single_step_alone() {
    let mut machine = machine();
    let space = space(&machine);
    place_code(&space, &COUNT_DOWN);
    let mut start = ZERO_PROCESSOR;
    start.program_counter = LOWEST_ADDRESS;
    start.general_purpose_registers[0] = 1;
    let mut handler = Counter::default();
    let (execution, interrupter) = machine.execute(start, &space, &mut handler);
    block_on(execution);
    interrupter.interrupt();
    let mut processor = ZERO_PROCESSOR;
    machine.execute_single_instruction_in_place(&mut processor, &[], |_| {});
    assert!(processor.exception.is_some(), "a fetch from a page that allows nothing faults");
    assert_eq!(handler.interrupts, usize::MIN);
}
