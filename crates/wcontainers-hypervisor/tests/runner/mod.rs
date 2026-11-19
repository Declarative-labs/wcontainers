//! Runs the native output's pieces at the guest's level on one virtual CPU and resolves every stop through the
//! position map into the guest's state, in layer 1's terms.
//!
//! The native cases program maps the regions named here and calls its gate once per run. At the gate the
//! monitor keeps the inside's registers, loads the guest's, and returns to the guest's level; every exception
//! from there comes to the monitor through vectors that only call it, before the inside's code could touch any
//! register. When the run ends the inside's registers go back and the gate returns.

#![allow(dead_code)]

pub mod function;

use aarch64_cpu::registers::{ESR_EL1, MDSCR_EL1, SPSR_EL1};
use wcontainers_abi::{PAGE_SIZE, RegisterBlock};
use wcontainers_arm64::carry::{LINK, PAGE, RegisterPage, TAG_LOW, WITHHELD, call_word};
use wcontainers_arm64::encode::{self, form};
use wcontainers_arm64::thread_state::{Exclusive, untag_address};
use wcontainers_arm64::{Address, Arm64, Instruction as I, Reg, decode};
use wcontainers_guest_cpu::position::Operand;
use wcontainers_guest_cpu::{Guest, HostCode, InsideCall, Native, Route, Span, Stopping, counted};
use wcontainers_hypervisor::{
    Exit, Host, Loaded, Memory, Register, VirtualCpu, VirtualMachine, boot, link as link_page,
};
use wcontainers_inside::layout::{
    Region as Area, TOP_LEVEL, VECTOR_BYTES, VECTOR_TABLE_BYTES, level_span,
};
use wcontainers_inside::link::{self, Fatal, Start};
use wcontainers_inside::stop::{self, FloatStatus, Interrupted};
use wcontainers_inside::syndrome::{ALIGNMENT_FAULT, FAULT_STATUS};
use wcontainers_translator::{LookAhead, translate_native};
use wcontainers_translator_test::layer1::{Case, Outcome, Stop};
use wcontainers_translator_test::layout::{
    CODE, CODE_SPAN, DATA, DATA_BYTES, DATA_READ_ONLY, DATA_WRITABLE, fill,
};

use crate::common::{ONE_CPU, build, machine};
use crate::protocol::{self, Code, GATE_OFFSET, Kind, Map, Region};

/// The native cases program.
const PROGRAM: &str = "native-cases";
/// Bytes of guest-physical memory: the regions' frames and tables fit with room.
const MEMORY: u64 = 1 << 29;
/// Bytes of the code area the pieces may fill.
const PIECE_BYTES: u64 = 1 << 20;
/// The gate's region, at the start of the code area.
const GATE: u64 = Area::Code.base();
/// Where the pieces start: the next table's worth of the code area.
const PIECES: u64 = Area::Code.base() + level_span(TOP_LEVEL + 1);
/// The thread's register page.
const REGISTER_PAGE: u64 = Area::Threads.base();
/// The gate's words: call the monitor, return to the guest's level, return to the caller.
const GATE_CALL: u64 = GATE + GATE_OFFSET;
const GATE_RETURN_TO_GUEST: u64 = GATE_CALL + Arm64::MAX_LENGTH;
const GATE_BACK: u64 = GATE_RETURN_TO_GUEST + Arm64::MAX_LENGTH;
/// The vectors are in four groups (current level with SP_EL0, with SP_ELx, lower level in AArch64, in
/// AArch32) of four kinds (synchronous, interrupt, fast interrupt, system error).
const VECTOR_KINDS: u64 = 4;
const LOWER_AARCH64_GROUP: u64 = 2;
/// The vector of a synchronous exception from the guest's level.
const LOWER_SYNCHRONOUS: u64 = GATE + LOWER_AARCH64_GROUP * VECTOR_KINDS * VECTOR_BYTES;
/// The general registers the hardware holds for the guest, x0 to x30.
use wcontainers_inside::stop::GENERAL;
/// The vector registers.
const VECTORS: usize = RegisterBlock::ZERO.v.len();
/// Layer 1's register arrays: x0 to x30, then sp, then (in a case) flags and thread pointer, (in an outcome)
/// pc, flags and thread pointer.
const CASE_SP: usize = GENERAL;
const CASE_FLAGS: usize = CASE_SP + 1;
const CASE_THREAD: usize = CASE_FLAGS + 1;
const OUT_PC: usize = CASE_SP + 1;
const OUT_FLAGS: usize = OUT_PC + 1;
const OUT_THREAD: usize = OUT_FLAGS + 1;

