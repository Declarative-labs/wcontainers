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
}

impl InsideCall {
    /// Every call.
    pub const ALL: [Self; 8] = [
        Self::Exit,
        Self::Answer,
        Self::FloatBefore,
        Self::FloatAfter,
        Self::Trap,
        Self::Misaligned,
        Self::Unreachable,
        Self::LastTarget,
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
    /// instructions; otherwise a direct branch replaces the word.
    pub retarget: Option<u32>,
}

/// A branch table of a known jump (the jump-table study): `entries` direct branches from host offset `start`, one
/// word each, entry k an exit to guest `base + (k << shift)` whose branch the loader points at that target's piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchTable {
    pub start: u32,
    pub base: u64,
    pub entries: u32,
    pub shift: u8,
    /// The bits of each branch that hold its offset in instructions.
    pub field: u32,
}

/// What an address place builds in its register.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Holds {
    /// The guest address itself.
    Address,
    /// The address of the guest address's entry slot.
    EntrySlot,
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
    /// The page test in front of a load or store of 16 bytes or fewer, which the host's model is not relied on
    /// to do as the meaning does.
    HostModelResultRouted,
}

impl NativeReason {
    pub const ALL: [Self; 3] = [
        Self::WithheldRegisterNamed,
        Self::AccessMayCrossPage,
        Self::HostModelResultRouted,
    ];

    /// The fixed names their counters and warnings carry, in the order of [`Self::ALL`].
    const NAMES: [&'static str; Self::ALL.len()] = [
        "withheld register named by the guest",
        "access that may cross a page probed",
        "host-model result routed",
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
/// register's low `bits` bits extended (signed or not) and shifted left.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Operand {
    pub base: Base,
    pub offset: i64,
    pub index: Option<Index>,
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
