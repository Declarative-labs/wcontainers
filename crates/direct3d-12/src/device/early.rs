//! Early depth and stencil tests a pixel shader forces (`[earlydepthstencil]`) run in the program
//! (`EarlyTestsInShader`, plan 4.12): the back ends test after the shader, so its storage writes
//! would happen for fragments the tests then fail. The draw runs in a pass whose depth-stencil
//! attachment is read only, the program reads depth and stencil from that view
//! (`EARLY_DEPTH_BINDING`, `EARLY_STENCIL_BINDING`) and tests them as `HelperValue::EarlyTests`
//! says before anything else, discarding the fragments that fail. A draw that also writes depth
//! or stencil draws each primitive alone, tested so and then written (`EarlyTestsPerPrimitive`).

use super::*;
use crate::list::{DSV_READ_ONLY_DEPTH, DSV_READ_ONLY_STENCIL};
use wcontainers_gpu::{
    BindingEntry, BindingResource, DepthStencilDesc, EARLY_DEPTH_BINDING, EARLY_STENCIL_BINDING,
    FrontFace, StencilOp,
};
use wcontainers_shaders::stage::TRIANGLE_VERTICES;

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
pub(super) type EarlyViews<B> = (<B as Gpu>::TextureView, <B as Gpu>::TextureView);

/// How a draw runs its pixel shader's forced early tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Early {
    /// As the back end tests, after the shader: the shader forces none that matter, or no
    /// depth-stencil view is bound.
    Off,
    /// In the program, in a pass reading the depth-stencil view only.
    InProgram,
    /// The draw writes depth or stencil, which earlier primitives' writes change for later
    /// ones: each primitive drawn twice (`EarlyTestsPerPrimitive`, `Replay::draw_per_primitive`).
    PerPrimitive,
}

/// Which of its two draws a primitive drawn alone for `Early::PerPrimitive` is in, and whether
/// it is an odd triangle of a strip, whose winding drawn alone is the other way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EarlyRun {
    /// The program tests early against the read-only attachment and makes the colour and
    /// storage writes; the pipeline's depth and stencil writes are off.
    Tested { odd: bool },
    /// The vertex program alone, with the pipeline's depth-stencil block, in a pass with no
    /// colour targets: the depth and stencil writes.
    Written { odd: bool },
}

impl EarlyRun {
    pub(super) fn odd(self) -> bool {
        match self {
            EarlyRun::Tested { odd } | EarlyRun::Written { odd } => odd,
        }
    }
}

/// What a pipeline whose forced early tests matter while it writes depth or stencil draws each
/// primitive with (`EarlyRun`).
pub(super) struct PerPrimitive<B: Gpu> {
    /// The pipeline's depth-stencil block with its writes off.
    pub tested: B::DepthStencil,
    /// The raster block with the other front face.
    pub flipped: B::Raster,
    /// The pipeline's vertex program with no fragment stage.
    pub written: B::Program,
}

impl<B: Gpu> Device<B> {
    /// The blocks and program of `PerPrimitive` for a pipeline of these blocks' descriptions and
    /// vertex program `vertex`.
    pub(super) fn per_primitive(
        &mut self,
        depth_stencil: &DepthStencilDesc,
        raster: &wcontainers_gpu::RasterDesc,
        vertex: &ShaderModule,
    ) -> Result<PerPrimitive<B>, Error> {
        let tested = self.backend.create_depth_stencil(&DepthStencilDesc {
            depth_write: false,
            stencil_write_mask: [0; 2],
            ..*depth_stencil
        })?;
        let flipped = self.backend.create_raster(&wcontainers_gpu::RasterDesc {
            front: match raster.front {
                FrontFace::CounterClockwise => FrontFace::Clockwise,
                FrontFace::Clockwise => FrontFace::CounterClockwise,
            },
            ..*raster
        })?;
        let shader = self.backend.create_shader(vertex)?;
        let written = self.backend.create_program(&Program::Render {
            vertex: &shader,
            fragment: None,
        });
        self.backend.destroy(Object::Shader(shader));
        Ok(PerPrimitive {
            tested,
            flipped,
            written: written?,
        })
    }
}

