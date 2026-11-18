//! Placed resources that overlap in a default heap (plan 5.2). Each has storage of its own; before
//! a command uses one, the bytes that overlapping resources written since have put there are copied
//! into it, oldest write first (`AliasCopied`), as Direct3D 12's data inheritance says: buffers
//! inherit from overlapping buffers, and textures from identically described textures at the same
//! offset. Every other overlap leaves contents undefined, so it takes no copies.
//!
//! A typeless texture viewed in a format whose bits the back end cannot view so (Direct3D's
//! `R8G8B8A8_UINT` view of an `R8G8B8A8_TYPELESS` texture stored as `rgba8unorm`) has a twin
//! texture in that format, and the two inherit from each other the same way (`ReinterpretCopy`).

use wcontainers_gpu::{
    Backend, Extent, Features, Format, FormatUse, Origin, SlowPath, TextureRegion, TextureUsage,
};

/// `DXGI_FORMAT_D32_FLOAT` and `DXGI_FORMAT_D16_UNORM`.
const DXGI_D32_FLOAT: u32 = 40;
const DXGI_D16_UNORM: u32 = 55;

use super::{Device, Resource, ResourceId, Storage, dxgi};
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
            // Writes made while nothing was noted are not stamped: the new resource takes the
            // bytes the old one holds now.
            if r.written == 0 {
                self.stamp += 1;
                r.written = self.stamp;
            }
        }
        if !pairs.is_empty() {
            self.aliased += 1;
        }
        self.resources[id.0 as usize].inherits = pairs;
    }

    /// The resource a view of texture `id` in DXGI format `format` reads and writes: `id` itself,
    /// or, when the format reinterprets the stored bits of a colour texture and the back end lacks
    /// `FORMAT_REINTERPRET`, its twin in that format, made at the first such view.
    pub fn view_holder(&mut self, id: ResourceId, format: u32) -> Result<ResourceId, Error> {
        let view = dxgi(format)?;
        let Some(Resource {
            storage: Storage::Texture { desc, .. },
            twins,
            ..
        }) = self.resources.get(id.0 as usize)
        else {
            return Ok(id);
        };
        let stored = desc.format;
        if view.linear() == stored.linear()
            || view.is_depth_stencil()
            || stored.is_depth_stencil()
            || self.backend.caps().has(Features::FORMAT_REINTERPRET)
        {
            return Ok(id);
        }
        let found = twins.iter().copied().find(|t| {
            matches!(&self.resources[t.0 as usize].storage,
                Storage::Texture { desc, .. } if desc.format.linear() == view.linear())
        });
        if let Some(t) = found {
            return Ok(t);
        }
        if desc.samples > 1 {
            return Err(Error::NotImplemented(
                "a multisampled texture viewed in a format reinterpreting its bits",
            ));
        }
        self.make_twin(id, view, format)
    }

    /// The twin of colour texture `id` stored in the depth format of the same bits, which
    /// comparison sampling needs (WebGPU compares only depth textures), made at the first such
    /// read; the two inherit each other's writes as other twins do. Gives it and its format.
    pub(super) fn depth_twin(&mut self, id: ResourceId) -> Result<(ResourceId, Format), Error> {
        let (stored, twins) = match self.resources.get(id.0 as usize) {
            Some(Resource {
                storage: Storage::Texture { desc, .. },
                twins,
                ..
            }) if desc.samples == 1 => (desc.format, twins),
            _ => {
                return Err(Error::NotImplemented(
                    "comparison sampling of a multisampled texture not stored in a depth format",
                ));
            }
        };
        let (depth, dxgi_depth) = match stored {
            Format::R32Float => (Format::Depth32Float, DXGI_D32_FLOAT),
            Format::R16Unorm => (Format::Depth16Unorm, DXGI_D16_UNORM),
            _ => {
                return Err(Error::NotImplemented(
                    "comparison sampling of a texture of no depth format's bits",
                ));
            }
        };
        let found = twins.iter().copied().find(|t| {
            matches!(&self.resources[t.0 as usize].storage,
                Storage::Texture { desc, .. } if desc.format == depth)
        });
        if let Some(t) = found {
            return Ok((t, depth));
        }
        // Bytes written before any twin existed were not stamped: the twin takes them as they are.
        let r = &mut self.resources[id.0 as usize];
        if r.written == 0 {
            self.stamp += 1;
            r.written = self.stamp;
        }
        self.backend.slow_paths().hit(SlowPath::DepthWrittenByDraw);
        Ok((self.make_twin(id, depth, dxgi_depth)?, depth))
    }

    /// Makes the twin of texture `id` stored as `view` (DXGI format `format`), and lists the two
    /// and the other twins as inheriting from each other.
    fn make_twin(
        &mut self,
        id: ResourceId,
        view: Format,
        format: u32,
    ) -> Result<ResourceId, Error> {
        let Some(Resource {
            storage: Storage::Texture { desc, .. },
            desc: rdesc,
            ..
        }) = self.resources.get(id.0 as usize)
        else {
            return Err(Error::Invalid("not a texture"));
        };
        let uses = self.backend.caps().formats.get(view);
        let mut tdesc = *desc;
        tdesc.format = view;
        if !uses.contains(FormatUse::RENDER) {
            tdesc.usage.remove(TextureUsage::RENDER);
        }
        if !uses.contains(FormatUse::STORAGE_WRITE) {
            tdesc.usage.remove(TextureUsage::STORAGE);
        }
        let rdesc = resource::ResourceDesc { format, ..*rdesc };
        let texture = self.backend.create_texture(&tdesc)?;
        let twin = ResourceId(self.resources.insert(Resource {
            desc: rdesc,
            heap: resource::HeapType::Default,
            storage: Storage::Texture {
                texture,
                desc: tdesc,
                views: Vec::new(),
                owned: true,
            },
            address: 0,
            placed: None,
            inherits: Vec::new(),
            written: 0,
            synced: 0,
            twins: Vec::new(),
            twin: true,
        }) as u32);
        let mut family = self.resources[id.0 as usize].twins.clone();
        family.push(id);
        for &o in &family {
            let r = &mut self.resources[o.0 as usize];
            if r.inherits.is_empty() {
                self.aliased += 1;
            }
            r.inherits.push(twin);
        }
        self.aliased += 1;
        self.resources[twin.0 as usize].inherits = family;
        self.resources[id.0 as usize].twins.push(twin);
        Ok(twin)
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
            let twins = |r: ResourceId| self.resources[r.0 as usize].twin;
            let path = if from.iter().any(|&o| twins(o)) || twins(id) {
                SlowPath::ReinterpretCopy
            } else {
                SlowPath::AliasCopied
            };
            self.backend.slow_paths().hit(path);
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
