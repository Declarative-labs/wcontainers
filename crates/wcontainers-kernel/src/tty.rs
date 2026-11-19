//! Pseudo-terminals as Linux's `pty.c` keeps them, each side with the `n_tty` line discipline
//! (`n_tty.c`): input handling, canonical editing, echo, keys that send signals, flow control and
//! output processing. What one side writes goes into the other side's buffer and is taken in at
//! once, as Linux's buffer work would take it; this module holds the state and the rules, and
//! `syscall::tty` joins them to files, processes and waiting.

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::rc::Weak;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::file::OpenFile;
use crate::fs::Node;
use crate::linux::general::*;
use crate::linux_extra::{BAUD_TABLE, INIT_C_CC, N_TTY_BUF_SIZE, PTY_BUFFER_LIMIT};
use crate::process::Pid;

pub(crate) const MASTER: usize = 0;
pub(crate) const SLAVE: usize = 1;

const SIZE: usize = N_TTY_BUF_SIZE;

fn mask(index: usize) -> usize {
    index & (SIZE - 1)
}

/// Bytes in the echo buffer that start an operation rather than stand for themselves.
const ECHO_OP_START: u8 = 0xff;
const ECHO_OP_MOVE_BACK_COL: u8 = 0x80;
const ECHO_OP_SET_CANON_COL: u8 = 0x81;
const ECHO_OP_ERASE_TAB: u8 = 0x82;
const ECHO_COMMIT_WATERMARK: usize = 256;
const ECHO_BLOCK: usize = 256;
const ECHO_DISCARD_WATERMARK: usize = SIZE - (ECHO_BLOCK + 32);
/// A control character set to this is disabled (`__DISABLED_CHAR`); the end-of-file mark in the
/// read buffer is this byte.
const DISABLED_CHAR: u8 = 0;

/// Linux's own `ctype` table (`lib/ctype.c`), which counts Latin-1 letters.
fn is_control(c: u8) -> bool {
    c < 32 || c == 127
}

fn is_upper(c: u8) -> bool {
    c.is_ascii_uppercase() || (192..=222).contains(&c) && c != 215
}

fn is_lower(c: u8) -> bool {
    c.is_ascii_lowercase() || c >= 223 && c != 247
}

fn is_alphanumeric(c: u8) -> bool {
    c.is_ascii_digit() || is_upper(c) || is_lower(c)
}

fn to_lower(c: u8) -> u8 {
    if is_upper(c) { c + 32 } else { c }
}

fn to_upper(c: u8) -> u8 {
    if is_lower(c) { c - 32 } else { c }
}

/// The `n_tty` line discipline's state (`struct n_tty_data`).
#[derive(Clone)]
struct Discipline {
    read_head: usize,
    commit_head: usize,
    canon_head: usize,
    echo_head: usize,
    echo_commit: usize,
    echo_mark: usize,
    char_map: [u64; 4],
    no_room: bool,
    lnext: bool,
    erasing: bool,
    raw: bool,
    real_raw: bool,
    icanon: bool,
    push: bool,
    read_buf: [u8; SIZE],
    read_flags: [u64; SIZE / 64],
    echo_buf: [u8; SIZE],
    read_tail: usize,
    line_start: usize,
    column: u32,
    canon_column: u32,
    echo_tail: usize,
}

impl Discipline {
    fn new() -> Box<Self> {
        Box::new(Self {
            read_head: 0,
            commit_head: 0,
            canon_head: 0,
            echo_head: 0,
            echo_commit: 0,
            echo_mark: 0,
            char_map: [0; 4],
            no_room: false,
            lnext: false,
            erasing: false,
            raw: false,
            real_raw: false,
            icanon: false,
            push: false,
            read_buf: [0; SIZE],
            read_flags: [0; SIZE / 64],
            echo_buf: [0; SIZE],
            read_tail: 0,
            line_start: 0,
            column: 0,
            canon_column: 0,
            echo_tail: 0,
        })
    }

    fn read_count(&self) -> usize {
        self.read_head - self.read_tail
    }

    fn read_byte(&self, index: usize) -> u8 {
        self.read_buf[mask(index)]
    }

    fn echo_byte(&self, index: usize) -> u8 {
        self.echo_buf[mask(index)]
    }

    fn flag(&self, index: usize) -> bool {
        let index = mask(index);
        self.read_flags[index / 64] & (1 << (index % 64)) != 0
    }

    fn set_flag(&mut self, index: usize, on: bool) {
        let index = mask(index);
        if on {
            self.read_flags[index / 64] |= 1 << (index % 64);
        } else {
            self.read_flags[index / 64] &= !(1 << (index % 64));
        }
    }

    fn special(&self, c: u8) -> bool {
        self.char_map[c as usize / 64] & (1 << (c % 64)) != 0
    }

    fn put(&mut self, c: u8) {
        self.read_buf[mask(self.read_head)] = c;
        self.read_head += 1;
    }

    fn add_echo(&mut self, c: u8) {
        self.echo_buf[mask(self.echo_head)] = c;
        self.echo_head += 1;
    }

    /// `reset_buffer_flags`.
    fn reset(&mut self) {
        self.read_head = 0;
        self.canon_head = 0;
        self.read_tail = 0;
        self.commit_head = 0;
        self.line_start = 0;
        self.erasing = false;
        self.read_flags = [0; SIZE / 64];
        self.push = false;
    }

    fn echo_char_raw(&mut self, c: u8) {
        if c == ECHO_OP_START {
            self.add_echo(ECHO_OP_START);
        }
        self.add_echo(c);
    }

    fn echo_move_back_col(&mut self) {
        self.add_echo(ECHO_OP_START);
        self.add_echo(ECHO_OP_MOVE_BACK_COL);
    }

    fn echo_set_canon_col(&mut self) {
        self.add_echo(ECHO_OP_START);
        self.add_echo(ECHO_OP_SET_CANON_COL);
    }

    fn echo_erase_tab(&mut self, chars: u32, after_tab: bool) {
        self.add_echo(ECHO_OP_START);
        self.add_echo(ECHO_OP_ERASE_TAB);
        let mut chars = (chars & 7) as u8;
        if after_tab {
            chars |= 0x80;
        }
        self.add_echo(chars);
    }

