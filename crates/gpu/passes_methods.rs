//! Methods of the types [`crate::passes`] declares.

use crate::draw::{
    AspectOps, ColorAttachment, DepthAttachment, DrawCall, LoadOp, PassDesc, ScissorRect, StoreOp,
    Viewport,
};
use crate::error::GpuError;
use crate::format::{Format, FormatKind};
use crate::helpers::{Builtin, Builtins, Helpers};
use crate::passes::*;
use crate::resource::{
    BindingEntry, BindingResource, Extent, Origin, TextureDesc, TextureDimension, TextureRegion,
    TextureUsage, TextureViewDesc, ViewDimension,
};
use crate::shader::StageSet;
use crate::slow::SlowPath;
use crate::{Gpu, Object};
use crate::{MAX_COLOR_TARGETS, caps::Features};
use alloc::vec::Vec;
use core::ops::Range;

impl<B: Gpu> Clone for Target<B> {
    fn clone(&self) -> Self {
        Target {
            view: self.view.clone(),
            texture: self.texture.clone(),
            format: self.format,
            mip: self.mip,
            layer: self.layer,
            depth_slice: self.depth_slice,
            width: self.width,
            height: self.height,
            samples: self.samples,
        }
    }
}

impl<B: Gpu> Clone for ColorTarget<B> {
    fn clone(&self) -> Self {
        ColorTarget {
            target: self.target.clone(),
            resolve: self.resolve.clone(),
        }
    }
}

impl<B: Gpu> Clone for PassTargets<B> {
    fn clone(&self) -> Self {
        PassTargets {
            colors: self.colors.clone(),
            depth: self.depth.clone(),
            empty: self.empty,
        }
    }
}

impl<B: Gpu> Target<B> {
    fn region(&self) -> TextureRegion<'_, B> {
        TextureRegion {
            texture: &self.texture,
            mip: self.mip,
            layer: self.layer,
            origin: Origin {
                x: 0,
                y: 0,
                z: self.depth_slice,
            },
            stencil: false,
        }
    }
}

impl PassOps {
    /// Every attachment loaded and stored: how an implicit front's pass treats its targets.
    pub const LOAD_STORE: PassOps = PassOps {
        colors: [(LoadOp::Load, StoreOp::Store); MAX_COLOR_TARGETS],
        depth: AspectOps {
            load: LoadOp::Load,
            store: StoreOp::Store,
            read_only: false,
        },
        stencil: AspectOps {
            load: LoadOp::Load,
            store: StoreOp::Store,
            read_only: false,
        },
    };
}

impl<B: Gpu> Default for PassTracker<B> {
    fn default() -> Self {
        PassTracker {
            targets: None,
            ops: PassOps::LOAD_STORE,
            open: None,
            pool: Vec::new(),
            queries: None,
            generation: 0,
        }
    }
}

impl<B: Gpu> PassTracker<B> {
    /// The targets set, if any.
    pub fn targets(&self) -> Option<&PassTargets<B>> {
        self.targets.as_ref()
    }