/// Everything of the guest's level the hardware holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hardware {
    pub x: [u64; GENERAL],
    pub sp: u64,
    pub pc: u64,
    pub state: u64,
    pub thread: u64,
    pub v: [u128; VECTORS],
    pub fpsr: u32,
    pub fpcr: u32,
}

/// The guest's own state at a stop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuestState {
    pub x: [u64; GENERAL],
    pub sp: u64,
    pub pc: u64,
    pub flags: u64,
    pub thread: u64,
    pub v: [u128; VECTORS],
    pub float: [u32; 2],
    pub count: u64,
    /// The exclusive monitor: bytes, address, low and high words, as the register page keeps it.
    pub exclusive: Exclusive,
}

/// A native run's outcome: layer 1's, and the count at the stop.
#[derive(Clone, Debug)]
pub struct NativeOutcome {
    pub outcome: Outcome,
    pub count: u64,
    pub expected_count: u64,
}

impl NativeOutcome {
    /// What differs from the WebAssembly output's outcome, or from definition E's count.
    pub fn differs(&self, expected: &Outcome) -> Option<String> {
        let got = &self.outcome;
        let mut found = Vec::new();
        if got.stop != expected.stop {
            found.push(format!("stop {:?}, expected {:?}", got.stop, expected.stop));
        }
        for (k, (g, e)) in got.registers.iter().zip(&expected.registers).enumerate() {
            if g != e {
                found.push(format!("register {k} {g:#x}, expected {e:#x}"));
            }
        }
        for (k, (g, e)) in got.vectors.iter().zip(&expected.vectors).enumerate() {
            if g != e {
                found.push(format!("v{k} {g:#x}, expected {e:#x}"));
            }
        }
        if got.float != expected.float {
            found.push(format!(
                "float {:x?}, expected {:x?}",
                got.float, expected.float
            ));
        }
        if got.changed != expected.changed {
            found.push(format!(
                "changed {:x?}, expected {:x?}",
                got.changed, expected.changed
            ));
        }
        if self.count != self.expected_count {
            found.push(format!(
                "count {}, expected {}",
                self.count, self.expected_count
            ));
        }
        (!found.is_empty()).then(|| found.join("; "))
    }
}

/// How a run at the guest's level came back.
#[derive(Clone, Debug)]
pub struct Trapped {
    pub syndrome: u64,
    pub fault: u64,
    pub hardware: Hardware,
    pub page: RegisterPage,
}

/// The inside's registers at the gate, put back when a run ends.
struct Inside {
    x: [u64; GENERAL],
    stack: u64,
    state: u64,
    v: [u128; VECTORS],
    fpsr: u64,
    fpcr: u64,
}

pub struct Runner {
    cpu: <<Host as wcontainers_hypervisor::Hypervisor>::VirtualMachine as VirtualMachine>::Cpu,
    _machine: <Host as wcontainers_hypervisor::Hypervisor>::VirtualMachine,
    memory: Memory,
    loaded: Loaded,
    /// Each region of fresh frames: its first address and first frame.
    regions: Vec<(Region, u64)>,
    inside: Option<Inside>,
}

/// The regions the program maps, in order.
fn regions() -> Vec<Region> {
    let region = |va, bytes, kind: Kind| Region {
        va,
        bytes,
        kind: kind as u64,
    };
    vec![
        region(DATA, DATA_WRITABLE, Kind::ReadWrite),
        region(DATA + DATA_WRITABLE, DATA_READ_ONLY, Kind::ReadOnly),
        region(CODE - CODE_SPAN, CODE_SPAN, Kind::Repeated),
        region(CODE, PAGE_SIZE, Kind::ReadOnly),
        region(CODE + PAGE_SIZE, CODE_SPAN - PAGE_SIZE, Kind::Repeated),
        region(PIECES, PIECE_BYTES, Kind::Code),
        region(REGISTER_PAGE, PAGE_SIZE, Kind::RegisterPage),
        region(GATE, PAGE_SIZE, Kind::Gate),
    ]
}

