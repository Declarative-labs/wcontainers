//! The native back end's CPU side inside the machine (docs/native.md sections 5.2, 5.3 and 6.3): a process is
//! an address space with its own identifier, its pages kept as the kernel says; shared blocks; threads; pieces
//! translated through the link and loaded into the code area; and a thread run to its next exit.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::ptr;

use aarch64_cpu::registers::{ESR_EL1, FAR_EL1, Read, TPIDR_EL0, Write as _};
use aarch64_paging::descriptor::El1Attributes;
use wcontainers_abi::{
    Access, AddressSpace as Addresses, Cpu, CpuError, Exit, MemoryId, PAGE_SIZE, Permissions,
    RegisterBlock, Resume, SharedId, Syscall, ThreadId, Warning,
};
use wcontainers_arm64::mirror::REACH_BITS;
use wcontainers_arm64::thread_state::{FPCR_BITS, RegisterPage};
use wcontainers_guest_cpu::position::{NativeReason, Span};

use crate::floor::{
    self, AddressSpace, Full, Table, Taken, UserRegisters, code, enter_user, guest_no_access,
    guest_read_only, guest_read_write, inside_code, make_code_visible, with_frames,
};
use crate::layout::{Region, linear};
use crate::routines::{FloatControl, Placed, Routines, Vectors};
use crate::stop::{self, Exception, FloatStatus, GENERAL, GuestState, Interrupted, Resolved, Stop};
use crate::translation::{self, Translate};

/// The general register a system call's number is in (Linux's ARM64 calling convention).
const SYSCALL_NUMBER: usize = 8;

/// A slow path of the CPU side, apart from the translation's own ([`NativeReason`]).
const MEMORY_COPIED_AT_FORK: &str = "memory copied at once when duplicated";

/// A read of the float status while its flags are not kept, under the WebAssembly output's name.
const FLOAT_STATUS_NOT_KEPT: &str = "exact floating point requested";

/// One guest page as the CPU side keeps it.
#[derive(Clone, Copy, Debug)]
struct Page {
    frame: u64,
    permissions: Permissions,
    /// The block whose frame this is, when shared.
    shared: Option<SharedId>,
}

/// A piece loaded into the code area.
struct Loaded {
    base: u64,
    length: usize,
    spans: Vec<Span>,
}

struct Memory {
    space: AddressSpace,
    pages: BTreeMap<u64, Page>,
    /// Loaded pieces by the guest address they start at.
    pieces: BTreeMap<u64, Loaded>,
}

struct Thread {
    memory: MemoryId,
    state: GuestState,
    float: FloatStatus,
    page: RegisterPage,
    /// The register page's address and frame.
    register_page: u64,
    page_frame: u64,
    /// The vector registers while another thread has the hardware's.
    vectors: Vectors,
    tpidr: u64,
}

/// The native CPU side of one container.
pub struct NativeCpu {
    global: Table,
    memories: BTreeMap<MemoryId, Memory>,
    shared: BTreeMap<SharedId, Vec<u64>>,
    threads: BTreeMap<ThreadId, Thread>,
    next_id: u32,
    next_asid: u16,
    /// The code area's next free byte and the end of what is mapped of it.
    code_next: u64,
    code_mapped: u64,
    /// Whether pieces carry the progress count (the process asks for exact replay).
    exact_replay: bool,
    /// The thread whose vector registers the hardware holds.
    live: Option<ThreadId>,
    /// The float state routines.
    read_float_control: extern "C" fn() -> FloatControl,
    write_float_control: extern "C" fn(u64, u64),
    save_vectors: extern "C" fn(*mut Vectors),
    load_vectors: extern "C" fn(*const Vectors),
    counters: BTreeMap<&'static str, u64>,
    /// Slow paths not yet handed to a run's `warn`: reason, count and detail.
    unsent: Vec<(&'static str, u64, u64)>,
}

fn out_of_memory(_: Full) -> CpuError {
    CpuError::OutOfMemory
}

fn page_of(address: u64) -> u64 {
    address & !(PAGE_SIZE - 1)
}

/// A frame's bytes through the linear map.
fn frame_bytes(frame: u64) -> &'static mut [u8] {
    // SAFETY: the frame is the CPU side's, and only reached through the linear map.
    unsafe { core::slice::from_raw_parts_mut(linear(frame) as *mut u8, PAGE_SIZE as usize) }
}

