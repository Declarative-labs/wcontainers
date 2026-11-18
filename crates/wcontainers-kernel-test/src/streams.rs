//! The outside of a container's standard streams, as a test drives them: on our kernel the other
//! sides of its ends, in Docker the client's input, output and terminal.

use std::collections::{BTreeMap, BTreeSet};

use rand::Rng;
use rand::rngs::StdRng;
use wcontainers_abi::{Cpu, End, Event};
use wcontainers_kernel::{Kernel, Standard};

/// The ends the test host names.
pub const INPUT: End = End(0);
pub const OUTPUT: End = End(1);
pub const ERROR: End = End(2);
pub const TERMINAL: End = End(3);

/// The output credit of an outside that reads everything as it comes.
pub const UNLIMITED: u64 = u64::MAX;

/// The largest piece the outside sends at once; pieces are drawn from the seed up to this.
const MOST_AT_ONCE: u64 = 1 << 16;

/// How many turns a container that never waits runs before the outside's next piece reaches it;
/// one that waits gets it when it waits, as a real outside's bytes take longer to come than a
/// program takes to reach its next wait.
const BUSY_TURNS: u64 = 10_000;

/// One step of what the outside does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Waits until what the container has shown (its output or error, or all its terminal shows)
    /// holds these bytes after what the last step that waited found.
    Expect(Vec<u8>),
    /// Sends bytes in, in pieces as credit allows.
    Send(Vec<u8>),
    /// Gives the terminal a new size.
    Resize { columns: u16, rows: u16 },
    /// Closes the input: end of file (or, for a terminal, the master's close).
    CloseInput,
    /// Stops reading the output: closes the output end.
    CloseOutput,
}

/// What the outside is and does.
#[derive(Clone, Debug, Default)]
pub struct Outside {
    /// Descriptor 0 takes input from outside (`docker run -i`).
    pub input: bool,
    /// Descriptors 0 to 2 are a terminal of this many columns and rows (`docker run -t`).
    pub terminal: Option<(u16, u16)>,
    pub script: Vec<Step>,
    /// How many bytes of output (not error) it takes at a time, and only when the container can go
    /// on no other way (none at all for 0); none to take everything as it comes.
    pub output_pace: Option<u64>,
    /// Breaks the credit rule on purpose: sends this many bytes past its credit with each piece.
    pub excess: u64,
}

impl Outside {
    /// What the kernel is asked for at start.
    pub fn standard(&self) -> Standard {
        match self.terminal {
            Some((columns, rows)) => Standard::Terminal {
                end: TERMINAL,
                columns,
                rows,
            },
            None => Standard::Pipes {
                input: self.input.then_some(INPUT),
                output: OUTPUT,
                error: ERROR,
            },
        }
    }

    /// The ends the container sends on.
    fn outputs(&self) -> Vec<End> {
        match self.terminal {
            Some(_) => vec![TERMINAL],
            None => vec![OUTPUT, ERROR],
        }
    }

    /// The end the outside sends on, if any.
    fn input_end(&self) -> Option<End> {
        match self.terminal {
            Some(_) => Some(TERMINAL),
            None => self.input.then_some(INPUT),
        }
    }
}

/// The test host's side of the ends.
#[derive(Clone, Debug, Default)]
pub struct Streams {
    pub outside: Outside,
    /// The next step of the script, and what is left to send of a step that sends.
    step: usize,
    sending: Option<Vec<u8>>,
    /// Turns run since the last piece was sent.
    since_sent: u64,
    /// How far into the output and the error the last step that waited found what it waited for.
    found: Found,
    /// Credit the kernel gave on the input end and not yet used, and the most it ever was.
    pub input_credit: u64,
    pub most_input_credit: u64,
    /// Credit given to the kernel on each output end and not yet used.
    given: BTreeMap<End, u64>,
    /// What came out: the output end's (or the terminal's) and the error end's.
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// The ends the kernel closed, and the ones this side closed.
    pub closed_by_kernel: BTreeSet<End>,
    closed_here: BTreeSet<End>,
    input_closed_first: bool,
    /// What the kernel did against the rules: sending past credit, or after its close.
    pub broken: Vec<String>,
    /// Whether what comes out also goes to this process's own streams.
    pub echo: bool,
}

impl Streams {
    pub fn new(outside: Outside) -> Self {
        Self {
            outside,
            ..Default::default()
        }
    }

