//! Guest functions: a piece for each instruction's address at one look-ahead, placed one after another and
//! run from piece to piece, with an interrupt forced at any host instruction the guest's level is about to run,
//! found by software step and at every return from the inside (docs/native.md section 4).

use wcontainers_abi::{Access, Permissions};
use wcontainers_arm64::carry::{SCRATCH, link_words};
use wcontainers_arm64::mirror::{
    LAST_TARGET_BRANCH, LAST_TARGET_CALL, LAST_TARGET_TEST, SLOT_MOVE_BITS, SLOT_MOVES,
};
use wcontainers_translator_test::runner::{Machine, Stopped, Syscalls};

use super::*;

/// The piece that starts at one guest address, in the code area.
struct Placed {
    guest: u64,
    va: u64,
    code: HostCode,
}

/// A function's words and pieces.
pub struct Function {
    pub entry: u64,
    words: Vec<u32>,
    pieces: Vec<Placed>,
    /// Every piece's bytes, one after another from [`PIECES`], as loaded.
    bytes: Vec<u8>,
    links: Links,
}

/// Which of a function's branches go straight to their targets' pieces, as the inside's loader points them once
/// both are loaded, and whether a return's or call's first miss makes its last target, as the inside does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Links {
    pub exits: bool,
    pub tables: bool,
    pub last_targets: bool,
}

/// A host instruction the guest's level was about to run: its address, its word and the guest instruction whose
/// host code it is when it is one of the pieces' (none at an entry slot, which this runner leaves unmapped), and the
/// guest state the position map gives there.
#[derive(Clone, Debug)]
pub struct Landing {
    pub host: u64,
    pub word: Option<u32>,
    pub owner: Option<u64>,
    pub state: GuestState,
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
    /// Every landing point reached while stepping, in order.
    pub landed: Vec<Landing>,
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

impl Function {
    /// The piece at guest address `guest`.
    fn piece_at(&self, guest: u64) -> Option<usize> {
        self.pieces.iter().position(|p| p.guest == guest)
    }

    /// The piece whose host bytes hold `host`.
    fn piece_holding(&self, host: u64) -> Option<usize> {
        self.pieces
            .iter()
            .position(|p| (p.va..p.va + p.code.bytes.len() as u64).contains(&host))
    }

    /// The word at host address `host`, when it is in the pieces.
    fn word(&self, host: u64) -> Option<u32> {
        let at = usize::try_from(host.checked_sub(PIECES)?).ok()?;
        let bytes = self.bytes.get(at..at + size_of::<u32>())?;
        Some(u32::from_le_bytes(bytes.try_into().expect("a word")))
    }

    /// Writes the word at host address `at` that branches to the piece at `to`: the word's own offset changed where
    /// `retarget` is set, else a direct branch, as the inside's loader links a site.
    fn link_site(&mut self, at: u64, to: u64, retarget: bool) {
        let offset = to as i64 - at as i64;
        let word = if retarget {
            encode::retarget(self.word(at).expect("a site in the pieces"), offset)
        } else {
            encode::branch(false, offset)
        }
        .expect("the pieces are near each other");
        let from = (at - PIECES) as usize;
        self.bytes[from..from + size_of::<u32>()].copy_from_slice(&word.to_le_bytes());
    }

    /// Points the exits and tables [`Links`] names at their targets' pieces.
    fn link(&mut self) {
        let mut sites = Vec::new();
        for p in &self.pieces {
            if self.links.exits {
                for exit in &p.code.exits {
                    sites.push((
                        p.va + u64::from(exit.start),
                        exit.target,
                        exit.retarget.is_some(),
                    ));
                }
            }
            if self.links.tables {
                for table in &p.code.tables {
                    for k in 0..table.entries {
                        let at = p.va + u64::from(table.start + k * size_of::<u32>() as u32);
                        sites.push((at, table.target(k), true));
                    }
                }
            }
        }
        for (at, target, retarget) in sites {
            if let Some(k) = self.piece_at(target) {
                let to = self.pieces[k].va;
                self.link_site(at, to, retarget);
            }
        }
    }

    /// Makes `target` the last target of the jump whose call into the inside is at host address `call`, as the
    /// inside does: its moves and test written, its direct branch linked when exits are.
    fn make_last_target(&mut self, call: u64, target: u64) {
        let word = |index: usize| call - ((LAST_TARGET_CALL - index) as u64 * Arm64::MAX_LENGTH);
        let words = link_words();
        let mut written: Vec<(u64, u32)> = (0..SLOT_MOVES)
            .map(|k| {
                let half = k as u32;
                let imm16 = (target >> (half * SLOT_MOVE_BITS)) as u16;
                let word_k = if k == 0 {
                    encode::movz(SCRATCH, imm16, half)
                } else {
                    encode::movk(SCRATCH, imm16, half)
                };
                (word(k), word_k)
            })
            .collect();
        written.push((word(LAST_TARGET_TEST), words.last_target_test));
        for (at, w) in written {
            let from = (at - PIECES) as usize;
            self.bytes[from..from + size_of::<u32>()].copy_from_slice(&w.to_le_bytes());
        }
        if self.links.exits
            && let Some(k) = self.piece_at(target)
        {
            let to = self.pieces[k].va;
            self.link_site(word(LAST_TARGET_BRANCH), to, false);
        }
    }
}

impl Runner {
    /// Writes `words` at the guest's code address and, at `look_ahead`, the piece of every instruction's address
    /// in the code area, and enters the gate.
    pub fn load_function(&mut self, words: &[u32], look_ahead: LookAhead) -> Function {
        self.load_linked(words, look_ahead, true, Links::default())
    }