/// The table attributes that give the guest's level what `permissions` allow of data. Code is never run from
/// guest pages: a page that may be run is read for translating.
fn attributes(permissions: Permissions) -> El1Attributes {
    if permissions.contains(Permissions::WRITE) {
        guest_read_write()
    } else if permissions.contains(Permissions::READ) {
        guest_read_only()
    } else {
        guest_no_access()
    }
}

/// The pages of a page-aligned range.
fn pages(address: u64, length: u64) -> impl Iterator<Item = u64> {
    (address..address + length).step_by(PAGE_SIZE as usize)
}

/// Reads guest memory through a page record, unmapped pages as zeros.
fn read_pages(pages: &BTreeMap<u64, Page>, mut address: u64, into: &mut [u8]) {
    let mut done = 0;
    while done < into.len() {
        let offset = (address - page_of(address)) as usize;
        let take = (PAGE_SIZE as usize - offset).min(into.len() - done);
        let part = &mut into[done..done + take];
        match pages.get(&page_of(address)) {
            Some(page) => part.copy_from_slice(&frame_bytes(page.frame)[offset..offset + take]),
            None => part.fill(0),
        }
        done += take;
        address += take as u64;
    }
}

impl NativeCpu {
    /// The CPU side over the global tables `global`; pieces carry the progress count when `exact_replay`.
    pub fn new(global: Table, exact_replay: bool) -> Result<NativeCpu, Full> {
        let frame = with_frames(|frames| frames.take())?;
        let placed: Placed = floor::ask(&Routines { at: frame }, &[]);
        let at = Region::Code.base();
        with_frames(|frames| global.map(frames, at, frame, inside_code()))?;
        make_code_visible(at, PAGE_SIZE);
        // SAFETY: the monitor wrote each routine at its place, with the C calling convention's signature
        // `Placed` names, and the frame is mapped for this level to run.
        let (read_float_control, write_float_control, save_vectors, load_vectors) = unsafe {
            (
                core::mem::transmute::<u64, extern "C" fn() -> FloatControl>(
                    at + placed.read_float_control,
                ),
                core::mem::transmute::<u64, extern "C" fn(u64, u64)>(
                    at + placed.write_float_control,
                ),
                core::mem::transmute::<u64, extern "C" fn(*mut Vectors)>(at + placed.save_vectors),
                core::mem::transmute::<u64, extern "C" fn(*const Vectors)>(
                    at + placed.load_vectors,
                ),
            )
        };
        Ok(NativeCpu {
            global,
            memories: BTreeMap::new(),
            shared: BTreeMap::new(),
            threads: BTreeMap::new(),
            next_id: 0,
            next_asid: 1,
            code_next: at + PAGE_SIZE,
            code_mapped: at + PAGE_SIZE,
            exact_replay,
            live: None,
            read_float_control,
            write_float_control,
            save_vectors,
            load_vectors,
            counters: BTreeMap::new(),
            unsent: Vec::new(),
        })
    }