impl<B: Gpu> Replay<B> {
    /// How a draw with pipeline `pid` runs its pixel shader's forced early tests: in the program
    /// where the shader forces them and changes what it does through them and a depth-stencil
    /// view is bound, each primitive alone where the draw also writes depth or stencil.
    pub(super) fn early_tests(&self, d: &Device<B>, pid: PipelineId) -> Result<Early, Error> {
        let r = d.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .ok_or(Error::Invalid("drawing with a compute pipeline state"))?;
        let (Some(ds), Some(Descriptor::DepthStencil { view, read_only })) =
            (&r.early_tests, &self.depth)
        else {
            return Ok(Early::Off);
        };
        if r.geometry.is_some() {
            return Err(Error::NotImplemented(
                "early depth-stencil tests in the program behind a geometry or tessellation stage",
            ));
        }
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
        Ok(match writes_depth || writes_stencil {
            true => Early::PerPrimitive,
            false => Early::InProgram,
        })
    }

    /// A draw of `Early::PerPrimitive`: each primitive drawn alone twice (`EarlyRun`), so the
    /// depth and stencil writes of earlier primitives are in the buffer later ones are tested
    /// against, as Direct3D's early tests see them.
    pub(super) fn draw_per_primitive(
        &mut self,
        d: &mut Device<B>,
        pid: PipelineId,
        range: DrawRange<'_, B>,
        record: Option<&(B::Buffer, u64)>,
    ) -> Result<(), Error> {
        let r = d.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .ok_or(Error::Invalid("drawing with a compute pipeline state"))?;
        if crate::state::staged_only(self.api_topology) {
            return Err(Error::NotImplemented(
                "early tests in the program per primitive of an adjacency draw",
            ));
        }
        if r.per_primitive.is_none() {
            return Err(Error::NotImplemented(
                "early depth-stencil tests per primitive of a pixel shader reading its primitive \
                 index",
            ));
        }
        // Each primitive is a pass of its own, which a query would have to span.
        if record.is_some()
            || self.query.is_some()
            || self.counted.is_some()
            || self.statistics.is_some()
        {
            return Err(Error::NotImplemented(
                "early depth-stencil tests per primitive of a predicated draw or while a query runs",
            ));
        }
        let (count, instances) = match range {
            DrawRange::Vertices {
                count, instances, ..
            }
            | DrawRange::Indexed {
                count, instances, ..
            } => (count, instances),
            _ => {
                return Err(Error::NotImplemented(
                    "early depth-stencil tests per primitive of an indirect draw",
                ));
            }
        };
        let indexed = matches!(range, DrawRange::Indexed { .. });
        // Vertices a primitive takes, and how far the next one starts.
        let (per, step) = match (self.topology, indexed) {
            (Topology::PointList, _) => (1, 1),
            (Topology::LineList, _) => (2, 2),
            (Topology::TriangleList, _) => (TRIANGLE_VERTICES, TRIANGLE_VERTICES),
            // An indexed strip's restart index cuts it where a primitive alone would not.
            (Topology::LineStrip, false) => (2, 1),
            (Topology::TriangleStrip, false) => (TRIANGLE_VERTICES, 1),
            _ => {
                return Err(Error::NotImplemented(
                    "early depth-stencil tests per primitive of an indexed strip",
                ));
            }
        };
        if instances != 1 {
            return Err(Error::NotImplemented(
                "early depth-stencil tests per primitive of an instanced draw",
            ));
        }
        d.backend.record_slow_path(SlowPath::EarlyTestsPerPrimitive);
        let primitives = match count.checked_sub(per) {
            Some(rest) => rest / step + 1,
            None => 0,
        };
        let mut result = Ok(());
        'primitives: for p in 0..primitives {
            let at = p * step;
            let one = match range {
                DrawRange::Vertices {
                    first,
                    first_instance,
                    ..
                } => DrawRange::Vertices {
                    first: first + at,
                    count: per,
                    first_instance,
                    instances,
                },
                DrawRange::Indexed {
                    first,
                    base_vertex,
                    first_instance,
                    ..
                } => DrawRange::Indexed {
                    first: first + at,
                    count: per,
                    base_vertex,
                    first_instance,
                    instances,
                },
                _ => unreachable!("refused above"),
            };
            let odd = self.topology == Topology::TriangleStrip && p % 2 == 1;
            for run in [EarlyRun::Tested { odd }, EarlyRun::Written { odd }] {
                self.end_pass(d)?;
                self.early_run = Some(run);
                result = self.draw(d, same_range(&one), None);
                self.early_run = None;
                if result.is_err() {
                    break 'primitives;
                }
            }
        }
        self.end_pass(d)?;
        result
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

impl<B: Gpu> Device<B> {
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
pub(super) fn entries<B: Gpu>(views: &EarlyViews<B>) -> [BindingEntry<'_, B>; 2] {
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
