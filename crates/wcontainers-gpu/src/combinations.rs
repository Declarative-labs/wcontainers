//! The combination tracker (plan 4.4): which state tuples each vertex shader is drawn with, so an
//! implicit front can build a new program's pipelines ahead of its first draw with
//! [`crate::Backend::prepare`], and how many of the combinations it prepared were drawn.
//!
//! The front names things by keys of its own: `V` a vertex shader, `P` a program (the pair it
//! prepares), `T` a state tuple (blend, depth-stencil, raster, vertex layout, target layout, topology
//! class, binding layout keys). Keys are compared, so content keys and the front's own numbers both
//! serve.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

#[derive(Debug)]
pub struct CombinationTracker<V, P, T> {
    /// The tuples each vertex shader was drawn with, in this run or in the cache's record, and how
    /// many draws used each.
    by_vertex: BTreeMap<V, Vec<(T, u64)>>,
    /// Draws per tuple, over every vertex shader.
    totals: BTreeMap<T, u64>,
    /// Combinations prepared ahead, and whether a draw has used each since.
    prepared: BTreeMap<(P, T), bool>,
}

impl<V, P, T> Default for CombinationTracker<V, P, T> {
    fn default() -> Self {
        CombinationTracker {
            by_vertex: BTreeMap::new(),
            totals: BTreeMap::new(),
            prepared: BTreeMap::new(),
        }
    }
}

impl<V: Ord + Clone, P: Ord + Clone, T: Ord + Clone> CombinationTracker<V, P, T> {
    /// Records `draws` draws of `vertex` with `tuple`: a draw of this run (1), or a combination of
    /// the cache's record.
    pub fn record(&mut self, vertex: &V, tuple: &T, draws: u64) {
        let tuples = match self.by_vertex.get_mut(vertex) {
            Some(t) => t,
            None => self.by_vertex.entry(vertex.clone()).or_default(),
        };
        match tuples.iter_mut().find(|(t, _)| t == tuple) {
            Some((_, n)) => *n += draws,
            None => tuples.push((tuple.clone(), draws)),
        }
        match self.totals.get_mut(tuple) {
            Some(n) => *n += draws,
            None => {
                self.totals.insert(tuple.clone(), draws);
            }
        }
    }

    /// Records one draw of `program`, whose vertex shader is `vertex`, with `tuple`.
    pub fn drawn(&mut self, vertex: &V, program: &P, tuple: &T) {
        self.record(vertex, tuple, 1);
        if !self.prepared.is_empty()
            && let Some(used) = self.prepared.get_mut(&(program.clone(), tuple.clone()))
        {
            *used = true;
        }
    }

    /// The tuples to prepare `program` with, a new pairing whose vertex shader is `vertex`: every
    /// tuple `vertex` was drawn with, or for a vertex shader not yet drawn the one tuple drawn most
    /// often so far for which `fits` holds (its vertex layout covers the shader's inputs). Each is
    /// counted as prepared, once.
    pub fn pick(&mut self, vertex: &V, program: &P, fits: impl Fn(&T) -> bool) -> Vec<T> {
        let picked: Vec<T> = match self.by_vertex.get(vertex) {
            Some(tuples) => tuples.iter().map(|(t, _)| t.clone()).collect(),
            None => self
                .totals
                .iter()
                .filter(|(t, _)| fits(t))
                // The first of the most drawn, so equal counts pick the smallest tuple.
                .fold(None::<(&T, u64)>, |best, (t, &n)| match best {
                    Some((_, m)) if m >= n => best,
                    _ => Some((t, n)),
                })
                .map(|(t, _)| t.clone())
                .into_iter()
                .collect(),
        };
        for t in &picked {
            self.prepared
                .entry((program.clone(), t.clone()))
                .or_insert(false);
        }
        picked
    }

    /// How many prepared combinations a draw has used since, and how many none has.
    pub fn prepared_use(&self) -> (u64, u64) {
        let used = self.prepared.values().filter(|&&u| u).count() as u64;
        (used, self.prepared.len() as u64 - used)
    }
}
