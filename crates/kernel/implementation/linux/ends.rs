//! Ends: byte streams between the kernel and the embedder, each one of the system's stream
//! endpoints. The kernel holds its side of each as an ordinary open file (a pipe's writer or reader,
//! or a pseudo-terminal's master) and moves bytes between that file and the system's endpoint as far
//! as each side takes them, waiting on the endpoint's readiness when it takes no more.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::Any;
use wcontainers_executor::Executor;

use wcontainers_abi::HostError;
use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor};
use wcontainers_system::StreamTransport;

use crate::file::FileRef;
use crate::process::Channel;
use crate::syscall::{pipe, tty};
use crate::waiting::{Answer, WaitFor, Waiting, waiting_call};
use crate::{Context, Kernel};

/// What the first process's descriptors 0 to 2 are, as `docker run` gives them, each end one of
/// the system's stream endpoints `E`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standard<E> {
    /// `docker run`: 0 is `/dev/null` without an input end (`-i` gives one), else the reading side
    /// of a pipe the input end writes; 1 and 2 are the writing sides of pipes the output and error
    /// ends read.
    Pipes {
        input: Option<E>,
        output: E,
        error: E,
    },
    /// `docker run -t`: 0 to 2 are a pseudo-terminal's slave at this size, whose master the end
    /// holds.
    Terminal { end: E, columns: u16, rows: u16 },
}

/// The kernel's number for one of its ends.
pub(crate) type EndNumber = u32;

impl<E> Standard<E> {
    /// The same descriptors with each end numbered in the order `number` is called, each with
    /// whether the kernel sends on it.
    pub(crate) fn numbered(
        self,
        mut number: impl FnMut(E, bool) -> EndNumber,
    ) -> Standard<EndNumber> {
        match self {
            Standard::Pipes {
                input,
                output,
                error,
            } => Standard::Pipes {
                input: input.map(|end| number(end, false)),
                output: number(output, true),
                error: number(error, true),
            },
            Standard::Terminal { end, columns, rows } => Standard::Terminal {
                end: number(end, true),
                columns,
                rows,
            },
        }
    }
}

impl Standard<EndNumber> {
    /// The ends it names.
    fn ends(&self) -> Vec<EndNumber> {
        match *self {
            Standard::Pipes {
                input,
                output,
                error,
            } => input.into_iter().chain([output, error]).collect(),
            Standard::Terminal { end, .. } => alloc::vec![end],
        }
    }
}

/// The kernel's side of one end.
struct Held {
    /// The system's endpoint, of its own type.
    endpoint: Arc<dyn Any + Send + Sync>,
    /// None until the start that named it has made the first process's descriptors (and for
    /// good if that start failed).
    file: Option<FileRef>,
    /// The kernel sends on it (an input pipe's end only receives).
    sends: bool,
    /// The system took less than it was given: nothing more is sent until it is ready to take.
    blocked: bool,
    /// The system had nothing to give: nothing more is asked until it is ready to give.
    drained: bool,
    /// The system said its side has let go: what it still gives is taken, then the end goes.
    hung_up: bool,
    /// What the wait kept on the endpoint asks for, beside the events every wait reports; none
    /// while no wait is kept.
    asked: Option<ReadyFor>,
    /// Bytes a terminal's master gave that the system has not taken yet, sent before any more is
    /// taken from the master.
    outbox: Vec<u8>,
}

/// Every end the kernel has not let go of.
#[derive(Default)]
pub(crate) struct Ends {
    held: BTreeMap<EndNumber, Held>,
    /// The channels of the pipes and terminals held, whose wakes mean something moved.
    watched: Vec<Channel>,
    /// Something may have moved since the ends were last served.
    pub moved: bool,
}

/// The system's endpoint kept for an end.
fn endpoint_of<S: crate::System>(
    endpoint: &Arc<dyn Any + Send + Sync>,
) -> &<S as StreamTransport>::Endpoint {
    endpoint
        .downcast_ref()
        .expect("an end's endpoint is its system's")
}

impl Ends {
    /// Notes a wake on `channel`: if an end's pipe or terminal is on it, something moved.
    pub(crate) fn woken(&mut self, channel: Channel) {
        if self.watched.contains(&channel) {
            self.moved = true;
        }
    }

