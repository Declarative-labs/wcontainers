//! The combination tracker (plan 4.4): which state tuples each vertex shader is drawn with, so an
//! implicit front can build a new program's pipelines ahead of its first draw with
//! [`wcontainers_gpu::Backend::prepare`], and how many of the combinations it prepared were drawn.
//!
//! The front names things by keys of its own: `V` a vertex shader, `P` a program (the pair it
//! prepares), `T` a state tuple (blend, depth-stencil, raster, vertex layout, target layout, topology
//! class, binding layout keys). Keys are compared, so content keys and the front's own numbers both
//! serve. A combination is looked up once, when the front's bound state changes, and each draw then
//! counts by its [`Combination`] alone.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// A combination of program and tuple the tracker knows, as [`CombinationTracker::combination`]
/// gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Combination(usize);

#[derive(Debug)]
struct Entry<V, T> {
    vertex: V,
    tuple: T,
    draws: u64,
    /// Its draws when it was first prepared.
    prepared_at: Option<u64>,
}

#[derive(Debug)]
pub struct CombinationTracker<V, P, T> {
    /// Every combination drawn or prepared in this run.
    entries: Vec<Entry<V, T>>,
    ids: BTreeMap<(P, T), Combination>,
    /// The cache's record: draws per vertex shader and tuple.
    recorded: BTreeMap<(V, T), u64>,
}

impl<V, P, T> Default for CombinationTracker<V, P, T> {
    fn default() -> Self {
        CombinationTracker {
            entries: Vec::new(),
            ids: BTreeMap::new(),
            recorded: BTreeMap::new(),
        }
    }
}

impl<V: Ord + Clone, P: Ord + Clone, T: Ord + Clone> CombinationTracker<V, P, T> {
    /// Records `draws` draws of `vertex` with `tuple` from the cache's record.
    pub fn record(&mut self, vertex: &V, tuple: &T, draws: u64) {
        *self
            .recorded
            .entry((vertex.clone(), tuple.clone()))
            .or_default() += draws;
    }

    /// The combination of `program`, whose vertex shader is `vertex`, with `tuple`.
    pub fn combination(&mut self, vertex: &V, program: &P, tuple: &T) -> Combination {
        let key = (program.clone(), tuple.clone());
        if let Some(&c) = self.ids.get(&key) {
            return c;
        }
        let c = Combination(self.entries.len());
        self.entries.push(Entry {
            vertex: vertex.clone(),
            tuple: tuple.clone(),
            draws: 0,
            prepared_at: None,
        });
        self.ids.insert(key, c);
        c
    }

    /// Counts one draw of a combination.
    pub fn drawn(&mut self, c: Combination) {
        if let Some(e) = self.entries.get_mut(c.0) {
            e.draws += 1;
        }
    }

    /// Draws per vertex shader and tuple, of this run and the cache's record taken in by
    /// [`CombinationTracker::record`]: the record a front writes back into the cache.
    pub fn totals(&self) -> BTreeMap<(&V, &T), u64> {
        let mut totals: BTreeMap<(&V, &T), u64> = BTreeMap::new();
        for e in self.entries.iter().filter(|e| e.draws > 0) {
            *totals.entry((&e.vertex, &e.tuple)).or_default() += e.draws;
        }
        for ((v, t), &n) in &self.recorded {
            *totals.entry((v, t)).or_default() += n;
        }
        totals
    }

