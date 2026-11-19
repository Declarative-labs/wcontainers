//! Guest functions: a piece for each instruction's address at one look-ahead, placed one after another and
//! run from piece to piece, with an interrupt forced after any number of host instructions by software step
//! (docs/native.md section 4).

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
}

/// How a function ran.
pub struct Run {
    /// The guest's state at each instruction boundary reached: the first at the entry, the last at the stop.
    pub trace: Vec<GuestState>,
    pub stop: Stop,
    /// Host instructions stepped.
    pub steps: u64,
    /// The state a forced interrupt resolved to, and how many boundaries had been passed before it.
    pub interrupted: Option<(GuestState, usize)>,
    /// The data area at the end.
    pub data: Vec<u8>,
}

/// The guest's state carried back to the hardware and the register page, as the inside returns to it.
fn rejoin(state: &GuestState, page: RegisterPage, block: &Block) -> (Hardware, RegisterPage) {
    let mut page = with_exclusive(page, state);
    let mut x = state.x;
    for (slot, &r) in page.withheld.iter_mut().zip(&WITHHELD) {
        *slot = x[usize::from(r)];
        x[usize::from(r)] = 0;
    }
    x[usize::from(COUNT)] = state.count;
    x[usize::from(PAGE)] = REGISTER_PAGE;
    let hardware = Hardware {
        x,
        sp: state.sp,
        pc: 0,
        state: state.flags,
        thread: state.thread,
        v: state.v,
        fpsr: block.fpsr,
        fpcr: block.fpcr,
    };
    (hardware, page)
}

/// The guest's state at an interrupt that landed at the hardware's pc while `p` ran; `memory` reads guest
/// memory for the fix-ups.
fn interrupted(
    p: &Placed,
    h: &Hardware,
    page: &RegisterPage,
    block: &mut Block,
    memory: &dyn Fn(u64, &mut [u8]),
) -> GuestState {
    if h.pc < p.va || h.pc >= p.va + p.code.bytes.len() as u64 {
        // The jump is done and the guest is at its target's entry slot.
        let span = Span {
            relocated: Vec::new(),
            lead: 0,
            side: Side::After,
            ..p.code.spans.last().expect("a span").clone()
        };
        let mut state = guest_state(&span, h, page, block);
        state.pc = slot_target(h.pc);
        return state;
    }
    let span = p.code.span((h.pc - p.va) as u32).expect("a span");
    if span.side == Side::Before {
        return guest_state(span, h, page, block);
    }
    let (fixed, fixed_page) = fixed_up(span, h, page, block, memory);
    let mut state = guest_state(span, &fixed, &fixed_page, block);
    state.pc = resume_address(span, h, page);
    state
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
            let code = translate_native::<Arm64>(&image[from..], guest, look_ahead)
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

    /// Runs `function` from the state of `case` (its word unused) until it returns to `returns` or stops,
    /// stepping every host instruction when `budget` is given and resolving an interrupt after that many, then
    /// going on from the state the interrupt resolved to, as the inside returns to it.
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
        let mut block = Block {
            fpsr: case.float[0],
            fpcr: case.float[1],
            tracking: case.tracking,
        };
        let at = |guest: u64| function.pieces.iter().position(|p| p.guest == guest);
        let mut current = at(function.entry).expect("the entry has a piece");
        hardware.pc = function.pieces[current].va;
        let entry_span = &function.pieces[current].code.spans[0];
        let mut trace = vec![guest_state(entry_span, &hardware, &page, &block)];
        let mut steps = 0;
        let end = |runner: &Runner, trace, stop, steps, interrupted| {
            let mut now = vec![0u8; DATA_BYTES as usize];
            runner.read(DATA, &mut now);
            Run {
                trace,
                stop,
                steps,
                interrupted,
                data: now,
            }
        };
        let mut stepping = budget.is_some();
        let mut interrupt: Option<(GuestState, usize)> = None;
        if budget == Some(0) {
            let p = &function.pieces[current];
            let state = interrupted(p, &hardware, &page, &mut block, &|va, b| self.read(va, b));
            interrupt = Some((state, 0));
            stepping = false;
        }
        // An interrupt that is a stop: the guest goes on from the state it resolved to, as the inside
        // returns to it, at that address's piece.
        if let Some((state, _)) = &interrupt {
            match at(state.pc).filter(|_| state.pc != returns) {
                Some(k) => {
                    current = k;
                    (hardware, page) = rejoin(state, page.clone(), &block);
                    hardware.pc = function.pieces[k].va;
                }
                None => {
                    let to = state.pc;
                    trace.push(state.clone());
                    return end(self, trace, Stop::Landed(to), 0, interrupt);
                }
            }
        }
        loop {
            let trapped = self.enter(&hardware, &page, stepping);
            let class = ESR_EL1::EC.read(trapped.syndrome);
            if class == ESR_EL1::EC::Value::SoftwareStepLowerEL as u64 {
                steps += 1;
                if Some(steps) == budget {
                    // A stop: the inside takes the guest's float status as at every entry.
                    block.enter(trapped.hardware.fpsr);
                    let p = &function.pieces[current];
                    let state =
                        interrupted(p, &trapped.hardware, &trapped.page, &mut block, &|va, b| {
                            self.read(va, b)
                        });
                    stepping = false;
                    match at(state.pc).filter(|_| state.pc != returns) {
                        Some(k) => {
                            current = k;
                            (hardware, page) = rejoin(&state, trapped.page, &block);
                            hardware.pc = function.pieces[k].va;
                            interrupt = Some((state, trace.len() - 1));
                        }
                        None => {
                            let to = state.pc;
                            interrupt = Some((state.clone(), trace.len() - 1));
                            trace.push(state);
                            return end(self, trace, Stop::Landed(to), steps, interrupt);
                        }
                    }
                    continue;
                }
                // An interrupt that is no stop goes back to the host instruction it came from, with the
                // hardware as it was.
                hardware = trapped.hardware;
                page = trapped.page;
                continue;
            }
            let p = &function.pieces[current];
            match resolve(&p.code, p.va, &function.words, &trapped, &mut block) {
                Resolved::Again(h, next) => (hardware, page) = (*h, next),
                Resolved::Stop(Stop::Landed(to), state) => {
                    trace.push((*state).clone());
                    match at(to).filter(|_| to != returns) {
                        Some(k) => {
                            current = k;
                            (hardware, page) = rejoin(&state, trapped.page, &block);
                            hardware.pc = function.pieces[k].va;
                        }
                        None => return end(self, trace, Stop::Landed(to), steps, interrupt),
                    }
                }
                Resolved::Stop(stop, state) => {
                    trace.push(*state);
                    return end(self, trace, stop, steps, interrupt);
                }
            }
        }
    }
}
