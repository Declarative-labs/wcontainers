//! The tests every implementation of wcontainers-random-source runs, written against its traits alone.

use core::cell::{Cell, RefCell};
use core::mem::size_of;
use std::thread;

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_random_source::RandomSource;
use wcontainers_volatile_bytes::VolatileBytes;

/// The length of each random fill, the size of a `u128`.
const RANDOM_FILL_LENGTH: usize = size_of::<u128>();

/// A fill long enough to hold every byte value, missing one by chance far less than once in 2^128, and longer than
/// a page on every host.
const LONG_FILL_LENGTH: usize = u16::MAX as usize + 1;

/// The lengths filled: none, one, a fill, and a long fill.
const FILL_LENGTHS: &[usize] = &[usize::MIN, 1, RANDOM_FILL_LENGTH, LONG_FILL_LENGTH];

/// Bytes held one by one in cells, which a source or a sink reaches only through a buffer of their own, as bytes
/// another thread may change are; they count how they were reached.
struct CellBytes {
    cells: Vec<Cell<u8>>,
    /// Calls of `read_to` and `write_from`, each of which moves all the bytes.
    whole_moves: Cell<usize>,
    /// Calls of `read` and `write`, which move bytes piece by piece.
    piecewise_moves: Cell<usize>,
}

impl CellBytes {
    fn new(length: usize) -> Self {
        Self {
            cells: (0..length).map(|_| Cell::new(u8::MIN)).collect(),
            whole_moves: Cell::new(usize::MIN),
            piecewise_moves: Cell::new(usize::MIN),
        }
    }

    /// Returns a copy of the bytes held, without counting it as a move.
    fn contents(&self) -> Vec<u8> {
        self.cells.iter().map(Cell::get).collect()
    }

    fn clear(&self) {
        self.cells.iter().for_each(|cell| cell.set(u8::MIN));
        self.whole_moves.set(usize::MIN);
        self.piecewise_moves.set(usize::MIN);
    }
}

impl VolatileBytes for CellBytes {
    fn len(&self) -> u64 {
        self.cells.len() as u64
    }

    fn read(&self, into: &mut [u8]) -> usize {
        self.piecewise_moves.set(self.piecewise_moves.get() + 1);
        into.iter_mut()
            .zip(&self.cells)
            .map(|(byte, cell)| *byte = cell.get())
            .count()
    }

    fn write(&self, bytes: &[u8]) -> usize {
        self.piecewise_moves.set(self.piecewise_moves.get() + 1);
        self.cells
            .iter()
            .zip(bytes)
            .map(|(cell, &byte)| cell.set(byte))
            .count()
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        self.whole_moves.set(self.whole_moves.get() + 1);
        sink.write(&self.contents())
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        self.whole_moves.set(self.whole_moves.get() + 1);
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

/// Checks that bytes which are not a slice are filled whole, each fill in one write from a source into all of them,
/// and that every byte is written in one of a fill's worth of fills.
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
                (bytes.whole_moves.get(), bytes.piecewise_moves.get()),
                (1, usize::MIN),
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