    /// The tuples to prepare `program` with, a new pairing whose vertex shader is `vertex`: every
    /// tuple `vertex` was drawn with, or for a vertex shader not yet drawn the one tuple drawn most
    /// often so far for which `fits` holds (its vertex layout covers the shader's inputs). Each is
    /// counted as prepared, once.
    pub fn pick(&mut self, vertex: &V, program: &P, fits: impl Fn(&T) -> bool) -> Vec<T> {
        let totals = self.totals();
        let mut picked: Vec<T> = totals
            .keys()
            .filter(|(v, _)| *v == vertex)
            .map(|(_, t)| (*t).clone())
            .collect();
        if picked.is_empty() {
            let mut by_tuple: BTreeMap<&T, u64> = BTreeMap::new();
            for ((_, t), n) in &totals {
                *by_tuple.entry(t).or_default() += n;
            }
            picked = by_tuple
                .into_iter()
                .filter(|(t, _)| fits(t))
                // The first of the most drawn, so equal counts pick the smallest tuple.
                .fold(None::<(&T, u64)>, |best, (t, n)| match best {
                    Some((_, m)) if m >= n => best,
                    _ => Some((t, n)),
                })
                .map(|(t, _)| t.clone())
                .into_iter()
                .collect();
        }
        for t in &picked {
            let c = self.combination(vertex, program, t);
            let e = &mut self.entries[c.0];
            e.prepared_at.get_or_insert(e.draws);
        }
        picked
    }

    /// How many prepared combinations a draw has used since they were prepared, and how many none
    /// has.
    pub fn prepared_use(&self) -> (u64, u64) {
        let mut counts = (0, 0);
        for e in &self.entries {
            match e.prepared_at {
                Some(at) if e.draws > at => counts.0 += 1,
                Some(_) => counts.1 += 1,
                None => {}
            }
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_combination_tracker_picks_the_tuples_drawn() {
        // Vertex shaders are letters, programs pairs, tuples numbers; tuple 30 has a vertex layout the
        // shader "c" cannot take.
        let mut t = CombinationTracker::<char, (char, char), u32>::default();
        for (vertex, program, tuple, draws) in [
            ('a', ('a', 'x'), 10, 3),
            ('a', ('a', 'x'), 20, 1),
            ('b', ('b', 'y'), 30, 5),
            ('b', ('b', 'y'), 20, 1),
        ] {
            let c = t.combination(&vertex, &program, &tuple);
            for _ in 0..draws {
                t.drawn(c);
            }
        }
        // A new pairing of a drawn vertex shader takes every tuple it was drawn with.
        assert_eq!(t.pick(&'a', &('a', 'z'), |_| true), [10, 20]);
        // An undrawn one takes the most drawn tuple that fits: 30 (5 draws) does not, 10 (3) does.
        assert_eq!(t.pick(&'c', &('c', 'z'), |&tuple| tuple != 30), [10]);
        // Equal counts pick the smaller tuple.
        t.record(&'d', &40, 3);
        assert_eq!(t.pick(&'e', &('e', 'z'), |&tuple| tuple != 30), [10]);
        // The cache's record counts as drawn for its vertex shader.
        assert_eq!(t.pick(&'d', &('d', 'z'), |_| true), [40]);
        // Of the five prepared combinations, two are drawn later.
        let az20 = t.combination(&'a', &('a', 'z'), &20);
        let cz10 = t.combination(&'c', &('c', 'z'), &10);
        t.drawn(az20);
        t.drawn(cz10);
        t.drawn(cz10);
        assert_eq!(t.prepared_use(), (2, 3));
        // Preparing a combination again counts it once.
        t.pick(&'a', &('a', 'z'), |_| true);
        assert_eq!(t.prepared_use(), (2, 3));
        // A pairing drawn before it is prepared counts as drawn only once a draw follows.
        assert_eq!(t.pick(&'a', &('a', 'x'), |_| true), [10, 20]);
        assert_eq!(t.prepared_use(), (2, 5));
        let ax10 = t.combination(&'a', &('a', 'x'), &10);
        t.drawn(ax10);
        assert_eq!(t.prepared_use(), (3, 4));
        // The record to write back: this run's draws per vertex shader and tuple, with the record's.
        // Two programs of vertex shader 'a' drew tuple 10 (3 and 1) and 20 (1 and 1).
        let totals: Vec<_> = t
            .totals()
            .into_iter()
            .map(|((&v, &tuple), n)| (v, tuple, n))
            .collect();
        assert_eq!(
            totals,
            [
                ('a', 10, 4),
                ('a', 20, 2),
                ('b', 20, 1),
                ('b', 30, 5),
                ('c', 10, 2),
                ('d', 40, 3),
            ]
        );
    }
}
