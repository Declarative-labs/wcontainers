//! Guest functions: a piece for each instruction's address at one look-ahead, placed one after another and
//! run from piece to piece, with an interrupt forced at any host instruction the guest's level is about to run,
//! found by software step and at every return from the inside (docs/native.md section 4).

use wcontainers_abi::{Access, Permissions};
use wcontainers_translator_test::runner::{Machine, Stopped, Syscalls};

use super::*;

/// The piece that starts at one guest address, in the code area.
struct Placed {
    guest: u64,
    va: u64,
    code: HostCode,
}

impl Placed {
    /// Points the hardware at the piece, entered as the inside enters it.
    fn enter(&self, hardware: &mut Hardware) {
        hardware.pc = self.va;
        enter_piece(&self.code, self.guest, hardware);
    }
}

/// A function's words and pieces.
pub struct Function {
    pub entry: u64,
    words: Vec<u32>,
    pieces: Vec<Placed>,
}

/// How a function ran.
pub struct Run {
    /// The guest's state at each instruction boundary reached: the first at the entry, the last at the stop.
    pub trace: Vec<GuestState>,
    pub stop: Stop,
    /// The landing points counted: each host instruction the guest's level was about to run, reached by a
    /// step or by a return from the inside.
    pub landings: u64,
    /// The state a forced interrupt resolved to, and how many boundaries had been passed before it.
    pub interrupted: Option<(GuestState, usize)>,
    /// The data area at the end.
    pub data: Vec<u8>,
}

/// The guest's state carried back to the hardware and the register page, as the inside returns to it.
fn rejoin(state: &GuestState, page: RegisterPage, float: &FloatStatus) -> (Hardware, RegisterPage) {
    let inside = stop::GuestState {
        x: state.x,
        sp: state.sp,
        pc: state.pc,
        flags: state.flags,
        count: state.count,
        exclusive: state.exclusive,
    };
    let (h, page) = stop::rejoin(&inside, page, float, REGISTER_PAGE);
    let hardware = Hardware {
        x: h.x,
        sp: h.sp,
        pc: h.pc,
        state: h.state,
        thread: state.thread,
        v: state.v,
        fpsr: h.fpsr,
        fpcr: h.fpcr,
    };
    (hardware, page)
}

/// The guest's state at an interrupt that landed at the hardware's pc while `p` ran; `memory` reads guest
/// memory for the fix-ups.
fn interrupted(
    p: &Placed,
    h: &Hardware,
    page: &RegisterPage,
    float: &mut FloatStatus,
    memory: &mut dyn FnMut(u64, &mut [u8]),
) -> GuestState {
    let piece = stop::Piece {
        base: p.va,
        length: p.code.bytes.len(),
        spans: &p.code.spans,
    };
    let state = stop::interrupted(&piece, &interrupted_of(h), page, float, memory)
        .expect("an interrupt in a piece or at its jump's slot");
    whole(state, h, float)
}

impl Runner {
    /// Writes `words` at the guest's code address and, at `look_ahead`, the piece of every instruction's address
    /// in the code area, and enters the gate.
    pub fn load_function(&mut self, words: &[u32], look_ahead: LookAhead) -> Function {
        let image: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let mut bytes = Vec::new();
        let mut pieces = Vec::new();
        for k in 0..words.len() {
            let from = k * Arm64::MAX_LENGTH as usize;
            let guest = CODE + from as u64;
            let code = translate_native::<Arm64>(&image[from..], guest, look_ahead, true)
                .expect("a decoded word");
            pieces.push(Placed {
                guest,
                va: PIECES + bytes.len() as u64,
                code: HostCode::default(),
            });
            bytes.extend_from_slice(&code.bytes);
            pieces.last_mut().expect("just placed").code = code;
        }
        self.write(CODE, &image);
        self.load(&bytes);
        Function {
            entry: CODE,
            words: words.to_vec(),
            pieces,
        }
    }

