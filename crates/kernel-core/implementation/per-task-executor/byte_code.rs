//! A proven executor for tests, which runs one-byte instructions from a task's shards.

use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};

use vstd::prelude::*;
use wcontainers_capability_table::CapabilityTable;
use wcontainers_kernel_core::execution::{Access, Interrupter, Stop};
use wcontainers_kernel_core::memory::MemoryCapabilityTable;
use wcontainers_memory_shard::MemoryShard;
use wcontainers_volatile_bytes::VolatileBytes;

use crate::disjoint_memory::{DisjointMemory, contains_address};
use crate::executor_boundary::{ExecutorBoundary, allows, permits};

verus! {

/// Makes a system call, which returns to the next instruction.
pub const SYSTEM_CALL: u8 = 0;

/// Adds one to the counter.
pub const COUNT: u8 = 1;

/// Runs itself again, until something stops the run.
pub const SPIN: u8 = 2;

/// Writes the low byte of the counter to the data address.
pub const STORE: u8 = 3;

/// Reads the byte at the data address into the counter.
pub const LOAD: u8 = 4;

/// Sets the data address to the counter.
pub const ADDRESS: u8 = 5;

/// How many instructions one run executes at most before it ends as if interrupted, which bounds every run.
pub const STEP_LIMIT: u64 = 1 << 40;

/// The registers of the byte-code processor.
pub struct ByteCodeProcessor {
    pub program_counter: u64,
    pub counter: u64,
    pub data_address: u64,
}

/// Runs byte code one instruction at a time; any byte not listed does nothing.
pub struct ByteCodeExecutor;

pub struct FlagInterrupter {
    flag: Arc<AtomicBool>,
}

impl Interrupter for FlagInterrupter {
    fn interrupt(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }
}

pub enum AccessKind {
    Read,
    Write,
    Execute,
}

pub open spec fn to_access(kind: AccessKind) -> Access {
    match kind {
        AccessKind::Read => Access::Read,
        AccessKind::Write => Access::Write,
        AccessKind::Execute => Access::Execute,
    }
}

fn to_stop_access(kind: &AccessKind) -> (access: Access)
    ensures
        access == to_access(*kind),
{
    match kind {
        AccessKind::Read => Access::Read,
        AccessKind::Write => Access::Write,
        AccessKind::Execute => Access::Execute,
    }
}

fn find_permitted<'memory, Shard: MemoryShard, Table: MemoryCapabilityTable<Shard>>(
    memory: &'memory DisjointMemory<Shard, Table>,
    address: u64,
    kind: &AccessKind,
) -> (found: Option<(u64, &'memory Shard)>)
    ensures
        match found {
            Some((start, shard)) => memory.state().contains_key(start) && *shard
                == memory.state().index(start) && contains_address(memory.state(), start, address),
            None => !permits(memory.state(), address, to_access(*kind)),
        },
{
    proof {
        DisjointMemory::lemma_shards_are_disjoint(*memory);
    }
    match memory.find_shard(address) {
        Some((start, shard)) => {
            let permissions = shard.permissions();
            let allowed = match kind {
                AccessKind::Read => permissions.read,
                AccessKind::Write => permissions.write,
                AccessKind::Execute => permissions.execute,
            };
            if allowed {
                Some((start, shard))
            } else {
                proof {
                    let shards = memory.state();
                    assert forall|other: u64| #[trigger]
                        shards.contains_key(other) && contains_address(
                            shards,
                            other,
                            address,
                        ) implies !allows(shards.index(other).spec_permissions(), to_access(*kind))
                        by {
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

fn read_byte<Shard: MemoryShard, Table: MemoryCapabilityTable<Shard>>(
    memory: &DisjointMemory<Shard, Table>,
    address: u64,
    kind: AccessKind,
) -> (read: Option<u8>)
    ensures
        read is None ==> !permits(memory.state(), address, to_access(kind)),
{
    match find_permitted(memory, address, &kind) {
        Some((start, shard)) => {
            let mut buffer = [0u8; 1];
            match shard.borrow(address - start, 1) {
                Ok(bytes) => {
                    bytes.read_at(0, &mut buffer);
                },
                Err(_) => {},
            }
            Some(buffer[0])
        },
        None => None,
    }
}

fn write_byte<Shard: MemoryShard, Table: MemoryCapabilityTable<Shard>>(
    memory: &DisjointMemory<Shard, Table>,
    address: u64,
    byte: u8,
) -> (written: bool)
    ensures
        !written ==> !permits(memory.state(), address, Access::Write),
{
    match find_permitted(memory, address, &AccessKind::Write) {
        Some((start, shard)) => {
            let buffer = [byte; 1];
            match shard.borrow(address - start, 1) {
                Ok(bytes) => {
                    bytes.write_at(0, &buffer);
                },
                Err(_) => {},
            }
            true
        },
        None => false,
    }
}

fn to_next(address: u64) -> u64 {
    if address < u64::MAX {
        address + 1
    } else {
        0
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
        let mut steps: u64 = 0;
        while steps < STEP_LIMIT
            decreases STEP_LIMIT - steps,
        {
            if interruption.load(Ordering::SeqCst) {
                return Stop::Interrupt;
            }
            let address = processor.program_counter;
            let instruction = match read_byte(memory, address, AccessKind::Execute) {
                Some(instruction) => instruction,
                None => {
                    return Stop::MemoryFault { address, access: Access::Execute };
                },
            };
            if instruction == SYSTEM_CALL {
                processor.program_counter = to_next(address);
                return Stop::SystemCall;
            } else if instruction == COUNT {
                processor.counter = to_next(processor.counter);
                processor.program_counter = to_next(address);
            } else if instruction == STORE {
                let data = processor.data_address;
                if !write_byte(memory, data, (processor.counter % 256) as u8) {
                    return Stop::MemoryFault { address: data, access: Access::Write };
                }
                processor.program_counter = to_next(address);
            } else if instruction == LOAD {
                let data = processor.data_address;
                match read_byte(memory, data, AccessKind::Read) {
                    Some(byte) => {
                        processor.counter = byte as u64;
                    },
                    None => {
                        return Stop::MemoryFault { address: data, access: Access::Read };
                    },
                }
                processor.program_counter = to_next(address);
            } else if instruction == ADDRESS {
                processor.data_address = processor.counter;
                processor.program_counter = to_next(address);
            } else if instruction != SPIN {
                processor.program_counter = to_next(address);
            }
            steps += 1;
        }
        Stop::Interrupt
    }
}

} // verus!
