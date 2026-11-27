//! An engine that records each call that changes its memory or runs it, and passes every call on to another engine.

use std::ops::Range;

use wcontainers_unicorn::{
    Arch, HookType, MemRegion, MemType, Mode, Prot, RegisterARM64CP, Requests, TlbEntry, TlbType,
    Unicorn, uc_error,
};

/// A call that changes an engine's memory, its translations or runs it, with its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    MemMapHostMemory {
        address: u64,
        size: u64,
        perms: u32,
        offset: u64,
    },
    MemUnmap {
        address: u64,
        size: u64,
    },
    MemProtect {
        address: u64,
        size: u64,
        perms: u32,
    },
    EmuStart {
        begin: u64,
        count: usize,
    },
    CtlRemoveCache {
        address: u64,
        end: u64,
    },
    CtlFlushTb,
}

/// The engine it passes calls on to, and the calls it recorded, in order.
pub struct RecordingUnicorn<Inner> {
    inner: Inner,
    calls: Vec<Call>,
}

impl<Inner> RecordingUnicorn<Inner> {
    /// Returns the calls recorded since the last take, in order, and forgets them.
    pub fn take_calls(&mut self) -> Vec<Call> {
        std::mem::take(&mut self.calls)
    }
}

impl<Data: 'static, Inner: Unicorn<Data>> Unicorn<Data> for RecordingUnicorn<Inner> {
    type HostMemory = Inner::HostMemory;
    type Bytes = Inner::Bytes;
    type Context = Inner::Context;
    type HookId = Inner::HookId;

    fn new(arch: Arch, mode: Mode, data: Data) -> Result<Self, uc_error> {
        Ok(Self {
            inner: Inner::new(arch, mode, data)?,
            calls: Vec::new(),
        })
    }

    fn new_host_memory(length: u64) -> Result<Inner::HostMemory, uc_error> {
        Inner::new_host_memory(length)
    }

    fn host_memory_bytes(memory: &Inner::HostMemory, offsets: Range<u64>) -> Option<Inner::Bytes> {
        Inner::host_memory_bytes(memory, offsets)
    }

    fn get_data(&self) -> &Data {
        self.inner.get_data()
    }

    fn get_data_mut(&mut self) -> &mut Data {
        self.inner.get_data_mut()
    }

    fn mem_map_host_memory(
        &mut self,
        address: u64,
        size: u64,
        perms: Prot,
        memory: &Inner::HostMemory,
        offset: u64,
    ) -> Result<(), uc_error> {
        self.calls.push(Call::MemMapHostMemory {
            address,
            size,
            perms: perms.0,
            offset,
        });
        self.inner
            .mem_map_host_memory(address, size, perms, memory, offset)
    }

    fn mem_unmap(&mut self, address: u64, size: u64) -> Result<(), uc_error> {
        self.calls.push(Call::MemUnmap { address, size });
        self.inner.mem_unmap(address, size)
    }

    fn mem_protect(&mut self, address: u64, size: u64, perms: Prot) -> Result<(), uc_error> {
        self.calls.push(Call::MemProtect {
            address,
            size,
            perms: perms.0,
        });
        self.inner.mem_protect(address, size, perms)
    }

    fn mem_regions(&self) -> Result<Vec<MemRegion>, uc_error> {
        self.inner.mem_regions()
    }

    fn add_intr_hook<Hook>(&mut self, hook: Hook) -> Result<Inner::HookId, uc_error>
    where
        Hook: FnMut(&mut Data, &mut Requests, u32) + 'static,
    {
        self.inner.add_intr_hook(hook)
    }

    fn add_block_hook<Hook>(
        &mut self,
        begin: u64,
        end: u64,
        hook: Hook,
    ) -> Result<Inner::HookId, uc_error>
    where
        Hook: FnMut(&mut Data, &mut Requests, u64, u32) + 'static,
    {
        self.inner.add_block_hook(begin, end, hook)
    }

    fn add_mem_hook<Hook>(
        &mut self,
        hook_type: HookType,
        begin: u64,
        end: u64,
        hook: Hook,
    ) -> Result<Inner::HookId, uc_error>
    where
        Hook: FnMut(&mut Data, &mut Requests, MemType, u64, usize, i64) -> bool + 'static,
    {
        self.inner.add_mem_hook(hook_type, begin, end, hook)
    }

    fn add_tlb_hook<Hook>(
        &mut self,
        begin: u64,
        end: u64,
        hook: Hook,
    ) -> Result<Inner::HookId, uc_error>
    where
        Hook: FnMut(&mut Data, &mut Requests, u64, MemType) -> Option<TlbEntry> + 'static,
    {
        self.inner.add_tlb_hook(begin, end, hook)
    }

    fn remove_hook(&mut self, hook: Inner::HookId) -> Result<(), uc_error> {
        self.inner.remove_hook(hook)
    }

    fn context_init(&self) -> Result<Inner::Context, uc_error> {
        self.inner.context_init()
    }

    fn context_restore(&self, context: &Inner::Context) -> Result<(), uc_error> {
        self.inner.context_restore(context)
    }

    fn emu_start(
        &mut self,
        begin: u64,
        until: u64,
        timeout: u64,
        count: usize,
    ) -> Result<(), uc_error> {
        self.calls.push(Call::EmuStart { begin, count });
        self.inner.emu_start(begin, until, timeout, count)
    }

    fn reg_read(&self, register: i32) -> Result<u64, uc_error> {
        self.inner.reg_read(register)
    }

    fn reg_write(&mut self, register: i32, value: u64) -> Result<(), uc_error> {
        self.inner.reg_write(register, value)
    }

    fn reg_read_long(&self, register: i32) -> Result<Box<[u8]>, uc_error> {
        self.inner.reg_read_long(register)
    }

    fn reg_write_long(&mut self, register: i32, value: &[u8]) -> Result<(), uc_error> {
        self.inner.reg_write_long(register, value)
    }

    fn reg_read_arm64_coproc(&self, register: &mut RegisterARM64CP) -> Result<(), uc_error> {
        self.inner.reg_read_arm64_coproc(register)
    }

    fn reg_write_arm64_coproc(&mut self, register: &RegisterARM64CP) -> Result<(), uc_error> {
        self.inner.reg_write_arm64_coproc(register)
    }

    fn ctl_set_cpu_model(&mut self, cpu_model: i32) -> Result<(), uc_error> {
        self.inner.ctl_set_cpu_model(cpu_model)
    }

    fn ctl_remove_cache(&mut self, address: u64, end: u64) -> Result<(), uc_error> {
        self.calls.push(Call::CtlRemoveCache { address, end });
        self.inner.ctl_remove_cache(address, end)
    }

    fn ctl_flush_tb(&mut self) -> Result<(), uc_error> {
        self.calls.push(Call::CtlFlushTb);
        self.inner.ctl_flush_tb()
    }

    fn ctl_flush_tlb(&mut self) -> Result<(), uc_error> {
        self.inner.ctl_flush_tlb()
    }

    fn ctl_set_tlb_type(&mut self, tlb_type: TlbType) -> Result<(), uc_error> {
        self.inner.ctl_set_tlb_type(tlb_type)
    }
}