/// The gate's region: vectors that each call the monitor, then the gate.
fn gate_words() -> Vec<u32> {
    let call = encode::exception(form::HVC, MONITOR_CALL);
    let mut words = vec![0u32; (VECTOR_TABLE_BYTES / Arm64::MAX_LENGTH) as usize];
    words.fill(call);
    words.extend([
        call,
        encode::exception_return(),
        encode::branch_register(form::RET, LINK),
    ]);
    words
}

/// The immediate of the vectors' calls to the monitor. Not 0: Hypervisor.framework answers an `hvc #0` whose
/// x0 reads as an SMCCC fast call itself (with "not supported" in x0) and never exits.
const MONITOR_CALL: u16 = 1;

impl Runner {
    /// Boots the program and serves it until it asks for its first run.
    pub fn start() -> Runner {
        let program = build(PROGRAM);
        let (machine, memory, loaded, start) = machine(&program, MEMORY, ONE_CPU);
        let mut cpu = machine.cpu().expect("a virtual CPU");
        boot(&mut cpu, &memory, &loaded, 0, Start { ..start }).expect("booting");
        let mut runner = Runner {
            cpu,
            _machine: machine,
            memory,
            loaded,
            regions: Vec::new(),
            inside: None,
        };
        let mut wanted = regions().into_iter();
        let mut last: Option<Region> = None;
        loop {
            match runner.serve() {
                protocol::Request::Map => {
                    let Map { frame } = runner.link().get();
                    if let Some(region) = last.take() {
                        runner.regions.push((region, frame));
                        if region.kind == Kind::Gate as u64 {
                            let bytes: Vec<u8> =
                                gate_words().iter().flat_map(|w| w.to_le_bytes()).collect();
                            runner.write_frames(frame, &bytes);
                        }
                    }
                    let next = wanted.next().unwrap_or_default();
                    last = (next.bytes != 0).then_some(next);
                    runner.link().put(&next);
                }
                protocol::Request::Next => return runner,
                other => panic!("unexpected request {other:?}"),
            }
        }
    }

