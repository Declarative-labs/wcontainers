//! The position map of native host code (docs/native.md section 4): what a stop at each host instruction
//! means for the guest. Both the translator, which writes it, and the inside, which resolves stops with it,
//! use these types; this part needs no standard library.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

/// A call into the inside from translated code, named by the code it carries (ARM64 `brk`'s immediate).
/// What it is about is the position map's span at the call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum InsideCall {
    /// Translated code ends here: the guest goes on where the span resumes.
    Exit,
    /// The inside answers the instruction as its meaning does (the Kernel route).
    Answer,
    /// Before a float status instruction: the hardware is loaded with the guest's flags.
    FloatBefore,
    /// After it: the block takes the hardware's status.
    FloatAfter,
    /// The instruction traps as its meaning does (the Trap route).
    Trap,
    /// The instruction's access is not a multiple of its size: an alignment trap at its address.
    Misaligned,
    /// A jump through a register to an address no piece can be at: the guest lands there and faults.
    Unreachable,
    /// A jump's first miss: the inside makes the target its last target, then the jump goes on.
    LastTarget,
    /// A copied `fldcw` unmasked an x87 exception: the inside enters the unmasked x87 mode, or raises the pending
    /// exception as the meaning does (x86).
    X87Control,
    /// After the instruction of a stepping piece: the meaning's debug trap of a single step (x86's process way).
    Step,
    /// The replay check found the thread's stop word equal to its count: the thread stops before the instruction.
    Stop,
}

impl InsideCall {
    /// Every call.
    pub const ALL: [Self; 11] = [
        Self::Exit,
        Self::Answer,
        Self::FloatBefore,
        Self::FloatAfter,
        Self::Trap,
        Self::Misaligned,
        Self::Unreachable,
        Self::LastTarget,
        Self::X87Control,
        Self::Step,
        Self::Stop,
    ];

    /// The code a call carries.
    pub fn code(self) -> u16 {
        self as u16
    }

    /// The call a code names.
    pub fn from_code(code: u16) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.code() == code)
    }
}

/// Which side of its instruction's commit point a host instruction is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    /// Nothing of the guest instruction is done: a stop here is at the instruction.
    Before,
    /// The guest instruction is done, or its fix-ups finish it: a stop here is where it goes on.
    After,
}

/// Where a guest value is while it is not in its own register: in a host register, or at a byte offset of the
/// thread's register page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Held {
    Register(u8),
    Page(u16),
}

/// A guest register whose value is held elsewhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Relocated {
    /// The guest register, by number.
    pub guest: u8,
    pub held: Held,
}

/// Where the guest goes on after a commit point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Resume {
    /// This guest address.
    At(u64),
    /// The address held there, untagged as the guest's branches untag it (a jump through a register).
    Through(Held),
    /// The guest address of the entry slot whose address is held there (a jump on its way to the slot).
    Slot(Held),
}

/// What the rest of a sequence would have done after its commit point (docs/native.md section 4), to the host
/// registers and the register page: a stop makes its fix-ups first, then reads the guest's values where the
/// span's relocations say they are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Fixup {
    /// The host register, by number, gets the constant being built in it.
    Constant { register: u8, value: u64 },
    /// The host register, by number, gets the guest address being built in it, which moves with the code.
    Address { register: u8, value: u64 },
    /// The host register, by number (the zero register's number: the stack pointer), gets host register
    /// `from`'s value.
    Move { register: u8, from: u8 },
    /// The exclusive monitor is written whole for an access of `bytes` (`pair`: two of them) at the address
    /// held there, its value read again from guest memory.
    Remember {
        bytes: u8,
        pair: bool,
        address: Held,
    },
    /// The exclusive monitor is forgotten and the status host register, by number, written: 0 if the swap read
    /// the remembered value, its low and (a pair of 64-bit registers) high words held at `read`, else 1
    /// (`read` empty: no swap was tried, 1).
    Forget { status: u8, read: [Option<Held>; 2] },
    /// The block takes the hardware's float status.
    FloatStatus,
    /// The register page's last x87 instruction pointer gets this guest address, not yet stored there (x86).
    X87Pointer(u64),
}

/// What a stop at one host instruction means for the guest. A span holds from its start to the next span.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// Its first host byte, from the start of the code.
    pub start: u32,
    /// The guest instruction's address.
    pub guest: u64,
    pub side: Side,
    /// How far the count register is ahead of the events completed here.
    pub lead: u32,
    /// Where the guest goes on, after the commit point.
    pub resume: Resume,
    pub relocated: Vec<Relocated>,
    pub fixups: Vec<Fixup>,
    pub stopping: Stopping,
}

/// A piece's exit to a direct target through its entry slot, which the loader may link straight to the target's
/// piece by writing a direct branch over the exit's first word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectExit {
    /// The exit's first host byte, from the start of the code.
    pub start: u32,
    /// The guest address it goes to.
    pub target: u64,
    /// Where the word is a branch the loader points at the target's piece itself, the bits holding its offset in
    /// instructions (x86: the byte offset of its rel32); otherwise a direct branch replaces the word.
    pub retarget: Option<u32>,
    /// The condition under which the exit's branch is taken, as the architecture numbers it (x86: the low four
    /// bits of a `jcc`); none for an unconditional one. A hit on a link's `int3` evaluates it (x86).
    pub condition: Option<u8>,
}

