//! Placed resources that overlap in a default heap (plan 5.2). Each has storage of its own; before
//! a command uses one, the bytes that overlapping resources written since have put there are copied
//! into it, oldest write first (`AliasCopied`), as Direct3D 12's data inheritance says: buffers
//! inherit from overlapping buffers, and textures from identically described textures at the same
//! offset. Every other overlap leaves contents undefined, so it takes no copies.

use wcontainers_gpu::{Backend, Extent, Origin, SlowPath, TextureRegion};

use super::{Device, ResourceId, Storage};
use crate::Error;
use crate::resource;

impl<B: Backend> Device<B> {
    /// Lists `id`, just placed, and the live resources it overlaps, `overlaps`, as inheriting from
    /// each other where Direct3D 12 says they do.
    pub(super) fn inherit(&mut self, id: ResourceId, overlaps: &[ResourceId]) {
        let new = &self.resources[id.0 as usize];
        let (desc, at) = (new.desc, new.placed);
        let pairs: Vec<ResourceId> = overlaps
            .iter()
            .copied()
            .filter(|o| {
                let r = &self.resources[o.0 as usize];
                let buffers = desc.dimension == resource::DIMENSION_BUFFER
                    && r.desc.dimension == resource::DIMENSION_BUFFER;
                buffers || (r.desc == desc && r.placed == at)
            })
            .collect();
        for &o in &pairs {
            let r = &mut self.resources[o.0 as usize];
            if r.inherits.is_empty() {
                self.aliased += 1;
            }
            r.inherits.push(id);
        }
        if !pairs.is_empty() {
            self.aliased += 1;
        }
        self.resources[id.0 as usize].inherits = pairs;
    }

    /// Takes `id`, being released, out of the lists of the resources in `inherits`.
    pub(super) fn disinherit(&mut self, id: ResourceId, inherits: &[ResourceId]) {
        if inherits.is_empty() {
            return;
        }
        self.aliased -= 1;
        for o in inherits {
            if let Some(r) = self.resources.get_mut(o.0 as usize) {
                r.inherits.retain(|&i| i != id);
                if r.inherits.is_empty() {
                    self.aliased -= 1;
                }
            }
        }
    }

    /// Notes that the command being replayed uses `id`, and writes it if `write`.
    pub(super) fn note(&mut self, id: ResourceId, write: bool) {
        if self.aliased != 0 {
            self.touched.push((id, write));
        }
    }

    /// The resources whose writes `id` has not inherited yet.
    fn stale(&self, id: ResourceId) -> Vec<ResourceId> {
        let Some(r) = self.resources.get(id.0 as usize) else {
            return Vec::new();
        };
        let mut from: Vec<ResourceId> = r
            .inherits
            .iter()
            .copied()
            .filter(|o| self.resources[o.0 as usize].written > r.synced)
            .collect();
        from.sort_by_key(|o| self.resources[o.0 as usize].written);
        from
    }

    /// Whether a noted use needs bytes copied first. The copies must come before a pass opens, so
    /// the caller ends any open pass when this holds.
    pub(super) fn alias_due(&self) -> bool {
        self.touched
            .iter()
            .any(|&(id, _)| !self.stale(id).is_empty())
    }

    /// Brings the noted resources up to date, then stamps the ones the command writes.
    pub(super) fn alias_settle(&mut self) -> Result<(), Error> {
        if self.touched.is_empty() {
            return Ok(());
        }
        let mut touched = std::mem::take(&mut self.touched);
        let result = touched.iter().try_for_each(|&(id, _)| {
            let from = self.stale(id);
            if from.is_empty() {
                return Ok(());
            }
            self.backend.slow_paths().hit(SlowPath::AliasCopied);
            for o in from {
                self.alias_copy(o, id)?;
            }
            self.resources[id.0 as usize].synced = self.stamp;
            Ok(())
        });
        for &(id, write) in &touched {
            if write && let Some(r) = self.resources.get_mut(id.0 as usize) {
                self.stamp += 1;
                r.written = self.stamp;
                r.synced = self.stamp;
            }
        }
        touched.clear();
        self.touched = touched;
        result
    }

    /// Copies the bytes `src` and `dst` share into `dst`: the overlap of two buffers, or every
    /// subresource of two identical textures.
    fn alias_copy(&mut self, src: ResourceId, dst: ResourceId) -> Result<(), Error> {
        let (s, t) = (
            &self.resources[src.0 as usize],
            &self.resources[dst.0 as usize],
        );
        match (&s.storage, &t.storage) {
            (Storage::Buffer { buffer: a, .. }, Storage::Buffer { buffer: b, .. }) => {
                let (so, to) = (s.placed.map_or(0, |p| p.1), t.placed.map_or(0, |p| p.1));
                let start = so.max(to);
                let end = (so + s.desc.width).min(to + t.desc.width);
                if start < end {
                    let (a, b) = (a.clone(), b.clone());
                    // Placed offsets are 64 KiB apart and buffers are made a multiple of 4 long.
                    self.backend.copy_buffer(
                        &a,
                        start - so,
                        &b,
                        start - to,
                        (end - start).next_multiple_of(4),
                    );
                }
            }
            (
                Storage::Texture {
                    texture: a, desc, ..
                },
                Storage::Texture { texture: b, .. },
            ) => {
                let (a, b, desc) = (a.clone(), b.clone(), *desc);
                let three_d = desc.dimension == wcontainers_gpu::TextureDimension::D3;
                let layers = if three_d { 1 } else { desc.depth_or_layers };
                for mip in 0..desc.mip_levels {
                    let extent = Extent {
                        width: (desc.width >> mip).max(1),
                        height: (desc.height >> mip).max(1),
                        depth_or_layers: if three_d {
                            (desc.depth_or_layers >> mip).max(1)
                        } else {
                            1
                        },
                    };
                    for layer in 0..layers {
                        let region = |texture| TextureRegion {
                            texture,
                            mip,
                            layer,
                            origin: Origin::default(),
                            stencil: false,
                        };
                        self.backend.copy_texture(&region(&a), &region(&b), extent);
                    }
                }
            }
            _ => return Err(Error::Invalid("inheriting between a buffer and a texture")),
        }
        Ok(())
    }
}