    fn link(&self) -> &'static mut link::Link {
        link_page(&self.memory, &self.loaded, 0)
    }

    /// Runs the inside until it asks something of this program's set, answering the floor's own requests.
    fn serve(&mut self) -> protocol::Request {
        loop {
            match self.cpu.run().expect("running") {
                Exit::Doorbell => {}
                other => panic!(
                    "the inside stopped: {other:?} at {:#x}",
                    self.cpu.get(Register::Pc).unwrap_or_default()
                ),
            }
            let page = self.link();
            if let Some(request) = link::Request::from_word(page.head.request) {
                match request {
                    link::Request::Print => {
                        println!("inside: {}", String::from_utf8_lossy(page.data()))
                    }
                    link::Request::Fatal => {
                        let Fatal { esr, elr, far } = page.get();
                        panic!("the inside faulted: ESR {esr:#x} ELR {elr:#x} FAR {far:#x}");
                    }
                    link::Request::GiveBack => panic!("no give-back here"),
                }
                continue;
            }
            return protocol::Request::from_word(page.head.request).expect("a request of the set");
        }
    }

    /// The host address of guest-physical `frame`.
    fn host(&self, frame: u64) -> *mut u8 {
        self.memory.host(frame)
    }

    fn write_frames(&self, frame: u64, bytes: &[u8]) {
        // SAFETY: the frames are the region's, in a row, and the guest is not running.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.host(frame), bytes.len()) };
    }

    /// The frame behind guest address `va` in a region of fresh frames.
    fn frame_of(&self, va: u64) -> u64 {
        let (region, frame) = self
            .regions
            .iter()
            .find(|(r, _)| va >= r.va && va < r.va + r.bytes && r.kind != Kind::Repeated as u64)
            .expect("an address of a region of fresh frames");
        frame + (va - region.va)
    }

    fn write(&self, va: u64, bytes: &[u8]) {
        for (k, b) in bytes.iter().enumerate() {
            let at = self.frame_of(va + k as u64);
            // SAFETY: a byte of a region's frame; the guest is not running.
            unsafe { *self.host(at) = *b };
        }
    }

    fn read(&self, va: u64, bytes: &mut [u8]) {
        for (k, b) in bytes.iter_mut().enumerate() {
            let at = self.frame_of(va + k as u64);
            // SAFETY: as above.
            *b = unsafe { *self.host(at) };
        }
    }

    fn page(&self) -> RegisterPage {
        // SAFETY: the register page's frame holds a `RegisterPage`; the guest is not running.
        unsafe { std::ptr::read(self.host(self.frame_of(REGISTER_PAGE)).cast()) }
    }

    fn set_page(&self, page: &RegisterPage) {
        // SAFETY: as above.
        unsafe { std::ptr::write(self.host(self.frame_of(REGISTER_PAGE)).cast(), page.clone()) }
    }

    /// Writes `code` at the start of the pieces and has the inside make it visible and call the gate.
    pub fn load(&mut self, code: &[u8]) {
        assert!(code.len() as u64 <= PIECE_BYTES, "the pieces fit");
        self.write(PIECES, code);
        self.link().put(&Code {
            va: PIECES,
            bytes: code.len() as u64,
        });
        match self.cpu.run().expect("running") {
            Exit::Doorbell => {}
            other => panic!("expected the gate, got {other:?}"),
        }
        let pc = self.cpu.get(Register::Pc).expect("pc");
        assert_eq!(pc, GATE_RETURN_TO_GUEST, "the gate called the monitor");
        let mut x = [0; GENERAL];
        for (n, value) in x.iter_mut().enumerate() {
            *value = self.cpu.get(Register::X(n as u8)).expect("x");
        }
        let mut v = [0; VECTORS];
        for (n, value) in v.iter_mut().enumerate() {
            *value = self.cpu.vector(n as u8).expect("v");
        }
        self.inside = Some(Inside {
            x,
            stack: self.cpu.get(Register::StackEl1).expect("sp"),
            state: self.cpu.get(Register::Pstate).expect("pstate"),
            v,
            fpsr: self.cpu.get(Register::FloatStatus).expect("fpsr"),
            fpcr: self.cpu.get(Register::FloatControl).expect("fpcr"),
        });
    }

    /// Ends the run: the inside's registers go back, the gate returns, and the inside asks for the next.
    pub fn unload(&mut self) {
        let inside = self.inside.take().expect("a run");
        for (n, value) in inside.x.iter().enumerate() {
            self.cpu.set(Register::X(n as u8), *value).expect("x");
        }
        for (n, value) in inside.v.iter().enumerate() {
            self.cpu.set_vector(n as u8, *value).expect("v");
        }
        for (register, value) in [
            (Register::StackEl1, inside.stack),
            (Register::FloatStatus, inside.fpsr),
            (Register::FloatControl, inside.fpcr),
            (Register::DebugControl, 0),
            (Register::Pstate, inside.state),
            (Register::Pc, GATE_BACK),
        ] {
            self.cpu
                .set(register, value)
                .expect("the inside's register");
        }
        assert_eq!(self.serve(), protocol::Request::Next);
    }

    /// Runs the guest's level from `hardware` until an exception, stepping one instruction when `step`.
    pub fn enter(&mut self, hardware: &Hardware, page: &RegisterPage, step: bool) -> Trapped {
        self.set_page(page);
        for (n, value) in hardware.x.iter().enumerate() {
            self.cpu.set(Register::X(n as u8), *value).expect("x");
        }
        for (n, value) in hardware.v.iter().enumerate() {
            self.cpu.set_vector(n as u8, *value).expect("v");
        }
        let masked =
            SPSR_EL1::M::EL0t + SPSR_EL1::A::Masked + SPSR_EL1::I::Masked + SPSR_EL1::F::Masked;
        let mut state = masked.value | (hardware.state & nzcv());
        let mut debug = 0;
        if step {
            state |= SPSR_EL1::SS::SET.value;
            debug = MDSCR_EL1::SS::SoftwareStepEnabled.value;
        }
        for (register, value) in [
            (Register::StackEl0, hardware.sp),
            (Register::ThreadPointer, hardware.thread),
            (Register::FloatStatus, hardware.fpsr.into()),
            (Register::FloatControl, hardware.fpcr.into()),
            (Register::ExceptionLink, hardware.pc),
            (Register::SavedState, state),
            (Register::DebugControl, debug),
            (Register::Pc, GATE_RETURN_TO_GUEST),
        ] {
            self.cpu.set(register, value).expect("the guest's register");
        }
        match self.cpu.run().expect("running") {
            Exit::Doorbell => {}
            other => panic!("expected a vector, got {other:?}"),
        }
        let pc = self.cpu.get(Register::Pc).expect("pc");
        let get = |r| self.cpu.get(r).expect("a register");
        assert_eq!(
            pc,
            LOWER_SYNCHRONOUS + Arm64::MAX_LENGTH,
            "a synchronous exception from the guest's level: ESR {:#x} ELR {:#x} FAR {:#x}",
            get(Register::Syndrome),
            get(Register::ExceptionLink),
            get(Register::FaultAddress)
        );
        let mut x = [0; GENERAL];
        for (n, value) in x.iter_mut().enumerate() {
            *value = get(Register::X(n as u8));
        }
        let mut v = [0; VECTORS];
        for (n, value) in v.iter_mut().enumerate() {
            *value = self.cpu.vector(n as u8).expect("v");
        }
        Trapped {
            syndrome: get(Register::Syndrome),
            fault: get(Register::FaultAddress),
            hardware: Hardware {
                x,
                sp: get(Register::StackEl0),
                pc: get(Register::ExceptionLink),
                state: get(Register::SavedState),
                thread: get(Register::ThreadPointer),
                v,
                fpsr: get(Register::FloatStatus) as u32,
                fpcr: get(Register::FloatControl) as u32,
            },
            page: self.page(),
        }
    }

    /// Runs one layer 1 case as the native output's piece.
    pub fn case(&mut self, case: &Case) -> NativeOutcome {
        let piece =
            translate_native::<Arm64>(&case.word.to_le_bytes(), CODE, LookAhead::None, true)
                .expect("a decoded word");
        let mut data = vec![0u8; DATA_BYTES as usize];
        fill(case.seed, &mut data);
        self.write(DATA, &data);
        self.write(CODE, &case.word.to_le_bytes());
        self.load(&piece.bytes);
        let (mut hardware, mut page) = start(case);
        let mut float = FloatStatus {
            fpsr: case.float[0],
            fpcr: case.float[1],
            tracking: case.tracking,
        };
        let i = decode(case.word);
        let (stop, state) = loop {
            let trapped = self.enter(&hardware, &page, false);
            let mut memory = |va, bytes: &mut [u8]| self.read(va, bytes);
            match resolve(&piece, PIECES, &trapped, &mut float, &mut memory) {
                Resolved::Stop(stop, state) => break (stop, *state),
                Resolved::Again(h, p) => (hardware, page) = (*h, p),
            }
        };
        self.unload();
        let mut now = vec![0u8; DATA_BYTES as usize];
        self.read(DATA, &mut now);
        let changed = now
            .iter()
            .zip(&data)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(k, (a, _))| (k as u32, *a))
            .collect();
        // The instruction ran unless the stop is at it.
        let executed = match stop {
            Stop::Landed(_) => true,
            Stop::Alignment(_) | Stop::MemoryFault(_) => state.pc != CODE,
            _ => false,
        };
        let expected_count = u64::from(executed && counted(CODE, &Arm64::flow(&i)));
        NativeOutcome {
            outcome: outcome(stop, &state, changed),
            count: state.count,
            expected_count,
        }
    }

    /// Runs a case's word copied as it is with nothing in front, the behaviour probe's way (docs/native.md
    /// section 5.8): what the hardware itself did, in layer 1's terms.
    pub fn raw(&mut self, case: &Case) -> Outcome {
        let mut data = vec![0u8; DATA_BYTES as usize];
        fill(case.seed, &mut data);
        self.write(DATA, &data);
        self.write(CODE, &case.word.to_le_bytes());
        let mut piece = case.word.to_le_bytes().to_vec();
        piece.extend(call_word(InsideCall::Exit).to_le_bytes());
        self.load(&piece);
        let mut x = [0u64; GENERAL];
        x.copy_from_slice(&case.registers[..GENERAL]);
        let hardware = Hardware {
            x,
            sp: case.registers[CASE_SP],
            pc: PIECES,
            state: case.registers[CASE_FLAGS],
            thread: case.registers[CASE_THREAD],
            v: case.vectors,
            fpsr: case.float[0],
            fpcr: case.float[1],
        };
        let trapped = self.enter(&hardware, &RegisterPage::default(), false);
        self.unload();
        let h = &trapped.hardware;
        let i = decode(case.word);
        let class = ESR_EL1::EC.read(trapped.syndrome);
        let status = ESR_EL1::ISS.read(trapped.syndrome) & FAULT_STATUS;
        let first = || access_address(&i, CODE, &h.x, h.sp);
        let next = CODE + Arm64::MAX_LENGTH;
        let (stop, pc) = if class == ESR_EL1::EC::Value::Brk64 as u64 {
            (Stop::Landed(next), next)
        } else if class == ESR_EL1::EC::Value::SPAlignmentFault as u64 {
            (Stop::Alignment(h.sp), CODE)
        } else if class == ESR_EL1::EC::Value::DataAbortLowerEL as u64 {
            if status == ALIGNMENT_FAULT {
                (Stop::Alignment(first()), CODE)
            } else {
                (
                    Stop::MemoryFault(stop::fault_address(first(), trapped.fault)),
                    CODE,
                )
            }
        } else {
            (Stop::Other(class as u32, trapped.fault), CODE)
        };
        let mut now = vec![0u8; DATA_BYTES as usize];
        self.read(DATA, &mut now);
        let changed = now
            .iter()
            .zip(&data)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(k, (a, _))| (k as u32, *a))
            .collect();
        let state = GuestState {
            x: h.x,
            sp: h.sp,
            pc,
            flags: h.state & nzcv(),
            thread: h.thread,
            v: h.v,
            float: case.float,
            count: 0,
            exclusive: RegisterPage::default().exclusive(),
        };
        outcome(stop, &state, changed)
    }

    /// Ends the program.
    pub fn finish(mut self) {
        self.link().put(&Code::default());
        assert_eq!(self.serve(), protocol::Request::Done);
    }
}

