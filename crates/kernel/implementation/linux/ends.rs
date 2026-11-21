//! Ends: byte streams between the kernel and the embedder. The kernel holds its side of each as an
//! ordinary open file (a pipe's writer or reader, or a pseudo-terminal's master) and moves bytes
//! between that file and the embedder within the credit each side gives the other.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use wcontainers_abi::End;

use crate::file::FileRef;
use crate::process::Channel;
use crate::syscall::{pipe, tty};
use crate::{Context, Kernel, reason};

/// What the first process's descriptors 0 to 2 are, as `docker run` gives them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standard {
    /// `docker run`: 0 is `/dev/null` without an input end (`-i` gives one), else the reading side
    /// of a pipe the input end writes; 1 and 2 are the writing sides of pipes the output and error
    /// ends read.
    Pipes {
        input: Option<End>,
        output: End,
        error: End,
    },
    /// `docker run -t`: 0 to 2 are a pseudo-terminal's slave at this size, whose master the end
    /// holds.
    Terminal { end: End, columns: u16, rows: u16 },
}

impl Standard {
    /// The ends it names, each with whether the kernel sends on it.
    pub(crate) fn ends(&self) -> Vec<(End, bool)> {
        match *self {
            Standard::Pipes {
                input,
                output,
                error,
            } => input
                .map(|end| (end, false))
                .into_iter()
                .chain([(output, true), (error, true)])
                .collect(),
            Standard::Terminal { end, .. } => alloc::vec![(end, true)],
        }
    }
}

/// The kernel's side of one end.
struct Held {
    /// None until the start that named it has made the first process's descriptors (and for
    /// good if that start failed).
    file: Option<FileRef>,
    /// Bytes the kernel may still send.
    credit: u64,
    /// The kernel sends on it (an input pipe's end only receives).
    sends: bool,
    /// The kernel has closed it: it sends nothing more on it.
    closed: bool,
}

/// Every end the embedder has not closed.
#[derive(Default)]
pub(crate) struct Ends {
    held: BTreeMap<End, Held>,
    /// Pipes' writers whose end the embedder closed while bytes it sent waited for space: let go of
    /// once those are in, as a writer outside Linux closes after writing what it holds.
    draining: Vec<FileRef>,
    /// The channels of the pipes and terminals held, whose wakes mean something moved.
    watched: Vec<Channel>,
    /// Something may have moved since the ends were last served.
    pub moved: bool,
}

impl Ends {
    /// Notes a wake on `channel`: if an end's pipe or terminal is on it, something moved.
    pub fn woken(&mut self, channel: Channel) {
        if self.watched.contains(&channel) {
            self.moved = true;
        }
    }

    /// Names an end whose file comes later; credit given on it before then is kept.
    pub fn name(&mut self, end: End, sends: bool) {
        let held = Held {
            file: None,
            credit: 0,
            sends,
            closed: false,
        };
        assert!(
            self.held.insert(end, held).is_none(),
            "an end is named once"
        );
    }

    /// Bytes the kernel may still send on an end.
    pub fn credit(&self, end: End) -> u64 {
        self.held
            .get(&end)
            .filter(|held| !held.closed)
            .map_or(0, |held| held.credit)
    }

    /// Sends bytes out of an end, within its credit.
    pub fn send(&mut self, cx: &mut Context<'_>, end: End, bytes: &[u8]) {
        let held = self.held.get_mut(&end).expect("an end held");
        let count = bytes.len() as u64;
        debug_assert!(count <= held.credit && !held.closed);
        held.credit -= count;
        cx.host.send_stream(end, bytes);
    }
}

