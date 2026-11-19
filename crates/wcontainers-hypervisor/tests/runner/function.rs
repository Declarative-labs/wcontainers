//! Guest functions: each instruction's piece placed one after another and run from piece to piece, with an
//! interrupt forced after any number of host instructions by software step (docs/native.md section 4).

use super::*;

/// One instruction's piece in the code area.
struct Placed {
    guest: u64,
    va: u64,
    instruction: I,
    code: HostCode,
}

/// A function's pieces.
pub struct Function {
    pub entry: u64,
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
fn rejoin(state: &GuestState, mut page: RegisterPage, block: &Block) -> (Hardware, RegisterPage) {
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

/// The guest's state at an interrupt that landed at the hardware's pc while `p` ran.
fn interrupted(p: &Placed, h: &Hardware, page: &RegisterPage, block: &mut Block) -> GuestState {
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
    let mut state = guest_state(span, h, page, block);
    if span.side == Side::After {
        state.pc = resume_address(span, h, page);
        for fixup in &span.fixups {
            match *fixup {
                Fixup::Constant { register, value } => state.x[usize::from(register)] = value,
                Fixup::Forget { status, read } => {
                    let remembered = [page.exclusive_low, page.exclusive_high];
                    let swapped = read[0].is_some()
                        && read
                            .iter()
                            .zip(remembered)
                            .all(|(r, m)| r.is_none_or(|r| held(r, h, page) == m));
                    state.x[usize::from(status)] = u64::from(!swapped);
                }
                // Only the exclusive monitor changes, which the register page holds.
                Fixup::Remember { .. } => {}
                Fixup::FloatStatus => {
                    block.fpsr = h.fpsr;
                    block.fpcr = h.fpcr & FPCR_BITS;
                    state.float = [block.fpsr, block.fpcr];
                }
            }
        }
    }
    state
}

impl Runner {
    /// Writes `words` at the guest's code address and their pieces in the code area, and enters the gate.
    pub fn load_function(&mut self, words: &[u32]) -> Function {
        let mut bytes = Vec::new();
        let mut pieces = Vec::new();
        for (k, &word) in words.iter().enumerate() {
            let guest = CODE + k as u64 * Arm64::MAX_LENGTH;
            let code =
                translate_native::<Arm64>(&word.to_le_bytes(), guest).expect("a decoded word");
            pieces.push(Placed {
                guest,
                va: PIECES + bytes.len() as u64,
                instruction: decode(word),
                code: HostCode::default(),
            });
            bytes.extend_from_slice(&code.bytes);
            pieces.last_mut().expect("just placed").code = code;
        }
        let image: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        self.write(CODE, &image);
        self.load(&bytes);
        Function {
            entry: CODE,
            pieces,
        }
    }

    /// Runs `function` from the state of `case` (its word unused) until it returns to `returns` or stops,
    /// stepping every host instruction when `budget` is given and resolving an interrupt after that many.
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
        if budget == Some(0) {
            let state = interrupted(&function.pieces[current], &hardware, &page, &mut block);
            let passed = trace.len() - 1;
            return end(self, trace, Stop::Other(0, 0), 0, Some((state, passed)));
        }
        loop {
            let trapped = self.enter(&hardware, &page, budget.is_some());
            let class = ESR_EL1::EC.read(trapped.syndrome);
            if class == ESR_EL1::EC::Value::SoftwareStepLowerEL as u64 {
                steps += 1;
                block.enter(trapped.hardware.fpsr);
                if Some(steps) == budget {
                    let p = &function.pieces[current];
                    let state = interrupted(p, &trapped.hardware, &trapped.page, &mut block);
                    let passed = trace.len() - 1;
                    return end(self, trace, Stop::Other(0, 0), steps, Some((state, passed)));
                }
                hardware = trapped.hardware;
                hardware.fpsr = block.fpsr;
                page = trapped.page;
                continue;
            }
            let p = &function.pieces[current];
            match resolve(&p.code, p.va, &p.instruction, &trapped, &mut block) {
                Resolved::Again(h, next) => (hardware, page) = (*h, next),
                Resolved::Stop(Stop::Landed(to), state) => {
                    trace.push((*state).clone());
                    match at(to).filter(|_| to != returns) {
                        Some(k) => {
                            current = k;
                            (hardware, page) = rejoin(&state, trapped.page, &block);
                            hardware.pc = function.pieces[k].va;
                        }
                        None => return end(self, trace, Stop::Landed(to), steps, None),
                    }
                }
                Resolved::Stop(stop, state) => {
                    trace.push(*state);
                    return end(self, trace, stop, steps, None);
                }
            }
        }
    }
}
