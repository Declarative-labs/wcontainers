//! Early depth and stencil tests a pixel shader forces (`[earlydepthstencil]`) run in the program
//! (`EarlyTestsInShader`, plan 4.12): the back ends test after the shader, so its storage writes
//! would happen for fragments the tests then fail. The draw runs in a pass whose depth-stencil
//! attachment is read only, the program reads depth and stencil from that view
//! (`EARLY_DEPTH_BINDING`, `EARLY_STENCIL_BINDING`) and tests them as `HelperValue::EarlyTests`
//! says before anything else, discarding the fragments that fail.

use super::*;
use crate::list::{DSV_READ_ONLY_DEPTH, DSV_READ_ONLY_STENCIL};
use wcontainers_gpu::{
    BindingEntry, BindingResource, EARLY_DEPTH_BINDING, EARLY_STENCIL_BINDING, StencilOp,
};

/// The texture bound as the stencil a program tests early where the view's format has none, of
/// the depth view's sample count `samples`; the program then never reads it (its stencil test is
/// off).
pub(super) fn stencil_stand_in(samples: u32) -> null::Shape {
    null::Shape {
        dimension: ViewDimension::D2,
        format: Format::Rgba8Uint,
        samples,
        storage: false,
    }
}

/// The views a draw testing early in the program binds: the depth and the stencil aspect of the
/// bound depth-stencil view.
pub(super) type EarlyViews<B> = (<B as Backend>::TextureView, <B as Backend>::TextureView);

impl<B: Backend> Replay<B> {
    /// Whether a draw with pipeline `pid` runs its pixel shader's forced early tests in the
    /// program: the shader forces them and changes what it does through them, and a
    /// depth-stencil view is bound. Such a draw writing depth or stencil needs each primitive
    /// drawn again without the shader (`EarlyTestsPerPrimitive`), which is refused by name.
    pub(super) fn early_tests(&self, d: &Device<B>, pid: PipelineId) -> Result<bool, Error> {
        let r = d.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .ok_or(Error::Invalid("drawing with a compute pipeline state"))?;
        let (Some(ds), Some(Descriptor::DepthStencil { view, read_only })) =
            (&r.early_tests, &self.depth)
        else {
            return Ok(false);
        };
        let (_, has_stencil) = aspects(dxgi(view.format)?);
        let writes_depth =
            ds.depth_enable && ds.depth_write_all && read_only & DSV_READ_ONLY_DEPTH == 0;
        let mut writes_stencil = has_stencil
            && ds.stencil_enable
            && ds.stencil_write_mask != 0
            && read_only & DSV_READ_ONLY_STENCIL == 0;
        if writes_stencil {
            let mut keeps = true;
            for op in [ds.front, ds.back]
                .iter()
                .flat_map(|f| [f.fail, f.depth_fail, f.pass])
            {
                keeps &= crate::state::stencil_op(op)? == StencilOp::Keep;
            }
            writes_stencil = !keeps;
        }
        if writes_depth || writes_stencil {
            return Err(Error::NotImplemented(
                "early depth-stencil tests in the program of a draw writing depth or stencil \
                 (EarlyTestsPerPrimitive)",
            ));
        }
        if r.geometry.is_some() {
            return Err(Error::NotImplemented(
                "early depth-stencil tests in the program behind a geometry or tessellation stage",
            ));
        }
        Ok(true)
    }

    /// The words of `HelperValue::EarlyTests` for a draw with pipeline `pid`: the depth compare,
    /// the front and back stencil compares (`CompareFn` codes, zero for a test that is off or an
    /// aspect the view lacks), the stencil reference and read mask.
    pub(super) fn early_words(&self, d: &Device<B>, pid: PipelineId) -> Result<pull::Words, Error> {
        let ds = d.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .and_then(|r| r.early_tests)
            .ok_or(Error::Invalid("early tests of a pipeline without them"))?;
        let Some(Descriptor::DepthStencil { view, .. }) = &self.depth else {
            return Err(Error::Invalid(
                "early tests with no depth-stencil view bound",
            ));
        };
        let (has_depth, has_stencil) = aspects(dxgi(view.format)?);
        let code = |on: bool, func: u32| -> Result<u32, Error> {
            Ok(if on {
                crate::state::compare(func)? as u32
            } else {
                0
            })
        };
        let stencil = has_stencil && ds.stencil_enable;
        Ok(pull::words(&[
            code(has_depth && ds.depth_enable, ds.depth_func)?,
            code(stencil, ds.front.func)?,
            code(stencil, ds.back.func)?,
            u32::from(self.stencil_reference),
            u32::from(ds.stencil_read_mask),
        ]))
    }
}

impl<B: Backend> Device<B> {
    /// The depth and stencil aspects of depth-stencil view `v`, as a program testing early reads
    /// them; for a format without stencil, the stand-in.
    pub(super) fn early_views(&mut self, v: &TargetView) -> Result<EarlyViews<B>, Error> {
        let format = dxgi(v.format)?;
        let (usage, samples) = self
            .texture(v.resource)
            .map(|(_, desc)| (desc.usage, desc.samples))?;
        if !usage.contains(TextureUsage::SAMPLE) {
            return Err(Error::NotImplemented(
                "early depth-stencil tests in the program on a depth buffer denying shader \
                 resources",
            ));
        }
        let depth = self.view(v)?;
        let stencil = match aspects(format) {
            (_, true) => self.texture_view(
                v.resource,
                TextureViewDesc {
                    format,
                    dimension: ViewDimension::D2,
                    base_mip: v.mip,
                    mip_count: 1,
                    base_layer: v.first_layer,
                    layer_count: 1,
                    stencil: true,
                },
            )?,
            (_, false) => self.null_view(stencil_stand_in(samples), false)?,
        };
        Ok((depth, stencil))
    }
}

/// The helper set's entries binding `views` for a program testing early.
pub(super) fn entries<B: Backend>(views: &EarlyViews<B>) -> [BindingEntry<'_, B>; 2] {
    [
        (EARLY_DEPTH_BINDING, &views.0),
        (EARLY_STENCIL_BINDING, &views.1),
    ]
    .map(|(binding, view)| BindingEntry {
        binding,
        stages: StageSet::FRAGMENT,
        resource: BindingResource::Texture(view),
    })
}