/// Which side of which object the kernel holds for an end.
enum Side {
    /// A pipe's reader: the end takes what the guest writes.
    PipeOut,
    /// A pipe's writer: the end gives what the guest reads.
    PipeIn,
    /// A pseudo-terminal's master: both.
    Terminal,
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

impl Kernel {
    /// Gives a named end the kernel's side of it; if the embedder has closed the end already, the
    /// file is let go of at once, as a close.
    pub(crate) fn attach_end(&mut self, cx: &mut Context<'_>, end: End, file: FileRef) {
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

    /// Lets go of an end's file, as a close of it.
    fn let_go_of_end(&mut self, cx: &mut Context<'_>, file: FileRef) {
        let watched = &mut self.ends.watched;
        let mine = channel(&file);
        if let Some(at) = watched.iter().position(|seen| *seen == mine) {
            watched.swap_remove(at);
        }
        self.released.push(file);
        self.files_released(cx);
    }

    /// `Event::StreamData`: bytes from the embedder, within the credit given; what is past it is
    /// dropped and counted. Bytes after the kernel closed the end are dropped, as a write to a pipe
    /// with no reader fails.
    pub(crate) fn stream_data(&mut self, cx: &mut Context<'_>, end: End, bytes: &[u8]) {
        self.ends.moved = true;
        let Some(held) = self.ends.held.get(&end) else {
            self.counters
                .count(cx.host, reason::EVENT_UNEXPECTED, end.0.into());
            return;
        };
        let past = match held.file.clone() {
            Some(file) => match side(&file) {
                Side::PipeIn => pipe::receive_from_end(self, cx, &file, bytes),
                Side::Terminal => tty::receive_from_end(self, cx, &file, bytes),
                Side::PipeOut => bytes.len(),
            },
            None => bytes.len(),
        };
        if past > 0 {
            self.counters
                .count(cx.host, reason::STREAM_PAST_CREDIT, end.0.into());
        }
    }

    /// `Event::StreamCredit`: the kernel may send this many more bytes. Credit on an end the kernel
    /// never sends on is counted as an event nothing waits for.
    pub(crate) fn credit_stream(&mut self, cx: &mut Context<'_>, end: End, bytes: u64) {
        self.ends.moved = true;
        match self.ends.held.get_mut(&end).filter(|held| held.sends) {
            Some(held) => held.credit = held.credit.saturating_add(bytes),
            None => self
                .counters
                .count(cx.host, reason::EVENT_UNEXPECTED, end.0.into()),
        }
    }

    /// `Event::StreamClosed`: the embedder has let go; so does the kernel, as a close of its file.
    pub(crate) fn stream_closed(&mut self, cx: &mut Context<'_>, end: End) {
        self.ends.moved = true;
        let Some(held) = self.ends.held.remove(&end) else {
            self.counters
                .count(cx.host, reason::EVENT_UNEXPECTED, end.0.into());
            return;
        };
        let Some(file) = held.file else {
            return;
        };
        match side(&file) {
            Side::PipeIn => {
                pipe::with_pipe(&file, |pipe| pipe.promised = 0);
                if !pipe::inbox_empty(&file) {
                    self.ends.draining.push(file);
                    return;
                }
            }
            Side::Terminal => tty::end(&file).expect("a terminal").0.borrow_mut().promised = 0,
            Side::PipeOut => pipe::with_pipe(&file, |pipe| pipe.reading_end = None),
        }
        self.let_go_of_end(cx, file);
    }

    /// `Event::TerminalResized`: a terminal end's new size.
    pub(crate) fn terminal_resized(
        &mut self,
        cx: &mut Context<'_>,
        end: End,
        columns: u16,
        rows: u16,
    ) {
        let file = self
            .ends
            .held
            .get(&end)
            .filter(|held| !held.closed)
            .and_then(|held| held.file.clone());
        match file.filter(|file| tty::end(file).is_some()) {
            Some(file) => tty::resize_from_end(self, cx, &file, columns, rows),
            None => self
                .counters
                .count(cx.host, reason::EVENT_UNEXPECTED, end.0.into()),
        }
    }

    /// Moves what can move between the ends and the embedder, once something may have moved:
    /// output within credit, new credit for space made, and closes for ends that are finished.
    /// Returns whether anything moved.
    pub(crate) fn serve_ends(&mut self, cx: &mut Context<'_>) -> bool {
        if !core::mem::take(&mut self.ends.moved) {
            return false;
        }
        let mut moved = false;
        for file in core::mem::take(&mut self.ends.draining) {
            moved |= pipe::from_inbox(self, cx, &file);
            match pipe::inbox_empty(&file) {
                true => self.let_go_of_end(cx, file),
                false => self.ends.draining.push(file),
            }
        }
        let ends: Vec<End> = self.ends.held.keys().copied().collect();
        for end in ends {
            let held = &self.ends.held[&end];
            let Some(file) = held.file.clone().filter(|_| !held.closed) else {
                continue;
            };
            let credit = held.credit;
            let side = side(&file);
            if matches!(side, Side::PipeOut | Side::Terminal) && credit > 0 {
                let bytes = match side {
                    Side::Terminal => tty::send_to_end(self, cx, &file, credit),
                    _ => pipe::send_to_end(self, cx, &file, credit),
                };
                if !bytes.is_empty() {
                    self.ends.send(cx, end, &bytes);
                    moved = true;
                }
            }
            if let Side::PipeIn = side {
                moved |= pipe::from_inbox(self, cx, &file);
            }
            let finished = match side {
                Side::PipeOut => pipe::drained(&file),
                Side::PipeIn => pipe::no_reader(&file),
                Side::Terminal => tty::drained(&file),
            };
            if finished {
                self.ends.held.get_mut(&end).expect("held").closed = true;
                cx.host.close_stream(end);
                moved = true;
                continue;
            }
            let credit = match side {
                Side::PipeIn => pipe::credit_for_end(&file),
                Side::Terminal => tty::credit_for_end(&file),
                Side::PipeOut => 0,
            };
            if credit > 0 {
                cx.host.credit_stream(end, credit);
                moved = true;
            }
        }
        // What moved may let more move.
        self.ends.moved |= moved;
        moved
    }

    /// The ends named at a start that failed: the kernel closes those the embedder has not.
    pub(crate) fn close_unmade(&mut self, cx: &mut Context<'_>, standard: &Standard) {
        for (end, _) in standard.ends() {
            if let Some(held) = self.ends.held.get_mut(&end) {
                held.closed = true;
                cx.host.close_stream(end);
            }
        }
    }
}