    fn finish_erasing(&mut self) {
        if self.erasing {
            self.echo_char_raw(b'/');
            self.erasing = false;
        }
    }
}

/// One side of a pair: its settings, its job control, its flow state and its line discipline
/// (`struct tty_struct`).
#[derive(Clone)]
pub(crate) struct Tty {
    pub termios: termios2,
    pub winsize: winsize,
    /// The foreground process group and the session it is the controlling terminal of.
    pub pgrp: Option<Pid>,
    pub session: Option<Pid>,
    pub stopped: bool,
    pub tco_stopped: bool,
    /// Packet mode (`TIOCPKT`), and the status bytes a packet-mode reader of the other side gets.
    pub packet: bool,
    pub pktstatus: u8,
    /// `TTY_IO_ERROR`, `TTY_OTHER_CLOSED` and `TTY_EXCLUSIVE`.
    pub io_error: bool,
    pub other_closed: bool,
    pub exclusive: bool,
    /// `TTY_DO_WRITE_WAKEUP`: a write here ended with bytes left and a file with `FASYNC` set, so
    /// the next room the other side's reader makes sends this side's I/O signal.
    pub write_wakeup: bool,
    /// What the other side wrote that the line discipline has not taken in yet (the port's buffer).
    incoming: VecDeque<u8>,
    ld: Box<Discipline>,
}

impl Tty {
    fn new(termios: termios2) -> Self {
        let mut tty = Self {
            termios,
            winsize: winsize {
                ws_row: 0,
                ws_col: 0,
                ws_xpixel: 0,
                ws_ypixel: 0,
            },
            pgrp: None,
            session: None,
            stopped: false,
            tco_stopped: false,
            packet: false,
            pktstatus: 0,
            io_error: false,
            other_closed: false,
            exclusive: false,
            write_wakeup: false,
            incoming: VecDeque::new(),
            ld: Discipline::new(),
        };
        tty.set_termios(None);
        tty
    }

    fn iflag(&self, flag: u32) -> bool {
        self.termios.c_iflag & flag != 0
    }

    fn oflag(&self, flag: u32) -> bool {
        self.termios.c_oflag & flag != 0
    }

    fn lflag(&self, flag: u32) -> bool {
        self.termios.c_lflag & flag != 0
    }

    fn cc(&self, index: u32) -> u8 {
        self.termios.c_cc[index as usize]
    }

    /// `n_tty_set_termios`: the line discipline follows new settings, `old` being the ones before.
    pub fn set_termios(&mut self, old: Option<&termios2>) {
        let canon_changed =
            old.is_none_or(|old| (old.c_lflag ^ self.termios.c_lflag) & (ICANON | EXTPROC) != 0);
        let icanon = self.lflag(ICANON);
        let ld = &mut self.ld;
        if canon_changed {
            ld.read_flags = [0; SIZE / 64];
            ld.line_start = ld.read_tail;
            if !icanon || ld.read_count() == 0 {
                ld.canon_head = ld.read_tail;
                ld.push = false;
            } else {
                let last = ld.read_head - 1;
                ld.set_flag(last, true);
                ld.canon_head = ld.read_head;
                ld.push = true;
            }
            ld.commit_head = ld.read_head;
            ld.erasing = false;
            ld.lnext = false;
        }
        ld.icanon = icanon;
        let cooked = self.iflag(ISTRIP)
            || self.iflag(IUCLC)
            || self.iflag(IGNCR)
            || self.iflag(ICRNL)
            || self.iflag(INLCR)
            || self.lflag(ICANON)
            || self.iflag(IXON)
            || self.lflag(ISIG)
            || self.lflag(ECHO)
            || self.iflag(PARMRK);
        if cooked {
            let mut map = [0u64; 4];
            let mut set = |c: u8| map[c as usize / 64] |= 1 << (c % 64);
            if self.iflag(IGNCR) || self.iflag(ICRNL) {
                set(b'\r');
            }
            if self.iflag(INLCR) {
                set(b'\n');
            }
            if self.lflag(ICANON) {
                set(self.cc(VERASE));
                set(self.cc(VKILL));
                set(self.cc(VEOF));
                set(b'\n');
                set(self.cc(VEOL));
                if self.lflag(IEXTEN) {
                    set(self.cc(VWERASE));
                    set(self.cc(VLNEXT));
                    set(self.cc(VEOL2));
                    if self.lflag(ECHO) {
                        set(self.cc(VREPRINT));
                    }
                }
            }
            if self.iflag(IXON) {
                set(self.cc(VSTART));
                set(self.cc(VSTOP));
            }
            if self.lflag(ISIG) {
                set(self.cc(VINTR));
                set(self.cc(VQUIT));
                set(self.cc(VSUSP));
            }
            map[0] &= !(1 << DISABLED_CHAR);
            self.ld.char_map = map;
            self.ld.raw = false;
            self.ld.real_raw = false;
        } else {
            self.ld.raw = true;
            // Pseudo-terminals are `TTY_DRIVER_REAL_RAW`.
            self.ld.real_raw = (self.iflag(IGNBRK) || !self.iflag(BRKINT) && !self.iflag(PARMRK))
                && (self.iflag(IGNPAR) || !self.iflag(INPCK));
        }
    }

    /// `input_available_p`: whether a read finds something; for `poll`, a non-canonical reader
    /// with no timer wants its minimum.
    pub fn input_available(&self, poll: bool) -> bool {
        let amount = if poll && self.cc(VTIME) == 0 && self.cc(VMIN) != 0 {
            self.cc(VMIN) as usize
        } else {
            1
        };
        if self.ld.icanon && !self.lflag(EXTPROC) {
            self.ld.canon_head != self.ld.read_tail
        } else {
            self.ld.commit_head - self.ld.read_tail >= amount
        }
    }

    /// n_tty's `chars_in_buffer`: what the line discipline holds up to its last line end, or all it
    /// holds when not canonical.
    pub fn chars_in_buffer(&self) -> usize {
        let ld = &self.ld;
        let head = if self.lflag(ICANON) {
            ld.canon_head
        } else {
            ld.commit_head
        };
        // With EXTPROC a canonical terminal's reads pass its line end, and Linux's unsigned
        // difference wraps.
        head.wrapping_sub(ld.read_tail)
    }

