//! Unicorn's own engine with host memory lent to it, under the mapping rule of the unicorn trait: each use of `unsafe`
//! is one marked line.

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::ops::Range;
use std::rc::Rc;

use memmap2::MmapMut;
use unicorn_engine::{Context, UcHookId};
use wcontainers_unicorn::{
    Arch, HookType, MemRegion, MemType, Mode, Prot, RegisterARM64CP, Requests, TlbEntry, TlbType,
    Unicorn, uc_error,
};
use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};
use wcontainers_volatile_bytes_pointer::{PointerAndLength, PointerBytes};

/// The page of unicorn's interface: every address, size and offset of a mapping is a multiple of it.
const PAGE: u64 = 4096;

/// An engine of this process's unicorn and the host memory each of its mappings holds.
///
/// The fields drop in order: the engine first, which closes it as nothing else holds its handle, then what it mapped.
pub struct DirectUnicorn<Data: 'static> {
    engine: unicorn_engine::Unicorn<'static, Data>,
    /// The addresses mapped from host memory, by their start, each with its end and the memory it shows.
    lent: BTreeMap<u64, (u64, DirectHostMemory)>,
}

/// Host memory: an anonymous mapping of this process, which no one reaches but through its pointer.
#[derive(Clone)]
pub struct DirectHostMemory(Rc<Allocation>);

struct Allocation {
    /// The mapping, kept only so that it is unmapped when the last holder goes; nothing forms a slice of it.
    _mapping: MmapMut,
    /// Its first byte, for unicorn.
    base: *mut u8,
    /// All its bytes, from which each view is cut.
    whole: PointerBytes,
}

impl DirectHostMemory {
    /// Returns how many hold the memory: this handle and its clones, each view of it, and each range of addresses an
    /// engine maps it at.
    pub fn holders(&self) -> usize {
        Rc::strong_count(&self.0)
    }
}

/// Bytes of a host memory, which keep it alive.
pub struct DirectBytes {
    bytes: PointerBytes,
    _keeps: Rc<Allocation>,
}

impl<Data: 'static> DirectUnicorn<Data> {
    /// Drops the record of every mapping over the addresses, which unicorn no longer maps, splitting those at its ends.
    fn forget_lent(&mut self, addresses: Range<u64>) {
        let overlapping: Vec<u64> = self
            .lent
            .range(..addresses.end)
            .filter(|(_, (end, _))| *end > addresses.start)
            .map(|(start, _)| *start)
            .collect();
        for start in overlapping {
            let (end, memory) = self.lent.remove(&start).expect("a mapping just listed");
            if start < addresses.start {
                self.lent.insert(start, (addresses.start, memory.clone()));
            }
            if end > addresses.end {
                self.lent.insert(addresses.end, (end, memory));
            }
        }
    }
}

/// Does what the hook asked once it returned.
fn answer<Data>(engine: &mut unicorn_engine::Unicorn<'_, Data>, requests: Requests) {
    for range in requests.forget_code {
        engine
            .ctl_remove_cache(range.start, range.end)
            .expect("dropping unicorn's translations");
    }
    if requests.stop {
        engine.emu_stop().expect("stopping unicorn");
    }
}

