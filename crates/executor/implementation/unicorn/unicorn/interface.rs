//! What [unicorn](https://www.unicorn-engine.org/docs/)'s API leaves unsafe: host memory lent to an engine as its guest
//! memory (`uc_mem_map_ptr`), and the engine that must not outlive it.
//!
//! THE MAPPING RULE, which every implementation keeps and the conformance suite checks:
//!
//! 1. Who owns the bytes. Host memory is made by [`Unicorn::new_host_memory`] and owned by the implementation. Each
//!    engine that maps any of it holds it from [`Unicorn::mem_map_host_memory`] until no address of the engine maps it
//!    any more, and each view of it ([`Unicorn::host_memory_bytes`]) holds it while the view lives; it is freed after
//!    both. No caller ever holds a pointer, a slice or a reference to the bytes.
//! 2. When a mapping ends. At [`Unicorn::mem_unmap`] of its addresses, or when the engine is dropped. The engine maps no
//!    other memory than host memory, and its unicorn handle never leaves the implementation, not into a hook either, so
//!    no copy of the handle can keep the engine running over freed bytes.
//! 3. What a guest write can reach. A run ([`Unicorn::emu_start`]) changes a byte of host memory only where some
//!    mapping that allows writes shows that byte; a byte only mappings refusing writes show, a byte no mapping shows, and
//!    every byte outside host memory keep what they held.
//! 4. How our side reaches the bytes. Only through the [`VolatileBytes`] of a view, a copy at a time, never through a
//!    slice or a reference, so a guest that changes them breaks nothing on our side.
//!
//! Unicorn is GPL-2.0, so this crate and its implementations live inside the stand-in's folder and no crate outside it
//! links them.

use std::ops::Range;

pub use unicorn_engine::{
    Arch, Arm64CpuModel, Arm64Insn, HookType, MemRegion, MemType, Mode, Prot, RegisterARM64,
    RegisterARM64CP, TlbEntry, TlbType, uc_error,
};
use wcontainers_volatile_bytes::VolatileBytes;

/// What a hook asks of its engine besides its answer, done as soon as the hook returns.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Requests {
    /// Stops the run, as `uc_emu_stop` does.
    pub stop: bool,
    /// Drops what the engine translated from each range of addresses, as `UC_CTL_TB_REMOVE_CACHE` does.
    pub forget_code: Vec<Range<u64>>,
}

