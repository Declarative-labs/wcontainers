//! A proven executor for tests, which runs one-byte instructions from a task's shards.

use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};

use vstd::prelude::*;
use wcontainers_kernel_core::execution::{Access, Interrupter, Stop};
use wcontainers_memory_capability_table::MemoryCapabilityTable;
use wcontainers_memory_shard::MemoryShard;
use wcontainers_volatile_bytes::VolatileBytes;

use crate::disjoint_memory::DisjointMemory;
use crate::executor_boundary::ExecutorBoundary;

verus! {

/// The instructions, each one byte; a byte not named here does nothing, as `Nothing` does.
pub enum Instruction {
    Nothing,
    /// Makes a system call, which returns to the next instruction.
    SystemCall,
    /// Adds one to the counter.
    Count,
}

/// How many bytes an instruction takes.
///
/// https://doc.rust-lang.org/reference/types/numeric.html#integer-types
pub const INSTRUCTION_LENGTH: u64 = 1;

/// The registers of the byte-code processor.
pub struct ByteCodeProcessor {
    pub program_counter: u64,
    pub counter: u64,
}

/// Runs byte code one instruction at a time, fetching each from a shard that allows executing it.
///
/// Every instruction moves the program counter up and no shard holds the last address, so every run ends.
pub struct ByteCodeExecutor;

pub struct FlagInterrupter {
    flag: Arc<AtomicBool>,
}

impl Interrupter for FlagInterrupter {
    fn interrupt(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }
}

/// Returns the byte at an address of a shard that allows executing it.
fn fetch<Shard: MemoryShard, Table: MemoryCapabilityTable<Shard>>(
    memory: &DisjointMemory<Shard, Table>,
    address: u64,
) -> (fetched: Option<u8>)
    ensures
        fetched is None ==> !crate::executor_boundary::permits(
            crate::disjoint_memory::to_shards(*memory),
            address,
            Access::Execute,
        ),
        fetched is Some ==> address < u64::MAX,
{
    proof {
        DisjointMemory::lemma_shards_are_disjoint(*memory);
    }
    match memory.find_shard(address) {
        Some((start, shard)) => {
            if shard.permissions().execute {
                let mut buffer = [u8::MIN];
                if let Ok(bytes) = shard.borrow(address - start, INSTRUCTION_LENGTH) {
                    bytes.read_at(u64::MIN, &mut buffer);
                }
                Some(buffer[usize::MIN])
            } else {
                proof {
                    let shards = crate::disjoint_memory::to_shards(*memory);
                    assert forall|other: u64| #[trigger]
                        shards.contains_key(other) && crate::disjoint_memory::contains_address(
                            shards,
                            other,
                            address,
                        ) implies !crate::executor_boundary::allows(
                        shards.index(other).spec_permissions(),
                        Access::Execute,
                    ) by {
                        if other != start {
                            assert(shards.contains_key(start));
                        }
                    }
                }
                None
            }
        },
        None => None,
    }
}

fn to_next(value: u64) -> u64 {
    if value < u64::MAX {
        value + 1
    } else {
        u64::MIN
    }
}

impl<Shard: MemoryShard, Table: MemoryCapabilityTable<Shard>> ExecutorBoundary<
    Shard,
    Table,
> for ByteCodeExecutor {
    type ProcessorState = ByteCodeProcessor;

    type Interruption = Arc<AtomicBool>;

    type Interrupter = FlagInterrupter;

    fn prepare_interruption() -> (prepared: (Arc<AtomicBool>, FlagInterrupter)) {
        let flag = Arc::new(AtomicBool::new(false));
        (flag.clone(), FlagInterrupter { flag })
    }

    fn run(
        &mut self,
        memory: &DisjointMemory<Shard, Table>,
        processor: &mut ByteCodeProcessor,
        interruption: Arc<AtomicBool>,
    ) -> (stop: Stop) {
        loop
            decreases u64::MAX - processor.program_counter,
        {
            if interruption.load(Ordering::SeqCst) {
                return Stop::Interrupt;
            }
            let address = processor.program_counter;
            let instruction = match fetch(memory, address) {
                Some(instruction) => instruction,
                None => {
                    return Stop::MemoryFault { address, access: Access::Execute };
                },
            };
            processor.program_counter = address + INSTRUCTION_LENGTH;
            if instruction == Instruction::SystemCall as u8 {
                return Stop::SystemCall;
            } else if instruction == Instruction::Count as u8 {
                processor.counter = to_next(processor.counter);
            }
        }
    }
}

} // verus!
