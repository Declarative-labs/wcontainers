//! Processes, threads and the ranges of a process's memory.

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

use wcontainers_abi::{MemoryId, Permissions, Resume, Stream, ThreadId};

pub(crate) type Pid = i32;
pub(crate) type Tid = i32;

/// One range of a process's memory, with what the program asked it to allow.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Mapping {
    pub start: u64,
    pub end: u64,
    pub permissions: Permissions,
}

/// What a file descriptor refers to.
#[derive(Clone, Copy, Debug)]
pub(crate) enum File {
    /// The null device: writes are taken whole and nothing is read from the buffer.
    Null,
    /// One of the container's standard output streams, handed to the host.
    HostStream(Stream),
}

pub(crate) struct Process {
    pub memory: MemoryId,
    pub threads: Vec<Tid>,
    /// Sorted by address and never overlapping.
    pub mappings: Vec<Mapping>,
    pub files: BTreeMap<u32, File>,
}

impl Process {
    pub fn new(memory: MemoryId, first_thread: Tid, mappings: Vec<Mapping>) -> Self {
        // The first process's descriptors as `docker run` without a terminal gives them.
        let files = BTreeMap::from([
            (0, File::Null),
            (1, File::HostStream(Stream::Output)),
            (2, File::HostStream(Stream::Error)),
        ]);
        Self {
            memory,
            threads: vec![first_thread],
            mappings,
            files,
        }
    }

    /// How many bytes from `address` on, up to `length`, the program may access as `wanted`.
    pub fn accessible(&self, address: u64, length: u64, wanted: Permissions) -> u64 {
        let mut at = address;
        let end = address.saturating_add(length);
        let first = self
            .mappings
            .partition_point(|mapping| mapping.end <= address);
        for mapping in &self.mappings[first..] {
            if at >= end || mapping.start > at || !mapping.permissions.contains(wanted) {
                break;
            }
            at = mapping.end.min(end);
        }
        at - address
    }
}

pub(crate) struct Thread {
    pub process: Pid,
    pub cpu: ThreadId,
    /// How the thread continues the next time it runs.
    pub resume: Resume,
}