/// The branch table of a jump through a register whose targets are known (docs/native.md section 3.2): one branch
/// word for each value of the jump's index, from host byte `start`, the word for value k an exit to
/// [`Self::target`] of k whose branch the loader points at that target's piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchTable {
    pub start: u32,
    /// The guest address of index 0.
    pub base: u64,
    /// How many values the index takes, a power of two.
    pub entries: u32,
    /// How far the index is shifted before it is added to the base.
    pub shift: u8,
    /// Whether the index is a signed number of its own bits.
    pub signed: bool,
    /// The bits of each branch that hold its offset in instructions.
    pub retarget: u32,
}

impl BranchTable {
    /// The guest address the jump goes to for index value `k`.
    pub fn target(&self, k: u32) -> u64 {
        let bits = self.entries.trailing_zeros();
        let value = if self.signed && bits > 0 {
            let unused = u64::BITS - bits;
            ((u64::from(k) << unused) as i64 >> unused) as u64
        } else {
            u64::from(k)
        };
        self.base
            .wrapping_add(value.wrapping_shl(u32::from(self.shift)))
    }
}

/// What an address place builds in its register.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Holds {
    /// The guest address itself.
    Address,
    /// The address of the guest address's entry slot in the slot table that starts at `table`.
    EntrySlot { table: u64 },
}

/// One word of host code that builds a guest address in a register, written again when the code is loaded for
/// another guest address (docs/native.md section 3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AddressPlace {
    /// The word's first host byte, from the start of the code.
    pub start: u32,
    /// The host register, by number.
    pub register: u8,
    /// Which word of the sequence that builds the value it is, from the first.
    pub part: u8,
    pub address: u64,
    pub holds: Holds,
}

/// A slow path of the native output that translating alone can count, per site (docs/native.md section 3.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeReason {
    /// A stand-in's moves, or a read from the register page, around a guest instruction naming a withheld
    /// register.
    WithheldRegisterNamed,
    /// The page test and probes in front of a load or store of more than 16 bytes.
    AccessMayCrossPage,
    /// A result the host's model is not relied on to give as the meaning does: ARM64's page test in front of a
    /// load or store of 16 bytes or fewer, x86's form the inside answers where the start-up probe found the host
    /// differs.
    HostModelResultRouted,
    /// A repeated string instruction the inside runs, so that the count takes its passes as the meaning's does.
    RepeatedStringCounted,
    /// A piece of one instruction for a guest thread whose trap flag is set, every way on into the inside.
    GuestSingleStep,
}

impl NativeReason {
    pub const ALL: [Self; 5] = [
        Self::WithheldRegisterNamed,
        Self::AccessMayCrossPage,
        Self::HostModelResultRouted,
        Self::RepeatedStringCounted,
        Self::GuestSingleStep,
    ];

    /// The fixed names their counters and warnings carry, in the order of [`Self::ALL`].
    const NAMES: [&'static str; Self::ALL.len()] = [
        "withheld register named by the guest",
        "access that may cross a page probed",
        "host-model result routed",
        "repeated string counted by the inside",
        "guest single step",
    ];

    /// Its place in [`Self::ALL`].
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|&r| r == self)
            .expect("every reason is listed")
    }

    /// The fixed name its counter and warning carry.
    pub fn name(self) -> &'static str {
        Self::NAMES[self.index()]
    }
}

/// What a stop in one guest instruction needs to know of it, found when translating so that the inside
/// resolves a stop without decoding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stopping {
    /// Nothing beyond the span.
    #[default]
    Plain,
    /// A load or store, whose first byte its operand gives.
    Access(Operand),
    /// A system call.
    Syscall,
    /// Code made visible to instruction fetch at the address in this general register (`None`: zero): the
    /// inside drops what it translated from that line and the guest goes on.
    CodeChanged(Option<u8>),
    /// A read of the counter into this general register (`None`: nowhere).
    Counter(Option<u8>),
    /// A breakpoint with its comment.
    Breakpoint(u16),
    /// An instruction the CPU we present does not have.
    Illegal,
    /// A write of the float status register: from then on its flags are kept exact.
    FloatStatusWrite,
    /// A read of the float status register, a slow path while its flags are not kept.
    FloatStatusRead,
}

/// A register an address is formed from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Base {
    /// A general register by number.
    Register(u8),
    StackPointer,
    /// Reads as zero.
    Zero,
    /// The instruction's own address.
    Pc,
}

/// How a load or store forms the address of its first byte: the base, plus the offset, plus the index
/// register's low `bits` bits extended (signed or not) and shifted left, at the address's width, plus the
/// segment's base.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Operand {
    pub base: Base,
    pub offset: i64,
    pub index: Option<Index>,
    /// The segment whose base the sum is added to, where it has one (x86's `fs:` and `gs:`).
    pub segment: Option<Segment>,
    /// The sum is taken modulo 2 to this power, before any segment base is added (x86's 0x67: 32).
    pub address_bits: u8,
}

/// A segment register whose base an address is formed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Segment {
    Fs,
    Gs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Index {
    pub register: Base,
    pub bits: u8,
    pub signed: bool,
    pub shift: u8,
}

/// The span holding the host byte at `offset` of code `length` bytes long whose position map is `spans`.
pub fn span_at(spans: &[Span], length: usize, offset: u32) -> Option<&Span> {
    let after = spans.partition_point(|s| s.start <= offset);
    after
        .checked_sub(1)
        .map(|i| &spans[i])
        .filter(|_| (offset as usize) < length)
}