/// One unicorn engine with the caller's data, whose guest memory is all host memory lent to it, under the mapping rule.
///
/// Its methods are named and behave as the unicorn methods they stand for, except that a hook reaches only the data
/// and its [`Requests`], never the engine.
pub trait Unicorn<Data: 'static>: Sized {
    /// Host memory that engines may map, which every engine mapping it and every view of it keeps alive.
    type HostMemory: Clone;
    /// Bytes of a host memory, which a guest may change between two reads.
    type Bytes: VolatileBytes;
    /// A saved CPU state, as `uc_context`.
    type Context;
    /// A hook added to the engine, as `uc_hook`.
    type HookId: Copy;

    /// Returns an engine for the architecture and mode holding the data, as `uc_open`.
    fn new(arch: Arch, mode: Mode, data: Data) -> Result<Self, uc_error>;

    /// Returns zeroed host memory of the length, whole pages of 4 KiB; `ARG` for a length that is no whole page or none,
    /// `NOMEM` when the host has none.
    fn new_host_memory(length: u64) -> Result<Self::HostMemory, uc_error>;

    /// Returns a view of the host memory's bytes at the offsets, none for offsets past its end.
    fn host_memory_bytes(memory: &Self::HostMemory, offsets: Range<u64>) -> Option<Self::Bytes>;

    fn get_data(&self) -> &Data;

    fn get_data_mut(&mut self) -> &mut Data;

    /// Maps the host memory's bytes from the offset over the addresses from the address with the permissions, as
    /// `uc_mem_map_ptr`; `ARG` for bytes past the memory's end or an offset that is no whole page, and unicorn's own
    /// errors for an address, size or permission it refuses or addresses already mapped.
    fn mem_map_host_memory(
        &mut self,
        address: u64,
        size: u64,
        perms: Prot,
        memory: &Self::HostMemory,
        offset: u64,
    ) -> Result<(), uc_error>;

    fn mem_unmap(&mut self, address: u64, size: u64) -> Result<(), uc_error>;

    fn mem_protect(&mut self, address: u64, size: u64, perms: Prot) -> Result<(), uc_error>;

    fn mem_regions(&self) -> Result<Vec<MemRegion>, uc_error>;

    fn add_intr_hook<Hook>(&mut self, hook: Hook) -> Result<Self::HookId, uc_error>
    where
        Hook: FnMut(&mut Data, &mut Requests, u32) + 'static;

    fn add_block_hook<Hook>(
        &mut self,
        begin: u64,
        end: u64,
        hook: Hook,
    ) -> Result<Self::HookId, uc_error>
    where
        Hook: FnMut(&mut Data, &mut Requests, u64, u32) + 'static;

    fn add_mem_hook<Hook>(
        &mut self,
        hook_type: HookType,
        begin: u64,
        end: u64,
        hook: Hook,
    ) -> Result<Self::HookId, uc_error>
    where
        Hook: FnMut(&mut Data, &mut Requests, MemType, u64, usize, i64) -> bool + 'static;

    fn add_tlb_hook<Hook>(
        &mut self,
        begin: u64,
        end: u64,
        hook: Hook,
    ) -> Result<Self::HookId, uc_error>
    where
        Hook: FnMut(&mut Data, &mut Requests, u64, MemType) -> Option<TlbEntry> + 'static;

    /// Adds a hook called before each system instruction of the kind runs at the addresses, with the register it moves
    /// and the system register it names; the instruction is skipped when the hook returns true.
    fn add_insn_sys_hook_arm64<Hook>(
        &mut self,
        insn_type: Arm64Insn,
        begin: u64,
        end: u64,
        hook: Hook,
    ) -> Result<Self::HookId, uc_error>
    where
        Hook: FnMut(&mut Data, &mut Requests, RegisterARM64, &RegisterARM64CP) -> bool + 'static;

    fn remove_hook(&mut self, hook: Self::HookId) -> Result<(), uc_error>;

    fn context_init(&self) -> Result<Self::Context, uc_error>;

    fn context_restore(&self, context: &Self::Context) -> Result<(), uc_error>;

    fn emu_start(
        &mut self,
        begin: u64,
        until: u64,
        timeout: u64,
        count: usize,
    ) -> Result<(), uc_error>;

    fn reg_read(&self, register: i32) -> Result<u64, uc_error>;

    fn reg_write(&mut self, register: i32, value: u64) -> Result<(), uc_error>;

    fn reg_read_long(&self, register: i32) -> Result<Box<[u8]>, uc_error>;

    fn reg_write_long(&mut self, register: i32, value: &[u8]) -> Result<(), uc_error>;

    fn reg_read_arm64_coproc(&self, register: &mut RegisterARM64CP) -> Result<(), uc_error>;

    fn reg_write_arm64_coproc(&mut self, register: &RegisterARM64CP) -> Result<(), uc_error>;

    fn ctl_set_cpu_model(&mut self, cpu_model: i32) -> Result<(), uc_error>;

    /// Makes a run stop only at the exits set, of which there are none until set, instead of at its `until` address.
    fn ctl_exits_enable(&mut self) -> Result<(), uc_error>;

    fn ctl_remove_cache(&mut self, address: u64, end: u64) -> Result<(), uc_error>;

    fn ctl_flush_tb(&mut self) -> Result<(), uc_error>;

    fn ctl_flush_tlb(&mut self) -> Result<(), uc_error>;

    fn ctl_set_tlb_type(&mut self, tlb_type: TlbType) -> Result<(), uc_error>;
}
