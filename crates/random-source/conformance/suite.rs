//! The tests every implementation of wcontainers-random-source runs, written against its traits alone.

use core::cell::{Cell, RefCell};
use core::mem::size_of;
use std::thread;

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_random_source::RandomSource;
use wcontainers_volatile_bytes::VolatileBytes;

/// A fill two fills match by chance once in 2^128, so two equal ones mean the bytes are not random.
const RANDOM_FILL_LENGTH: usize = size_of::<u128>();

/// A fill longer than a page that holds every byte value, missing one by chance far less than once in 2^128.
const LONG_FILL_LENGTH: usize = u16::MAX as usize + 1;

/// The lengths filled: none, one, a fill, and a long fill.
const FILL_LENGTHS: &[usize] = &[usize::MIN, 1, RANDOM_FILL_LENGTH, LONG_FILL_LENGTH];

/// How bytes were reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Move {
    /// A call of `read_to` or `write_from`, which moves all the bytes at once.
    Whole,
    /// A call of `read` or `write`, which moves bytes piece by piece.
    Piecewise,
}

/// Bytes in cells, reached only through a buffer of their own as bytes another thread may change are.
struct CellBytes {
    cells: Vec<Cell<u8>>,
    moves: RefCell<Vec<Move>>,
}

impl CellBytes {
    fn new(length: usize) -> Self {
        Self {
            cells: (0..length).map(|_| Cell::new(u8::MIN)).collect(),
            moves: RefCell::new(Vec::new()),
        }
    }

    /// Returns a copy of the bytes held, without counting it as a move.
    fn contents(&self) -> Vec<u8> {
        self.cells.iter().map(Cell::get).collect()
    }

    /// Returns the moves made so far, in order.
    fn moves(&self) -> Vec<Move> {
        self.moves.borrow().clone()
    }

    fn clear(&self) {
        self.cells.iter().for_each(|cell| cell.set(u8::MIN));
        self.moves.borrow_mut().clear();
    }
}

impl VolatileBytes for CellBytes {
    fn len(&self) -> u64 {
        self.cells.len() as u64
    }

    fn read(&self, into: &mut [u8]) -> usize {
        self.moves.borrow_mut().push(Move::Piecewise);
        into.iter_mut()
            .zip(&self.cells)
            .map(|(byte, cell)| *byte = cell.get())
            .count()
    }

    fn write(&self, bytes: &[u8]) -> usize {
        self.moves.borrow_mut().push(Move::Piecewise);
        self.cells
            .iter()
            .zip(bytes)
            .map(|(cell, &byte)| cell.set(byte))
            .count()
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        self.moves.borrow_mut().push(Move::Whole);
        sink.write(&self.contents())
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        self.moves.borrow_mut().push(Move::Whole);
        let mut buffer = vec![u8::MIN; self.cells.len()];
        let count = source.read(&mut buffer)?;
        self.cells
            .iter()
            .zip(&buffer[..count])
            .for_each(|(cell, &byte)| cell.set(byte));
        Ok(count)
    }
}

/// Checks that two fills of random bytes differ.
pub fn fill_random_bytes_differently<R: RandomSource>(source: R) {
    let mut first = [u8::MIN; RANDOM_FILL_LENGTH];
    let mut second = [u8::MIN; RANDOM_FILL_LENGTH];
    source
        .fill_random_bytes(&RefCell::new(&mut first[..]))
        .expect("the random source must fill what it is given");
    source
        .fill_random_bytes(&RefCell::new(&mut second[..]))
        .expect("the random source must fill what it is given");
    assert_ne!(first, second);
}

/// Checks that every byte of what is given is written, in one of a fill's worth of fills.
pub fn fill_every_byte_given<R: RandomSource>(source: R) {
    for &length in FILL_LENGTHS {
        let mut written = vec![false; length];
        let mut buffer = vec![u8::MIN; length];
        for _ in 0..RANDOM_FILL_LENGTH {
            buffer.fill(u8::MIN);
            source
                .fill_random_bytes(&RefCell::new(&mut buffer[..]))
                .expect("the random source must fill what it is given");
            for (written, &byte) in written.iter_mut().zip(&buffer) {
                *written |= byte != u8::MIN;
            }
        }
        let unwritten = written.iter().position(|&written| !written);
        assert_eq!(
            unwritten, None,
            "a fill of {length} bytes left a byte unwritten"
        );
    }
}

/// Checks that bytes not in a slice are filled in one write from a source, every byte in one of a fill's worth of fills.
pub fn fill_bytes_that_are_not_a_slice<R: RandomSource>(source: R) {
    for &length in FILL_LENGTHS {
        let mut written = vec![false; length];
        let bytes = CellBytes::new(length);
        for _ in 0..RANDOM_FILL_LENGTH {
            bytes.clear();
            source
                .fill_random_bytes(&bytes)
                .expect("the random source must fill what it is given");
            assert_eq!(
                bytes.moves(),
                [Move::Whole],
                "a fill of {length} bytes was not one write from a source"
            );
            for (written, byte) in written.iter_mut().zip(bytes.contents()) {
                *written |= byte != u8::MIN;
            }
        }
        let unwritten = written.iter().position(|&written| !written);
        assert_eq!(
            unwritten, None,
            "a fill of {length} bytes that are not a slice left a byte unwritten"
        );
    }
}

/// Checks that one long fill holds every byte value.
pub fn fill_random_bytes_with_every_byte_value<R: RandomSource>(source: R) {
    let mut buffer = vec![u8::MIN; LONG_FILL_LENGTH];
    source
        .fill_random_bytes(&RefCell::new(&mut buffer[..]))
        .expect("the random source must fill what it is given");
    let mut seen = [false; u8::MAX as usize + 1];
    for &byte in &buffer {
        seen[usize::from(byte)] = true;
    }
    let missing = seen.iter().position(|&seen| !seen);
    assert_eq!(missing, None, "a long fill lacked a byte value");
}

/// Checks that fills made at once on every processor all differ.
pub fn fill_random_bytes_differently_across_threads<R: RandomSource + Sync>(source: R) {
    let threads = thread::available_parallelism()
        .expect("the host must say how many processors it has")
        .get();
    let mut fills = thread::scope(|scope| {
        let fillers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut fill = [u8::MIN; RANDOM_FILL_LENGTH];
                    source
                        .fill_random_bytes(&RefCell::new(&mut fill[..]))
                        .expect("the random source must fill what it is given");
                    fill
                })
            })
            .collect();
        fillers
            .into_iter()
            .map(|filler| filler.join().expect("a filling thread panicked"))
            .collect::<Vec<_>>()
    });
    fills.sort_unstable();
    fills.dedup();
    assert_eq!(fills.len(), threads, "two threads' fills were equal");
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn fill_random_bytes_differently() {
            $crate::fill_random_bytes_differently($value);
        }

        #[test]
        fn fill_every_byte_given() {
            $crate::fill_every_byte_given($value);
        }

        #[test]
        fn fill_bytes_that_are_not_a_slice() {
            $crate::fill_bytes_that_are_not_a_slice($value);
        }

        #[test]
        fn fill_random_bytes_with_every_byte_value() {
            $crate::fill_random_bytes_with_every_byte_value($value);
        }

        #[test]
        fn fill_random_bytes_differently_across_threads() {
            $crate::fill_random_bytes_differently_across_threads($value);
        }
    };
}
