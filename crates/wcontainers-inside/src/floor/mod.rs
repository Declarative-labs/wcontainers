//! The floor: what runs under the kernel inside the virtual machine. Entry and exceptions, frames and page
//! tables, address spaces, interrupts and the timer, code made visible, and the link out.

pub mod entry;
pub mod tables;

use core::arch::asm;
use core::fmt::{self, Write};
use core::ptr::{self, NonNull};
use core::sync::atomic::{AtomicU64, Ordering};

use aarch64_cpu::asm::barrier::{self, ISH, SY};
use aarch64_cpu::registers::{
    CNTFRQ_EL0, CNTV_CTL_EL0, CNTV_CVAL_EL0, CNTVCT_EL0, MPIDR_EL1, Read, TPIDRRO_EL0, TTBR0_EL1,
    TTBR1_EL1, VBAR_EL1, Write as _,
};
use aarch64_paging::descriptor::El1Attributes;
use arm_gic::gicv3::registers::{Gicd, GicrSgi};
use arm_gic::gicv3::{GicCpuInterface, GicV3};
use arm_gic::{IntId, InterruptGroup};
use arm_sysregs::el0::accessors::read_ctr_el0;
use safe_mmio::UniqueMmioPointer;
use spin::Mutex;
use wcontainers_abi::PAGE_SIZE;

pub use entry::{Taken, UserRegisters, enter_user};
pub use tables::{Frames, Full, Table, space_full};

use crate::layout::{
    ATTRIBUTE_NORMAL, GIC_DISTRIBUTOR, GIC_REDISTRIBUTORS, MEMORY_BASE, Region, linear,
};
use crate::link::{Fatal, LINK_BYTES, Link, Message, Print, Start};

/// Guest-physical frames, shared by every virtual CPU.
pub static FRAMES: Mutex<Option<Frames>> = Mutex::new(None);

/// The guest-physical address of virtual CPU 0's link page; each next CPU's follows it.
static FIRST_LINK: AtomicU64 = AtomicU64::new(0);

/// Takes frames under the lock.
pub fn with_frames<T>(f: impl FnOnce(&mut Frames) -> T) -> T {
    f(FRAMES.lock().as_mut().expect("frames before boot"))
}

/// The running virtual CPU's index.
pub fn cpu() -> u64 {
    MPIDR_EL1.read(MPIDR_EL1::Aff0)
}

/// The running virtual CPU's link page.
fn link_page() -> &'static mut Link {
    let page = FIRST_LINK.load(Ordering::Relaxed) + cpu() * PAGE_SIZE;
    // SAFETY: each virtual CPU has its own link page, reached through the linear map.
    unsafe { &mut *(linear(page) as *mut Link) }
}

/// Asks the host with `message` and `bytes` as data, and gives back the answer.
pub fn ask<M: Message>(message: &M, bytes: &[u8]) -> M::Answer {
    let link = link_page();
    link.ask(message, bytes);
    doorbell();
    link.get()
}

/// Rings the doorbell: the virtual CPU leaves to the monitor and comes back answered.
fn doorbell() {
    barrier::dsb(SY);
    // SAFETY: the monitor answers in the link page and resumes at the next instruction.
    unsafe { asm!("hvc #0", options(nostack)) };
}

/// Text built in the link page and sent with one request.
pub struct Text {
    length: usize,
}

impl Write for Text {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let link = link_page();
        let take = s.len().min(LINK_BYTES - self.length);
        link.bytes[self.length..self.length + take].copy_from_slice(&s.as_bytes()[..take]);
        self.length += take;
        Ok(())
    }
}

/// Sends `message` with formatted text as its data.
pub fn send<M: Message>(message: &M, text: fmt::Arguments) {
    let mut out = Text { length: 0 };
    let _ = out.write_fmt(text);
    let link = link_page();
    link.ask(message, &[]);
    link.head.length = out.length as u64;
    doorbell();
}

/// Prints a line in the host's log.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => { $crate::floor::send(&$crate::link::Print {}, format_args!($($arg)*)) };
}