    /// What `TIOCINQ` reports: the bytes a read could take, the end-of-file marks left out in
    /// canonical mode (`inq_canon`).
    pub fn queued(&self) -> u32 {
        let ld = &self.ld;
        if !(self.lflag(ICANON) && !self.lflag(EXTPROC)) {
            return ld.read_count() as u32;
        }
        if ld.canon_head == ld.read_tail {
            return 0;
        }
        let mut count = ld.canon_head - ld.read_tail;
        let mut tail = ld.read_tail;
        while mask(ld.canon_head) != mask(tail) {
            if ld.flag(tail) && ld.read_byte(tail) == DISABLED_CHAR {
                count -= 1;
            }
            tail += 1;
        }
        count as u32
    }

    /// The minimum and the timer of a non-canonical read, in tenths of a second.
    pub fn read_limits(&self) -> (u8, u8) {
        (self.cc(VMIN), self.cc(VTIME))
    }

    pub fn canonical(&self) -> bool {
        self.ld.icanon && !self.lflag(EXTPROC)
    }

    pub fn tostop(&self) -> bool {
        self.lflag(TOSTOP)
    }

    /// `copy_from_read_buf`: up to `wanted` bytes from the non-canonical buffer, as far as the ring
    /// goes; and whether more is left.
    fn copy_from_read_buf(&mut self, out: &mut Vec<u8>, wanted: &mut usize) -> bool {
        let extproc_eof = self.lflag(EXTPROC) && self.ld.icanon;
        let eof = self.cc(VEOF);
        let ld = &mut self.ld;
        let head = ld.commit_head;
        let tail = mask(ld.read_tail);
        let n = (head - ld.read_tail).min(SIZE - tail).min(*wanted);
        if n == 0 {
            return false;
        }
        let from = &mut ld.read_buf[tail..tail + n];
        let is_eof = n == 1 && from[0] == eof;
        let start = out.len();
        out.extend_from_slice(from);
        from.fill(0);
        ld.read_tail += n;
        if extproc_eof && is_eof && head == ld.read_tail {
            out.truncate(start);
            return false;
        }
        *wanted -= n;
        head != ld.read_tail
    }

    /// `canon_copy_from_read_buf`: one line, up to and with its end, the end-of-file mark left
    /// out; and whether more of the line is left.
    fn canon_copy_from_read_buf(&mut self, out: &mut Vec<u8>, wanted: &mut usize) -> bool {
        if *wanted == 0 {
            return false;
        }
        let ld = &mut self.ld;
        let canon_head = ld.canon_head;
        let n = (*wanted).min(canon_head - ld.read_tail);
        let mut end = None;
        for offset in 0..n {
            if ld.flag(ld.read_tail + offset) {
                end = Some(offset);
                break;
            }
        }
        let found = end.is_some();
        let mut take = end.unwrap_or(n);
        let consumed = take + usize::from(found);
        if !found || ld.read_byte(ld.read_tail + take) != DISABLED_CHAR {
            take = consumed;
        }
        for offset in 0..take {
            let c = ld.read_byte(ld.read_tail + offset);
            out.push(c);
            ld.read_buf[mask(ld.read_tail + offset)] = 0;
        }
        *wanted -= take;
        if let Some(offset) = end {
            let at = ld.read_tail + offset;
            ld.set_flag(at, false);
        }
        ld.read_tail += consumed;
        if found {
            if !ld.push {
                ld.line_start = ld.read_tail;
            } else {
                ld.push = false;
            }
            return false;
        }
        ld.read_tail != canon_head
    }

    /// `canon_skip_eof`: a read that ended just before an end-of-file mark takes the mark too.
    fn canon_skip_eof(&mut self) {
        let ld = &mut self.ld;
        if ld.read_tail == ld.canon_head {
            return;
        }
        if !ld.flag(ld.read_tail) || ld.read_byte(ld.read_tail) != DISABLED_CHAR {
            return;
        }
        let tail = ld.read_tail;
        ld.set_flag(tail, false);
        ld.read_tail += 1;
    }

    /// The copying part of `n_tty_read` once input is there: up to `wanted` bytes, with the
    /// packet-mode data byte first when `packet`.
    pub fn take(&mut self, out: &mut Vec<u8>, wanted: usize, packet: bool) {
        let mut wanted = wanted;
        if self.canonical() {
            while self.canon_copy_from_read_buf(out, &mut wanted) {}
            if wanted == 0 {
                self.canon_skip_eof();
            }
        } else {
            if packet && out.is_empty() {
                out.push(TIOCPKT_DATA as u8);
                wanted -= 1;
            }
            while self.copy_from_read_buf(out, &mut wanted) && wanted > 0 {}
        }
    }

    /// `n_tty_flush_buffer`'s reset of the read side.
    fn flush_input(&mut self) {
        self.ld.reset();
    }
}

/// A pair of terminals, `/dev/pts/<index>` being the slave.
pub(crate) struct Pty {
    pub index: u32,
    pub sides: [Tty; 2],
    /// `TTY_PTY_LOCK`: the slave may not be opened.
    pub locked: bool,
    /// The slave's node in devpts, until the master's last close takes it away.
    pub slave_node: Node,
    /// The open files of each side, for their counts and for hanging up.
    pub files: [Vec<Weak<RefCell<OpenFile>>>; 2],
    /// Whether each side has had its last close dealt with since it was last opened.
    pub closed: [bool; 2],
    /// Signals for process groups that keys and size changes raised, for the caller to send.
    pub signals: Vec<(Pid, u32)>,
    /// Sides whose input became readable, for the caller to send their I/O signals
    /// (`kill_fasync` with `POLL_IN`).
    pub input_signals: Vec<usize>,
    /// Bytes of room toward the slave promised to an outside writing the master (credit given on
    /// its end) that have not come.
    pub promised: u64,
}

