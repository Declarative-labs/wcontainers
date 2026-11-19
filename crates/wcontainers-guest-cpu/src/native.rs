//! What the native output needs of a guest that can also be the host: how each decoded instruction is
//! carried over to host code of the same architecture. It never says what an instruction computes; the
//! hardware computes it, and the meanings stay the definition the routes are proven against.

use crate::{Flow, FlowKind, Guest};

/// How one decoded instruction is carried over (docs/native.md section 3.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Route {
    /// The bytes as they are.
    Copy,
    /// A load or store whose access may cross a page: the page test and probes, then the bytes as they are.
    Split,
    /// A copy whose register fields collide where the architecture leaves the result open (a load or store
    /// writing back to its own transfer register, a load pair into one register): the colliding transfer
    /// register is replaced by the scratch register, which gives the meaning's result.
    Collision,
    /// Nothing: the meaning only goes on to the next instruction.
    Nothing,
    /// A copy with an alignment test in front.
    Aligned,
    /// A pc-relative instruction rebuilt on the guest address.
    Rebase,
    /// A branch, call or return.
    Transfer,
    /// A call into the inside, which answers as the meaning does.
    Kernel,
    /// A register read answered with the presented value.
    Constant,
    /// A read or write of the float status or mode, between two calls into the inside.
    FloatStatus,
    /// A trap with the meaning's kind.
    Trap,
    /// The exclusive monitor kept as the meaning keeps it.
    Exclusive,
}

/// What the native output does with a hook or a control a meaning uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeClass {
    /// The hardware holds it as presented.
    Hardware,
    /// The WebAssembly output's own bookkeeping, with nothing to keep natively.
    Bookkeeping,
    /// Kept by the native output or answered by the inside: a form touching it takes this route.
    Routed(Route),
}

/// A guest whose instructions the native output carries over to the same architecture.
pub trait Native: Guest {
    /// The route of one decoded instruction.
    fn route(i: &Self::Instruction) -> Route;
    /// The class of a hook, given the constant its meaning passes as the first argument where it passes one
    /// (the float hook's operation).
    fn hook_class(hook: Self::Hook, first: Option<i64>) -> NativeClass;
    /// The class of a control, read or written.
    fn control_class(control: Self::Control, write: bool) -> NativeClass;
    /// Writes the host code of one decoded instruction, `bytes` at guest `address`, as its route says. Its
    /// spans start at the instruction, before its commit point, with the count already placed.
    fn carry(i: &Self::Instruction, bytes: &[u8], address: u64, code: &mut HostCode);
    /// Writes the count's add.
    fn count(code: &mut HostCode);
    /// Writes a call into the inside.
    fn call(code: &mut HostCode, call: InsideCall);
}

/// A call into the inside from translated code, named by the code it carries (ARM64 `brk`'s immediate).
/// What it is about is the position map's span at the call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
}

impl InsideCall {
    /// Every call.
    pub const ALL: [Self; 7] = [
        Self::Exit,
        Self::Answer,
        Self::FloatBefore,
        Self::FloatAfter,
        Self::Trap,
        Self::Misaligned,
        Self::Unreachable,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// Nothing of the guest instruction is done: a stop here is at the instruction.
    Before,
    /// The guest instruction is done, or its fix-ups finish it: a stop here is where it goes on.
    After,
}

/// Where a guest value is while it is not in its own register: in a host register, or at a byte offset of the
/// thread's register page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Held {
    Register(u8),
    Page(u16),
}

/// A guest register whose value is held elsewhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Relocated {
    /// The guest register, by number.
    pub guest: u8,
    pub held: Held,
}

/// Where the guest goes on after a commit point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Resume {
    /// This guest address.
    At(u64),
    /// The address held there, untagged as the guest's branches untag it (a jump through a register).
    Through(Held),
    /// The guest address of the entry slot whose address is held there (a jump on its way to the slot).
    Slot(Held),
}

/// What the rest of a sequence would have done after its commit point (docs/native.md section 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fixup {
    /// The register, by number, gets the constant being built in it.
    Constant { register: u8, value: u64 },
    /// The exclusive monitor is written whole for an access of `bytes` (`pair`: two of them) at the address
    /// held there, its value read again from guest memory.
    Remember {
        bytes: u8,
        pair: bool,
        address: Held,
    },
    /// The exclusive monitor is forgotten and the status register, by number, written: 0 if the swap read
    /// the remembered value, its low and (a pair of 64-bit registers) high words held at `read`, else 1
    /// (`read` empty: no swap was tried, 1).
    Forget { status: u8, read: [Option<Held>; 2] },
    /// The block takes the hardware's float status.
    FloatStatus,
}

/// What a stop at one host instruction means for the guest. A span holds from its start to the next span.
#[derive(Clone, Debug, PartialEq, Eq)]
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
}

/// Host code with its position map, written one guest instruction after another.
#[derive(Debug, Default)]
pub struct HostCode {
    pub bytes: Vec<u8>,
    /// In order of their starts, the first at 0.
    pub spans: Vec<Span>,
    now: Option<Span>,
    /// Whether the current instruction is a counted event whose add is still ahead of its commit point.
    owed: bool,
    /// Code placed out of line, written after the piece.
    deferred: Deferred,
}

/// Code placed out of line, in the order it was deferred.
#[derive(Default)]
struct Deferred(Vec<Placement>);

impl std::fmt::Debug for Deferred {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} to place", self.0.len())
    }
}

/// Code to place out of line once the piece is written.
type Placement = Box<dyn FnOnce(&mut HostCode)>;