    /// Takes an endpoint whose file comes later, numbered after those held before.
    fn name<S: crate::System>(
        &mut self,
        endpoint: <S as StreamTransport>::Endpoint,
        sends: bool,
    ) -> EndNumber {
        let number = self
            .held
            .last_key_value()
            .map_or(0, |(number, _)| number + 1);
        let held = Held {
            endpoint: Arc::new(endpoint),
            file: None,
            sends,
            blocked: false,
            drained: false,
            hung_up: false,
            asked: None,
            outbox: Vec::new(),
        };
        self.held.insert(number, held);
        number
    }

    /// Whether the kernel can send on an end now.
    pub(crate) fn takes(&self, end: EndNumber) -> bool {
        self.held
            .get(&end)
            .is_some_and(|held| held.sends && !held.blocked && !held.hung_up)
    }

    /// Sends bytes out of an end: how many the system took. If it took fewer, the end waits until
    /// the system can take more; if its side has let go, the end goes when next served.
    pub(crate) fn send<S: crate::System, C: Executor + 'static>(
        &mut self,
        cx: &Context<'_, S, C>,
        end: EndNumber,
        bytes: &mut [u8],
    ) -> usize {
        let held = self.held.get_mut(&end).expect("an end held");
        debug_assert!(held.sends && !held.blocked && !held.hung_up);
        let payload = core::cell::RefCell::new(&mut *bytes);
        let taken = match cx
            .host
            .send_stream(endpoint_of::<S>(&held.endpoint), &payload)
        {
            Ok(taken) => taken.min(bytes.len()),
            Err(HostError::WouldBlock) => 0,
            Err(_) => {
                held.hung_up = true;
                self.moved = true;
                return 0;
            }
        };
        if taken < bytes.len() {
            held.blocked = true;
            self.moved = true;
        }
        taken
    }
}

/// Which side of which object the kernel holds for an end.
#[derive(Clone, Copy)]
enum Side {
    /// A pipe's reader: the end takes what the guest writes.
    PipeOut,
    /// A pipe's writer: the end gives what the guest reads.
    PipeIn,
    /// A pseudo-terminal's master: both.
    Terminal,
}

impl Side {
    fn sends(self) -> bool {
        matches!(self, Side::PipeOut | Side::Terminal)
    }

    fn receives(self) -> bool {
        matches!(self, Side::PipeIn | Side::Terminal)
    }
}

/// The channel whose wakes mean an end's pipe or terminal moved.
fn channel(file: &FileRef) -> Channel {
    match tty::end(file) {
        Some((pty, ..)) => tty::channel(&pty),
        None => pipe::channel(&file.borrow().node),
    }
}

fn side(file: &FileRef) -> Side {
    if tty::end(file).is_some() {
        Side::Terminal
    } else if file.borrow().readable() {
        Side::PipeOut
    } else {
        Side::PipeIn
    }
}

/// The system's wait on an endpoint, owning what it waits on.
async fn ready_of<S: crate::System>(
    system: Arc<S>,
    endpoint: Arc<dyn Any + Send + Sync>,
    interest: ReadyFor,
) -> Answer {
    let endpoint = endpoint_of::<S>(&endpoint);
    Answer::Ready(
        Readiness::<<S as StreamTransport>::Endpoint>::wait_until_ready(
            &*system, endpoint, interest,
        )
        .await,
    )
}

impl Kernel {
    /// Takes the endpoints a start names, numbering them for the kernel.
    pub(crate) fn name_ends<S: crate::System>(
        &mut self,
        standard: Standard<<S as StreamTransport>::Endpoint>,
    ) -> Standard<EndNumber> {
        let ends = &mut self.ends;
        standard.numbered(|endpoint, sends| ends.name::<S>(endpoint, sends))
    }