    /// Every slow path taken so far, by reason.
    pub fn counters(&self) -> impl Iterator<Item = (&'static str, u64)> + '_ {
        self.counters
            .iter()
            .map(|(&reason, &count)| (reason, count))
    }

    fn slow(&mut self, reason: &'static str, times: u64, detail: u64) {
        let count = self.counters.entry(reason).or_default();
        *count += times;
        self.unsent.push((reason, *count, detail));
    }

    fn id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    fn memory(&mut self, memory: MemoryId) -> Result<&mut Memory, CpuError> {
        self.memories.get_mut(&memory).ok_or(CpuError::BadAddress)
    }

    fn check(&self, address: u64, length: u64) -> Result<(), CpuError> {
        match address.checked_add(length) {
            Some(end) if end <= self.address_space().end => Ok(()),
            _ => Err(CpuError::BadAddress),
        }
    }

    /// The frame of a guest page, made (allowing nothing) when the page has none.
    fn frame_for(&mut self, memory: MemoryId, page: u64) -> Result<u64, CpuError> {
        let m = self.memory(memory)?;
        if let Some(p) = m.pages.get(&page) {
            return Ok(p.frame);
        }
        let table = m.space.user;
        let frame = with_frames(|frames| {
            let frame = frames.take()?;
            table.map(frames, page, frame, guest_no_access())?;
            Ok(frame)
        })
        .map_err(out_of_memory)?;
        m.pages.insert(
            page,
            Page {
                frame,
                permissions: Permissions::empty(),
                shared: None,
            },
        );
        Ok(frame)
    }

    /// Points a mapped guest page at another frame, keeping its permissions.
    fn replace(&mut self, memory: MemoryId, page: u64, with: Page) -> Result<(), CpuError> {
        let m = self.memory(memory)?;
        let table = m.space.user;
        let attributes = attributes(with.permissions);
        with_frames(|frames| table.replace(frames, page, with.frame, attributes))
            .map_err(out_of_memory)?;
        m.pages.insert(page, with);
        Ok(())
    }

    /// Drops the pieces translated from a range.
    fn forget_code(&mut self, memory: MemoryId, address: u64, length: u64) {
        if let Some(m) = self.memories.get_mut(&memory) {
            m.pieces
                .retain(|&at, _| !(address..address + length).contains(&at));
        }
    }

    fn save_vectors(&mut self, thread: ThreadId) {
        let mut vectors: Vectors = Default::default();
        (self.save_vectors)(&mut vectors);
        if let Some(t) = self.threads.get_mut(&thread) {
            t.vectors = vectors;
        }
    }

    /// Gives `thread` the hardware's vector registers.
    fn take_hardware(&mut self, thread: ThreadId, vectors: &Vectors) {
        if self.live == Some(thread) {
            return;
        }
        if let Some(previous) = self.live {
            self.save_vectors(previous);
        }
        (self.load_vectors)(vectors);
        self.live = Some(thread);
    }

    /// Writes host code into the code area and makes it visible, giving its address.
    fn place(&mut self, code_bytes: &[u8]) -> Result<u64, Full> {
        let start = self.code_next;
        let end = start + code_bytes.len() as u64;
        let global = self.global;
        while self.code_mapped < end {
            let at = self.code_mapped;
            with_frames(|frames| {
                let frame = frames.take()?;
                global.map(frames, at, frame, code())
            })?;
            self.code_mapped += PAGE_SIZE;
        }
        // SAFETY: the code area is mapped writable at this level, and nothing runs this range yet.
        unsafe {
            ptr::copy_nonoverlapping(code_bytes.as_ptr(), start as *mut u8, code_bytes.len())
        };
        make_code_visible(start, end - start);
        self.code_next = end;
        Ok(start)
    }

    /// Makes sure a piece starting at guest `pc` is loaded, or gives the exit that running from there is.
    fn load(&mut self, memory: MemoryId, pc: u64) -> Result<(), Exit> {
        let m = &self.memories[&memory];
        if m.pieces.contains_key(&pc) {
            return Ok(());
        }
        if !pc.is_multiple_of(size_of::<u32>() as u64) {
            return Err(Exit::AlignmentFault { address: pc });
        }
        let refused = Exit::MemoryFault {
            address: pc,
            access: Access::Execute,
        };
        let page = m.pages.get(&page_of(pc)).ok_or(refused)?;
        if !page.permissions.contains(Permissions::EXECUTE) {
            return Err(refused);
        }
        let mut word = [0; size_of::<u32>()];
        read_pages(&m.pages, pc, &mut word);
        let piece = floor::ask_link(
            &Translate {
                address: pc,
                exact: u64::from(self.exact_replay),
            },
            &word,
            translation::piece,
        )
        .ok_or(Exit::IllegalInstruction)?;
        let base = self
            .place(&piece.code)
            .expect("the code area has room for the pieces");
        for reason in NativeReason::ALL {
            let sites = piece.sites[reason.index()];
            if sites != 0 {
                self.slow(reason.name(), sites, pc);
            }
        }
        self.memories
            .get_mut(&memory)
            .expect("the memory is there")
            .pieces
            .insert(
                pc,
                Loaded {
                    base,
                    length: piece.code.len(),
                    spans: piece.spans,
                },
            );
        Ok(())
    }

    /// Runs `thread` from `resume` to its next exit. `clock` reads the monotonic clock for the guest's counter
    /// reads; `warn` takes every slow path since the last run. An interrupt is [`Exit::Stop`], left pending for
    /// the caller to take.
    pub fn run(
        &mut self,
        thread: ThreadId,
        resume: Resume,
        clock: &mut dyn FnMut() -> u64,
        warn: &mut dyn FnMut(Warning<'_>),
    ) -> Exit {
        let mut t = self.threads.remove(&thread).expect("a thread of this side");
        if let Resume::Return(value) = resume {
            t.state.x[0] = value;
        }
        self.take_hardware(thread, &t.vectors.clone());
        let mut not_kept = 0;
        let exit = self.run_taken(&mut t, clock, &mut not_kept);
        if not_kept != 0 {
            self.slow(FLOAT_STATUS_NOT_KEPT, not_kept, t.state.pc);
        }
        self.threads.insert(thread, t);
        for (reason, count, detail) in core::mem::take(&mut self.unsent) {
            warn(Warning {
                reason,
                count,
                detail,
            });
        }
        exit
    }

    /// Runs `t` to its next exit, adding to `not_kept` each read of the float status while its flags are not
    /// kept.
    fn run_taken(
        &mut self,
        t: &mut Thread,
        clock: &mut dyn FnMut() -> u64,
        not_kept: &mut u64,
    ) -> Exit {
        let memory = t.memory;
        self.memories[&memory].space.switch();
        TPIDR_EL0.set(t.tpidr);
        loop {
            let pc = t.state.pc;
            if let Err(exit) = self.load(memory, pc) {
                return exit;
            }
            let m = &self.memories[&memory];
            let loaded = &m.pieces[&pc];
            let piece = stop::Piece {
                base: loaded.base,
                length: loaded.length,
                spans: &loaded.spans,
            };
            let (mut hardware, mut page) =
                stop::rejoin(&t.state, t.page.clone(), &t.float, t.register_page);
            hardware.pc = piece.base;
            let stop = loop {
                write_page(t.page_frame, &page);
                let mut user = UserRegisters {
                    x: hardware.x,
                    sp: hardware.sp,
                    pc: hardware.pc,
                    pstate: hardware.state,
                };
                (self.write_float_control)(u64::from(hardware.fpsr), u64::from(hardware.fpcr));
                let taken = enter_user(&mut user);
                let control = (self.read_float_control)();
                let h = Interrupted {
                    x: user.x,
                    sp: user.sp,
                    pc: user.pc,
                    state: user.pstate,
                    fpsr: control.fpsr as u32,
                    fpcr: control.fpcr as u32,
                };
                let held = read_page(t.page_frame);
                let mut memory_of = |at: u64, into: &mut [u8]| read_pages(&m.pages, at, into);
                match taken {
                    Taken::Synchronous => {
                        let exception = Exception {
                            syndrome: ESR_EL1.get(),
                            fault: FAR_EL1.get(),
                            hardware: &h,
                            page: &held,
                        };
                        match stop::resolve(&piece, &exception, &mut t.float, &mut memory_of) {
                            Ok(Resolved::Again(next, next_page)) => {
                                hardware = next;
                                page = next_page;
                            }
                            Ok(Resolved::FloatStatusNotKept(next, next_page)) => {
                                hardware = next;
                                page = next_page;
                                *not_kept += 1;
                            }
                            Ok(Resolved::Clock {
                                into,
                                then,
                                page: then_page,
                            }) => {
                                hardware = then;
                                page = then_page;
                                if let Some(register) = into {
                                    stop::set_register(register, clock(), &mut hardware, &mut page);
                                }
                            }
                            Ok(Resolved::Stop(stop, state)) => {
                                t.state = state;
                                t.page = held;
                                break stop;
                            }
                            Err(unexpected) => {
                                panic!("an exception translated code never raises: {unexpected:?}")
                            }
                        }
                    }
                    Taken::Interrupt => {
                        t.state =
                            stop::interrupted(&piece, &h, &held, &mut t.float, &mut memory_of)
                                .expect("an interrupt lands in a span");
                        t.page = held;
                        return Exit::Stop;
                    }
                    Taken::Other => {
                        panic!("a fast interrupt or a system error from the guest's level")
                    }
                }
            };
            match stop {
                Stop::Landed(_) => {}
                Stop::Syscall => {
                    let mut call = Syscall {
                        number: t.state.x[SYSCALL_NUMBER],
                        arguments: Default::default(),
                    };
                    let arguments = call.arguments.len();
                    call.arguments.copy_from_slice(&t.state.x[..arguments]);
                    return Exit::Syscall(call);
                }
                Stop::Breakpoint(comment) => return Exit::Breakpoint { comment },
                Stop::Illegal => return Exit::IllegalInstruction,
                Stop::Alignment(address) => return Exit::AlignmentFault { address },
                Stop::MemoryFault { address, write } => {
                    return Exit::MemoryFault {
                        address,
                        access: if write { Access::Write } else { Access::Read },
                    };
                }
            }
        }
    }
}

fn write_page(frame: u64, page: &RegisterPage) {
    // SAFETY: the register page's frame is the thread's; `RegisterPage` is `#[repr(C)]` words.
    unsafe { ptr::write_volatile(linear(frame) as *mut RegisterPage, page.clone()) };
}

fn read_page(frame: u64) -> RegisterPage {
    // SAFETY: as in `write_page`.
    unsafe { ptr::read_volatile(linear(frame) as *const RegisterPage) }
}

impl Cpu for NativeCpu {
    fn address_space(&self) -> Addresses {
        Addresses {
            end: 1 << REACH_BITS,
        }
    }

    fn memory_new(&mut self) -> Result<MemoryId, CpuError> {
        let asid = self.next_asid;
        self.next_asid = asid.checked_add(1).ok_or(CpuError::OutOfMemory)?;
        let global = self.global;
        let space = with_frames(|frames| AddressSpace::new(frames, &global, asid))
            .map_err(out_of_memory)?;
        let id = MemoryId(self.id());
        self.memories.insert(
            id,
            Memory {
                space,
                pages: BTreeMap::new(),
                pieces: BTreeMap::new(),
            },
        );
        Ok(id)
    }

    fn memory_duplicate(&mut self, source: MemoryId) -> Result<MemoryId, CpuError> {
        let copy = self.memory_new()?;
        let pages: Vec<(u64, Page)> = self
            .memory(source)?
            .pages
            .iter()
            .map(|(&a, &p)| (a, p))
            .collect();
        let table = self.memory(copy)?.space.user;
        let mut copied = 0;
        for &(at, page) in &pages {
            let frame = match page.shared {
                Some(_) => page.frame,
                None => {
                    let frame = with_frames(|frames| frames.take()).map_err(out_of_memory)?;
                    frame_bytes(frame).copy_from_slice(frame_bytes(page.frame));
                    copied += 1;
                    frame
                }
            };
            with_frames(|frames| table.map(frames, at, frame, attributes(page.permissions)))
                .map_err(out_of_memory)?;
            self.memory(copy)?.pages.insert(at, Page { frame, ..page });
        }
        self.slow(MEMORY_COPIED_AT_FORK, 1, copied);
        Ok(copy)
    }

    fn memory_drop(&mut self, memory: MemoryId) {
        let Some(m) = self.memories.remove(&memory) else {
            return;
        };
        with_frames(|frames| {
            for page in m.pages.values().filter(|p| p.shared.is_none()) {
                frames.give(page.frame);
            }
        });
    }

    fn read(&mut self, memory: MemoryId, address: u64, into: &mut [u8]) -> Result<(), CpuError> {
        self.check(address, into.len() as u64)?;
        read_pages(&self.memory(memory)?.pages, address, into);
        Ok(())
    }

    fn write(&mut self, memory: MemoryId, mut address: u64, from: &[u8]) -> Result<(), CpuError> {
        self.check(address, from.len() as u64)?;
        let mut done = 0;
        while done < from.len() {
            let offset = (address - page_of(address)) as usize;
            let take = (PAGE_SIZE as usize - offset).min(from.len() - done);
            let frame = self.frame_for(memory, page_of(address))?;
            frame_bytes(frame)[offset..offset + take].copy_from_slice(&from[done..done + take]);
            done += take;
            address += take as u64;
        }
        Ok(())
    }

    fn zero(&mut self, memory: MemoryId, address: u64, length: u64) -> Result<(), CpuError> {
        self.check(address, length)?;
        for at in pages(address, length) {
            let Some(page) = self.memory(memory)?.pages.get(&at).copied() else {
                continue;
            };
            match page.shared {
                None => frame_bytes(page.frame).fill(0),
                Some(_) => {
                    let frame = with_frames(|frames| frames.take()).map_err(out_of_memory)?;
                    self.replace(
                        memory,
                        at,
                        Page {
                            frame,
                            shared: None,
                            ..page
                        },
                    )?;
                }
            }
        }
        Ok(())
    }

    fn set_permissions(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
        permissions: Permissions,
    ) -> Result<(), CpuError> {
        self.check(address, length)?;
        if !permissions.contains(Permissions::EXECUTE) {
            self.forget_code(memory, address, length);
        }
        let m = self.memory(memory)?;
        let table = m.space.user;
        for at in pages(address, length) {
            match m.pages.get_mut(&at) {
                Some(page) => {
                    if attributes(page.permissions) != attributes(permissions) {
                        with_frames(|frames| table.protect(frames, at, attributes(permissions)))
                            .map_err(out_of_memory)?;
                    }
                    page.permissions = permissions;
                }
                None if permissions.is_empty() => {}
                None => {
                    let frame = with_frames(|frames| {
                        let frame = frames.take()?;
                        table.map(frames, at, frame, attributes(permissions))?;
                        Ok(frame)
                    })
                    .map_err(out_of_memory)?;
                    m.pages.insert(
                        at,
                        Page {
                            frame,
                            permissions,
                            shared: None,
                        },
                    );
                }
            }
        }
        Ok(())
    }

    fn code_changed(&mut self, memory: MemoryId, address: u64, length: u64, _starts: &[u64]) {
        self.forget_code(memory, address, length);
    }

    fn shared_new(&mut self, length: u64) -> Result<SharedId, CpuError> {
        let frames = with_frames(|frames| {
            pages(0, length)
                .map(|_| frames.take())
                .collect::<Result<Vec<u64>, Full>>()
        })
        .map_err(out_of_memory)?;
        let id = SharedId(self.id());
        self.shared.insert(id, frames);
        Ok(id)
    }

    fn shared_map(
        &mut self,
        shared: SharedId,
        offset: u64,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        self.check(address, length)?;
        let block = self.shared.get(&shared).ok_or(CpuError::BadAddress)?;
        let first = (offset / PAGE_SIZE) as usize;
        let frames: Vec<u64> = block
            .get(first..first + (length / PAGE_SIZE) as usize)
            .ok_or(CpuError::BadAddress)?
            .to_vec();
        for (at, frame) in pages(address, length).zip(frames) {
            let mapped = Page {
                frame,
                permissions: Permissions::empty(),
                shared: Some(shared),
            };
            match self.memory(memory)?.pages.get(&at).copied() {
                Some(page) => {
                    self.replace(
                        memory,
                        at,
                        Page {
                            permissions: page.permissions,
                            ..mapped
                        },
                    )?;
                    if page.shared.is_none() {
                        with_frames(|frames| frames.give(page.frame));
                    }
                }
                None => {
                    let table = self.memory(memory)?.space.user;
                    with_frames(|frames| table.map(frames, at, frame, guest_no_access()))
                        .map_err(out_of_memory)?;
                    self.memory(memory)?.pages.insert(at, mapped);
                }
            }
        }
        self.forget_code(memory, address, length);
        Ok(())
    }

    fn shared_unmap(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        self.check(address, length)?;
        for at in pages(address, length) {
            let Some(page) = self.memory(memory)?.pages.get(&at).copied() else {
                continue;
            };
            if page.shared.is_some() {
                let frame = with_frames(|frames| frames.take()).map_err(out_of_memory)?;
                frame_bytes(frame).copy_from_slice(frame_bytes(page.frame));
                self.replace(
                    memory,
                    at,
                    Page {
                        frame,
                        shared: None,
                        ..page
                    },
                )?;
            }
        }
        Ok(())
    }

    fn shared_read(
        &mut self,
        shared: SharedId,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), CpuError> {
        let block = self.shared.get(&shared).ok_or(CpuError::BadAddress)?;
        let mut done = 0;
        while done < into.len() {
            let at = offset + done as u64;
            let within = (at % PAGE_SIZE) as usize;
            let take = (PAGE_SIZE as usize - within).min(into.len() - done);
            let frame = *block
                .get((at / PAGE_SIZE) as usize)
                .ok_or(CpuError::BadAddress)?;
            into[done..done + take].copy_from_slice(&frame_bytes(frame)[within..within + take]);
            done += take;
        }
        Ok(())
    }

    fn shared_write(&mut self, shared: SharedId, offset: u64, from: &[u8]) -> Result<(), CpuError> {
        let block = self.shared.get(&shared).ok_or(CpuError::BadAddress)?;
        let mut done = 0;
        while done < from.len() {
            let at = offset + done as u64;
            let within = (at % PAGE_SIZE) as usize;
            let take = (PAGE_SIZE as usize - within).min(from.len() - done);
            let frame = *block
                .get((at / PAGE_SIZE) as usize)
                .ok_or(CpuError::BadAddress)?;
            frame_bytes(frame)[within..within + take].copy_from_slice(&from[done..done + take]);
            done += take;
        }
        Ok(())
    }

    fn shared_drop(&mut self, shared: SharedId) {
        if let Some(block) = self.shared.remove(&shared) {
            with_frames(|frames| block.into_iter().for_each(|frame| frames.give(frame)));
        }
    }

    fn thread_new(&mut self, memory: MemoryId) -> Result<ThreadId, CpuError> {
        let id = ThreadId(self.id());
        let register_page = Region::Threads.base() + u64::from(id.0) * PAGE_SIZE;
        let inside = self.memory(memory)?.space.inside;
        let page_frame = with_frames(|frames| {
            let frame = frames.take()?;
            inside.map(frames, register_page, frame, floor::register_page())?;
            Ok(frame)
        })
        .map_err(out_of_memory)?;
        self.threads.insert(
            id,
            Thread {
                memory,
                state: GuestState {
                    x: [0; GENERAL],
                    sp: 0,
                    pc: 0,
                    flags: 0,
                    count: 0,
                    exclusive: Default::default(),
                },
                float: FloatStatus::default(),
                page: RegisterPage::default(),
                register_page,
                page_frame,
                vectors: Default::default(),
                tpidr: 0,
            },
        );
        Ok(id)
    }

    fn thread_drop(&mut self, thread: ThreadId) {
        let Some(t) = self.threads.remove(&thread) else {
            return;
        };
        if self.live == Some(thread) {
            self.live = None;
        }
        if let Some(m) = self.memories.get(&t.memory) {
            let inside = m.space.inside;
            inside.unmap(t.register_page);
        }
        with_frames(|frames| frames.give(t.page_frame));
    }

    fn registers(&mut self, thread: ThreadId, into: &mut RegisterBlock) {
        if self.live == Some(thread) {
            self.save_vectors(thread);
        }
        let t = &self.threads[&thread];
        into.x = t.state.x;
        into.sp = t.state.sp;
        into.pc = t.state.pc;
        into.pstate = t.state.flags;
        into.v = t.vectors;
        into.fpsr = t.float.fpsr;
        into.fpcr = t.float.fpcr;
        into.tpidr_el0 = t.tpidr;
    }

    fn set_registers(&mut self, thread: ThreadId, from: &RegisterBlock) {
        let t = self
            .threads
            .get_mut(&thread)
            .expect("a thread of this side");
        t.state.x = from.x;
        t.state.sp = from.sp;
        t.state.pc = from.pc;
        t.state.flags = from.pstate;
        t.vectors = from.v;
        t.float.fpsr = from.fpsr;
        t.float.fpcr = from.fpcr & FPCR_BITS;
        t.tpidr = from.tpidr_el0;
        if self.live == Some(thread) {
            (self.load_vectors)(&from.v);
        }
    }
}