/// Reports a fault in the inside itself and stops.
#[unsafe(no_mangle)]
extern "C" fn floor_fatal(esr: u64, elr: u64, far: u64) -> ! {
    loop {
        ask(&Fatal { esr, elr, far }, &[]);
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    send(&Print {}, format_args!("inside panicked: {info}"));
    floor_fatal(0, 0, 0)
}

/// One virtual CPU's floor.
pub struct Floor {
    /// What the monitor said at start.
    pub start: Start,
    /// The inside's half as booted: the global entries every process shares.
    pub global: Table,
    /// The interrupt controller, on the virtual CPU that set it up.
    gic: Option<GicV3<'static>>,
}

/// Lowest priority, so every interrupt is let in.
const LOWEST_PRIORITY: u8 = u8::MAX;

/// The priority given to every interrupt the floor takes.
const INTERRUPT_PRIORITY: u8 = LOWEST_PRIORITY / 2;

impl Floor {
    /// Sets up the floor on this virtual CPU. The first one makes the frames and the global tables.
    pub fn boot(start: &Start) -> Floor {
        VBAR_EL1.set(entry::vectors());
        barrier::isb(SY);
        FIRST_LINK.store(start.link - start.cpu * PAGE_SIZE, Ordering::Relaxed);
        let global = Table {
            root: start.boot_root,
        };
        if start.cpu == 0 {
            *FRAMES.lock() = Some(Frames::new(start.free, MEMORY_BASE + start.memory));
            with_frames(|frames| global.reserve_top(frames, Region::Code.base()))
                .expect("the boot frames fit guest-physical memory");
        }
        Floor {
            start: *start,
            global,
            gic: None,
        }
    }

    /// Sets up the interrupt controller: the timer and `raised` host interrupts,
    /// each sent to this virtual CPU.
    pub fn interrupts(&mut self, raised: u32) {
        let distributor = NonNull::new(linear(GIC_DISTRIBUTOR) as *mut Gicd).expect("distributor");
        let redistributors =
            NonNull::new(linear(GIC_REDISTRIBUTORS) as *mut GicrSgi).expect("redistributors");
        // Only the redistributors of virtual CPUs up to this one are known to exist yet.
        let known = self.start.cpu as usize + 1;
        // SAFETY: the interrupt controller's registers are mapped as device memory through the linear map, and
        // nothing else uses them.
        let mut gic =
            unsafe { GicV3::new(UniqueMmioPointer::new(distributor), redistributors, known) }
                .expect("interrupt controller");
        let cpu = self.start.cpu as usize;
        gic.setup(cpu);
        let first = self.start.first_raised_interrupt as u32;
        let ids = core::iter::once(self.start.timer_interrupt as u32)
            .chain(first..first + raised)
            .map(|id| IntId::try_from(id).expect("interrupt number"));
        for id in ids {
            gic.set_interrupt_priority(id, Some(cpu), INTERRUPT_PRIORITY)
                .expect("priority");
            gic.enable_interrupt(id, Some(cpu), true).expect("enable");
        }
        GicCpuInterface::set_priority_mask(LOWEST_PRIORITY);
        self.gic = Some(gic);
    }

    /// Takes the interrupt that brought the guest's level back: acknowledges and ends it, giving its number.
    pub fn take_interrupt(&mut self) -> Option<u32> {
        let id = GicCpuInterface::get_and_acknowledge_interrupt(InterruptGroup::Group1)?;
        GicCpuInterface::end_interrupt(id, InterruptGroup::Group1);
        Some(id.into())
    }
}

/// The counter's ticks per second.
pub fn counter_frequency() -> u64 {
    CNTFRQ_EL0.get()
}

/// The virtual counter.
pub fn counter() -> u64 {
    barrier::isb(SY);
    CNTVCT_EL0.get()
}

/// Arms the virtual timer to fire when the counter reaches `at`.
pub fn arm_timer(at: u64) {
    CNTV_CVAL_EL0.set(at);
    CNTV_CTL_EL0.write(CNTV_CTL_EL0::ENABLE::SET + CNTV_CTL_EL0::IMASK::CLEAR);
}

/// Turns the virtual timer off.
pub fn disarm_timer() {
    CNTV_CTL_EL0.write(CNTV_CTL_EL0::ENABLE::CLEAR);
}

/// Sets the read-only thread register the guest's level reads (the native back end's stop word).
pub fn set_stop_word(value: u64) {
    TPIDRRO_EL0.set(value);
}

/// One process's view: its tables for the guest's half and for the inside's half, and its identifier.
#[derive(Clone, Copy, Debug)]
pub struct AddressSpace {
    pub user: Table,
    pub inside: Table,
    pub asid: u16,
}

impl AddressSpace {
    /// An empty process sharing the global entries of `global`.
    pub fn new(frames: &mut Frames, global: &Table, asid: u16) -> Result<AddressSpace, Full> {
        let inside = Table::new(frames)?;
        inside.share_top(global, Region::Linear.base());
        inside.share_top(global, Region::Code.base());
        Ok(AddressSpace {
            user: Table::new(frames)?,
            inside,
            asid,
        })
    }

    /// The same inside half with its own table for the guest's half and its own identifier: one thread's
    /// view of its process's pages.
    pub fn view(&self, frames: &mut Frames, asid: u16) -> Result<AddressSpace, Full> {
        Ok(AddressSpace {
            user: Table::new(frames)?,
            inside: self.inside,
            asid,
        })
    }

    /// Switches to this view. No TLB invalidation: entries are tagged by identifier.
    pub fn switch(&self) {
        let asid = u64::from(self.asid);
        TTBR0_EL1.write(
            TTBR0_EL1::ASID.val(asid)
                + TTBR0_EL1::BADDR.val(self.user.root >> TTBR0_EL1::BADDR.shift),
        );
        TTBR1_EL1.write(
            TTBR1_EL1::ASID.val(asid)
                + TTBR1_EL1::BADDR.val(self.inside.root >> TTBR1_EL1::BADDR.shift),
        );
        barrier::isb(SY);
    }
}

fn normal() -> El1Attributes {
    El1Attributes::VALID
        | El1Attributes::ACCESSED
        | El1Attributes::INNER_SHAREABLE
        | El1Attributes::from_bits_retain(
            (ATTRIBUTE_NORMAL as usize) << El1Attributes::ATTRIBUTE_INDEX_1.bits().trailing_zeros(),
        )
}

/// Guest memory the guest's level may read and write, never run.
pub fn guest_read_write() -> El1Attributes {
    normal()
        | El1Attributes::USER
        | El1Attributes::NON_GLOBAL
        | El1Attributes::UXN
        | El1Attributes::PXN
}

/// Guest memory the guest's level may only read.
pub fn guest_read_only() -> El1Attributes {
    guest_read_write() | El1Attributes::READ_ONLY
}

/// Translated code: run by the guest's level, never read or written by it. Global in the code area.
pub fn code() -> El1Attributes {
    normal() | El1Attributes::PXN
}

/// Code of one process only (the entry mirror).
pub fn process_code() -> El1Attributes {
    code() | El1Attributes::NON_GLOBAL
}

/// A register page: the guest's level reads and writes it; one process's only.
pub fn register_page() -> El1Attributes {
    guest_read_write()
}

/// One container's heap: the inside's only, one process's tables.
pub fn heap() -> El1Attributes {
    normal() | El1Attributes::NON_GLOBAL | El1Attributes::UXN | El1Attributes::PXN
}

/// Makes code written at `va` (and its `length` bytes) visible to instruction fetch on every virtual CPU.
pub fn make_code_visible(va: u64, length: u64) {
    let ctr = read_ctr_el0();
    let data_line = (size_of::<u32>() as u64) << ctr.dminline();
    let instruction_line = (size_of::<u32>() as u64) << ctr.iminline();
    let mut at = va & !(data_line - 1);
    while at < va + length {
        // SAFETY: cache maintenance only.
        unsafe { asm!("dc cvau, {0}", in(reg) at, options(nostack, preserves_flags)) };
        at += data_line;
    }
    barrier::dsb(ISH);
    let mut at = va & !(instruction_line - 1);
    while at < va + length {
        // SAFETY: cache maintenance only.
        unsafe { asm!("ic ivau, {0}", in(reg) at, options(nostack, preserves_flags)) };
        at += instruction_line;
    }
    barrier::dsb(ISH);
    barrier::isb(SY);
}

/// Copies `bytes` to guest-physical `frame` through the linear map.
pub fn write_frame(frame: u64, offset: u64, bytes: &[u8]) {
    // SAFETY: the frame is ours.
    unsafe {
        ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            (linear(frame) + offset) as *mut u8,
            bytes.len(),
        )
    };
}