impl HostCode {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts the guest instruction at `address`, `counted` when it is one of the count's events. The count's
    /// lead carries over from what came before: an add placed earlier in the run may be this one's.
    pub fn start(&mut self, address: u64, counted: bool) {
        let lead = self.now.as_ref().map_or(0, |s| s.lead);
        self.now = Some(Span {
            start: 0,
            guest: address,
            side: Side::Before,
            lead,
            resume: Resume::At(address),
            relocated: Vec::new(),
            fixups: Vec::new(),
        });
        self.owed = counted;
    }

    fn state(&mut self) -> &mut Span {
        self.now.as_mut().expect("an instruction was started")
    }

    /// The current state, to come back to for code placed out of line.
    pub fn snapshot(&self) -> (Span, bool) {
        (
            self.now.clone().expect("an instruction was started"),
            self.owed,
        )
    }

    /// Goes back to a state taken with [`Self::snapshot`].
    pub fn restore(&mut self, (span, owed): (Span, bool)) {
        self.now = Some(span);
        self.owed = owed;
    }

    /// The offset the next byte is written at.
    pub fn offset(&self) -> usize {
        self.bytes.len()
    }

    /// Writes host bytes under the current state.
    pub fn push(&mut self, bytes: &[u8]) {
        let start = u32::try_from(self.bytes.len()).expect("host code fits in u32 offsets");
        let mut now = self.now.clone().expect("an instruction was started");
        now.start = start;
        let same = self.spans.last().is_some_and(|last| {
            Span {
                start,
                ..last.clone()
            } == now
        });
        if !same {
            if self.spans.last().is_some_and(|last| last.start == start) {
                self.spans.pop();
            }
            self.spans.push(now);
        }
        self.bytes.extend_from_slice(bytes);
    }

    /// Writes bytes over earlier ones (a branch whose target is now known).
    pub fn patch(&mut self, at: usize, bytes: &[u8]) {
        self.bytes[at..at + bytes.len()].copy_from_slice(bytes);
    }

    /// Writes the count's add for an event at or after the current instruction in the same straight-line run:
    /// from here until that event's commit point the count register is one ahead.
    pub fn count(&mut self, bytes: &[u8]) {
        self.push(bytes);
        self.state().lead += 1;
    }

    /// Writes the commit point; after it the guest goes on at `resume`.
    pub fn commit(&mut self, bytes: &[u8], resume: Resume) {
        self.push(bytes);
        self.after(resume);
    }

    /// From here on the instruction is done (or a stop finishes it) and the guest goes on at `resume`.
    pub fn after(&mut self, resume: Resume) {
        let owed = std::mem::take(&mut self.owed);
        let now = self.state();
        now.side = Side::After;
        now.resume = resume;
        now.lead -= u32::from(owed);
    }

    /// From here on a guest register's value is held elsewhere.
    pub fn relocate(&mut self, guest: u8, held: Held) {
        let now = self.state();
        now.relocated.retain(|r| r.guest != guest);
        now.relocated.push(Relocated { guest, held });
    }

    /// From here on a guest register's value is in its own place again.
    pub fn home(&mut self, guest: u8) {
        self.state().relocated.retain(|r| r.guest != guest);
    }

    /// From here on a stop also makes this fix-up.
    pub fn fixup(&mut self, fixup: Fixup) {
        self.state().fixups.push(fixup);
    }

    /// From here on a stop makes no fix-up.
    pub fn fixed(&mut self) {
        self.state().fixups.clear();
    }

    /// Places code out of line once the piece is written: a test's other way, entered by a branch it patches.
    pub fn defer(&mut self, place: impl FnOnce(&mut HostCode) + 'static) {
        self.deferred.0.push(Box::new(place));
    }

    /// Writes the code placed out of line, after the piece's last exit.
    pub fn finish(&mut self) {
        while !self.deferred.0.is_empty() {
            for place in std::mem::take(&mut self.deferred.0) {
                place(self);
            }
        }
    }

    /// The span holding the host byte at `offset`.
    pub fn span(&self, offset: u32) -> Option<&Span> {
        let after = self.spans.partition_point(|s| s.start <= offset);
        after
            .checked_sub(1)
            .map(|i| &self.spans[i])
            .filter(|_| (offset as usize) < self.bytes.len())
    }
}

/// Whether an executed instruction at `address` is one counted event of the progress count (definition E):
/// every instruction that can transfer backward, a direct branch whose target is at or below its own
/// address, taken or not, every call, every return, every jump through a register.
pub fn counted<R>(address: u64, flow: &Flow<R>) -> bool {
    match flow.kind {
        FlowKind::Call | FlowKind::CallThrough(_) | FlowKind::Return | FlowKind::JumpThrough(_) => {
            true
        }
        FlowKind::Plain => flow
            .target
            .is_some_and(|offset| address.wrapping_add_signed(offset) <= address),
    }
}

#[cfg(test)]
mod tests {
    use super::counted;
    use crate::{Flow, FlowKind, Through};

    fn direct(target: i64) -> Flow<u8> {
        Flow {
            falls_through: true,
            target: Some(target),
            kind: FlowKind::Plain,
        }
    }

    #[test]
    fn counts_what_can_go_backward() {
        let at = 0x40_1000;
        let step = 4;
        assert!(counted(at, &direct(-step)));
        assert!(counted(at, &direct(0)));
        assert!(!counted(at, &direct(step)));
        let plain = Flow::<u8> {
            falls_through: true,
            target: None,
            kind: FlowKind::Plain,
        };
        assert!(!counted(at, &plain));
        for kind in [
            FlowKind::Call,
            FlowKind::CallThrough(Through::Register(0)),
            FlowKind::Return,
            FlowKind::JumpThrough(Through::Memory),
        ] {
            let flow = Flow {
                falls_through: false,
                target: Some(step),
                kind,
            };
            assert!(counted(at, &flow));
        }
    }
}