impl Pty {
    pub fn new(index: u32, slave_node: Node) -> Self {
        let mut base = termios2 {
            c_iflag: 0,
            c_oflag: 0,
            c_cflag: B38400 | CS8 | CREAD,
            c_lflag: 0,
            c_line: 0,
            c_cc: INIT_C_CC,
            c_ispeed: 38400,
            c_ospeed: 38400,
        };
        let master = Tty::new(base);
        base.c_iflag = ICRNL | IXON;
        base.c_oflag = OPOST | ONLCR;
        base.c_lflag = ISIG | ICANON | ECHO | ECHOE | ECHOK | ECHOCTL | ECHOKE | IEXTEN;
        let slave = Tty::new(base);
        Self {
            index,
            sides: [master, slave],
            locked: true,
            slave_node,
            files: [Vec::new(), Vec::new()],
            closed: [false, true],
            signals: Vec::new(),
            input_signals: Vec::new(),
            promised: 0,
        }
    }

    /// Open files of one side that are still open.
    pub fn count(&self, side: usize) -> usize {
        self.files[side]
            .iter()
            .filter(|file| file.strong_count() > 0)
            .count()
    }

    /// `pty_write_room`.
    pub fn write_room(&self, side: usize) -> usize {
        if self.sides[side].stopped {
            return 0;
        }
        PTY_BUFFER_LIMIT.saturating_sub(self.sides[1 - side].incoming.len())
    }

    /// `pty_write`: bytes into the other side's buffer, as many as fit.
    fn driver_write(&mut self, side: usize, bytes: &[u8]) -> usize {
        if self.sides[side].stopped || bytes.is_empty() {
            return 0;
        }
        let room = PTY_BUFFER_LIMIT.saturating_sub(self.sides[1 - side].incoming.len());
        let n = bytes.len().min(room);
        self.sides[1 - side].incoming.extend(&bytes[..n]);
        n
    }

    fn put_char(&mut self, side: usize, c: u8) {
        self.driver_write(side, &[c]);
    }

