//! The client of a container's standard streams, as a test drives them: on our kernel the other
//! sides of its ends, in Docker the client's input, output and terminal.

use std::collections::{BTreeMap, BTreeSet};

use std::sync::{Arc, MutexGuard};

use rand::Rng;
use wcontainers_abi::{Cpu, Event, HostError};
use wcontainers_kernel_linux::{Kernel, Standard};
use wcontainers_readiness::{ReadinessReport, ReadyFor, UnconditionalEvents};
use wcontainers_system_seeded_test_policy::Schedule;

/// One of the test host's stream endpoints, by its number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct End(pub u32);

/// The endpoints the test host names.
pub const INPUT: End = End(0);
pub const OUTPUT: End = End(1);
pub const ERROR: End = End(2);
pub const TERMINAL: End = End(3);

/// The output room of a client that reads everything as it comes.
pub const UNLIMITED: u64 = u64::MAX;

/// The largest piece the client sends at once; pieces are drawn from the seed up to this.
const MOST_AT_ONCE: u64 = 1 << 16;

/// How many turns a container that never waits runs before the client's next piece reaches it;
/// one that waits gets it when it waits, as a real client's bytes take longer to come than a
/// program takes to reach its next wait.
const BUSY_TURNS: u64 = 10_000;

/// One step of what the client does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Waits until what the container has shown (its output or error, or all its terminal shows)
    /// holds these bytes after what the last step that waited found.
    Expect(Vec<u8>),
    /// Sends bytes in, in pieces, each once the kernel waits for input.
    Send(Vec<u8>),
    /// Gives the terminal a new size.
    Resize { columns: u16, rows: u16 },
    /// Closes the input: end of file (or, for a terminal, the master's close).
    CloseInput,
    /// Stops reading the output: closes the output end.
    CloseOutput,
    /// Writes bytes over the start of a file the backend serves (a path in its directory), as
    /// another program beside the backend would, not through the container's kernel.
    ChangeServed { path: String, bytes: Vec<u8> },
}

/// What the client is and does.
#[derive(Clone, Debug, Default)]
pub struct StreamClient {
    /// Descriptor 0 takes input from the client (`docker run -i`).
    pub input: bool,
    /// Descriptors 0 to 2 are a terminal of this many columns and rows (`docker run -t`).
    pub terminal: Option<(u16, u16)>,
    pub script: Vec<Step>,
    /// How many bytes of output (not error) it takes at a time, and only when the container can go
    /// on no other way (none at all for 0); none to take everything as it comes.
    pub output_pace: Option<u64>,
}

impl StreamClient {
    /// What the kernel is asked for at start.
    pub fn standard(&self) -> Standard<End> {
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
    fn outputs(&self) -> &'static [End] {
        match self.terminal {
            Some(_) => &[TERMINAL],
            None => &[OUTPUT, ERROR],
        }
    }

    /// The end the client sends on, if any.
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
    pub client: StreamClient,
    /// The next step of the script, and what is left to send of a step that sends.
    step: usize,
    sending: Option<Vec<u8>>,
    /// Turns run since the last piece was sent.
    since_sent: u64,
    /// How far into the output and the error the last step that waited found what it waited for.
    found: Found,
    /// The piece sent that the kernel has not taken yet, and the most it ever asked for at once.
    pending: Vec<u8>,
    pub most_input_asked: u64,
    /// Room given to the kernel on each output end and not yet used.
    given: BTreeMap<End, u64>,
    /// What the kernel's wait on each endpoint asks for.
    armed: BTreeMap<End, ReadyFor>,
    /// A wait was armed, or room, input or a close came, since the waits were last looked at.
    stirred: bool,
    /// What came out: the output end's (or the terminal's) and the error end's.
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// The ends the kernel closed, and the ones this side closed.
    pub closed_by_kernel: BTreeSet<End>,
    closed_here: BTreeSet<End>,
    input_closed_first: bool,
    /// What the kernel did against the rules: sending after its close, or closing twice.
    pub broken: Vec<String>,
    /// Whether what comes out also goes to this process's own streams.
    pub echo: bool,
}