/// NZCV's bits in a saved state.
pub fn nzcv() -> u64 {
    stop::condition_flags()
}

/// The hardware and the register page at the start of a case: the withheld registers' guest values in the
/// page, the count at 0, the page's address in its register.
fn start(case: &Case) -> (Hardware, RegisterPage) {
    let mut x = [0u64; GENERAL];
    x.copy_from_slice(&case.registers[..GENERAL]);
    let mut withheld = [0u64; WITHHELD.len()];
    for (slot, &r) in withheld.iter_mut().zip(&WITHHELD) {
        *slot = x[usize::from(r)];
        x[usize::from(r)] = 0;
    }
    x[usize::from(PAGE)] = REGISTER_PAGE;
    let page = RegisterPage {
        withheld,
        saved: Default::default(),
        exclusive_bytes: 0,
        exclusive_address: 0,
        exclusive_low: 0,
        exclusive_high: 0,
    };
    let hardware = Hardware {
        x,
        sp: case.registers[CASE_SP],
        pc: PIECES,
        state: case.registers[CASE_FLAGS],
        thread: case.registers[CASE_THREAD],
        v: case.vectors,
        fpsr: case.float[0],
        fpcr: case.float[1],
    };
    (hardware, page)
}

fn outcome(stop: Stop, state: &GuestState, changed: Vec<(u32, u8)>) -> Outcome {
    let mut registers = [0u64; OUT_THREAD + 1];
    registers[..GENERAL].copy_from_slice(&state.x);
    registers[CASE_SP] = state.sp;
    registers[OUT_PC] = state.pc;
    registers[OUT_FLAGS] = state.flags;
    registers[OUT_THREAD] = state.thread;
    Outcome {
        stop,
        registers,
        vectors: state.v,
        float: state.float,
        changed,
    }
}