    /// Takes what each side's buffer holds into its line discipline, as far as there is room
    /// (`flush_to_ldisc`), until nothing moves.
    pub fn settle(&mut self) {
        loop {
            let mut moved = false;
            for side in [MASTER, SLAVE] {
                if self.sides[side].incoming.is_empty() {
                    continue;
                }
                let bytes: Vec<u8> = self.sides[side].incoming.iter().copied().collect();
                let taken = self.receive(side, &bytes, true);
                if taken > 0 {
                    self.sides[side].incoming.drain(..taken);
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
    }

    /// `n_tty_receive_buf_common`: how many of `bytes` the side's line discipline took.
    pub fn receive(&mut self, side: usize, bytes: &[u8], flow: bool) -> usize {
        let mut done = 0;
        loop {
            let tty = &mut self.sides[side];
            let ld = &mut tty.ld;
            let mut room = SIZE as isize - (ld.read_head - ld.read_tail) as isize;
            if tty.termios.c_iflag & PARMRK != 0 {
                room = (room + 2) / 3;
            }
            room -= 1;
            if room <= 0 {
                // A canonical line with no end yet takes every byte, so the erase keys still work;
                // only the last one is kept.
                let overflow = ld.icanon && ld.canon_head == ld.read_tail;
                if overflow && room < 0 {
                    ld.read_head -= 1;
                }
                room = isize::from(overflow);
                ld.no_room = flow && room == 0;
            }
            let n = (bytes.len() - done).min(room as usize);
            if n == 0 {
                break;
            }
            self.receive_some(side, &bytes[done..done + n]);
            done += n;
        }
        done
    }

    /// `__receive_buf`.
    fn receive_some(&mut self, side: usize, bytes: &[u8]) {
        let tty = &self.sides[side];
        let preops = tty.iflag(ISTRIP) || tty.iflag(IUCLC) && tty.lflag(IEXTEN);
        let extproc = tty.lflag(EXTPROC);
        if tty.ld.real_raw || tty.ld.raw || extproc && !preops {
            let ld = &mut self.sides[side].ld;
            for &c in bytes {
                ld.put(c);
            }
        } else {
            for &c in bytes {
                self.receive_standard(side, c);
            }
            self.flush_echoes(side);
        }
        let tty = &mut self.sides[side];
        if tty.ld.icanon && !tty.lflag(EXTPROC) {
            return;
        }
        tty.ld.commit_head = tty.ld.read_head;
        if tty.ld.read_head != tty.ld.read_tail {
            self.input_signals.push(side);
        }
    }

    /// One byte of `n_tty_receive_buf_standard`.
    fn receive_standard(&mut self, side: usize, mut c: u8) {
        let tty = &mut self.sides[side];
        if tty.ld.lnext {
            tty.ld.lnext = false;
            if tty.iflag(ISTRIP) {
                c &= 0x7f;
            }
            if tty.iflag(IUCLC) && tty.lflag(IEXTEN) {
                c = to_lower(c);
            }
            self.receive_char(side, c);
            return;
        }
        if tty.iflag(ISTRIP) {
            c &= 0x7f;
        }
        if tty.iflag(IUCLC) && tty.lflag(IEXTEN) {
            c = to_lower(c);
        }
        if tty.lflag(EXTPROC) {
            tty.ld.put(c);
            return;
        }
        if tty.ld.special(c) {
            self.receive_special(side, c);
        } else {
            self.receive_char(side, c);
        }
    }

    /// `n_tty_receive_char`.
    fn receive_char(&mut self, side: usize, c: u8) {
        self.restart_on_any(side);
        let tty = &mut self.sides[side];
        if tty.lflag(ECHO) {
            tty.ld.finish_erasing();
            if tty.ld.canon_head == tty.ld.read_head {
                tty.ld.echo_set_canon_col();
            }
            self.echo_char(side, c);
            self.commit_echoes(side);
        }
        let tty = &mut self.sides[side];
        if c == 0xff && tty.iflag(PARMRK) {
            tty.ld.put(c);
        }
        tty.ld.put(c);
    }

    /// With `IXANY`, any key restarts output a stop key stopped.
    fn restart_on_any(&mut self, side: usize) {
        let tty = &self.sides[side];
        if tty.stopped && !tty.tco_stopped && tty.iflag(IXON) && tty.iflag(IXANY) {
            self.start(side);
            self.process_echoes(side);
        }
    }

    /// `n_tty_receive_char_special`.
    fn receive_special(&mut self, side: usize, mut c: u8) {
        let tty = &self.sides[side];
        if tty.iflag(IXON) && (c == tty.cc(VSTART) || c == tty.cc(VSTOP)) {
            if c == tty.cc(VSTART) {
                self.start(side);
                self.process_echoes(side);
            } else {
                self.stop(side);
            }
            return;
        }
        if tty.lflag(ISIG) {
            let signal = if c == tty.cc(VINTR) {
                Some(SIGINT)
            } else if c == tty.cc(VQUIT) {
                Some(SIGQUIT)
            } else if c == tty.cc(VSUSP) {
                Some(SIGTSTP)
            } else {
                None
            };
            if let Some(signal) = signal {
                self.isig(side, signal);
                if self.sides[side].iflag(IXON) {
                    self.start(side);
                }
                if self.sides[side].lflag(ECHO) {
                    self.echo_char(side, c);
                    self.commit_echoes(side);
                } else {
                    self.process_echoes(side);
                }
                return;
            }
        }
        self.restart_on_any(side);
        let tty = &self.sides[side];
        if c == b'\r' {
            if tty.iflag(IGNCR) {
                return;
            }
            if tty.iflag(ICRNL) {
                c = b'\n';
            }
        } else if c == b'\n' && tty.iflag(INLCR) {
            c = b'\r';
        }
        if self.sides[side].ld.icanon && self.receive_canonical(side, c) {
            return;
        }
        let tty = &mut self.sides[side];
        if tty.lflag(ECHO) {
            tty.ld.finish_erasing();
            if c == b'\n' {
                tty.ld.echo_char_raw(b'\n');
            } else {
                if tty.ld.canon_head == tty.ld.read_head {
                    tty.ld.echo_set_canon_col();
                }
                self.echo_char(side, c);
            }
            self.commit_echoes(side);
        }
        let tty = &mut self.sides[side];
        if c == 0xff && tty.iflag(PARMRK) {
            tty.ld.put(c);
        }
        tty.ld.put(c);
    }

    /// `n_tty_receive_char_canon`: the editing keys and line ends; whether `c` was one.
    fn receive_canonical(&mut self, side: usize, c: u8) -> bool {
        let tty = &mut self.sides[side];
        let iexten = tty.lflag(IEXTEN);
        let echo = tty.lflag(ECHO);
        if c == tty.cc(VERASE) || c == tty.cc(VKILL) || c == tty.cc(VWERASE) && iexten {
            self.eraser(side, c);
            self.commit_echoes(side);
            return true;
        }
        if c == tty.cc(VLNEXT) && iexten {
            tty.ld.lnext = true;
            if echo {
                tty.ld.finish_erasing();
                if tty.lflag(ECHOCTL) {
                    tty.ld.echo_char_raw(b'^');
                    tty.ld.echo_char_raw(b'\x08');
                    self.commit_echoes(side);
                }
            }
            return true;
        }
        if c == tty.cc(VREPRINT) && echo && iexten {
            tty.ld.finish_erasing();
            self.echo_char(side, c);
            let ld = &mut self.sides[side].ld;
            ld.echo_char_raw(b'\n');
            let (mut tail, head) = (ld.canon_head, ld.read_head);
            while mask(tail) != mask(head) {
                let byte = self.sides[side].ld.read_byte(tail);
                self.echo_char(side, byte);
                tail += 1;
            }
            self.commit_echoes(side);
            return true;
        }
        if c == b'\n' {
            if echo || tty.lflag(ECHONL) {
                tty.ld.echo_char_raw(b'\n');
                self.commit_echoes(side);
            }
            self.end_line(side, c);
            return true;
        }
        if c == tty.cc(VEOF) {
            self.end_line(side, DISABLED_CHAR);
            return true;
        }
        if c == tty.cc(VEOL) || c == tty.cc(VEOL2) && iexten {
            if echo {
                if tty.ld.canon_head == tty.ld.read_head {
                    tty.ld.echo_set_canon_col();
                }
                self.echo_char(side, c);
                self.commit_echoes(side);
            }
            let tty = &mut self.sides[side];
            if c == 0xff && tty.iflag(PARMRK) {
                tty.ld.put(c);
            }
            self.end_line(side, c);
            return true;
        }
        false
    }

    /// `n_tty_receive_handle_newline`.
    fn end_line(&mut self, side: usize, c: u8) {
        let ld = &mut self.sides[side].ld;
        let head = ld.read_head;
        ld.set_flag(head, true);
        ld.put(c);
        ld.canon_head = ld.read_head;
        self.input_signals.push(side);
    }

    /// `echo_char`: control characters tagged to show as `^X`.
    fn echo_char(&mut self, side: usize, c: u8) {
        let tty = &mut self.sides[side];
        if c == ECHO_OP_START {
            tty.ld.add_echo(ECHO_OP_START);
            tty.ld.add_echo(ECHO_OP_START);
        } else {
            if tty.lflag(ECHOCTL) && is_control(c) && c != b'\t' {
                tty.ld.add_echo(ECHO_OP_START);
            }
            tty.ld.add_echo(c);
        }
    }

    /// `eraser`: the erase, word erase and kill keys.
    fn eraser(&mut self, side: usize, c: u8) {
        #[derive(PartialEq)]
        enum Kind {
            Erase,
            Werase,
            Kill,
        }
        let tty = &mut self.sides[side];
        if tty.ld.read_head == tty.ld.canon_head {
            return;
        }
        let echo = tty.lflag(ECHO);
        let kind = if c == tty.cc(VERASE) {
            Kind::Erase
        } else if c == tty.cc(VWERASE) {
            Kind::Werase
        } else {
            if !echo {
                tty.ld.read_head = tty.ld.canon_head;
                return;
            }
            if !tty.lflag(ECHOK) || !tty.lflag(ECHOKE) || !tty.lflag(ECHOE) {
                tty.ld.read_head = tty.ld.canon_head;
                tty.ld.finish_erasing();
                let kill = tty.cc(VKILL);
                let echok = tty.lflag(ECHOK);
                self.echo_char(side, kill);
                if echok {
                    self.sides[side].ld.echo_char_raw(b'\n');
                }
                return;
            }
            Kind::Kill
        };
        let utf8 = tty.iflag(IUTF8);
        let continuation = |c: u8| utf8 && c & 0xc0 == 0x80;
        let mut seen_alphanumerics = 0;
        loop {
            let tty = &mut self.sides[side];
            if mask(tty.ld.read_head) == mask(tty.ld.canon_head) {
                break;
            }
            let mut head = tty.ld.read_head;
            let mut c;
            loop {
                head -= 1;
                c = tty.ld.read_byte(head);
                if !(continuation(c) && mask(head) != mask(tty.ld.canon_head)) {
                    break;
                }
            }
            if continuation(c) {
                break;
            }
            if kind == Kind::Werase {
                if is_alphanumeric(c) || c == b'_' {
                    seen_alphanumerics += 1;
                } else if seen_alphanumerics > 0 {
                    break;
                }
            }
            let mut count = tty.ld.read_head - head;
            tty.ld.read_head = head;
            if echo {
                if tty.lflag(ECHOPRT) {
                    if !tty.ld.erasing {
                        tty.ld.echo_char_raw(b'\\');
                        tty.ld.erasing = true;
                    }
                    self.echo_char(side, c);
                    while count > 1 {
                        count -= 1;
                        head += 1;
                        let ld = &mut self.sides[side].ld;
                        let byte = ld.read_byte(head);
                        ld.echo_char_raw(byte);
                        ld.echo_move_back_col();
                    }
                } else if kind == Kind::Erase && !tty.lflag(ECHOE) {
                    let erase = tty.cc(VERASE);
                    self.echo_char(side, erase);
                } else if c == b'\t' {
                    let mut chars = 0u32;
                    let mut after_tab = false;
                    let mut tail = tty.ld.read_head;
                    let echoctl = tty.lflag(ECHOCTL);
                    while mask(tail) != mask(tty.ld.canon_head) {
                        tail -= 1;
                        let c = tty.ld.read_byte(tail);
                        if c == b'\t' {
                            after_tab = true;
                            break;
                        } else if is_control(c) {
                            if echoctl {
                                chars += 2;
                            }
                        } else if !continuation(c) {
                            chars += 1;
                        }
                    }
                    tty.ld.echo_erase_tab(chars, after_tab);
                } else {
                    let echoctl = tty.lflag(ECHOCTL);
                    let times = usize::from(is_control(c) && echoctl)
                        + usize::from(!is_control(c) || echoctl);
                    for _ in 0..times {
                        tty.ld.echo_char_raw(b'\x08');
                        tty.ld.echo_char_raw(b' ');
                        tty.ld.echo_char_raw(b'\x08');
                    }
                }
            }
            if kind == Kind::Erase {
                break;
            }
        }
        let tty = &mut self.sides[side];
        if tty.ld.read_head == tty.ld.canon_head && echo {
            tty.ld.finish_erasing();
        }
    }

    /// `isig`: a signal to the foreground group, and unless `NOFLSH` the queues flushed.
    fn isig(&mut self, side: usize, signal: u32) {
        if let Some(pgrp) = self.sides[side].pgrp {
            self.signals.push((pgrp, signal));
        }
        if self.sides[side].lflag(NOFLSH) {
            return;
        }
        let ld = &mut self.sides[side].ld;
        ld.echo_head = 0;
        ld.echo_tail = 0;
        ld.echo_mark = 0;
        ld.echo_commit = 0;
        self.flush_output(side);
        self.sides[side].ld.reset();
        self.packet_flush_read(side);
    }

    /// `n_tty_packet_mode_flush`: a packet-mode reader of the other side is told input was
    /// flushed.
    fn packet_flush_read(&mut self, side: usize) {
        if self.sides[1 - side].packet {
            self.sides[side].pktstatus |= TIOCPKT_FLUSHREAD as u8;
        }
    }

    /// `n_tty_flush_buffer`: the side's input thrown away.
    pub fn flush_input(&mut self, side: usize) {
        self.sides[side].flush_input();
        self.packet_flush_read(side);
    }

    /// `pty_flush_buffer`: what the side wrote that the other side has not taken in is thrown
    /// away.
    pub fn flush_output(&mut self, side: usize) {
        self.sides[1 - side].incoming.clear();
        if self.sides[1 - side].packet {
            self.sides[side].pktstatus |= TIOCPKT_FLUSHWRITE as u8;
        }
    }

    /// `start_tty` and `pty_start`.
    pub fn start(&mut self, side: usize) {
        let tty = &mut self.sides[side];
        if !tty.stopped || tty.tco_stopped {
            return;
        }
        tty.stopped = false;
        if self.sides[1 - side].packet {
            let tty = &mut self.sides[side];
            tty.pktstatus &= !(TIOCPKT_STOP as u8);
            tty.pktstatus |= TIOCPKT_START as u8;
        }
    }

    /// `stop_tty` and `pty_stop`.
    pub fn stop(&mut self, side: usize) {
        let tty = &mut self.sides[side];
        if tty.stopped {
            return;
        }
        tty.stopped = true;
        if self.sides[1 - side].packet {
            let tty = &mut self.sides[side];
            tty.pktstatus &= !(TIOCPKT_START as u8);
            tty.pktstatus |= TIOCPKT_STOP as u8;
        }
    }

    /// `TCOOFF` and `TCOON`.
    pub fn output_off(&mut self, side: usize, off: bool) {
        let tty = &mut self.sides[side];
        if off && !tty.tco_stopped {
            tty.tco_stopped = true;
            self.stop(side);
        } else if !off && tty.tco_stopped {
            tty.tco_stopped = false;
            self.start(side);
        }
    }

    /// `tty_send_xchar`: one byte to the other side even while output is stopped.
    pub fn send_xchar(&mut self, side: usize, c: u8) {
        let was_stopped = self.sides[side].stopped;
        if was_stopped {
            self.start(side);
        }
        self.driver_write(side, &[c]);
        if was_stopped {
            self.stop(side);
        }
    }

    /// `do_output_char`: one byte with output processing, in at most `space` bytes; how many it
    /// used, or none when it does not fit.
    fn output_char(&mut self, side: usize, mut c: u8, space: usize) -> Option<usize> {
        if space == 0 {
            return None;
        }
        let tty = &mut self.sides[side];
        let utf8 = tty.iflag(IUTF8);
        let ld = &mut tty.ld;
        match c {
            b'\n' => {
                if tty.termios.c_oflag & ONLRET != 0 {
                    ld.column = 0;
                }
                if tty.termios.c_oflag & ONLCR != 0 {
                    if space < 2 {
                        return None;
                    }
                    ld.column = 0;
                    ld.canon_column = 0;
                    self.driver_write(side, b"\r\n");
                    return Some(2);
                }
                ld.canon_column = ld.column;
            }
            b'\r' => {
                if tty.termios.c_oflag & ONOCR != 0 && ld.column == 0 {
                    return Some(0);
                }
                if tty.termios.c_oflag & OCRNL != 0 {
                    c = b'\n';
                    if tty.termios.c_oflag & ONLRET != 0 {
                        ld.column = 0;
                        ld.canon_column = 0;
                    }
                } else {
                    ld.column = 0;
                    ld.canon_column = 0;
                }
            }
            b'\t' => {
                let spaces = 8 - (ld.column & 7);
                if tty.termios.c_oflag & TABDLY == XTABS {
                    if space < spaces as usize {
                        return None;
                    }
                    ld.column += spaces;
                    self.driver_write(side, &b"        "[..spaces as usize]);
                    return Some(spaces as usize);
                }
                ld.column += spaces;
            }
            b'\x08' => {
                if ld.column > 0 {
                    ld.column -= 1;
                }
            }
            _ => {
                if !is_control(c) {
                    if tty.termios.c_oflag & OLCUC != 0 {
                        c = to_upper(c);
                    }
                    if !(utf8 && c & 0xc0 == 0x80) {
                        ld.column += 1;
                    }
                }
            }
        }
        self.put_char(side, c);
        Some(1)
    }

    /// `process_output_block`: the leading bytes that need no processing beyond the column, as
    /// one write.
    fn output_block(&mut self, side: usize, bytes: &[u8]) -> usize {
        let space = self.write_room(side);
        if space == 0 {
            return 0;
        }
        let bytes = &bytes[..bytes.len().min(space)];
        let tty = &mut self.sides[side];
        let oflag = tty.termios.c_oflag;
        let utf8 = tty.iflag(IUTF8);
        let ld = &mut tty.ld;
        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i];
            match c {
                b'\n' => {
                    if oflag & ONLRET != 0 {
                        ld.column = 0;
                    }
                    if oflag & ONLCR != 0 {
                        break;
                    }
                    ld.canon_column = ld.column;
                }
                b'\r' => {
                    if oflag & ONOCR != 0 && ld.column == 0 || oflag & OCRNL != 0 {
                        break;
                    }
                    ld.column = 0;
                    ld.canon_column = 0;
                }
                b'\t' => break,
                b'\x08' => {
                    if ld.column > 0 {
                        ld.column -= 1;
                    }
                }
                _ => {
                    if !is_control(c) {
                        if oflag & OLCUC != 0 {
                            break;
                        }
                        if !(utf8 && c & 0xc0 == 0x80) {
                            ld.column += 1;
                        }
                    }
                }
            }
            i += 1;
        }
        self.driver_write(side, &bytes[..i])
    }

    /// One pass of `n_tty_write`'s loop: as much of `bytes` as the other side takes now.
    pub fn write(&mut self, side: usize, bytes: &[u8]) -> usize {
        let mut done = 0;
        if self.sides[side].oflag(OPOST) {
            while done < bytes.len() {
                done += self.output_block(side, &bytes[done..]);
                if done == bytes.len() {
                    break;
                }
                let space = self.write_room(side);
                if self.output_char(side, bytes[done], space).is_none() {
                    break;
                }
                done += 1;
            }
        } else {
            while done < bytes.len() {
                let n = self.driver_write(side, &bytes[done..]);
                if n == 0 {
                    break;
                }
                done += n;
            }
        }
        self.settle();
        done
    }

    /// `n_tty_process_echo_ops`: one operation at `tail`; the space left, or why it stopped.
    fn echo_op(&mut self, side: usize, tail: &mut usize, mut space: usize) -> Result<usize, bool> {
        let ld = &self.sides[side].ld;
        if mask(ld.echo_commit) == mask(*tail + 1) {
            return Err(true);
        }
        let op = ld.echo_byte(*tail + 1);
        match op {
            ECHO_OP_ERASE_TAB => {
                if mask(ld.echo_commit) == mask(*tail + 2) {
                    return Err(true);
                }
                let mut chars = u32::from(ld.echo_byte(*tail + 2));
                if chars & 0x80 == 0 {
                    chars += ld.canon_column;
                }
                let backspaces = (8 - (chars & 7)) as usize;
                if backspaces > space {
                    return Err(false);
                }
                space -= backspaces;
                for _ in 0..backspaces {
                    self.put_char(side, b'\x08');
                    let ld = &mut self.sides[side].ld;
                    if ld.column > 0 {
                        ld.column -= 1;
                    }
                }
                *tail += 3;
            }
            ECHO_OP_SET_CANON_COL => {
                let ld = &mut self.sides[side].ld;
                ld.canon_column = ld.column;
                *tail += 2;
            }
            ECHO_OP_MOVE_BACK_COL => {
                let ld = &mut self.sides[side].ld;
                if ld.column > 0 {
                    ld.column -= 1;
                }
                *tail += 2;
            }
            ECHO_OP_START => {
                if space == 0 {
                    return Err(false);
                }
                self.put_char(side, ECHO_OP_START);
                self.sides[side].ld.column += 1;
                space -= 1;
                *tail += 2;
            }
            _ => {
                if space < 2 {
                    return Err(false);
                }
                self.put_char(side, b'^');
                self.put_char(side, op ^ 0o100);
                self.sides[side].ld.column += 2;
                space -= 2;
                *tail += 2;
            }
        }
        Ok(space)
    }

    /// `__process_echoes`: what the echo buffer holds up to its commit point, to the other side.
    fn write_echoes(&mut self, side: usize) {
        let mut space = self.write_room(side);
        let mut tail = self.sides[side].ld.echo_tail;
        let mut stored = true;
        while mask(self.sides[side].ld.echo_commit) != mask(tail) {
            let c = self.sides[side].ld.echo_byte(tail);
            if c == ECHO_OP_START {
                match self.echo_op(side, &mut tail, space) {
                    Ok(left) => space = left,
                    Err(true) => {
                        stored = false;
                        break;
                    }
                    Err(false) => break,
                }
            } else {
                if self.sides[side].oflag(OPOST) {
                    match self.output_char(side, c, space) {
                        Some(used) => space -= used,
                        None => break,
                    }
                } else {
                    if space == 0 {
                        break;
                    }
                    self.put_char(side, c);
                    space -= 1;
                }
                tail += 1;
            }
        }
        if stored {
            let ld = &self.sides[side].ld;
            while ld.echo_commit > tail && ld.echo_commit - tail >= ECHO_DISCARD_WATERMARK {
                if ld.echo_byte(tail) == ECHO_OP_START {
                    tail += if ld.echo_byte(tail + 1) == ECHO_OP_ERASE_TAB {
                        3
                    } else {
                        2
                    };
                } else {
                    tail += 1;
                }
            }
        }
        self.sides[side].ld.echo_tail = tail;
    }

    /// `commit_echoes`: echoes go out once a block of them has gathered.
    fn commit_echoes(&mut self, side: usize) {
        let ld = &mut self.sides[side].ld;
        let head = ld.echo_head;
        ld.echo_mark = head;
        let old = ld.echo_commit - ld.echo_tail;
        let pending = head - ld.echo_tail;
        if pending < ECHO_COMMIT_WATERMARK || pending % ECHO_BLOCK > old % ECHO_BLOCK {
            return;
        }
        ld.echo_commit = head;
        self.write_echoes(side);
    }

    /// `process_echoes`: echoes up to the last mark go out.
    pub fn process_echoes(&mut self, side: usize) {
        let ld = &mut self.sides[side].ld;
        if ld.echo_mark == ld.echo_tail {
            return;
        }
        ld.echo_commit = ld.echo_mark;
        self.write_echoes(side);
    }

    /// `flush_echoes`: every echo goes out, at the end of a block of input.
    fn flush_echoes(&mut self, side: usize) {
        let tty = &mut self.sides[side];
        if !tty.lflag(ECHO) && !tty.lflag(ECHONL) || tty.ld.echo_commit == tty.ld.echo_head {
            return;
        }
        tty.ld.echo_commit = tty.ld.echo_head;
        self.write_echoes(side);
    }

    /// `tty_set_termios` with `pty_set_termios` and the line discipline's: new settings for the
    /// slave.
    pub fn set_termios(&mut self, side: usize, mut new: termios2) {
        let old = self.sides[side].termios;
        // Pseudo-terminals keep 8 bit characters and reception on (`pty_set_termios`), after
        // telling a packet-mode master about flow control changes.
        if self.sides[1 - side].packet {
            let extproc = (old.c_lflag | new.c_lflag) & EXTPROC != 0;
            let flow = |termios: &termios2| {
                termios.c_iflag & IXON != 0
                    && termios.c_cc[VSTOP as usize] == 0o23
                    && termios.c_cc[VSTART as usize] == 0o21
            };
            let (old_flow, new_flow) = (flow(&old), flow(&new));
            let tty = &mut self.sides[side];
            if old_flow != new_flow {
                tty.pktstatus &= !((TIOCPKT_DOSTOP | TIOCPKT_NOSTOP) as u8);
                tty.pktstatus |= if new_flow {
                    TIOCPKT_DOSTOP
                } else {
                    TIOCPKT_NOSTOP
                } as u8;
            }
            if extproc {
                tty.pktstatus |= TIOCPKT_IOCTL as u8;
            }
        }
        new.c_cflag &= !(CSIZE | PARENB);
        new.c_cflag |= CS8 | CREAD;
        let tty = &mut self.sides[side];
        tty.termios = new;
        tty.set_termios(Some(&old));
        if !tty.iflag(IXON) && old.c_iflag & IXON != 0 && !tty.tco_stopped {
            self.start(side);
            self.process_echoes(side);
        }
    }

    /// `pty_resize`: a new window size, and SIGWINCH for both sides' foreground groups when it
    /// changed.
    pub fn resize(&mut self, size: winsize) {
        let old = self.sides[SLAVE].winsize;
        if (old.ws_row, old.ws_col, old.ws_xpixel, old.ws_ypixel)
            == (size.ws_row, size.ws_col, size.ws_xpixel, size.ws_ypixel)
        {
            return;
        }
        let groups = [self.sides[SLAVE].pgrp, self.sides[MASTER].pgrp];
        if let Some(pgrp) = groups[0] {
            self.signals.push((pgrp, SIGWINCH));
        }
        if groups[1] != groups[0]
            && let Some(pgrp) = groups[1]
        {
            self.signals.push((pgrp, SIGWINCH));
        }
        self.sides[SLAVE].winsize = size;
        self.sides[MASTER].winsize = size;
    }
}

/// The input and output baud rates of settings, as `set_termios` loads them into the speed
/// fields (`tty_termios_input_baud_rate`, `tty_termios_baud_rate`).
pub(crate) fn load_speeds(termios: &mut termios2) {
    let rate = |code: u32, other: u32| -> u32 {
        if code == BOTHER {
            return other;
        }
        let mut index = code;
        if index & CBAUDEX != 0 {
            index = (index & !CBAUDEX) + 15;
        }
        BAUD_TABLE.get(index as usize).copied().unwrap_or(0)
    };
    let output = rate(termios.c_cflag & CBAUD, termios.c_ospeed);
    let input_code = (termios.c_cflag >> IBSHIFT) & CBAUD;
    let input = if input_code == B0 {
        output
    } else {
        rate(input_code, termios.c_ispeed)
    };
    termios.c_ispeed = input;
    termios.c_ospeed = output;
}