impl Streams {
    pub fn new(client: StreamClient) -> Self {
        Self {
            client,
            ..Default::default()
        }
    }

    /// The kernel's send: as much as the room given takes.
    pub fn send(&mut self, end: End, bytes: &[u8]) -> Result<usize, HostError> {
        if self.closed_by_kernel.contains(&end) {
            self.broken.push(format!("{end:?}: sent after its close"));
        }
        if self.closed_here.contains(&end) {
            return Err(HostError::Closed);
        }
        let given = self.given.entry(end).or_default();
        let taken = usize::try_from(*given)
            .unwrap_or(usize::MAX)
            .min(bytes.len());
        if taken == 0 {
            return Err(HostError::WouldBlock);
        }
        *given -= taken as u64;
        self.show(end, &bytes[..taken]);
        Ok(taken)
    }

    /// What the kernel sent on an output end, as the client shows it.
    pub fn show(&mut self, end: End, bytes: &[u8]) {
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

    /// The kernel's receive: what is left of the piece sent; after the input's close, its end.
    pub fn receive(&mut self, end: End, into: &mut [u8]) -> Result<usize, HostError> {
        if Some(end) != self.client.input_end() {
            return Err(HostError::NotSupported);
        }
        self.most_input_asked = self.most_input_asked.max(into.len() as u64);
        if self.pending.is_empty() {
            return Err(match self.closed_here.contains(&end) {
                true => HostError::Closed,
                false => HostError::WouldBlock,
            });
        }
        let count = into.len().min(self.pending.len());
        into[..count].copy_from_slice(&self.pending[..count]);
        self.pending.drain(..count);
        Ok(count)
    }

    /// The kernel let go of an endpoint.
    pub fn close(&mut self, end: End) {
        if Some(end) == self.client.input_end() && !self.closed_here.contains(&end) {
            self.input_closed_first = true;
        }
        if !self.closed_by_kernel.insert(end) {
            self.broken.push(format!("{end:?}: closed twice"));
        }
        self.armed.remove(&end);
    }

    /// The kernel waits on an endpoint for `interest`.
    pub fn arm(&mut self, end: End, interest: ReadyFor) {
        self.armed.insert(end, interest);
        self.stirred = true;
    }

    /// What an endpoint is ready for now.
    fn ready(&self, end: End) -> ReadinessReport {
        let mut ready = crate::network::report(ReadyFor::empty(), UnconditionalEvents::empty());
        let closed = self.closed_here.contains(&end);
        if closed {
            ready.unconditional |= UnconditionalEvents::HANG_UP;
        }
        if self.client.outputs().contains(&end)
            && self.given.get(&end).is_some_and(|room| *room > 0)
        {
            ready.ready_for |= ReadyFor::WRITABLE;
        }
        if Some(end) == self.client.input_end() && (!self.pending.is_empty() || closed) {
            ready.ready_for |= ReadyFor::READABLE;
        }
        ready
    }

    /// The waits whose endpoint is now ready for something they ask for, each taken off with
    /// what it is ready for.
    fn take_ready(&mut self) -> Vec<(End, ReadinessReport)> {
        if !core::mem::take(&mut self.stirred) {
            return Vec::new();
        }
        let ready: Vec<(End, ReadinessReport)> = self
            .armed
            .iter()
            .map(|(end, interest)| {
                let mut ready = self.ready(*end);
                ready.ready_for &= *interest;
                (*end, ready)
            })
            .filter(|(_, ready)| !ready.ready_for.is_empty() || !ready.unconditional.is_empty())
            .collect();
        for (end, _) in &ready {
            self.armed.remove(end);
        }
        ready
    }

    /// Whether the kernel waits for input.
    fn input_wanted(&self) -> bool {
        self.client
            .input_end()
            .and_then(|end| self.armed.get(&end))
            .is_some_and(|interest| interest.contains(ReadyFor::READABLE))
    }

    /// Whether the script has steps left.
    pub fn waiting(&self) -> bool {
        self.step < self.client.script.len()
    }

    /// Whether every output end has been closed by the kernel or here.
    pub fn outputs_closed(&self) -> bool {
        self.client
            .outputs()
            .iter()
            .all(|end| self.closed_by_kernel.contains(end) || self.closed_here.contains(end))
    }

    /// Whether the kernel closed the input end (it had no reader left) before this side did.
    pub fn input_closed_by_kernel(&self) -> bool {
        self.input_closed_first
    }

    /// How much output room the client gives on `end` at a time, if it is slow.
    fn pace(&self, end: End) -> Option<u64> {
        self.client.output_pace.filter(|_| end != ERROR)
    }

    /// The room the client gives on an output end now: what it gives at a time, once the end
    /// is open and the room given before is used up.
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
        self.client.script.get(self.step)
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

/// The host's streams and the seeded choices of a run, as [`act`] needs them. Each is held only
/// while it is used, never while the kernel is handed something.
pub trait StreamHost: wcontainers_kernel_linux::System {
    fn streams(&self) -> MutexGuard<'_, Streams>;
    fn choices(&self) -> MutexGuard<'_, Schedule>;
    /// Does [`Step::ChangeServed`].
    fn change_served(&self, path: &str, bytes: &[u8]);
    /// Hands the kernel an event.
    fn hand(self: &Arc<Self>, kernel: &mut Kernel, cpu: &mut dyn Cpu, event: Event);
    /// Ends the kernel's wait on an endpoint with what it is ready for.
    fn ready(
        self: &Arc<Self>,
        kernel: &mut Kernel,
        cpu: &mut dyn Cpu,
        end: End,
        ready: ReadinessReport,
    );
}

/// Ends every wait whose endpoint is now ready. Whether any did.
fn fire(kernel: &mut Kernel, cpu: &mut dyn Cpu, host: &Arc<impl StreamHost>) -> bool {
    let ready = host.streams().take_ready();
    for (end, ready) in &ready {
        host.ready(kernel, cpu, *end, *ready);
    }
    !ready.is_empty()
}

/// What the client does at the start, which may still be waiting on the backend: room for all
/// the output when it takes everything, and the script's first steps that wait for nothing.
pub fn begin(kernel: &mut Kernel, cpu: &mut dyn Cpu, host: &Arc<impl StreamHost>) {
    let outputs = host.streams().client.outputs();
    for &end in outputs {
        let unpaced = host.streams().pace(end).is_none();
        if unpaced {
            give(kernel, cpu, host, end, UNLIMITED);
        }
    }
    act(kernel, cpu, host, false);
}

fn give(kernel: &mut Kernel, cpu: &mut dyn Cpu, host: &Arc<impl StreamHost>, end: End, bytes: u64) {
    {
        let mut streams = host.streams();
        let given = streams.given.entry(end).or_default();
        *given = given.saturating_add(bytes);
        streams.stirred = true;
    }
    fire(kernel, cpu, host);
}

fn close_here(kernel: &mut Kernel, cpu: &mut dyn Cpu, host: &Arc<impl StreamHost>, end: End) {
    let closes = {
        let mut streams = host.streams();
        streams.stirred = true;
        streams.closed_here.insert(end)
    };
    if closes {
        fire(kernel, cpu, host);
    }
}

/// The client's next move, after a turn or (`idle`) when the container can go on no other way:
/// a piece of what it sends (when idle, or after [`BUSY_TURNS`] turns without), output room
/// when it is slow and the container idle, and the script's other steps. Whether it did anything.
pub fn act(
    kernel: &mut Kernel,
    cpu: &mut dyn Cpu,
    host: &Arc<impl StreamHost>,
    idle: bool,
) -> bool {
    // A wait that began after its endpoint became ready ends now.
    let mut acted = fire(kernel, cpu, host);
    if idle {
        let outputs = host.streams().client.outputs();
        for &end in outputs {
            let due = host.streams().due(end);
            if due > 0 {
                give(kernel, cpu, host, end, due);
                acted = true;
            }
        }
    }
    loop {
        let mut streams = host.streams();
        let Some(step) = streams.client.script.get(streams.step).cloned() else {
            return acted;
        };
        match step {
            Step::Expect(bytes) => {
                let streams = &mut *streams;
                if !streams.found.look(&streams.stdout, &streams.stderr, &bytes) {
                    return acted;
                }
                streams.step += 1;
            }
            Step::Send(bytes) => {
                let end = streams
                    .client
                    .input_end()
                    .expect("a script that sends has an input");
                let left = streams.sending.get_or_insert(bytes).clone();
                let wanted = streams.input_wanted() && streams.pending.is_empty();
                let closed = streams.closed_by_kernel.contains(&end);
                if closed {
                    // The kernel takes no more: what is left is never sent.
                    streams.sending = None;
                    streams.step += 1;
                    continue;
                }
                streams.since_sent += u64::from(!idle);
                if !wanted || !(idle || streams.since_sent >= BUSY_TURNS) {
                    return acted;
                }
                streams.since_sent = 0;
                // A piece drawn from the seed.
                let most = (left.len() as u64).min(MOST_AT_ONCE);
                let piece = host.choices().draws().random_range(1..=most) as usize;
                streams.pending = left[..piece].to_vec();
                streams.stirred = true;
                streams.sending = Some(left[piece..].to_vec());
                if piece == left.len() {
                    streams.sending = None;
                    streams.step += 1;
                }
                drop(streams);
                fire(kernel, cpu, host);
                return true;
            }
            Step::Resize { columns, rows } => {
                streams.step += 1;
                drop(streams);
                host.hand(kernel, cpu, Event::TerminalResized { columns, rows });
                acted = true;
            }
            Step::CloseInput => {
                streams.step += 1;
                let end = streams.client.input_end().expect("an input to close");
                drop(streams);
                close_here(kernel, cpu, host, end);
                acted = true;
            }
            Step::CloseOutput => {
                streams.step += 1;
                drop(streams);
                close_here(kernel, cpu, host, OUTPUT);
                acted = true;
            }
            Step::ChangeServed { path, bytes } => {
                streams.step += 1;
                drop(streams);
                host.change_served(&path, &bytes);
                acted = true;
            }
        }
    }
}

/// After the container's end: takes the output still held, at the client's pace, until the kernel
/// closes every output end; then lets go of every end. Whether every output end was closed.
pub fn finish(kernel: &mut Kernel, cpu: &mut dyn Cpu, host: &Arc<impl StreamHost>) -> bool {
    loop {
        if host.streams().outputs_closed() {
            break;
        }
        let before = {
            let streams = host.streams();
            (streams.stdout.len(), streams.stderr.len())
        };
        let mut gave = fire(kernel, cpu, host);
        let outputs = host.streams().client.outputs();
        for &end in outputs {
            let due = host.streams().due(end);
            if due > 0 {
                give(kernel, cpu, host, end, due);
                gave = true;
            }
        }
        let after = {
            let streams = host.streams();
            (streams.stdout.len(), streams.stderr.len())
        };
        if !gave && before == after {
            break;
        }
        if gave && before == after && !host.streams().outputs_closed() {
            // Room taken nowhere: nothing is held, and an end is still open.
            break;
        }
    }
    let closed = host.streams().outputs_closed();
    let mut all: Vec<End> = host.streams().client.outputs().to_vec();
    let input = host.streams().client.input_end();
    all.extend(input);
    all.dedup();
    for end in all {
        close_here(kernel, cpu, host, end);
    }
    closed
}