    /// As [`Self::load_function`], with the count when `exact_replay`, and the branches `links` names pointed at
    /// their targets' pieces with the words the inside's loader writes.
    pub fn load_linked(
        &mut self,
        words: &[u32],
        look_ahead: LookAhead,
        exact_replay: bool,
        links: Links,
    ) -> Function {
        let image: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let mut bytes = Vec::new();
        let mut pieces = Vec::new();
        for k in 0..words.len() {
            let from = k * Arm64::MAX_LENGTH as usize;
            let guest = CODE + from as u64;
            let code = translate_native::<Arm64>(&image[from..], guest, look_ahead, exact_replay)
                .expect("a decoded word");
            pieces.push(Placed {
                guest,
                va: PIECES + bytes.len() as u64,
                code: HostCode::default(),
            });
            bytes.extend_from_slice(&code.bytes);
            pieces.last_mut().expect("just placed").code = code;
        }
        let mut function = Function {
            entry: CODE,
            words: words.to_vec(),
            pieces,
            bytes,
            links,
        };
        function.link();
        self.write_code(words);
        self.load(&function.bytes);
        function
    }

    /// Runs `function` from the state of `case` (its word unused) until it returns to `returns` or stops. With a
    /// `budget`, every landing point is counted (each host instruction the guest's level is about to run, reached
    /// by a step or by a return from the inside) and an interrupt is taken at the one of that index, after which
    /// the guest goes on from the state it resolved to, as the inside returns to it.
    pub fn run_function(
        &mut self,
        function: &mut Function,
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
        let mut current = function
            .piece_at(function.entry)
            .expect("the entry has a piece");
        hardware.pc = function.pieces[current].va;
        let entry_span = &function.pieces[current].code.spans[0];
        let mut trace = vec![entry_state(entry_span, &hardware, &page, &block)];
        let end = |runner: &Runner, trace, stop, landings, interrupted, landed| {
            let mut now = vec![0u8; DATA_BYTES as usize];
            runner.read(DATA, &mut now);
            Run {
                trace,
                stop,
                landings,
                interrupted,
                data: now,
                landed,
            }
        };
        let mut landed = Vec::new();
        let mut stepping = budget.is_some();
        let mut landings = 0;
        // Whether the hardware's pc is a landing point no step reached: the entry and every return from the
        // inside.
        let mut returned = true;
        let mut interrupt: Option<(GuestState, usize)> = None;
        loop {
            let mut taken = None;
            if stepping && returned {
                landed.push(landing(function, current, &hardware, &page, &block, self));
                if Some(landings) == budget {
                    taken = Some((hardware.clone(), page.clone()));
                }
                landings += 1;
            }
            if taken.is_none() {
                let trapped = self.enter(&hardware, &page, stepping);
                returned = false;
                let class = ESR_EL1::EC.read(trapped.syndrome);
                // Linked code goes from piece to piece without coming back here.
                if let Some(k) = function.piece_holding(trapped.hardware.pc) {
                    current = k;
                }
                if class == ESR_EL1::EC::Value::SoftwareStepLowerEL as u64 {
                    landed.push(landing(
                        function,
                        current,
                        &trapped.hardware,
                        &trapped.page,
                        &block,
                        self,
                    ));
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
                        Resolved::LastTarget {
                            call,
                            target,
                            then,
                            page: next,
                        } => {
                            if function.links.last_targets {
                                function.make_last_target(call, target);
                                self.unload();
                                self.load(&function.bytes);
                            }
                            (hardware, page) = (*then, next);
                            returned = true;
                        }
                        Resolved::Stop(Stop::Landed(to), state) => {
                            trace.push((*state).clone());
                            match function.piece_at(to).filter(|_| to != returns) {
                                Some(k) => {
                                    current = k;
                                    (hardware, page) = rejoin(&state, trapped.page, &block);
                                    hardware.pc = function.pieces[k].va;
                                    returned = true;
                                }
                                None => {
                                    return end(
                                        self,
                                        trace,
                                        Stop::Landed(to),
                                        landings,
                                        interrupt,
                                        landed,
                                    );
                                }
                            }
                        }
                        Resolved::Stop(stop, state) => {
                            trace.push(*state);
                            return end(self, trace, stop, landings, interrupt, landed);
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
            match function.piece_at(state.pc).filter(|_| state.pc != returns) {
                Some(k) => {
                    current = k;
                    (hardware, page) = rejoin(&state, interrupted_page, &block);
                    hardware.pc = function.pieces[k].va;
                }
                None => {
                    let to = state.pc;
                    trace.push(state);
                    return end(self, trace, Stop::Landed(to), landings, interrupt, landed);
                }
            }
        }
    }
}

/// The landing at the hardware's state `h` while piece `current` runs (or jumped from it to an entry slot).
fn landing(
    function: &Function,
    current: usize,
    h: &Hardware,
    page: &RegisterPage,
    block: &FloatStatus,
    runner: &Runner,
) -> Landing {
    let mut float = *block;
    let state = interrupted(
        &function.pieces[current],
        h,
        page,
        &mut float,
        &mut |va, b| runner.read(va, b),
    );
    let piece = &function.pieces[current];
    let owner = h.pc.checked_sub(piece.va).and_then(|offset| {
        piece
            .code
            .spans
            .iter()
            .rev()
            .find(|span| u64::from(span.start) <= offset)
            .filter(|_| offset < piece.code.bytes.len() as u64)
            .map(|span| span.guest)
    });
    Landing {
        host: h.pc,
        word: function.word(h.pc),
        owner,
        state,
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
