//! Form I's runs of translated instructions on a thread of this process (docs/native-without-virtual-machine.md section 5):
//! the floor, whose every entry at the in-process way's [`IN_PROCESS_ENTRIES`] ends a run, and a thread's frame, whose
//! own words are the thread's register page and the words after it.

use std::sync::OnceLock;

use rustix::io::Errno;
use wcontainers_arm64::Way;
use wcontainers_arm64::thread_state::{PAGE, RegisterPage, SCRATCH};
use wcontainers_arm64::way::{
    AFTER_PAGE_WORDS, BASE, IN_PROCESS_ENTRIES, IN_PROCESS_WITHHELD_AT, PROCESS_ENTRY_BYTES,
};
use wcontainers_executor_arm64_stop::{
    AfterPage, FloatStatus, GENERAL, GuestState, Interrupted, Taken, in_process,
};
use wcontainers_linux_process::{
    EXIT_BYTES, EXIT_SCRATCH_REGISTER, FRAME_REGISTER, GENERAL_REGISTERS, LinuxProcess,
    RUN_FRAME_WORDS, RunEnd, RunFrame, VECTOR_REGISTERS, WINDOW_BASE_REGISTER,
};
use wcontainers_linux_process_direct::{
    DirectInstructionMapping, DirectLinuxProcess, DirectRunThread,
};

use crate::memory::GUEST_PAGE;

/// The page the floor's entries are in.
const FLOOR_PAGE: u64 = IN_PROCESS_ENTRIES & !(GUEST_PAGE - 1);
/// The link register, which holds the address after a call into the floor.
const LINK: usize = GENERAL - 1;
/// The floor's window: none, since no run starts in it; its exits are reached only by a branch from a piece.
const NO_WINDOW: core::ops::Range<u64> = 0..0;
/// The words of the register page, then where the words after it start, in the frame's own words.
const PAGE_WORDS: usize = size_of::<RegisterPage>() / size_of::<u64>();
const AFTER_PAGE_AT: usize = IN_PROCESS_WITHHELD_AT / size_of::<u64>();

// The way's instructions find their page in x28 and the window's base in x24, and call the floor through x30,
// keeping t in x23, as the exits do; an exit is one entry long; the page and the words after it are the frame's own.
const _: () = assert!(
    FRAME_REGISTER == PAGE
        && WINDOW_BASE_REGISTER == BASE
        && EXIT_SCRATCH_REGISTER == SCRATCH
        && EXIT_BYTES as u64 == PROCESS_ENTRY_BYTES
        && GENERAL_REGISTERS == GENERAL
        && AFTER_PAGE_AT + AFTER_PAGE_WORDS <= RUN_FRAME_WORDS
        && PAGE_WORDS <= AFTER_PAGE_AT
);

/// The floor of this process: a page of exits where the in-process way's instructions call its entries.
#[derive(Debug)]
pub struct Floor {
    _exits: DirectInstructionMapping,
}

static FLOOR: OnceLock<Result<Floor, Errno>> = OnceLock::new();

/// This process's floor, mapped on first use: `EEXIST` where something else holds its page.
pub fn floor() -> Result<&'static Floor, Errno> {
    FLOOR
        .get_or_init(|| {
            let linux = DirectLinuxProcess;
            let exits = linux
                .exit_instructions()
                .repeat(GUEST_PAGE as usize / EXIT_BYTES);
            linux
                .map_instructions(FLOOR_PAGE as usize, &exits, NO_WINDOW, true)
                .map(|exits| Floor { _exits: exits })
        })
        .as_ref()
        .map_err(|error| *error)
}

/// Where a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// A call into the floor, with its code.
    Call(u16),
    /// An exit at no entry of the floor, which translated instructions never reach.
    Elsewhere,
    /// The host refused the run: its pc or stack pointer is not one the piece may start with.
    Refused,
    /// The stop came before the instructions started: the thread is as it was entered.
    NotEntered,
    /// The stop came while the instructions ran, at the hardware's pc.
    Stopped,
    /// An exception of the instructions, at the hardware's pc.
    Taken(Taken),
}

/// A thread of form I: its frame, which holds its registers between runs, on a host thread prepared to run code.
#[derive(Debug)]
pub struct InProcessThread {
    linux: DirectLinuxProcess,
    frame: Box<RunFrame>,
    host: DirectRunThread,
}

impl InProcessThread {
    /// A thread whose registers are all zero, run on this host thread; refused where the floor cannot be mapped or
    /// this host thread already runs one.
    pub fn new() -> Result<Self, Errno> {
        floor()?;
        let linux = DirectLinuxProcess;
        Ok(Self {
            host: linux.prepare_run_thread()?,
            linux,
            frame: Box::default(),
        })
    }

    /// The address of the thread's register page: its frame's.
    pub fn register_page(&self) -> u64 {
        core::ptr::from_ref(&*self.frame) as u64
    }