/// When the runs began, for the clock a counter read gets.
fn started() -> std::time::Instant {
    static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    *STARTED.get_or_init(std::time::Instant::now)
}

/// What a stop comes to.
pub enum Resolved {
    /// The guest stopped there.
    Stop(Stop, Box<GuestState>),
    /// The inside answered; the guest's level goes on from this.
    Again(Box<Hardware>, RegisterPage),
}

/// The inside's view of the hardware: all but the vector registers and the thread pointer.
pub fn interrupted_of(h: &Hardware) -> Interrupted {
    Interrupted {
        x: h.x,
        sp: h.sp,
        pc: h.pc,
        state: h.state,
        fpsr: h.fpsr,
        fpcr: h.fpcr,
    }
}

/// The hardware from the inside's view of it, the vector registers and thread pointer from `rest`.
pub fn hardware_of(i: Interrupted, rest: &Hardware) -> Hardware {
    Hardware {
        x: i.x,
        sp: i.sp,
        pc: i.pc,
        state: i.state,
        thread: rest.thread,
        v: rest.v,
        fpsr: i.fpsr,
        fpcr: i.fpcr,
    }
}

/// The whole guest state from the inside's resolution, with the hardware's vectors and thread pointer and the
/// thread's float status.
pub fn whole(state: stop::GuestState, h: &Hardware, float: &FloatStatus) -> GuestState {
    GuestState {
        x: state.x,
        sp: state.sp,
        pc: state.pc,
        flags: state.flags,
        thread: h.thread,
        v: h.v,
        float: [float.fpsr, float.fpcr],
        count: state.count,
        exclusive: state.exclusive,
    }
}

