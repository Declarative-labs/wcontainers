//! What translated code and the inside both know of a native thread's state (docs/native.md section 3.3):
//! the withheld registers, the register page, and the bits of the guest's float status and addresses.

/// A register number: 0 to 30, or 31 for the zero register or the stack pointer as the form says.
pub type Register = u8;

/// The progress count of definition E (section 4).
pub const COUNT: Register = 18;
/// The scratch register t.
pub const SCRATCH: Register = 23;
/// The thread's register page, which the guest's level reads and writes.
pub const PAGE: Register = 28;
/// The withheld registers, in the order the register page keeps their guest values: the three general registers
/// named least by the benchmark set's executed instructions and by real programs' code (docs/native.md section 3.3).
pub const WITHHELD: [Register; 3] = [COUNT, SCRATCH, PAGE];

/// Guest registers one sequence moves or borrows at most at once: `casp`'s two even pairs.
pub const BORROWED: usize = 4;

/// The thread's register page (section 3.3).
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegisterPage {
    /// The guest's values of the withheld registers.
    pub withheld: [u64; WITHHELD.len()],
    /// A stand-in's own value while it holds another register's, then the registers a sequence borrows.
    pub saved: [u64; 2 * BORROWED],
    /// The exclusive monitor, as the meanings keep it.
    pub exclusive_bytes: u64,
    pub exclusive_address: u64,
    pub exclusive_low: u64,
    pub exclusive_high: u64,
}

/// The exclusive monitor's words: bytes, address, low and high, the register page's last fields.
pub type Exclusive = [u64; (size_of::<RegisterPage>()
    - core::mem::offset_of!(RegisterPage, exclusive_bytes))
    / size_of::<u64>()];

impl RegisterPage {
    /// The word at a byte offset of the page.
    pub fn word(&self, offset: u16) -> u64 {
        let exclusive = self.exclusive();
        let mut all = self.withheld.iter().chain(&self.saved).chain(&exclusive);
        *all.nth(usize::from(offset) / size_of::<u64>())
            .expect("an offset inside the register page")
    }

    /// The exclusive monitor: bytes, address, low and high words.
    pub fn exclusive(&self) -> Exclusive {
        [
            self.exclusive_bytes,
            self.exclusive_address,
            self.exclusive_low,
            self.exclusive_high,
        ]
    }

    /// Sets the exclusive monitor from [`Self::exclusive`]'s order.
    pub fn set_exclusive(&mut self, monitor: Exclusive) {
        [
            self.exclusive_bytes,
            self.exclusive_address,
            self.exclusive_low,
            self.exclusive_high,
        ] = monitor;
    }
}

/// The stack pointer's alignment when it is a base (SA0): a quadword.
pub use wcontainers_abi::STACK_ALIGNMENT;
/// The lowest bit of the tag the top byte holds.
pub const TAG_LOW: u32 = u64::BITS - u8::BITS;

/// The bits of FPCR the CPU we present has: alternative half precision, default not-a-number, flush to zero
/// and the rounding mode.
pub const FPCR_BITS: u32 = 0x07c0_0000;
/// The bits of FPSR the CPU we present has: saturation and the cumulative exception flags.
pub const FPSR_BITS: u32 = 0x0800_009f;

/// A guest address as the guest's branches and accesses use it: the top byte cleared unless bit 55 is set.
pub fn untag_address(address: u64) -> u64 {
    let select = TAG_LOW - 1;
    if address >> select & 1 == 0 {
        address & ((1 << TAG_LOW) - 1)
    } else {
        address
    }
}