impl<Data: 'static> Unicorn<Data> for DirectUnicorn<Data> {
    type HostMemory = DirectHostMemory;
    type Bytes = DirectBytes;
    type Context = Context;
    type HookId = UcHookId;

    fn new(arch: Arch, mode: Mode, data: Data) -> Result<Self, uc_error> {
        Ok(Self {
            engine: unicorn_engine::Unicorn::new_with_data(arch, mode, data)?,
            lent: BTreeMap::new(),
        })
    }

    fn new_host_memory(length: u64) -> Result<DirectHostMemory, uc_error> {
        if length == u64::MIN || !length.is_multiple_of(PAGE) {
            return Err(uc_error::ARG);
        }
        let count = usize::try_from(length).map_err(|_| uc_error::NOMEM)?;
        let mut mapping = MmapMut::map_anon(count).map_err(|_| uc_error::NOMEM)?;
        let base = mapping.as_mut_ptr();
        #[expect(
            unsafe_code,
            reason = "the promise of the whole mapping's pointer and length: the anonymous mapping of `count` bytes at \
                      `base` is readable and writable from any thread and never protected, so reaching it never traps; \
                      it lives as long as `Allocation`, which every view cut from `whole` keeps alive; and nothing forms \
                      a slice or a reference over it, since `_mapping` is never dereferenced and unicorn reaches it only \
                      by its pointer"
        )]
        // SAFETY: as the reason says.
        let pointer = unsafe { PointerAndLength::new(base, count) };
        Ok(DirectHostMemory(Rc::new(Allocation {
            _mapping: mapping,
            base,
            whole: PointerBytes::new(pointer),
        })))
    }

    fn host_memory_bytes(memory: &DirectHostMemory, offsets: Range<u64>) -> Option<DirectBytes> {
        Some(DirectBytes {
            bytes: memory.0.whole.part(offsets)?,
            _keeps: Rc::clone(&memory.0),
        })
    }

    fn get_data(&self) -> &Data {
        self.engine.get_data()
    }

    fn get_data_mut(&mut self) -> &mut Data {
        self.engine.get_data_mut()
    }

    fn mem_map_host_memory(
        &mut self,
        address: u64,
        size: u64,
        perms: Prot,
        memory: &DirectHostMemory,
        offset: u64,
    ) -> Result<(), uc_error> {
        let fits = offset.is_multiple_of(PAGE)
            && offset
                .checked_add(size)
                .is_some_and(|end| end <= memory.0.whole.len());
        if !fits {
            return Err(uc_error::ARG);
        }
        let end = address.checked_add(size).ok_or(uc_error::ARG)?;
        let at = usize::try_from(offset).map_err(|_| uc_error::ARG)?;
        let pointer = memory.0.base.wrapping_add(at).cast::<c_void>();
        #[expect(
            unsafe_code,
            reason = "lends unicorn the `size` bytes of the host memory from `offset`, which lie inside it (checked \
                      above): the record pushed into `lent` below keeps the memory alive until `mem_unmap` ends every \
                      mapping of those addresses or the engine, dropped before `lent`, is closed; the engine's handle \
                      never leaves this type, so no copy of it outlives the record; our side reaches the bytes only by \
                      volatile copies, so a guest store breaks no reference"
        )]
        // SAFETY: as the reason says.
        let mapped = unsafe { self.engine.mem_map_ptr(address, size, perms, pointer) };
        mapped?;
        self.lent.insert(address, (end, memory.clone()));
        Ok(())
    }

    fn mem_unmap(&mut self, address: u64, size: u64) -> Result<(), uc_error> {
        self.engine.mem_unmap(address, size)?;
        // A failed unmap keeps every record, which keeps memory alive longer and never shorter.
        self.forget_lent(address..address.saturating_add(size));
        Ok(())
    }

    fn mem_protect(&mut self, address: u64, size: u64, perms: Prot) -> Result<(), uc_error> {
        self.engine.mem_protect(address, size, perms)
    }

    fn mem_regions(&self) -> Result<Vec<MemRegion>, uc_error> {
        self.engine.mem_regions()
    }

    fn add_intr_hook<Hook>(&mut self, mut hook: Hook) -> Result<UcHookId, uc_error>
    where
        Hook: FnMut(&mut Data, &mut Requests, u32) + 'static,
    {
        self.engine.add_intr_hook(move |engine, exception| {
            let mut requests = Requests::default();
            hook(engine.get_data_mut(), &mut requests, exception);
            answer(engine, requests);
        })
    }

    fn add_block_hook<Hook>(
        &mut self,
        begin: u64,
        end: u64,
        mut hook: Hook,
    ) -> Result<UcHookId, uc_error>
    where
        Hook: FnMut(&mut Data, &mut Requests, u64, u32) + 'static,
    {
        self.engine
            .add_block_hook(begin, end, move |engine, address, size| {
                let mut requests = Requests::default();
                hook(engine.get_data_mut(), &mut requests, address, size);
                answer(engine, requests);
            })
    }

    fn add_mem_hook<Hook>(
        &mut self,
        hook_type: HookType,
        begin: u64,
        end: u64,
        mut hook: Hook,
    ) -> Result<UcHookId, uc_error>
    where
        Hook: FnMut(&mut Data, &mut Requests, MemType, u64, usize, i64) -> bool + 'static,
    {
        self.engine.add_mem_hook(
            hook_type,
            begin,
            end,
            move |engine, kind, address, size, value| {
                let mut requests = Requests::default();
                let handled = hook(
                    engine.get_data_mut(),
                    &mut requests,
                    kind,
                    address,
                    size,
                    value,
                );
                answer(engine, requests);
                handled
            },
        )
    }

    fn add_tlb_hook<Hook>(
        &mut self,
        begin: u64,
        end: u64,
        mut hook: Hook,
    ) -> Result<UcHookId, uc_error>
    where
        Hook: FnMut(&mut Data, &mut Requests, u64, MemType) -> Option<TlbEntry> + 'static,
    {
        self.engine
            .add_tlb_hook(begin, end, move |engine, address, access| {
                let mut requests = Requests::default();
                let entry = hook(engine.get_data_mut(), &mut requests, address, access);
                answer(engine, requests);
                entry
            })
    }

    fn remove_hook(&mut self, hook: UcHookId) -> Result<(), uc_error> {
        self.engine.remove_hook(hook)
    }

    fn context_init(&self) -> Result<Context, uc_error> {
        self.engine.context_init()
    }

    fn context_restore(&self, context: &Context) -> Result<(), uc_error> {
        self.engine.context_restore(context)
    }

    fn emu_start(
        &mut self,
        begin: u64,
        until: u64,
        timeout: u64,
        count: usize,
    ) -> Result<(), uc_error> {
        self.engine.emu_start(begin, until, timeout, count)
    }

    fn reg_read(&self, register: i32) -> Result<u64, uc_error> {
        self.engine.reg_read(register)
    }

    fn reg_write(&mut self, register: i32, value: u64) -> Result<(), uc_error> {
        self.engine.reg_write(register, value)
    }

    fn reg_read_long(&self, register: i32) -> Result<Box<[u8]>, uc_error> {
        self.engine.reg_read_long(register)
    }

    fn reg_write_long(&mut self, register: i32, value: &[u8]) -> Result<(), uc_error> {
        self.engine.reg_write_long(register, value)
    }

    fn reg_read_arm64_coproc(&self, register: &mut RegisterARM64CP) -> Result<(), uc_error> {
        self.engine.reg_read_arm64_coproc(register)
    }

    fn reg_write_arm64_coproc(&mut self, register: &RegisterARM64CP) -> Result<(), uc_error> {
        self.engine.reg_write_arm64_coproc(register)
    }

    fn ctl_set_cpu_model(&mut self, cpu_model: i32) -> Result<(), uc_error> {
        self.engine.ctl_set_cpu_model(cpu_model)
    }

    fn ctl_remove_cache(&mut self, address: u64, end: u64) -> Result<(), uc_error> {
        self.engine.ctl_remove_cache(address, end)
    }

    fn ctl_flush_tb(&mut self) -> Result<(), uc_error> {
        self.engine.ctl_flush_tb()
    }

    fn ctl_flush_tlb(&mut self) -> Result<(), uc_error> {
        self.engine.ctl_flush_tlb()
    }

    fn ctl_set_tlb_type(&mut self, tlb_type: TlbType) -> Result<(), uc_error> {
        self.engine.ctl_set_tlb_type(tlb_type)
    }
}

/// Reads and writes from the view's start, as many bytes as the shorter of the view and the buffer holds.
impl VolatileBytes for DirectBytes {
    fn len(&self) -> u64 {
        self.bytes.len()
    }

    fn read(&self, into: &mut [u8]) -> usize {
        self.bytes.read(into)
    }

    fn write(&self, bytes: &[u8]) -> usize {
        self.bytes.write(bytes)
    }

    fn read_to<Sink: wcontainers_byte_sink::ByteSink>(
        &self,
        sink: &mut Sink,
    ) -> Result<usize, Sink::Error> {
        self.bytes.read_to(sink)
    }

    fn write_from<Source: wcontainers_byte_source::ByteSource>(
        &self,
        source: &mut Source,
    ) -> Result<usize, Source::Error> {
        self.bytes.write_from(source)
    }
}