/// A load or store's operand, as the position map has it.
fn operand_of(i: &I) -> Operand {
    match Arm64::stopping(i, CODE) {
        Stopping::Access(operand) => operand,
        other => panic!("{i:?} is no access: {other:?}"),
    }
}

/// The first byte of a guest access with the guest's registers `x` and stack pointer `sp`, untagged.
fn access_address(i: &I, at: u64, x: &[u64; GENERAL], sp: u64) -> u64 {
    untag_address(stop::access_first(&operand_of(i), at, x, sp))
}

/// The state at the start of a piece: the guest's, before anything of it ran.
pub fn entry_state(
    span: &Span,
    hardware: &Hardware,
    page: &RegisterPage,
    float: &FloatStatus,
) -> GuestState {
    whole(
        stop::guest_state(span, &interrupted_of(hardware), page),
        hardware,
        float,
    )
}

/// The page boundaries of the data area where a page refuses: below it nothing is mapped, then the writable
/// pages, the read-only page, and nothing again.
pub const REFUSING: [u64; 3] = [DATA, DATA + DATA_WRITABLE, DATA + DATA_BYTES];

/// The kinds of the bar's own cases made from a drawn load or store.
const EDGE_KINDS: u64 = 4;

/// The bar's own case made from a drawn case of a load or store (docs/native.md section 9, N2), chosen by the
/// case's seed: its access placed across one of the data area's boundaries where a page refuses, at an offset
/// within it; the same with a tag in the base's top byte; the base with bit 55 set; or, for a form that checks
/// alignment, the base moved off it. `None` for a form with no base register to move.
pub fn edge(case: &Case) -> Option<Case> {
    let seed = case.seed;
    let (i, bytes, _, _) = base_of(case)?;
    let within = (seed >> u8::BITS) % bytes.max(2);
    let boundary = REFUSING[(seed >> u16::BITS) as usize % REFUSING.len()];
    let mut edge = across(case, boundary, within)?;
    let (_, _, slot, crossing) = base_of(&edge)?;
    let original = case.registers[slot];
    let tag = (seed >> u32::BITS) << TAG_LOW;
    edge.registers[slot] = match seed % EDGE_KINDS {
        0 => crossing,
        1 => crossing | tag & !(1 << (TAG_LOW - 1)),
        2 => original | tag | 1 << (TAG_LOW - 1),
        _ if matches!(Arm64::route(&i), Route::Aligned | Route::Exclusive) => {
            original + within.max(1)
        }
        _ => crossing,
    };
    Some(edge)
}

