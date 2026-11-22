//! Draws whose targets blend differently where the back end has one blend for all targets
//! (`TargetsSplit`, plan 4.12): the draw runs once per group of bound targets that agree, each run
//! drawing only its group's targets, every run after the first starting from a saved copy of depth
//! and stencil so that each sees the tests the first saw.

use super::*;
use crate::desc::COLOR_WRITE_ENABLE_ALL;

/// What a later run of a split draw would do again.
const SPLIT_WHILE_QUERIED: &str = "a draw run once per group of targets while queries run";
const SPLIT_WITH_STORAGE: &str = "a draw run once per group of targets with storage writes";
const SPLIT_WITH_STREAM_OUTPUT: &str = "a draw run once per group of targets with stream output";

/// A group of targets (a mask of target slots) and the blend state its run draws with.
type Group<B> = (u8, Rc<Blend<B>>);

impl<B: Backend> Device<B> {
    /// The groups of the bound blend state with a bound target that writes something (at least
    /// one, where every group writes nothing), and the bound targets; `None` where the state has
    /// no groups.
    fn split_groups(&self) -> Option<(u8, Vec<Group<B>>)> {
        let blend = self.ctx.blend.as_ref()?;
        if blend.groups.is_empty() {
            return None;
        }
        let bound = self
            .ctx
            .targets
            .iter()
            .enumerate()
            .filter(|(_, t)| t.is_some())
            .fold(0u8, |m, (i, _)| m | 1 << i);
        let mut groups: Vec<_> = blend
            .groups
            .iter()
            .filter(|(members, _)| members & bound != 0)
            .cloned()
            .collect();
        // A group writing no channel draws nothing; one run still draws depth and stencil.
        let writing = groups
            .iter()
            .filter(|(_, g)| g.desc.targets[0].write_mask & COLOR_WRITE_ENABLE_ALL != 0)
            .count();
        if writing > 0 {
            groups.retain(|(_, g)| g.desc.targets[0].write_mask & COLOR_WRITE_ENABLE_ALL != 0);
        } else {
            groups.truncate(1);
        }
        Some((bound, groups))
    }

    /// Draws as [`Device::draw_once`] does, once per group of targets where the bound blend state
    /// needs it.
    pub(super) fn try_draw(
        &mut self,
        range: Range,
        first_instance: u32,
        instances: u32,
        args: Option<&Rc<Buffer<B>>>,
    ) -> Result<()> {
        let groups = match self.split_groups() {
            None => return self.draw_once(range, first_instance, instances, args),
            // No target bound: the state's own block (the first group's) draws the rest.
            Some((_, groups)) if groups.is_empty() => {
                return self.draw_once(range, first_instance, instances, args);
            }
            // Every bound target in one group: one run with its block, on the pass as it is.
            Some((bound, groups)) if groups.len() == 1 && bound & !groups[0].0 == 0 => {
                let blend = self.ctx.blend.replace(groups[0].1.clone());
                let r = self.draw_once(range, first_instance, instances, args);
                self.ctx.blend = blend;
                return r;
            }
            Some((_, groups)) => groups,
        };
        if self.removed.is_some() || self.predicated_out() {
            return Ok(());
        }
        // What a later run would do again, and must not.
        if self.queries.running_any() {
            return not_implemented(SPLIT_WHILE_QUERIED);
        }
        if self.ctx.uavs[PIXEL].iter().any(Option::is_some) {
            return not_implemented(SPLIT_WITH_STORAGE);
        }
        if self.ctx.so_targets.iter().any(Option::is_some) {
            return not_implemented(SPLIT_WITH_STREAM_OUTPUT);
        }
        self.backend.slow_paths().hit(SlowPath::TargetsSplit);
        let targets = self.ctx.targets.clone();
        let blend = self.ctx.blend.clone();
        let saved = self.save_depth_stencil()?;
        let mut r = Ok(());
        for (run, (members, group)) in groups.iter().enumerate() {
            self.end_pass();
            if run > 0
                && let Some(saved) = &saved
            {
                self.copy_depth_stencil(saved, false);
            }
            for (i, t) in self.ctx.targets.iter_mut().enumerate() {
                *t = targets[i].clone().filter(|_| members & 1 << i != 0);
            }
            self.ctx.blend = Some(group.clone());
            r = self.draw_once(range, first_instance, instances, args);
            if r.is_err() {
                break;
            }
        }
        self.end_pass();
        self.ctx.targets = targets;
        self.ctx.blend = blend;
        r
    }

    /// Copies the bound depth-stencil view's subresource aside where the draw may write it.
    fn save_depth_stencil(&mut self) -> Result<Option<Rc<View<B>>>> {
        let Some(view) = self.ctx.depth.clone() else {
            return Ok(None);
        };
        let d = self.drawn_depth_stencil().desc;
        let writes =
            (d.depth_enable && d.depth_write) || (d.stencil_enable && d.stencil_write_mask != 0);
        if !writes {
            return Ok(None);
        }
        let e = view.texture.mip_extent(view.desc.first_mip);
        let desc = wcontainers_gpu::TextureDesc {
            width: e.width,
            height: e.height,
            depth_or_layers: 1,
            mip_levels: 1,
            usage: TextureUsage::RENDER | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
            cube: false,
            ..view.texture.gpu_desc
        };
        if self.split_depth.as_ref().is_none_or(|(d, _)| *d != desc) {
            let made = self.backend.create_texture(&desc);
            let made = self.gpu(made)?;
            let id = self.id();
            self.split_depth = Some((desc, Kept::new(made, id, Object::Texture, &self.graveyard)));
        }
        // Copies run outside a pass, and a pending clear of the view first, so the copy holds it.
        if self.ctx.clears.iter().any(|c| Rc::ptr_eq(&c.view, &view)) {
            self.settle();
        } else {
            self.end_pass();
        }
        self.copy_depth_stencil(&view, true);
        Ok(Some(view))
    }

    /// Copies `view`'s subresource into the saved texture (`out`) or back.
    fn copy_depth_stencil(&mut self, view: &View<B>, out: bool) {
        let Some((desc, saved)) = &self.split_depth else {
            return;
        };
        let bound = TextureRegion {
            texture: &view.texture.gpu.handle,
            mip: view.desc.first_mip,
            layer: view.desc.first_layer,
            origin: Origin::default(),
            stencil: false,
        };
        let aside = TextureRegion {
            texture: &saved.handle,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        };
        let extent = Extent {
            width: desc.width,
            height: desc.height,
            depth_or_layers: 1,
        };
        let (src, dst) = if out {
            (&bound, &aside)
        } else {
            (&aside, &bound)
        };
        self.backend.copy_texture(src, dst, extent);
    }
}