    /// Runs `function` from the state of `case` (its word unused) until it returns to `returns` or stops. With a
    /// `budget`, every landing point is counted (each host instruction the guest's level is about to run, reached
    /// by a step or by a return from the inside) and an interrupt is taken at the one of that index, after which
    /// the guest goes on from the state it resolved to, as the inside returns to it.
    pub fn run_function(
        &mut self,
        function: &Function,
        case: &Case,
        returns: u64,
        budget: Option<u64>,
    ) -> Run {
        let mut data = vec![0u8; DATA_BYTES as usize];
        fill(case.seed, &mut data);
        self.write(DATA, &data);
        let (mut hardware, mut page) = start(case);
        let mut block = FloatStatus {
            fpsr: case.float[0],
            fpcr: case.float[1],
            tracking: case.tracking,
        };
        let at = |guest: u64| function.pieces.iter().position(|p| p.guest == guest);
        let mut current = at(function.entry).expect("the entry has a piece");
        let mut trace = vec![entry_state(function.entry, &hardware, &page, &block)];
        function.pieces[current].enter(&mut hardware);
        let end = |runner: &Runner, trace, stop, landings, interrupted| {
            let mut now = vec![0u8; DATA_BYTES as usize];
            runner.read(DATA, &mut now);
            Run {
                trace,
                stop,
                landings,
                interrupted,
                data: now,
            }
        };
        let mut stepping = budget.is_some();
        let mut landings = 0;
        // Whether the hardware's pc is a landing point no step reached: the entry and every return from the
        // inside.
        let mut returned = true;
        let mut interrupt: Option<(GuestState, usize)> = None;
        loop {
            let mut taken = None;
            if stepping && returned {
                if Some(landings) == budget {
                    taken = Some((hardware.clone(), page.clone()));
                }
                landings += 1;
            }
            if taken.is_none() {
                let trapped = self.enter(&hardware, &page, stepping);
                returned = false;
                let class = ESR_EL1::EC.read(trapped.syndrome);
                if class == ESR_EL1::EC::Value::SoftwareStepLowerEL as u64 {
                    if Some(landings) == budget {
                        taken = Some((trapped.hardware, trapped.page));
                    } else {
                        // An interrupt that is no stop goes back to the host instruction it came from, with
                        // the hardware as it was.
                        hardware = trapped.hardware;
                        page = trapped.page;
                    }
                    landings += 1;
                } else {
                    let p = &function.pieces[current];
                    let mut memory = |va, bytes: &mut [u8]| self.read(va, bytes);
                    match resolve(&p.code, p.va, &trapped, &mut block, &mut memory) {
                        Resolved::Again(h, next) => {
                            (hardware, page) = (*h, next);
                            returned = true;
                        }
                        Resolved::Stop(Stop::Landed(to), state) => {
                            trace.push((*state).clone());
                            match at(to).filter(|_| to != returns) {
                                Some(k) => {
                                    current = k;
                                    (hardware, page) = rejoin(&state, trapped.page, &block);
                                    function.pieces[k].enter(&mut hardware);
                                    returned = true;
                                }
                                None => {
                                    return end(self, trace, Stop::Landed(to), landings, interrupt);
                                }
                            }
                        }
                        Resolved::Stop(stop, state) => {
                            trace.push(*state);
                            return end(self, trace, stop, landings, interrupt);
                        }
                    }
                }
            }
            let Some((h, interrupted_page)) = taken else {
                continue;
            };
            // A stop: the inside takes the guest's float status as at every entry, and the guest goes on
            // from the state it resolved to, as the inside returns to it, at that address's piece.
            let p = &function.pieces[current];
            let state = interrupted(p, &h, &interrupted_page, &mut block, &mut |va, b| {
                self.read(va, b)
            });
            stepping = false;
            interrupt = Some((state.clone(), trace.len() - 1));
            match at(state.pc).filter(|_| state.pc != returns) {
                Some(k) => {
                    current = k;
                    (hardware, page) = rejoin(&state, interrupted_page, &block);
                    function.pieces[k].enter(&mut hardware);
                }
                None => {
                    let to = state.pc;
                    trace.push(state);
                    return end(self, trace, Stop::Landed(to), landings, interrupt);
                }
            }
        }
    }
}

/// The WebAssembly output's run of `words` at the code address from the state of `case`, at `look_ahead`: its
/// stop, registers and data area.
pub fn webassembly(
    words: &[u32],
    case: &Case,
    look_ahead: LookAhead,
) -> (Stop, RegisterBlock, Vec<u8>) {
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let slots = u32::try_from(words.len()).expect("a short function");
    let mut machine = Machine::new(CODE + CODE_SPAN, CODE, slots, Syscalls::Stop);
    machine.set_permissions(
        DATA..DATA + DATA_WRITABLE,
        Permissions::READ | Permissions::WRITE,
    );
    machine.set_permissions(DATA + DATA_WRITABLE..DATA + DATA_BYTES, Permissions::READ);
    machine.set_permissions(
        CODE - CODE_SPAN..CODE + CODE_SPAN,
        Permissions::READ | Permissions::EXECUTE,
    );
    let mut data = vec![0u8; DATA_BYTES as usize];
    fill(case.seed, &mut data);
    machine.write(DATA, &data);
    let starts: Vec<u64> = (0..bytes.len() as u64)
        .step_by(Arm64::MAX_LENGTH as usize)
        .collect();
    machine.load(&bytes, 0..bytes.len() as u64, CODE, &starts, look_ahead);
    let mut registers = RegisterBlock::ZERO;
    registers.x.copy_from_slice(&case.registers[..GENERAL]);
    registers.sp = case.registers[CASE_SP];
    registers.pc = CODE;
    [registers.fpsr, registers.fpcr] = case.float;
    registers.pstate = case.registers[CASE_FLAGS];
    registers.tpidr_el0 = case.registers[CASE_THREAD];
    registers.v = case.vectors.map(|v| [v as u64, (v >> u64::BITS) as u64]);
    machine.set_registers(&registers);
    if case.tracking {
        machine.track_float_flags();
    }
    let (stopped, after) = machine.run();
    machine.read(DATA, &mut data);
    let stop = match stopped {
        Stopped::NoCode(address)
        | Stopped::Exit(wcontainers_abi::Exit::MemoryFault {
            address,
            access: Access::Execute,
        }) => Stop::Landed(address),
        Stopped::Exit(wcontainers_abi::Exit::MemoryFault { address, .. }) => {
            Stop::MemoryFault(address)
        }
        Stopped::Exit(wcontainers_abi::Exit::AlignmentFault { address }) => {
            Stop::Alignment(address)
        }
        other => panic!("not a stop these functions make: {other:?}"),
    };
    (stop, after, data)
}

/// Whether the WebAssembly output's end is the native one's.
pub fn same_end(stop: Stop, after: &RegisterBlock, native_stop: Stop, last: &GuestState) -> bool {
    stop == native_stop
        && after.x == last.x
        && after.sp == last.sp
        && after.pc == last.pc
        && after
            .v
            .map(|[low, high]| u128::from(high) << u64::BITS | u128::from(low))
            == last.v
        && [after.fpsr, after.fpcr] == last.float
        && after.pstate == last.flags
        && after.tpidr_el0 == last.thread
}