    /// Gives a named end the kernel's side of it; if the end has gone already, the file is let go
    /// of at once, as a close.
    pub(crate) fn attach_end<S: crate::System, C: Executor + 'static>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        end: EndNumber,
        file: FileRef,
    ) {
        let Some(held) = self.ends.held.get_mut(&end) else {
            self.released.push(file);
            self.files_released(cx);
            return;
        };
        if let Side::PipeOut = side(&file) {
            pipe::with_pipe(&file, |pipe| pipe.reading_end = Some(end));
        }
        self.ends.watched.push(channel(&file));
        self.ends.moved = true;
        held.file = Some(file);
    }

    /// Lets go of an end: its wait is dropped, the system hears that the kernel let go, and its
    /// file is closed.
    fn let_go_of_end<S: crate::System, C: Executor + 'static>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        end: EndNumber,
    ) {
        let Some(held) = self.ends.held.remove(&end) else {
            return;
        };
        if held.asked.is_some() {
            self.waiting
                .drop_where(|about| *about == WaitFor::Stream(end));
        }
        cx.host.close_stream(endpoint_of::<S>(&held.endpoint));
        let Some(file) = held.file else {
            return;
        };
        match side(&file) {
            Side::PipeOut => pipe::with_pipe(&file, |pipe| pipe.reading_end = None),
            Side::PipeIn | Side::Terminal => {}
        }
        let watched = &mut self.ends.watched;
        let mine = channel(&file);
        if let Some(at) = watched.iter().position(|seen| *seen == mine) {
            watched.swap_remove(at);
        }
        self.released.push(file);
        self.files_released(cx);
    }

    /// The system's wait on an end's endpoint ended: what it is ready for lets the end send or
    /// receive again, or says its side let go.
    pub(crate) fn end_ready<S: crate::System, C: Executor + 'static>(
        &mut self,
        _cx: &mut Context<'_, S, C>,
        end: EndNumber,
        ready: Result<ReadinessReport, HostError>,
    ) {
        let report = crate::waiting::report_of(ready);
        let ready = report.ready_for;
        let Some(held) = self.ends.held.get_mut(&end) else {
            return;
        };
        held.asked = None;
        let gone = !report.unconditional.is_empty();
        if gone || ready.contains(ReadyFor::WRITABLE) {
            held.blocked = false;
        }
        if gone || ready.contains(ReadyFor::READABLE) {
            held.drained = false;
        }
        held.hung_up |= gone;
        self.ends.moved = true;
    }

    /// `Event::TerminalResized`: the terminal end's new size; with no terminal end it is counted.
    pub(crate) fn terminal_resized<S: crate::System, C: Executor + 'static>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        columns: u16,
        rows: u16,
    ) {
        let file = self
            .ends
            .held
            .values()
            .filter_map(|held| held.file.clone())
            .find(|file| tty::end(file).is_some());
        match file {
            Some(file) => tty::resize_from_end(self, cx, &file, columns, rows),
            None => self
                .counters
                .count(cx.host, crate::reason::EVENT_UNEXPECTED, 0),
        }
    }

    /// Sends what an end's pipe or terminal holds, as far as the system takes it. Whether any went.
    fn send_held<S: crate::System, C: Executor + 'static>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        end: EndNumber,
        file: &FileRef,
        side: Side,
    ) -> bool {
        let mut moved = false;
        while self.ends.takes(end) {
            match side {
                Side::Terminal => {
                    let held = self.ends.held.get_mut(&end).expect("held");
                    let mut outbox = core::mem::take(&mut held.outbox);
                    if outbox.is_empty() {
                        outbox = tty::send_to_end(self, cx, file, u64::MAX);
                    }
                    if outbox.is_empty() {
                        break;
                    }
                    let taken = self.ends.send(cx, end, &mut outbox);
                    moved |= taken > 0;
                    self.ends.held.get_mut(&end).expect("held").outbox = outbox[taken..].to_vec();
                }
                _ => {
                    let Some(mut bytes) = pipe::held_for_end(self, file) else {
                        break;
                    };
                    let taken = self.ends.send(cx, end, &mut bytes);
                    if taken > 0 {
                        pipe::sent_to_end(self, cx, file, taken);
                        moved = true;
                    }
                }
            }
        }
        moved
    }

    /// Takes what the system gives an end into its pipe or terminal, as far as there is space.
    /// Whether any came.
    fn receive_held<S: crate::System, C: Executor + 'static>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        end: EndNumber,
        file: &FileRef,
        side: Side,
    ) -> bool {
        let mut moved = false;
        loop {
            let held = &self.ends.held[&end];
            if held.drained {
                return moved;
            }
            let space = match side {
                Side::Terminal => tty::room_for_end(file),
                _ => pipe::room_for_end(file),
            };
            if space == 0 {
                return moved;
            }
            let mut bytes = alloc::vec![0u8; space];
            let held = self.ends.held.get_mut(&end).expect("held");
            match cx.host.receive_stream(
                endpoint_of::<S>(&held.endpoint),
                &core::cell::RefCell::new(&mut bytes[..]),
            ) {
                Ok(count) if count > 0 => {
                    bytes.truncate(count.min(space));
                    match side {
                        Side::Terminal => tty::receive_from_end(self, cx, file, &bytes),
                        _ => pipe::receive_from_end(self, cx, file, &bytes),
                    }
                    moved = true;
                }
                Err(HostError::WouldBlock) if !held.hung_up => {
                    held.drained = true;
                    return moved;
                }
                // Nothing more will come: its side has let go.
                _ => {
                    held.hung_up = true;
                    held.drained = true;
                    return true;
                }
            }
        }
    }

    /// Whether an end is finished: the guest's side is done with it, or the system's side let go
    /// and gave everything it had.
    fn finished(&self, end: EndNumber, file: &FileRef, side: Side) -> bool {
        let held = &self.ends.held[&end];
        let guest_done = match side {
            Side::PipeOut => pipe::drained(file),
            Side::PipeIn => pipe::no_reader(file),
            Side::Terminal => tty::drained(file) && held.outbox.is_empty(),
        };
        let system_done = held.hung_up && (side.sends() && !side.receives() || held.drained);
        guest_done || system_done
    }

    /// Keeps the wait on an end's endpoint asking for what the end needs now: for the system's
    /// side letting go always, to send once it took less than it was given, and to receive once
    /// it had nothing. A new wait's first poll comes when the kernel next settles.
    fn watch_end<S: crate::System, C: Executor + 'static>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        end: EndNumber,
        side: Side,
    ) {
        let held = self.ends.held.get_mut(&end).expect("held");
        let wanted = (!held.hung_up).then(|| {
            let mut wanted = ReadyFor::empty();
            if side.sends() && held.blocked {
                wanted |= ReadyFor::WRITABLE;
            }
            if side.receives() && held.drained {
                wanted |= ReadyFor::READABLE;
            }
            wanted
        });
        if wanted == held.asked {
            return;
        }
        let had = held.asked.is_some();
        held.asked = wanted;
        let endpoint = held.endpoint.clone();
        if had {
            self.waiting
                .drop_where(|about| *about == WaitFor::Stream(end));
        }
        let Some(wanted) = wanted else {
            return;
        };
        let call: crate::waiting::Call =
            waiting_call(ready_of(cx.system.clone(), endpoint, wanted));
        if Waiting::on_heap(&call) {
            self.counters
                .count(cx.host, crate::reason::WAITING_CALL_ON_HEAP, u64::from(end));
        }
        self.waiting.start_woken(WaitFor::Stream(end), call);
    }

    /// Moves what can move between the ends and the system, once something may have moved:
    /// output as far as the system takes it, input as far as there is space, and the ends that
    /// are finished let go of. Returns whether anything moved.
    pub(crate) fn serve_ends<S: crate::System, C: Executor + 'static>(
        &mut self,
        cx: &mut Context<'_, S, C>,
    ) -> bool {
        if !core::mem::take(&mut self.ends.moved) {
            return false;
        }
        let mut moved = false;
        let ends: Vec<EndNumber> = self.ends.held.keys().copied().collect();
        for end in ends {
            let Some(file) = self.ends.held[&end].file.clone() else {
                continue;
            };
            let side = side(&file);
            if side.sends() {
                moved |= self.send_held(cx, end, &file, side);
            }
            if side.receives() {
                moved |= self.receive_held(cx, end, &file, side);
            }
            if self.finished(end, &file, side) {
                // The release that follows wakes the pipe's other side only if it holds the
                // last reference to the end's file (`released`), so this one goes first.
                drop(file);
                self.let_go_of_end(cx, end);
                moved = true;
                continue;
            }
            self.watch_end(cx, end, side);
        }
        // What moved may let more move.
        self.ends.moved |= moved;
        moved
    }

    /// The ends named at a start that failed: the kernel lets go of them.
    pub(crate) fn close_unmade<S: crate::System, C: Executor + 'static>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        standard: &Standard<EndNumber>,
    ) {
        for end in standard.ends() {
            self.let_go_of_end(cx, end);
        }
    }

    /// After a restore the old system's waits are gone: each end is asked about again.
    pub(crate) fn ends_restored(&mut self) {
        self.waiting
            .drop_where(|about| matches!(about, WaitFor::Stream(_)));
        for held in self.ends.held.values_mut() {
            held.asked = None;
            held.blocked = false;
            held.drained = false;
        }
        self.ends.moved = true;
    }
}