    pub fn send(&mut self, end: End, bytes: &[u8]) {
        let given = self.given.entry(end).or_default();
        if bytes.len() as u64 > *given {
            self.broken.push(format!(
                "{end:?}: {} bytes sent with {given} credit",
                bytes.len()
            ));
        }
        *given = given.saturating_sub(bytes.len() as u64);
        if self.closed_by_kernel.contains(&end) {
            self.broken.push(format!("{end:?}: sent after its close"));
        }
        if self.echo {
            use std::io::Write;
            let _ = match end {
                ERROR => std::io::stderr().write_all(bytes),
                _ => std::io::stdout().write_all(bytes),
            };
        }
        match end {
            ERROR => self.stderr.extend_from_slice(bytes),
            _ => self.stdout.extend_from_slice(bytes),
        }
    }

    pub fn credit(&mut self, end: End, bytes: u64) {
        if Some(end) != self.outside.input_end() {
            self.broken
                .push(format!("{end:?}: credit on an end that takes nothing"));
        }
        if self.closed_by_kernel.contains(&end) {
            self.broken.push(format!("{end:?}: credit after its close"));
        }
        self.input_credit = self.input_credit.saturating_add(bytes);
        self.most_input_credit = self.most_input_credit.max(self.input_credit);
    }

    pub fn close(&mut self, end: End) {
        if Some(end) == self.outside.input_end() && !self.closed_here.contains(&end) {
            self.input_closed_first = true;
        }
        if !self.closed_by_kernel.insert(end) {
            self.broken.push(format!("{end:?}: closed twice"));
        }
    }

    /// Whether the script has steps left.
    pub fn waiting(&self) -> bool {
        self.step < self.outside.script.len()
    }

    /// Whether every output end has been closed by the kernel or here.
    pub fn outputs_closed(&self) -> bool {
        self.outside
            .outputs()
            .iter()
            .all(|end| self.closed_by_kernel.contains(end) || self.closed_here.contains(end))
    }

    /// Whether the kernel closed the input end (it had no reader left) before this side did.
    pub fn input_closed_by_kernel(&self) -> bool {
        self.input_closed_first
    }

    /// How much output credit the outside gives on `end` at a time, if it is slow.
    fn pace(&self, end: End) -> Option<u64> {
        self.outside.output_pace.filter(|_| end != ERROR)
    }

    /// The credit the outside gives on an output end now: what it gives at a time, once the end
    /// is open and the credit given before is used up.
    fn due(&self, end: End) -> u64 {
        let open = !self.closed_by_kernel.contains(&end) && !self.closed_here.contains(&end);
        let used = self.given.get(&end).copied().unwrap_or(0) == 0;
        match open && used {
            true => self.pace(end).unwrap_or(UNLIMITED),
            false => 0,
        }
    }

    /// What the script waits for, for a run that can go on no further.
    pub fn stuck_at(&self) -> Option<&Step> {
        self.outside.script.get(self.step)
    }
}

/// How far into what a container has shown the waiting steps have found what they waited for.
#[derive(Clone, Copy, Debug, Default)]
pub struct Found {
    output: usize,
    error: usize,
}

impl Found {
    /// Whether `bytes` is in the output or the error past what was found before; if so, it is
    /// found up to its end.
    pub fn look(&mut self, output: &[u8], error: &[u8], bytes: &[u8]) -> bool {
        let at = |shown: &[u8], from: usize| {
            shown[from.min(shown.len())..]
                .windows(bytes.len())
                .position(|window| window == bytes)
                .map(|position| from + position + bytes.len())
        };
        if let Some(end) = at(output, self.output) {
            self.output = end;
            return true;
        }
        if let Some(end) = at(error, self.error) {
            self.error = end;
            return true;
        }
        false
    }
}

/// The host's streams and the seeded choices of a run, as [`act`] needs them.
pub trait StreamHost: wcontainers_abi::Host {
    fn streams(&mut self) -> &mut Streams;
    fn choices(&mut self) -> &mut StdRng;
}

/// What the outside does at the start: credit for all the output when it takes everything.
pub fn begin(kernel: &mut Kernel, cpu: &mut dyn Cpu, host: &mut impl StreamHost) {
    for end in host.streams().outside.outputs() {
        if host.streams().pace(end).is_none() {
            give(kernel, cpu, host, end, UNLIMITED);
        }
    }
}

