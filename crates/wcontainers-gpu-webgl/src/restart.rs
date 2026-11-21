//! Where a buffer may hold the largest index value of each width. WebGL 2 restarts at that value in
//! every indexed draw, lists included, and cannot turn restart off, so a draw that must not restart
//! is checked only where a write may have put one, and its indices are rewritten when one is there.

use rangemap::RangeSet;
use std::ops::Range;
use wcontainers_gpu::IndexFormat;

/// Bytes per index of each width, in the order of [`Maxima`]'s sets.
const WIDTHS: [u64; 3] = [1, 2, 4];

fn slot(format: IndexFormat) -> usize {
    match format {
        IndexFormat::U8 => 0,
        IndexFormat::U16 => 1,
        IndexFormat::U32 => 2,
    }
}

/// For 8, 16 and 32-bit indices, the byte ranges of a buffer that may hold the largest value.
/// Indices lie at multiples of their width, so each width is tracked apart.
#[derive(Debug, Default)]
pub struct Maxima([RangeSet<u64>; 3]);

impl Maxima {
    /// `bytes` now lie at `at`. An index they touch may hold the largest value only if every byte
    /// they wrote into it is 0xff; the bytes of an index they cover in part are not known.
    pub fn wrote(&mut self, at: u64, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let end = at + bytes.len() as u64;
        let none = !bytes.contains(&0xff);
        for (set, w) in self.0.iter_mut().zip(WIDTHS) {
            let (lo, hi) = (at / w * w, end.next_multiple_of(w));
            set.remove(lo..hi);
            if none {
                continue;
            }
            let full = |i: u64| {
                let written = &bytes[(i.max(at) - at) as usize..((i + w).min(end) - at) as usize];
                written.iter().all(|&b| b == 0xff)
            };
            let mut found = (0..(hi - lo) / w).map(|k| lo + k * w).filter(|&i| full(i));
            if let Some(first) = found.next() {
                let last = found.next_back().unwrap_or(first);
                set.insert(first..last + w);
            }
        }
    }

    /// Bytes `range` were written with what the host does not know (by the GPU).
    pub fn unknown(&mut self, range: Range<u64>) {
        if range.is_empty() {
            return;
        }
        for set in &mut self.0 {
            set.insert(range.clone());
        }
    }

    /// Bytes `range` hold no index of the largest value, of any width.
    pub fn cleared(&mut self, range: Range<u64>) {
        for (set, w) in self.0.iter_mut().zip(WIDTHS) {
            let (start, end) = (range.start.next_multiple_of(w), range.end / w * w);
            if start < end {
                set.remove(start..end);
            }
        }
    }

    /// Whether indices of `format` in bytes `range` may hold its largest value.
    pub fn may_hold(&self, format: IndexFormat, range: Range<u64>) -> bool {
        !range.is_empty() && self.0[slot(format)].overlaps(&range)
    }

    /// Whether bytes `range` may hold the largest index value of any width.
    pub fn any(&self, range: Range<u64>) -> bool {
        [IndexFormat::U8, IndexFormat::U16, IndexFormat::U32]
            .into_iter()
            .any(|f| self.may_hold(f, range.clone()))
    }
}

/// Indices of a draw that must not restart, rewritten so GL never meets its restart value.
#[derive(Debug, PartialEq)]
pub enum Rewrite {
    /// No index is the largest value: draw them as they are.
    Unneeded,
    /// 32-bit indices to draw instead, with the base vertex raised by the second value: 8 and 16-bit
    /// indices widened, 32-bit ones lowered by their smallest.
    Widened(Vec<u8>, u32),
    /// 32-bit indices holding both 0 and the largest value, which no shift brings into GL's range.
    Unreachable,
}

