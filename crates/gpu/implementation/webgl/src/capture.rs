//! The vertices a draw makes, in the list order `capture_vertices` writes them, for draws GLES 3.0's
//! transform feedback cannot take (strips, fans, loops and indexed draws).

use wcontainers_gpu::{state::ProvokingVertex, state::Topology};

/// Each primitive's vertices as the list it draws, from the draw's vertex numbers in order (`None`
/// restarts the strip, fan or loop and drops an unfinished primitive). Odd strip triangles and fan
/// triangles keep the provoking vertex where `provoking` puts it, as the interface's converted fans
/// do; loops close when their piece ends.
pub fn list_order(
    topology: Topology,
    provoking: ProvokingVertex,
    numbers: impl Iterator<Item = Option<u32>>,
    out: &mut Vec<u32>,
) {
    // The piece's first vertex, the two before the current one, and how many it has had.
    let (mut first, mut a, mut b, mut n) = (0, 0, 0, 0usize);
    let close = |out: &mut Vec<u32>, n: usize, first: u32, last: u32| {
        if topology == Topology::LineLoop && n >= 2 {
            out.extend([last, first]);
        }
    };
    for v in numbers {
        let Some(v) = v else {
            close(out, n, first, b);
            n = 0;
            continue;
        };
        if n == 0 {
            first = v;
        }
        n += 1;
        match topology {
            Topology::PointList => out.push(v),
            Topology::LineList if n % 2 == 0 => out.extend([b, v]),
            Topology::TriangleList if n % 3 == 0 => out.extend([a, b, v]),
            Topology::LineStrip | Topology::LineLoop if n >= 2 => out.extend([b, v]),
            Topology::TriangleStrip if n >= 3 => out.extend(match (n % 2, provoking) {
                // Triangle n - 3 of the strip: even ones in order, odd ones swapped so the
                // provoking vertex stays first or last.
                (1, _) => [a, b, v],
                (_, ProvokingVertex::First) => [a, v, b],
                (_, ProvokingVertex::Last) => [b, a, v],
            }),
            Topology::TriangleFan if n >= 3 => out.extend(match provoking {
                ProvokingVertex::First => [b, v, first],
                ProvokingVertex::Last => [first, b, v],
            }),
            _ => {}
        }
        (a, b) = (b, v);
    }
    close(out, n, first, b);
}

/// Runs of consecutive vertex numbers in `list`, as (first, count): one draw each.
pub fn runs(list: &[u32]) -> impl Iterator<Item = (u32, u32)> + '_ {
    let mut at = 0;
    core::iter::from_fn(move || {
        let start = *list.get(at)?;
        let mut count = 1;
        while list.get(at + count as usize) == Some(&start.wrapping_add(count)) {
            count += 1;
        }
        at += count as usize;
        Some((start, count))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(topology: Topology, provoking: ProvokingVertex, numbers: &[Option<u32>]) -> Vec<u32> {
        let mut out = Vec::new();
        list_order(topology, provoking, numbers.iter().copied(), &mut out);
        out
    }

    fn all(n: u32) -> Vec<Option<u32>> {
        (0..n).map(Some).collect()
    }

    #[test]
    fn lists_keep_whole_primitives() {
        use ProvokingVertex::Last;
        assert_eq!(order(Topology::PointList, Last, &all(3)), [0, 1, 2]);
        assert_eq!(order(Topology::LineList, Last, &all(5)), [0, 1, 2, 3]);
        assert_eq!(
            order(Topology::TriangleList, Last, &all(7)),
            [0, 1, 2, 3, 4, 5]
        );
        let cut = [Some(0), Some(1), None, Some(2), Some(3), Some(4)];
        assert_eq!(order(Topology::TriangleList, Last, &cut), [2, 3, 4]);
    }

    #[test]
    fn strips_keep_the_provoking_vertex() {
        assert_eq!(
            order(Topology::TriangleStrip, ProvokingVertex::First, &all(5)),
            [0, 1, 2, 1, 3, 2, 2, 3, 4]
        );
        assert_eq!(
            order(Topology::TriangleStrip, ProvokingVertex::Last, &all(5)),
            [0, 1, 2, 2, 1, 3, 2, 3, 4]
        );
        assert_eq!(
            order(Topology::LineStrip, ProvokingVertex::Last, &all(3)),
            [0, 1, 1, 2]
        );
        let cut = [Some(5), Some(6), Some(7), None, Some(1), Some(2), Some(3)];
        assert_eq!(
            order(Topology::TriangleStrip, ProvokingVertex::Last, &cut),
            [5, 6, 7, 1, 2, 3]
        );
    }

    #[test]
    fn fans_and_loops_close() {
        assert_eq!(
            order(Topology::TriangleFan, ProvokingVertex::First, &all(4)),
            [1, 2, 0, 2, 3, 0]
        );
        assert_eq!(
            order(Topology::TriangleFan, ProvokingVertex::Last, &all(4)),
            [0, 1, 2, 0, 2, 3]
        );
        let cut = [
            Some(0),
            Some(1),
            Some(2),
            None,
            Some(3),
            Some(4),
            None,
            Some(5),
        ];
        assert_eq!(
            order(Topology::LineLoop, ProvokingVertex::Last, &cut),
            [0, 1, 1, 2, 2, 0, 3, 4, 4, 3]
        );
    }

    #[test]
    fn runs_join_consecutive_numbers() {
        let r: Vec<_> = runs(&[0, 1, 2, 1, 3, 2, 2, 3, 4]).collect();
        assert_eq!(r, [(0, 3), (1, 1), (3, 1), (2, 1), (2, 3)]);
        assert_eq!(runs(&[]).count(), 0);
    }
}