/// A load or store case's instruction, its access's bytes, the slot of its base register in the case's
/// registers and the base's value; `None` for an instruction with no base register to move.
fn base_of(case: &Case) -> Option<(I, u64, usize, u64)> {
    let i = decode(case.word);
    let m = i.memory_access()?;
    let (base, index) = match m.address {
        Address::Offset { base, .. }
        | Address::PreIndex { base, .. }
        | Address::PostIndex { base, .. } => (base, None),
        Address::Indexed { base, index, .. } => (base, Some(index)),
        Address::Literal { .. } => return None,
    };
    if index == Some(base) {
        return None;
    }
    let slot = match base {
        Reg::X(n) => usize::from(n),
        Reg::Sp => CASE_SP,
        Reg::Zero => return None,
    };
    Some((i, m.bytes.into(), slot, case.registers[slot]))
}

/// The case with its access's base moved so that the access starts `within` bytes before `boundary`.
pub fn across(case: &Case, boundary: u64, within: u64) -> Option<Case> {
    let (i, _, slot, _) = base_of(case)?;
    let mut x = [0u64; GENERAL];
    x.copy_from_slice(&case.registers[..GENERAL]);
    let mut sp = case.registers[CASE_SP];
    if slot == CASE_SP {
        sp = 0;
    } else {
        x[slot] = 0;
    }
    // The access's first byte with a base of zero.
    let delta = stop::access_first(&operand_of(&i), CODE, &x, sp);
    let mut moved = case.clone();
    moved.registers[slot] = boundary.wrapping_sub(within).wrapping_sub(delta);
    Some(moved)
}

/// The bytes a case's access moves, when it is a load or store with a base register.
pub fn access_bytes(case: &Case) -> Option<u64> {
    base_of(case).map(|(_, bytes, ..)| bytes)
}

/// The stop a host exception in `piece`, placed at `base`, comes to, through the inside's resolution; a
/// counter read is answered with nanoseconds since the runs began, as the WebAssembly runner answers its clock.
fn resolve(
    piece: &HostCode,
    base: u64,
    trapped: &Trapped,
    float: &mut FloatStatus,
    memory: &mut dyn FnMut(u64, &mut [u8]),
) -> Resolved {
    let h = interrupted_of(&trapped.hardware);
    let exception = stop::Exception {
        syndrome: trapped.syndrome,
        fault: trapped.fault,
        hardware: &h,
        page: &trapped.page,
    };
    let placed = stop::Piece {
        base,
        length: piece.bytes.len(),
        spans: &piece.spans,
    };
    match stop::resolve(&placed, &exception, float, memory) {
        Ok(stop::Resolved::Stop(stopped, state)) => {
            let stop = match stopped {
                stop::Stop::Landed(to) => Stop::Landed(to),
                stop::Stop::Syscall => Stop::Syscall,
                stop::Stop::Breakpoint(_) => Stop::Breakpoint,
                stop::Stop::Illegal => Stop::Illegal,
                stop::Stop::Alignment(at) => Stop::Alignment(at),
                stop::Stop::MemoryFault { address, .. } => Stop::MemoryFault(address),
            };
            Resolved::Stop(stop, Box::new(whole(state, &trapped.hardware, float)))
        }
        Ok(stop::Resolved::Again(next, page)) => {
            Resolved::Again(Box::new(hardware_of(next, &trapped.hardware)), page)
        }
        Ok(stop::Resolved::Clock {
            into,
            mut then,
            mut page,
        }) => {
            let now = started().elapsed().as_nanos() as u64;
            if let Some(n) = into {
                stop::set_register(n, now, &mut then, &mut page);
            }
            Resolved::Again(Box::new(hardware_of(then, &trapped.hardware)), page)
        }
        Err(unexpected) => {
            let class = ESR_EL1::EC.read(unexpected.syndrome);
            Resolved::Stop(
                Stop::Other(class as u32, unexpected.fault),
                Box::new(entry_state(
                    &piece.spans[0],
                    &trapped.hardware,
                    &trapped.page,
                    float,
                )),
            )
        }
    }
}