    /// Makes the thread go on from the guest's `state` at the host address `at`, a piece's first instruction for
    /// `way`; its vector registers and thread pointer stay as they are.
    pub fn enter(
        &mut self,
        way: Way,
        state: &GuestState,
        page: RegisterPage,
        float: &FloatStatus,
        at: u64,
    ) {
        let (hardware, page, after) =
            in_process::rejoin(way, state, page, float, self.register_page());
        self.set_hardware(&Interrupted { pc: at, ..hardware });
        self.set_page(&page);
        self.frame.words[AFTER_PAGE_AT..AFTER_PAGE_AT + AFTER_PAGE_WORDS].copy_from_slice(&after);
    }

    /// Runs the piece's instructions from the thread's pc until they call the floor, take an exception or are
    /// stopped.
    pub fn run(&mut self, instructions: &DirectInstructionMapping) -> Exit {
        match self
            .linux
            .run_instructions(&mut self.host, instructions, &mut self.frame)
        {
            RunEnd::Exit(exit) => call_at(exit),
            RunEnd::Refused => Exit::Refused,
            RunEnd::NotEntered => Exit::NotEntered,
            RunEnd::Stopped => {
                let registers = &mut self.frame.registers;
                // Stopped in an exit before it changed a register: the call it was making, made.
                if registers.pc & !(GUEST_PAGE - 1) == FLOOR_PAGE {
                    let exit = registers.pc & !(PROCESS_ENTRY_BYTES - 1);
                    registers.pc = registers.x[LINK];
                    return call_at(exit);
                }
                Exit::Stopped
            }
            RunEnd::Fault {
                address,
                write,
                alignment,
            } => {
                // A translated piece branches only to its own instructions, the slot table and the floor, all aligned:
                // an alignment fault at the pc is never one, and is an access like any other.
                Exit::Taken(if alignment && address == self.frame.registers.sp {
                    Taken::StackAlignment
                } else {
                    Taken::Access {
                        alignment,
                        fault: address,
                        write,
                    }
                })
            }
            RunEnd::Raised(_) => Exit::Taken(Taken::Other),
        }
    }

    /// The hardware as the thread left it, its pc the address after its call into the floor.
    pub fn hardware(&self) -> Interrupted {
        let registers = &self.frame.registers;
        Interrupted {
            x: registers.x,
            sp: registers.sp,
            pc: registers.pc,
            state: registers.nzcv,
            fpsr: registers.fpsr as u32,
            fpcr: registers.fpcr as u32,
        }
    }

    /// Makes the thread go on from the hardware and register page an exception left, which the stop crate resolved.
    pub fn resume(&mut self, hardware: &Interrupted, page: &RegisterPage) {
        self.set_hardware(hardware);
        self.set_page(page);
    }

    fn set_hardware(&mut self, hardware: &Interrupted) {
        let registers = &mut self.frame.registers;
        registers.x = hardware.x;
        registers.sp = hardware.sp;
        registers.pc = hardware.pc;
        registers.nzcv = hardware.state;
        registers.fpsr = hardware.fpsr.into();
        registers.fpcr = hardware.fpcr.into();
    }

    /// The thread's register page.
    pub fn page(&self) -> RegisterPage {
        let mut words = self.frame.words.iter().copied();
        let mut next = || words.next().unwrap_or_default();
        let mut page = RegisterPage::default();
        for word in page.withheld.iter_mut().chain(&mut page.saved) {
            *word = next();
        }
        page.link = next();
        page.set_exclusive([next(), next(), next(), next()]);
        page
    }

    fn set_page(&mut self, page: &RegisterPage) {
        let words = (page.withheld.iter().chain(&page.saved))
            .copied()
            .chain([page.link])
            .chain(page.exclusive());
        for (into, word) in self.frame.words.iter_mut().zip(words) {
            *into = word;
        }
    }

    /// The words after the register page.
    pub fn after(&self) -> AfterPage {
        let mut after = [0; AFTER_PAGE_WORDS];
        after.copy_from_slice(&self.frame.words[AFTER_PAGE_AT..AFTER_PAGE_AT + AFTER_PAGE_WORDS]);
        after
    }

    /// The vector registers.
    pub fn vectors(&self) -> [u128; VECTOR_REGISTERS] {
        self.frame.registers.v
    }

    pub fn set_vectors(&mut self, vectors: [u128; VECTOR_REGISTERS]) {
        self.frame.registers.v = vectors;
    }

    /// The thread pointer, TPIDR_EL0.
    pub fn thread_pointer(&self) -> u64 {
        self.frame.registers.tpidr_el0
    }

    pub fn set_thread_pointer(&mut self, pointer: u64) {
        self.frame.registers.tpidr_el0 = pointer;
    }
}

/// Where a run that ended at the exit at `exit` ended.
fn call_at(exit: u64) -> Exit {
    match exit
        .checked_sub(IN_PROCESS_ENTRIES)
        .filter(|offset| offset.is_multiple_of(PROCESS_ENTRY_BYTES))
        .and_then(|offset| u16::try_from(offset / PROCESS_ENTRY_BYTES).ok())
    {
        Some(code) => Exit::Call(code),
        None => Exit::Elsewhere,
    }
}