    /// A number that changes whenever the targets are set or the depth-stencil target is cleared.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Whether a pass is open.
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Sets the targets later draws go into (an implicit front's target change), closing the open pass.
    pub fn set_targets(&mut self, b: &mut B, targets: PassTargets<B>) -> Result<(), GpuError> {
        self.close(b)?;
        self.targets = Some(targets);
        self.ops = PassOps::LOAD_STORE;
        self.generation += 1;
        Ok(())
    }

    /// Clears colour target `index`: the next pass loads it with `Clear`. A pass with draws in it
    /// is closed first.
    pub fn clear_color(
        &mut self,
        b: &mut B,
        index: usize,
        value: [u32; 4],
    ) -> Result<(), GpuError> {
        if self.open.is_some() {
            self.close(b)?;
        }
        self.ops.colors[index].0 = LoadOp::Clear(value);
        Ok(())
    }

    /// Clears the depth and stencil aspects named: the next pass loads them with `Clear`.
    pub fn clear_depth_stencil(
        &mut self,
        b: &mut B,
        depth: Option<u32>,
        stencil: Option<u8>,
    ) -> Result<(), GpuError> {
        if self.open.is_some() {
            self.close(b)?;
        }
        self.generation += 1;
        if let Some(d) = depth {
            self.ops.depth.load = LoadOp::Clear(d);
        }
        if let Some(s) = stencil {
            self.ops.stencil.load = LoadOp::Clear(s);
        }
        Ok(())
    }

    /// Begins an explicit front's pass on `targets` with `ops`, closing any open one. A pass whose
    /// draws write storage may be split (see [`PassTracker::before_draw`]); every part but the last
    /// must keep what it drew, so the front begins such a pass with `Store` where its API discards,
    /// and a split of a pass that discards is refused.
    pub fn begin(
        &mut self,
        b: &mut B,
        targets: PassTargets<B>,
        ops: PassOps,
    ) -> Result<(), GpuError> {
        self.close(b)?;
        self.targets = Some(targets);
        self.ops = ops;
        self.generation += 1;
        self.open_pass(b)
    }

    /// Ends an explicit front's pass. The targets stay set, loaded and stored by any later pass.
    pub fn end(&mut self, b: &mut B) -> Result<(), GpuError> {
        self.close(b)
    }

    /// Makes ready for a draw using `uses`: opens a pass on the targets if none is open, and splits
    /// the open one where the draw would use a resource another way than an earlier draw wrote it as
    /// storage, or write as storage one an earlier draw used another way: the part ends as the pass
    /// would ([`PassTracker::close`]) and the next begins with every attachment loaded and the pass's
    /// store and read-only aspects. Returns the area every
    /// attachment covers where the pass was fitted; the front cuts the draw's scissors to it, with the
    /// scissor test on.
    pub fn before_draw(
        &mut self,
        b: &mut B,
        uses: &[ScopeUse],
    ) -> Result<Option<ScissorRect>, GpuError> {
        if self.open.is_none() {
            self.open_pass(b)?;
        } else if let Some(open) = &self.open {
            // Without storage writes in the pass or the draw, nothing conflicts.
            let conflict = (!open.written.is_empty() || uses.iter().any(|u| u.storage_write))
                && uses.iter().any(|u| {
                    let list = if u.storage_write {
                        &open.other
                    } else {
                        &open.written
                    };
                    list.binary_search(&u.resource).is_ok()
                });
            if conflict {
                // Each part keeps what the last drew; the next loads it with the pass's other ops.
                if self.discards() {
                    return Err(GpuError::Invalid(
                        "a pass split at a storage write discards an attachment",
                    ));
                }
                b.record_slow_path(SlowPath::UsageScopeSplit);
                let mut ops = self.ops;
                for (load, _) in &mut ops.colors {
                    *load = LoadOp::Load;
                }
                ops.depth.load = LoadOp::Load;
                ops.stencil.load = LoadOp::Load;
                self.close(b)?;
                self.ops = ops;
                self.open_pass(b)?;
            }
        }
        let open = self.open.as_mut().ok_or(GpuError::Invalid("no pass"))?;
        for u in uses {
            let list = if u.storage_write {
                &mut open.written
            } else {
                &mut open.other
            };
            if let Err(at) = list.binary_search(&u.resource) {
                list.insert(at, u.resource);
            }
        }
        Ok(open.common)
    }

    /// Closes the open pass (before a copy, a dispatch, a read back or a clear outside it): copies
    /// fitted attachments back, and ends a running query segment, which the next pass continues.
    /// A clear no draw followed is made by a pass of its own.
    pub fn close(&mut self, b: &mut B) -> Result<(), GpuError> {
        if self.open.is_none() && self.clears_pending() {
            self.open_pass(b)?;
        }
        let Some(open) = self.open.take() else {
            return Ok(());
        };
        if open.query {
            b.end_query();
        }
        b.end_pass();
        let targets = self
            .targets
            .as_ref()
            .ok_or(GpuError::Invalid("no targets"))?;
        if let Some(area) = open.common {
            let extent = Extent {
                width: area.width,
                height: area.height,
                depth_or_layers: 1,
            };
            let targets = targets.clone();
            for (i, c) in targets.colors.iter().enumerate() {
                let Some(c) = c else { continue };
                match open.colors[i] {
                    Some((t, true)) if c.target.samples > 1 => {
                        let temporary = self.pool[t].target(c.target.format);
                        self.copy_samples(b, &temporary, &c.target, extent)?;
                    }
                    Some((t, true)) => {
                        b.copy_texture(&self.pool[t].region(), &c.target.region(), extent)
                    }
                    _ => {}
                }
                if let (Some(t), Some(r)) = (open.resolves[i], &c.resolve) {
                    b.copy_texture(&self.pool[t].region(), &r.region(), extent);
                }
            }
        }
        for t in &mut self.pool {
            t.in_use = false;
        }
        // What the pass stored is what the next one loads.
        self.ops = PassOps::LOAD_STORE;
        Ok(())
    }

    /// Gives the occlusion query set later passes name, with room for `count` queries; queries
    /// begun after it take indices from 0.
    pub fn set_occlusion(
        &mut self,
        b: &mut B,
        set: B::QuerySet,
        count: u32,
    ) -> Result<(), GpuError> {
        self.close(b)?;
        self.queries = Some(PassQueries {
            set,
            count,
            next: 0,
            running: None,
        });
        Ok(())
    }

    /// Begins an occlusion query. It runs in the open pass and in every pass opened until
    /// [`PassTracker::end_query`], one interface query per pass.
    pub fn begin_query(&mut self, b: &mut B) -> Result<(), GpuError> {
        let q = self
            .queries
            .as_mut()
            .ok_or(GpuError::Invalid("begin_query without an occlusion set"))?;
        if q.running.is_some() {
            return Err(GpuError::Invalid("begin_query while a query runs"));
        }
        q.running = Some(q.next);
        if self.open.is_some() {
            self.begin_segment(b)?;
        }
        Ok(())
    }

    /// Ends the running occlusion query, and gives the indices of its interface queries, one per pass
    /// it ran in; the front adds their counts or combines their answers.
    pub fn end_query(&mut self, b: &mut B) -> Result<Range<u32>, GpuError> {
        let q = self
            .queries
            .as_mut()
            .ok_or(GpuError::Invalid("end_query without an occlusion set"))?;
        let first = q
            .running
            .take()
            .ok_or(GpuError::Invalid("end_query without a running query"))?;
        if let Some(open) = &mut self.open
            && open.query
        {
            open.query = false;
            b.end_query();
        }
        Ok(first..q.next)
    }

    /// Whether the pass ends by discarding an attachment the targets have.
    fn discards(&self) -> bool {
        let Some(t) = &self.targets else {
            return false;
        };
        let colors = t
            .colors
            .iter()
            .zip(&self.ops.colors)
            .any(|(c, (_, store))| c.is_some() && *store == StoreOp::Discard);
        let depth = t.depth.as_ref().is_some_and(|d| {
            let (has_depth, has_stencil) = d.format.aspects();
            (has_depth && self.ops.depth.store == StoreOp::Discard)
                || (has_stencil && self.ops.stencil.store == StoreOp::Discard)
        });
        colors || depth
    }

    /// Whether the next pass would clear an attachment the targets have.
    fn clears_pending(&self) -> bool {
        let clear = |l: LoadOp<_>| matches!(l, LoadOp::Clear(_));
        let Some(t) = &self.targets else {
            return false;
        };
        let colors = t
            .colors
            .iter()
            .zip(&self.ops.colors)
            .any(|(c, (load, _))| c.is_some() && clear(*load));
        let depth = t.depth.as_ref().is_some_and(|d| {
            let (has_depth, has_stencil) = d.format.aspects();
            (has_depth && matches!(self.ops.depth.load, LoadOp::Clear(_)))
                || (has_stencil && matches!(self.ops.stencil.load, LoadOp::Clear(_)))
        });
        colors || depth
    }

    fn begin_segment(&mut self, b: &mut B) -> Result<(), GpuError> {
        let (Some(q), Some(open)) = (&mut self.queries, &mut self.open) else {
            return Ok(());
        };
        if q.next == q.count {
            return Err(GpuError::Invalid("occlusion query set full"));
        }
        b.begin_query(q.next);
        q.next += 1;
        open.query = true;
        Ok(())
    }

    /// A pooled temporary of this size, format and sample count, made if none is free.
    fn temporary(&mut self, b: &mut B, key: (u32, u32, Format, u32)) -> Result<usize, GpuError> {
        if let Some(i) = self.pool.iter().position(|t| !t.in_use && t.key == key) {
            self.pool[i].in_use = true;
            return Ok(i);
        }
        let (width, height, format, samples) = key;
        let texture = b.create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format,
            width,
            height,
            depth_or_layers: 1,
            mip_levels: 1,
            samples,
            usage: TextureUsage::RENDER
                | TextureUsage::COPY_SRC
                | TextureUsage::COPY_DST
                | TextureUsage::SAMPLE,
            cube: false,
            reinterpret: false,
        })?;
        let view = b.create_view(
            &texture,
            &TextureViewDesc {
                format,
                dimension: ViewDimension::D2,
                base_mip: 0,
                mip_count: 1,
                base_layer: 0,
                layer_count: 1,
                stencil: false,
            },
        )?;
        self.pool.push(TemporaryTarget {
            key,
            texture,
            view,
            in_use: true,
        });
        Ok(self.pool.len() - 1)
    }

    /// Copies the area `extent` of multisampled `src` into `dst` by a draw: each sample read and
    /// written where the back end reads samples in programs, otherwise the source resolved and drawn
    /// into every sample ([`SlowPath::MultisampleCopyResolved`], a listed loss).
    fn copy_samples(
        &mut self,
        b: &mut B,
        src: &Target<B>,
        dst: &Target<B>,
        extent: Extent,
    ) -> Result<(), GpuError> {
        let kind = match crate::format::FORMAT_FACTS[src.format as usize].kind {
            k @ (FormatKind::Uint | FormatKind::Sint) => k,
            _ => FormatKind::Float,
        };
        let direct = b
            .caps()
            .features
            .contains(Features::MULTISAMPLE_READ | Features::SAMPLE_SHADING);
        let (view, which) = if direct {
            (src.view.clone(), Builtin::SampleCopy(kind, true))
        } else if kind != FormatKind::Float {
            // Integer samples cannot be averaged by a resolve.
            return Err(GpuError::Absent(Features::MULTISAMPLE_READ));
        } else {
            b.record_slow_path(SlowPath::MultisampleCopyResolved);
            let k = self.temporary(b, (src.width, src.height, src.format, 1))?;
            b.resolve(&src.region(), &self.pool[k].region(), src.format);
            (self.pool[k].view.clone(), Builtin::SampleCopy(kind, false))
        };
        let builtins = Builtins::new(b)?;
        let program = Helpers::program(b, which)?;
        let set = b.create_binding_set(&[BindingEntry {
            binding: 32,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(&view),
        }])?;
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(ColorAttachment {
            view: &dst.view,
            depth_slice: dst.depth_slice,
            resolve: None,
            load: LoadOp::Load,
            store: StoreOp::Store,
        });
        let viewport = [Viewport::full(dst.width, dst.height)];
        let area = ScissorRect {
            x: 0,
            y: 0,
            width: extent.width,
            height: extent.height,
        };
        b.begin_pass(&pass);
        b.draw(&DrawCall {
            bindings: [Some(&set), None, None, None],
            scissors: core::slice::from_ref(&area),
            ..DrawCall::builtin(&builtins, &program, &viewport)
        });
        b.end_pass();
        b.destroy(Object::BindingSet(set));
        Ok(())
    }

    fn open_pass(&mut self, b: &mut B) -> Result<(), GpuError> {
        let targets = self
            .targets
            .clone()
            .ok_or(GpuError::Invalid("a draw with no targets set"))?;
        let sizes = targets
            .colors
            .iter()
            .flatten()
            .flat_map(|c| core::iter::once(&c.target).chain(&c.resolve))
            .chain(&targets.depth)
            .map(|t| (t.width, t.height));
        let size = targets
            .depth
            .as_ref()
            .or(targets.colors.iter().flatten().map(|c| &c.target).next())
            .map(|t| (t.width, t.height));
        let mut open = OpenPass {
            colors: [None; MAX_COLOR_TARGETS],
            resolves: [None; MAX_COLOR_TARGETS],
            common: None,
            written: Vec::new(),
            other: Vec::new(),
            query: false,
        };
        if let Some((width, height)) = size {
            let common = sizes.fold((width, height), |a, s| (a.0.min(s.0), a.1.min(s.1)));
            let fitted = targets
                .colors
                .iter()
                .flatten()
                .flat_map(|c| core::iter::once(&c.target).chain(&c.resolve))
                .any(|t| (t.width, t.height) != (width, height));
            if fitted {
                b.record_slow_path(SlowPath::AttachmentsFitted);
                let extent = Extent {
                    width: common.0,
                    height: common.1,
                    depth_or_layers: 1,
                };
                for (i, c) in targets.colors.iter().enumerate() {
                    let Some(c) = c else { continue };
                    let t = &c.target;
                    if (t.width, t.height) != (width, height) {
                        let k = self.temporary(b, (width, height, t.format, t.samples))?;
                        let (load, store) = self.ops.colors[i];
                        if load == LoadOp::Load && t.samples > 1 {
                            let temporary = self.pool[k].target(t.format);
                            self.copy_samples(b, t, &temporary, extent)?;
                        } else if load == LoadOp::Load {
                            b.copy_texture(&t.region(), &self.pool[k].region(), extent);
                        }
                        open.colors[i] = Some((k, store == StoreOp::Store));
                    }
                    if let Some(r) = &c.resolve
                        && (r.width, r.height) != (width, height)
                    {
                        open.resolves[i] = Some(self.temporary(b, (width, height, r.format, 1))?);
                    }
                }
                open.common = Some(ScissorRect {
                    x: 0,
                    y: 0,
                    width: common.0,
                    height: common.1,
                });
            }
        }
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: targets.empty,
            occlusion: self.queries.as_ref().map(|q| &q.set),
        };
        for (i, c) in targets.colors.iter().enumerate() {
            let Some(c) = c else { continue };
            let (load, store) = self.ops.colors[i];
            let (view, depth_slice) = match open.colors[i] {
                Some((k, _)) => (&self.pool[k].view, 0),
                None => (&c.target.view, c.target.depth_slice),
            };
            pass.colors[i] = Some(ColorAttachment {
                view,
                depth_slice,
                resolve: match (open.resolves[i], &c.resolve) {
                    (Some(k), _) => Some(&self.pool[k].view),
                    (None, r) => r.as_ref().map(|r| &r.view),
                },
                load,
                // A replaced attachment's temporary is copied back after the pass.
                store: if open.colors[i].is_some() {
                    StoreOp::Store
                } else {
                    store
                },
            });
        }
        if let Some(d) = &targets.depth {
            let (has_depth, has_stencil) = d.format.aspects();
            pass.depth = Some(DepthAttachment {
                view: &d.view,
                depth: has_depth.then_some(self.ops.depth),
                stencil: has_stencil.then_some(self.ops.stencil),
            });
        }
        b.begin_pass(&pass);
        self.open = Some(open);
        if self.queries.as_ref().is_some_and(|q| q.running.is_some()) {
            self.begin_segment(b)?;
        }
        Ok(())
    }
}

impl<B: Gpu> TemporaryTarget<B> {
    /// The temporary as an attachment seen through `format`, its own.
    fn target(&self, format: Format) -> Target<B> {
        Target {
            view: self.view.clone(),
            texture: self.texture.clone(),
            format,
            mip: 0,
            layer: 0,
            depth_slice: 0,
            width: self.key.0,
            height: self.key.1,
            samples: self.key.3,
        }
    }

    fn region(&self) -> TextureRegion<'_, B> {
        TextureRegion {
            texture: &self.texture,
            mip: 0,
            layer: 0,
            origin: Origin { x: 0, y: 0, z: 0 },
            stencil: false,
        }
    }
}