/// The rewrite `bytes`, indices of `format`, need to draw without restarting.
pub fn rewrite(bytes: &[u8], format: IndexFormat) -> Rewrite {
    let width = format.size() as usize;
    let largest = format.restart_value();
    let values = || {
        bytes.chunks_exact(width).map(|c| {
            let mut word = [0; 4];
            word[..width].copy_from_slice(c);
            u32::from_le_bytes(word)
        })
    };
    if !values().any(|v| v == largest) {
        return Rewrite::Unneeded;
    }
    let shift = match format {
        IndexFormat::U32 => match values().min() {
            Some(0) | None => return Rewrite::Unreachable,
            Some(m) => m,
        },
        IndexFormat::U8 | IndexFormat::U16 => 0,
    };
    Rewrite::Widened(
        values().flat_map(|v| (v - shift).to_le_bytes()).collect(),
        shift,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_track_each_width_at_its_own_alignment() {
        let mut m = Maxima::default();
        // A 16-bit 0xffff at bytes 3..5 straddles two 16-bit indices: only 8-bit ones hold it.
        m.wrote(0, &[0, 0, 0, 0xff, 0xff, 0, 0, 0]);
        assert!(m.may_hold(IndexFormat::U8, 3..4));
        assert!(m.may_hold(IndexFormat::U8, 4..5));
        assert!(!m.may_hold(IndexFormat::U8, 5..8));
        assert!(!m.may_hold(IndexFormat::U16, 0..8));
        assert!(!m.may_hold(IndexFormat::U32, 0..8));
        m.wrote(8, &[0xff; 4]);
        assert!(m.may_hold(IndexFormat::U16, 10..12));
        assert!(m.may_hold(IndexFormat::U32, 8..12));
        assert!(!m.may_hold(IndexFormat::U32, 0..8));
        // Writing zeros over part of an index forgets it at every width that index touches.
        m.wrote(9, &[0; 3]);
        assert!(!m.may_hold(IndexFormat::U32, 8..12));
        assert!(!m.may_hold(IndexFormat::U16, 10..12));
        assert!(!m.may_hold(IndexFormat::U16, 8..10));
        assert!(m.may_hold(IndexFormat::U8, 8..9));
        m.wrote(0, &[0; 12]);
        assert!(!m.any(0..12));
    }

    #[test]
    fn writes_of_part_of_an_index_may_complete_the_largest_value() {
        let mut m = Maxima::default();
        // 16-bit indices 1, 2 and 0x00ff; one byte then makes the third 0xffff.
        m.wrote(0, &[1, 0, 2, 0, 0xff, 0, 0, 0]);
        assert!(!m.may_hold(IndexFormat::U16, 0..8));
        m.wrote(5, &[0xff]);
        assert!(m.may_hold(IndexFormat::U16, 4..6));
        assert!(!m.may_hold(IndexFormat::U16, 0..4));
        // So does a write of the low half of a 32-bit index whose high half is unknown.
        m.wrote(8, &[0xff, 0xff]);
        assert!(m.may_hold(IndexFormat::U32, 8..12));
        // A byte other than 0xff in an index rules it out, whatever else it holds.
        m.wrote(9, &[0]);
        assert!(!m.may_hold(IndexFormat::U32, 8..12));
        assert!(!m.may_hold(IndexFormat::U16, 8..10));
        assert!(m.may_hold(IndexFormat::U8, 8..9));
    }

    #[test]
    fn gpu_writes_are_unknown_until_cleared_or_written() {
        let mut m = Maxima::default();
        m.unknown(100..200);
        m.unknown(300..400);
        assert!(m.may_hold(IndexFormat::U16, 150..152));
        assert!(!m.any(200..300));
        m.cleared(120..380);
        assert!(m.may_hold(IndexFormat::U32, 100..120));
        assert!(!m.any(120..380));
        assert!(m.may_hold(IndexFormat::U8, 380..400));
        m.unknown(0..1000);
        assert!(m.any(500..501));
        m.wrote(0, &[1; 1000]);
        assert!(!m.any(0..1000));
    }

    #[test]
    fn rewrites_widen_or_lower_indices() {
        assert_eq!(rewrite(&[0, 1, 2], IndexFormat::U8), Rewrite::Unneeded);
        assert_eq!(
            rewrite(&[0, 1, 0xff], IndexFormat::U8),
            Rewrite::Widened(vec![0, 0, 0, 0, 1, 0, 0, 0, 0xff, 0, 0, 0], 0)
        );
        assert_eq!(
            rewrite(&[0xff, 0xff, 2, 0], IndexFormat::U16),
            Rewrite::Widened(vec![0xff, 0xff, 0, 0, 2, 0, 0, 0], 0)
        );
        let u32s = |v: &[u32]| v.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>();
        assert_eq!(
            rewrite(&u32s(&[5, u32::MAX, 7]), IndexFormat::U32),
            Rewrite::Widened(u32s(&[0, u32::MAX - 5, 2]), 5)
        );
        assert_eq!(
            rewrite(&u32s(&[0, u32::MAX]), IndexFormat::U32),
            Rewrite::Unreachable
        );
    }
}
