//! Ends: byte streams between the kernel and the outside. The kernel holds its side of each as an
//! ordinary open file (a pipe's writer or reader, or a pseudo-terminal's master) and moves bytes
//! between that file and the outside within the credit each side gives the other.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use wcontainers_abi::End;

use crate::file::FileRef;
use crate::fs::Node;
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
    /// The ends it names.
    pub(crate) fn ends(&self) -> Vec<End> {
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
    /// None for an end named at a start that failed, closed at once.
    file: Option<FileRef>,
    /// Bytes the kernel may still send.
    credit: u64,
    /// The kernel has closed it: it sends nothing more on it.
    closed: bool,
}

/// Every end the outside has not closed.
#[derive(Default)]
pub(crate) struct Ends {
    held: BTreeMap<End, Held>,
}

impl Ends {
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    pub fn add(&mut self, end: End, file: Option<FileRef>) {
        let held = Held {
            closed: file.is_none(),
            file,
            credit: 0,
        };
        assert!(
            self.held.insert(end, held).is_none(),
            "an end is named once"
        );
    }

    /// The end that reads a pipe, if one does and has not closed.
    pub fn reader_of(&self, node: &Node) -> Option<End> {
        self.held.iter().find_map(|(end, held)| {
            let file = held.file.as_ref()?.borrow();
            (!held.closed && file.readable() && alloc::rc::Rc::ptr_eq(&file.node, node))
                .then_some(*end)
        })
    }

    /// Bytes the kernel may still send on an end.
    pub fn credit(&self, end: End) -> u64 {
        self.held.get(&end).map_or(0, |held| held.credit)
    }

    /// Sends bytes out of an end, within its credit.
    pub fn send(&mut self, cx: &mut Context<'_>, end: End, bytes: &[u8]) {
        let held = self.held.get_mut(&end).expect("an end held");
        let count = bytes.len() as u64;
        debug_assert!(count <= held.credit && !held.closed);
        held.credit -= count;
        cx.host.stream_send(end, bytes);
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
    /// `Event::StreamData`: bytes from outside, within the credit given; what is past it is
    /// dropped and counted. Bytes after the kernel closed the end are dropped, as a write to a pipe
    /// with no reader fails.
    pub(crate) fn stream_data(&mut self, cx: &mut Context<'_>, end: End, bytes: &[u8]) {
        let Some(held) = self.ends.held.get(&end) else {
            self.counters
                .count(cx.host, reason::EVENT_UNEXPECTED, end.0.into());
            return;
        };
        let past = match held.file.clone() {
            Some(file) => match side(&file) {
                Side::PipeIn => pipe::from_outside(self, cx, &file, bytes),
                Side::Terminal => tty::from_outside(self, cx, &file, bytes),
                Side::PipeOut => bytes.len(),
            },
            None => bytes.len(),
        };
        if past > 0 {
            self.counters
                .count(cx.host, reason::STREAM_PAST_CREDIT, end.0.into());
        }
    }

    /// `Event::StreamCredit`: the kernel may send this many more bytes.
    pub(crate) fn stream_credit(&mut self, cx: &mut Context<'_>, end: End, bytes: u64) {
        match self.ends.held.get_mut(&end) {
            Some(held) => held.credit = held.credit.saturating_add(bytes),
            None => self
                .counters
                .count(cx.host, reason::EVENT_UNEXPECTED, end.0.into()),
        }
    }

    /// `Event::StreamClosed`: the outside has let go; so does the kernel, as a close of its file.
    pub(crate) fn stream_closed(&mut self, cx: &mut Context<'_>, end: End) {
        let Some(held) = self.ends.held.remove(&end) else {
            self.counters
                .count(cx.host, reason::EVENT_UNEXPECTED, end.0.into());
            return;
        };
        if let Some(file) = held.file {
            match side(&file) {
                Side::PipeIn => pipe::with_pipe(&file, |pipe| pipe.promised = 0),
                Side::Terminal => tty::end(&file).expect("a terminal").0.borrow_mut().promised = 0,
                Side::PipeOut => {}
            }
            self.released.push(file);
            self.files_released(cx);
        }
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
            Some(file) => tty::resize_from_outside(self, cx, &file, columns, rows),
            None => self
                .counters
                .count(cx.host, reason::EVENT_UNEXPECTED, end.0.into()),
        }
    }

    /// Moves what can move between the ends and the outside: output within credit, new credit
    /// for room made, and closes for ends that are finished. Returns whether anything moved.
    pub(crate) fn serve_ends(&mut self, cx: &mut Context<'_>) -> bool {
        let mut moved = false;
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
                    Side::Terminal => tty::to_outside(self, cx, &file, credit),
                    _ => pipe::to_outside(self, cx, &file, credit),
                };
                if !bytes.is_empty() {
                    self.ends.send(cx, end, &bytes);
                    moved = true;
                }
            }
            let finished = match side {
                Side::PipeOut => pipe::drained(&file),
                Side::PipeIn => pipe::no_reader(&file),
                Side::Terminal => tty::drained(&file),
            };
            if finished {
                self.ends.held.get_mut(&end).expect("held").closed = true;
                cx.host.stream_close(end);
                moved = true;
                continue;
            }
            let grant = match side {
                Side::PipeIn => pipe::promise_outside(&file),
                Side::Terminal => tty::promise_outside(&file),
                Side::PipeOut => 0,
            };
            if grant > 0 {
                cx.host.stream_credit(end, grant);
                moved = true;
            }
        }
        moved
    }

    /// The ends named at a start that failed: the kernel closes them at once.
    pub(crate) fn close_unmade(&mut self, cx: &mut Context<'_>, standard: &Standard) {
        for end in standard.ends() {
            self.ends.add(end, None);
            cx.host.stream_close(end);
        }
    }
}