fn give(kernel: &mut Kernel, cpu: &mut dyn Cpu, host: &mut impl StreamHost, end: End, bytes: u64) {
    let given = host.streams().given.entry(end).or_default();
    *given = given.saturating_add(bytes);
    kernel.event(cpu, host, Event::StreamCredit { end, bytes });
}

fn close_here(kernel: &mut Kernel, cpu: &mut dyn Cpu, host: &mut impl StreamHost, end: End) {
    if host.streams().closed_here.insert(end) {
        kernel.event(cpu, host, Event::StreamClosed { end });
    }
}

/// The outside's next move, after a turn or (`idle`) when the container can go on no other way:
/// a piece of what it sends (when idle, or after [`BUSY_TURNS`] turns without), output credit
/// when it is slow and the container idle, and the script's other steps. Whether it did anything.
pub fn act(kernel: &mut Kernel, cpu: &mut dyn Cpu, host: &mut impl StreamHost, idle: bool) -> bool {
    let mut acted = false;
    if idle {
        for end in host.streams().outside.outputs() {
            let due = host.streams().due(end);
            if due > 0 {
                give(kernel, cpu, host, end, due);
                acted = true;
            }
        }
    }
    loop {
        let streams = host.streams();
        let Some(step) = streams.outside.script.get(streams.step).cloned() else {
            return acted;
        };
        match step {
            Step::Expect(bytes) => {
                if !streams.found.look(&streams.stdout, &streams.stderr, &bytes) {
                    return acted;
                }
                streams.step += 1;
            }
            Step::Send(bytes) => {
                let end = streams
                    .outside
                    .input_end()
                    .expect("a script that sends has an input");
                let left = streams.sending.get_or_insert(bytes).clone();
                let credit = streams.input_credit;
                let closed = streams.closed_by_kernel.contains(&end);
                if closed {
                    // The kernel takes no more: what is left is never sent.
                    streams.sending = None;
                    streams.step += 1;
                    continue;
                }
                streams.since_sent += u64::from(!idle);
                if credit == 0 || !(idle || streams.since_sent >= BUSY_TURNS) {
                    return acted;
                }
                streams.since_sent = 0;
                let most = credit.min(left.len() as u64).min(MOST_AT_ONCE);
                let piece = host.choices().random_range(1..=most) as usize;
                let excess = host.streams().outside.excess as usize;
                let sent = &left[..(piece + excess).min(left.len())];
                let streams = host.streams();
                streams.input_credit -= piece as u64;
                streams.sending = Some(left[sent.len()..].to_vec());
                if sent.len() == left.len() {
                    streams.sending = None;
                    streams.step += 1;
                }
                kernel.event(cpu, host, Event::StreamData { end, bytes: sent });
                return true;
            }
            Step::Resize { columns, rows } => {
                streams.step += 1;
                kernel.event(
                    cpu,
                    host,
                    Event::TerminalResized {
                        end: TERMINAL,
                        columns,
                        rows,
                    },
                );
                acted = true;
            }
            Step::CloseInput => {
                streams.step += 1;
                let end = streams.outside.input_end().expect("an input to close");
                close_here(kernel, cpu, host, end);
                acted = true;
            }
            Step::CloseOutput => {
                streams.step += 1;
                close_here(kernel, cpu, host, OUTPUT);
                acted = true;
            }
        }
    }
}

/// After the container's end: takes the output still held, at the outside's pace, until the kernel
/// closes every output end; then lets go of every end. Whether every output end was closed.
pub fn finish(kernel: &mut Kernel, cpu: &mut dyn Cpu, host: &mut impl StreamHost) -> bool {
    loop {
        if host.streams().outputs_closed() {
            break;
        }
        let before = (host.streams().stdout.len(), host.streams().stderr.len());
        let mut gave = false;
        for end in host.streams().outside.outputs() {
            let due = host.streams().due(end);
            if due > 0 {
                give(kernel, cpu, host, end, due);
                gave = true;
            }
        }
        let after = (host.streams().stdout.len(), host.streams().stderr.len());
        if !gave && before == after {
            break;
        }
        if gave && before == after && !host.streams().outputs_closed() {
            // Credit taken nowhere: nothing is held, and an end is still open.
            break;
        }
    }
    let closed = host.streams().outputs_closed();
    let mut all: Vec<End> = host.streams().outside.outputs();
    all.extend(host.streams().outside.input_end());
    all.dedup();
    for end in all {
        close_here(kernel, cpu, host, end);
    }
    closed
}
