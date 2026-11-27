//! An engine whose guest is hostile: before each run it writes bytes of its own choosing over every byte a mapping lets
//! it write, which the mapping rule allows a guest, then runs as the engine it passes every call on to.

use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::ops::Range;

use wcontainers_unicorn::{
    Arch, HookType, MemRegion, MemType, Mode, Prot, RegisterARM64CP, Requests, TlbEntry, TlbType,
    Unicorn, uc_error,
};
use wcontainers_volatile_bytes::VolatileBytes;

/// The most bytes it writes at once.
const CHUNK: usize = 4096;
/// The state its bytes start from, any odd number: [xorshift64](https://www.jstatsoft.org/article/view/v008i14) never
/// leaves zero.
const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// A range of addresses mapped from host memory, by its start.
struct Mapped<Memory> {
    end: u64,
    perms: Prot,
    memory: Memory,
    /// Where in the memory the start lies.
    offset: u64,
}

/// The engine it passes calls on to, what it has mapped, and the state of the bytes it writes.
pub struct HostileUnicorn<Data: 'static, Inner: Unicorn<Data>> {
    inner: Inner,
    mapped: BTreeMap<u64, Mapped<Inner::HostMemory>>,
    state: u64,
    /// How many bytes it wrote over the guest's memory, all runs together.
    written: u64,
    data: PhantomData<fn() -> Data>,
}

impl<Data: 'static, Inner: Unicorn<Data>> HostileUnicorn<Data, Inner> {
    /// Returns how many bytes it wrote over the guest's memory, all runs together.
    pub fn written(&self) -> u64 {
        self.written
    }

    /// Cuts the mapped ranges at the address, so that none crosses it.
    fn cut_at(&mut self, address: u64) {
        let Some((start, mapped)) = self.mapped.range_mut(..address).next_back() else {
            return;
        };
        if mapped.end <= address {
            return;
        }
        let start = *start;
        let right = Mapped {
            end: mapped.end,
            perms: mapped.perms,
            memory: mapped.memory.clone(),
            offset: mapped.offset + (address - start),
        };
        mapped.end = address;
        self.mapped.insert(address, right);
    }

    /// Returns the starts of the mapped ranges inside the addresses, cut at its ends first.
    fn cut_out(&mut self, addresses: &Range<u64>) -> Vec<u64> {
        self.cut_at(addresses.start);
        self.cut_at(addresses.end);
        self.mapped
            .range(addresses.clone())
            .map(|(start, _)| *start)
            .collect()
    }

    /// Returns the next byte of its own choosing.
    fn next_byte(&mut self) -> u8 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state.to_le_bytes()[0]
    }

    /// Writes bytes of its own choosing over every byte of every mapping that allows writes.
    fn scribble(&mut self) {
        let writable: Vec<(u64, Inner::HostMemory, u64)> = self
            .mapped
            .iter()
            .filter(|(_, mapped)| mapped.perms.0 & Prot::WRITE.0 == Prot::WRITE.0)
            .map(|(start, mapped)| (mapped.offset, mapped.memory.clone(), mapped.end - start))
            .collect();
        let mut chunk = [u8::MIN; CHUNK];
        for (offset, memory, length) in writable {
            let mut done = u64::MIN;
            while done < length {
                let count = (length - done).min(CHUNK as u64);
                let filled = &mut chunk[..usize::try_from(count).expect("a chunk")];
                for byte in filled.iter_mut() {
                    *byte = self.next_byte();
                }
                let at = offset + done;
                let bytes = Inner::host_memory_bytes(&memory, at..at + count)
                    .expect("a mapping lies inside its host memory");
                self.written += bytes.write(filled) as u64;
                done += count;
            }
        }
    }
}

impl<Data: 'static, Inner: Unicorn<Data>> Unicorn<Data> for HostileUnicorn<Data, Inner> {
    type HostMemory = Inner::HostMemory;
    type Bytes = Inner::Bytes;
    type Context = Inner::Context;
    type HookId = Inner::HookId;

    fn new(arch: Arch, mode: Mode, data: Data) -> Result<Self, uc_error> {
        Ok(Self {
            inner: Inner::new(arch, mode, data)?,
            mapped: BTreeMap::new(),
            state: SEED,
            written: u64::MIN,
            data: PhantomData,
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
        self.inner
            .mem_map_host_memory(address, size, perms, memory, offset)?;
        self.mapped.insert(
            address,
            Mapped {
                end: address + size,
                perms,
                memory: memory.clone(),
                offset,
            },
        );
        Ok(())
    }

    fn mem_unmap(&mut self, address: u64, size: u64) -> Result<(), uc_error> {
        self.inner.mem_unmap(address, size)?;
        for start in self.cut_out(&(address..address.saturating_add(size))) {
            self.mapped.remove(&start);
        }
        Ok(())
    }

    fn mem_protect(&mut self, address: u64, size: u64, perms: Prot) -> Result<(), uc_error> {
        self.inner.mem_protect(address, size, perms)?;
        for start in self.cut_out(&(address..address.saturating_add(size))) {
            if let Some(mapped) = self.mapped.get_mut(&start) {
                mapped.perms = perms;
            }
        }
        Ok(())
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

    /// Writes over every byte the guest may write, then runs.
    fn emu_start(
        &mut self,
        begin: u64,
        until: u64,
        timeout: u64,
        count: usize,
    ) -> Result<(), uc_error> {
        self.scribble();
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
        self.inner.ctl_remove_cache(address, end)
    }

    fn ctl_flush_tb(&mut self) -> Result<(), uc_error> {
        self.inner.ctl_flush_tb()
    }

    fn ctl_flush_tlb(&mut self) -> Result<(), uc_error> {
        self.inner.ctl_flush_tlb()
    }

    fn ctl_set_tlb_type(&mut self, tlb_type: TlbType) -> Result<(), uc_error> {
        self.inner.ctl_set_tlb_type(tlb_type)
    }
}
