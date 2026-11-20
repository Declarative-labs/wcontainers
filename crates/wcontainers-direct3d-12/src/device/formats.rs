//! A stage's programs for what is bound (compute, pixel, and vertex drawn directly): typed views
//! whose format only the bound view names, and sampling the back end's sampler lacks.
//!
//! WebGPU's storage textures name their format in the program, and typed buffers read through
//! storage buffers decode theirs there (`TexelBufferAsStorage`). A shader declaring such views is
//! converted at pipeline creation with each one's format taken as the 32-bit single-channel
//! format of its declared kind (the only formats Direct3D guarantees typed loads of)
//! (`VariantKey::storage_formats`, `VariantKey::texel_formats`). Samplers with a level-of-detail
//! bias (`LodBiasInShader`), or with border or mirror-once addressing the back end lacks
//! (`BorderInShader`), are applied by the program (`VariantKey::lod_bias`,
//! `VariantKey::border_in_shader`), as is filtering of textures bound in formats the back end
//! cannot filter where a filtering sampler samples them (`FilterInShader`,
//! `VariantKey::filter_in_shader`). For other bound formats or such samplers the program is
//! converted again, at the first draw or dispatch binding them, and kept per pipeline.

use super::*;
use crate::dxbc::{Kind, TypedView};
use pull::Helpers;
use std::ops::Range;
use wcontainers_gpu::{Filter, SlotKind, ViewComponent};

/// A typed view's slot: a storage slot (else a texture slot) and its place in its kind's range
/// (`ReflectedBinding::slot`).
pub(super) type Slot = (bool, u32);

/// What a stage's program is made for: the typed views' formats by slot, the texture slots'
/// component mappings (`swizzled`) and multisampled arrays' layer counts (`layers`), the sampler
/// slots whose bias (`lod_bias`) and addressing (`border_in_shader`) it applies, the
/// storage slots it reads through buffers (`texels_through_buffer`), and the texture
/// slots under a minimum level-of-detail clamp (`min_lod_clamped`) and filtered in the
/// program (`filter_in_shader`), and whether a pixel stage runs its early tests itself
/// (`VariantFlags::EARLY_TESTS`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Bound {
    formats: Vec<(Slot, Format)>,
    swizzled: Vec<(u32, [ViewComponent; 4])>,
    layers: Vec<(u32, u32)>,
    lod_bias: u16,
    border: u16,
    through: u64,
    clamped: u128,
    filtered: u128,
    /// Storage views at offsets the back end cannot bind (`VariantFlags::STORAGE_OFFSET`).
    offsets: bool,
    early: bool,
}

/// A program made for what is bound, with its helper layout; `None` where the conversion
/// failed.
type Variant<B> = (Bound, Option<(<B as Backend>::Program, Helpers)>);

/// A pipeline's stage whose program depends on what is bound.
pub(super) struct Formats<B: Backend> {
    /// The API stage (`D3D12_SHADER_VISIBILITY`, 0 for compute) and its bytecode.
    visibility: u32,
    code: Vec<u8>,
    /// The key it was converted with, the formats aside.
    key: VariantKey,
    /// The formats the pipeline's own program was made with, by slot.
    made: Vec<(Slot, Format)>,
    /// The texture and sampler slots the program samples together.
    pairs: Vec<(u32, u32)>,
    /// Programs made for other bindings (pixel and compute stages; a vertex stage's are its
    /// pipeline's `Combination`s).
    variants: Vec<Variant<B>>,
}

/// The typed views a DXBC or DXIL shader declares, one per element of an array, whose unbounded
/// arrays `ranges` sizes; none where its declarations cannot be read.
fn typed_views(code: &[u8], ranges: &[(SlotKind, u32, Range<u32>)]) -> Vec<TypedView> {
    crate::dxbc::typed_views(code, ranges)
        .or_else(|| crate::dxil::typed_views(code))
        .unwrap_or_default()
}

/// The key fields a shader's typed views are first converted with: their formats by slot,
/// `route` where it declares typed buffers, and its multisampled arrays' layer counts. Default
/// for shaders declaring none.
pub(super) fn typed_key(
    code: &[u8],
    route: TexelBufferRoute,
    root: &RootSignatureDesc,
    visibility: u32,
) -> VariantKey {
    let mut key = VariantKey::default();
    // Arrays declared without a length take the rest of the root signature's range.
    if crate::dxbc::declares_unbounded_array(code) {
        key.register_ranges = root.register_ranges(visibility);
    }
    for v in typed_views(code, &key.register_ranges) {
        let format = match v.kind {
            Kind::Float => Format::R32Float,
            Kind::Uint => Format::R32Uint,
            Kind::Sint => Format::R32Sint,
        };
        match v.storage {
            true => key.storage_formats.push((v.slot, format)),
            false => key.texel_formats.push((v.slot, format)),
        }
        if v.buffer {
            key.texel_buffers = route;
        }
    }
    key.storage_formats.sort_by_key(|f| f.0);
    key.texel_formats.sort_by_key(|f| f.0);
    // Converted reading one layer of each; the bound views' layer counts come at the draw.
    let arrays = crate::dxbc::multisampled_arrays(code, &key.register_ranges)
        .or_else(|| crate::dxil::multisampled_arrays(code));
    key.texture_layers = arrays
        .unwrap_or_default()
        .into_iter()
        .map(|s| (s, 1))
        .collect();
    key.texture_layers.sort_unstable();
    key
}

/// The slots of a shader's typed unordered access views of textures, one bit each, its unbounded
/// arrays sized by `ranges`.
pub(super) fn typed_texture_slots(code: &[u8], ranges: &[(SlotKind, u32, Range<u32>)]) -> u64 {
    typed_views(code, ranges)
        .iter()
        .filter(|v| v.storage && !v.buffer && v.slot < 64)
        .fold(0, |m, v| m | (1 << v.slot))
}

/// The formats of `key` by slot, sorted.
fn slots(key: &VariantKey) -> Vec<(Slot, Format)> {
    let storage = key.storage_formats.iter().map(|&(r, f)| ((true, r), f));
    let texel = key.texel_formats.iter().map(|&(r, f)| ((false, r), f));
    let mut all: Vec<_> = storage.chain(texel).collect();
    all.sort_by_key(|s| s.0);
    all
}

impl<B: Backend> Formats<B> {
    /// The record of a stage converted with `key`, whose formats it made the program with.
    pub(super) fn of(
        visibility: u32,
        code: &[u8],
        key: &VariantKey,
        pairs: &[(u32, u32)],
    ) -> Option<Box<Self>> {
        let made = slots(key);
        Some(Box::new(Formats {
            visibility,
            code: code.to_vec(),
            key: VariantKey {
                storage_formats: Vec::new(),
                texel_formats: Vec::new(),
                ..key.clone()
            },
            made,
            pairs: pairs.to_vec(),
            variants: Vec::new(),
        }))
    }

    pub(super) fn destroy(self, backend: &mut B) {
        for (_, p) in self.variants {
            if let Some((p, _)) = p {
                backend.destroy(Object::Program(p));
            }
        }
    }
}

/// The vertex program a draw uses before what is bound: the pipeline's own, the one reading the
/// bases from indirect arguments, one fetching inputs itself (`PulledKey`), or the stage helper's
/// (behind a geometry or domain stage, never converted for what is bound).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum VertexKind {
    Own,
    Arguments,
    Pulled(PulledKey),
    Staged,
}

/// A program made for what is bound: the program, its pixel (or compute) stage's helper layout,
/// and where its vertex stage was converted for what is bound too, that stage's helper layout and
/// pulled streams (binding, input location).
pub(super) type BoundProgram<B> = (
    <B as Backend>::Program,
    Helpers,
    Option<(Helpers, Vec<(u32, u32)>)>,
);

/// What a program converted for both stages is made for: the vertex program it starts from and
/// what is bound to each stage where that differs from the stage's own.
pub(super) type Combination = (VertexKind, Option<Bound>, Option<Bound>);

/// `base` with the formats and bound fields of `bound`, or the formats `made` where none.
fn keyed(base: &VariantKey, made: &[(Slot, Format)], bound: Option<&Bound>) -> VariantKey {
    let mut key = base.clone();
    if let Some(b) = bound {
        key.lod_bias = b.lod_bias;
        key.border_in_shader = b.border;
        key.texels_through_buffer = b.through;
        key.min_lod_clamped = b.clamped;
        key.filter_in_shader = b.filtered;
        key.flags.set(VariantFlags::STORAGE_OFFSET, b.offsets);
        key.flags.set(VariantFlags::EARLY_TESTS, b.early);
        key.swizzled = b.swizzled.clone();
        key.texture_layers = b.layers.clone();
    }
    for &((storage, slot), format) in bound.map_or(made, |b| &b.formats) {
        match storage {
            true => key.storage_formats.push((slot, format)),
            false => key.texel_formats.push((slot, format)),
        }
    }
    key
}

impl<B: Backend> Device<B> {
    /// The program to draw or dispatch with for the typed views and samplers the last
    /// `binding_sets` bound: `None` for the program `vertex` names, else the one converted for
    /// them. Views bound with no format (null views) keep the program's. A vertex stage drawn
    /// directly is converted for what is bound to it as the pixel stage is; then both stages are
    /// made together, from the vertex program `vertex` names.
    pub(super) fn bound_program(
        &mut self,
        pid: PipelineId,
        vertex: VertexKind,
    ) -> Result<Option<BoundProgram<B>>, Error> {
        let i = pid.0 as usize;
        let pixel = match self.pipelines[i].formats.take() {
            Some(f) => {
                let bound = self.stage_bound(&f);
                self.pipelines[i].formats = Some(f);
                bound?
            }
            None => None,
        };
        let own = self.pipelines[i]
            .render
            .as_mut()
            .and_then(|r| r.vertex_formats.take());
        let drawn = match own {
            Some(f) => {
                let bound = self.stage_bound(&f);
                self.render_mut(pid).vertex_formats = Some(f);
                bound?
            }
            None => None,
        };
        if drawn.is_none() && (pixel.is_none() || vertex == VertexKind::Own) {
            return match pixel {
                Some(bound) => self.pixel_program(pid, bound).map(Some),
                None => Ok(None),
            };
        }
        if vertex == VertexKind::Staged {
            return Err(Error::NotImplemented(
                "a pixel program converted for the bound views and samplers behind a geometry or \
                 domain shader",
            ));
        }
        let combination = (vertex, drawn, pixel);
        let refused = Error::NotImplemented("a program for the bound views and samplers");
        if let Some((_, p)) = self
            .render_mut(pid)
            .bound
            .iter()
            .find(|b| b.0 == combination)
        {
            return p.clone().map(Some).ok_or(refused);
        }
        // A program made at the draw, as a state combination first met there.
        self.backend.slow_paths().hit(SlowPath::PipelineMiss);
        let made = self.combined_program(pid, &combination);
        self.render_mut(pid)
            .bound
            .push((combination, made.as_ref().ok().cloned()));
        made.map(Some)
    }

    /// The pipeline's pixel or compute program converted for `bound`, with the pipeline's own
    /// vertex program.
    fn pixel_program(&mut self, pid: PipelineId, bound: Bound) -> Result<BoundProgram<B>, Error> {
        let f = self.pipelines[pid.0 as usize]
            .formats
            .as_ref()
            .expect("formats");
        let refused = Error::NotImplemented("a program for the bound views and samplers");
        if let Some((_, p)) = f.variants.iter().find(|v| v.0 == bound) {
            return p.clone().map(|(p, h)| (p, h, None)).ok_or(refused);
        }
        let (visibility, code) = (f.visibility, f.code.clone());
        let key = keyed(&f.key, &f.made, Some(&bound));
        // A program made at the draw, as a state combination first met there.
        self.backend.slow_paths().hit(SlowPath::PipelineMiss);
        let made = self.formats_program(pid, visibility, &code, key);
        let f = self.pipelines[pid.0 as usize]
            .formats
            .as_mut()
            .expect("formats");
        f.variants.push((bound, made.as_ref().ok().cloned()));
        made.map(|(p, h)| (p, h, None))
    }

    /// A program whose vertex stage is converted from the vertex program `combination` names
    /// for what is bound to it, and whose pixel stage is the pipeline's own or converted for
    /// what is bound to it.
    fn combined_program(
        &mut self,
        pid: PipelineId,
        combination: &Combination,
    ) -> Result<BoundProgram<B>, Error> {
        let (kind, drawn, pixel) = combination;
        let p = &self.pipelines[pid.0 as usize];
        let r = p.render.as_ref().expect("a graphics pipeline");
        let v = r
            .vertex_formats
            .as_ref()
            .ok_or(Error::Invalid("a vertex program for the bound views"))?;
        let mut vertex_key = keyed(&v.key, &v.made, drawn.as_ref());
        match kind {
            VertexKind::Own => {}
            VertexKind::Arguments => vertex_key.draw_base = DrawBaseSource::Arguments,
            VertexKind::Pulled((inputs, arguments)) => {
                vertex_key.pulled = inputs.clone();
                if *arguments {
                    vertex_key.draw_base = DrawBaseSource::Arguments;
                }
            }
            VertexKind::Staged => unreachable!("refused before"),
        }
        let vertex_code = v.code.clone();
        let fragment = match (&p.formats, pixel) {
            (Some(f), Some(b)) => Some((f.code.clone(), keyed(&f.key, &f.made, Some(b)))),
            _ => None,
        };
        let own_fragment = r.fragment.clone();
        let own_helpers = p.helpers.clone();
        let vs = self.convert(&vertex_code, ShaderStage::Vertex, vertex_key)?;
        let (fragment, helpers) = match fragment {
            Some((code, key)) => {
                let ps = self.convert(&code, ShaderStage::Fragment, key)?;
                (Some(ps.modules[0].clone()), Helpers::of(&ps))
            }
            None => (own_fragment, own_helpers),
        };
        let made = self.program_of(&vs, fragment.as_ref())?;
        Ok((made.program, helpers, Some((made.helpers, made.streams))))
    }

    /// What the views and samplers the last `binding_sets` bound ask of stage `f`'s program,
    /// where that differs from what it was made with; the helper values of the samplers the
    /// program then reads are added to the bound ones.
    fn stage_bound(&mut self, f: &Formats<B>) -> Result<Option<Bound>, Error> {
        let (mut lod_bias, mut border) = (0u16, 0u16);
        for &(visibility, sampler, bias, address) in &self.bound_sampling {
            if visibility == f.visibility {
                lod_bias |= u16::from(bias) << sampler;
                border |= u16::from(address) << sampler;
            }
        }
        let mut formats = f.made.clone();
        for &(visibility, slot, format) in &self.bound_formats {
            if visibility != f.visibility {
                continue;
            }
            match formats.iter_mut().find(|b| b.0 == slot) {
                Some(b) => b.1 = format,
                // A texture view of another component kind than the program declares.
                None if !slot.0 => formats.push((slot, format)),
                None => {}
            }
        }
        formats.sort_by_key(|b| b.0);
        let through = self
            .bound_through
            .iter()
            .filter(|t| t.0 == f.visibility)
            .fold(0u64, |m, t| m | (1 << t.1));
        let mut swizzled: Vec<(u32, [ViewComponent; 4])> = self
            .bound_swizzles
            .iter()
            .filter(|s| s.0 == f.visibility)
            .map(|s| (s.1, s.2))
            .collect();
        swizzled.sort_by_key(|s| s.0);
        let mut layers = f.key.texture_layers.clone();
        for (visibility, slot, _, rest) in &self.bound_layers {
            if *visibility == f.visibility
                && let Some(l) = layers.iter_mut().find(|l| l.0 == *slot)
            {
                l.1 = rest.len() as u32 + 1;
            }
        }
        let clamped = self
            .bound_clamps
            .iter()
            .filter(|c| c.0 == f.visibility)
            .fold(0u128, |m, c| m | (1 << c.1));
        // Textures the back end cannot filter, sampled through a filtering sampler, with the
        // helper values of the samplers the program then reads.
        let mut filtered = 0u128;
        let mut states = Vec::new();
        for &(texture, sampler) in &f.pairs {
            if texture >= 128 || !self.bound_unfiltered.contains(&(f.visibility, texture)) {
                continue;
            }
            let Some(&(_, _, view)) = self
                .bound_samplers
                .iter()
                .find(|s| (s.0, s.1) == (f.visibility, sampler))
            else {
                continue;
            };
            let desc = crate::state::sampler(&view, self.backend.caps())?;
            if desc.compare.is_none() && [desc.min, desc.mag, desc.mip].contains(&Filter::Linear) {
                filtered |= 1 << texture;
                states.push((
                    HelperValue::SamplerState { sampler },
                    crate::state::sampler_state(&view, &desc),
                ));
                states.push((
                    HelperValue::BorderColor { sampler },
                    pull::words(&view.border),
                ));
            }
        }
        let offsets = self.bound_offsets.contains(&f.visibility);
        let early = self.early_draw && f.visibility == bind::PIXEL;
        if formats == f.made
            && !offsets
            && !early
            && lod_bias == 0
            && border == 0
            && clamped == 0
            && filtered == 0
            && through == f.key.texels_through_buffer
            && swizzled == f.key.swizzled
            && layers == f.key.texture_layers
        {
            return Ok(None);
        }
        let visibility = f.visibility;
        for (value, words) in states {
            if !self
                .bound_helpers
                .iter()
                .any(|h| h.0 == visibility && h.1 == value)
            {
                self.bound_helpers.push((visibility, value, words));
            }
        }
        if lod_bias != 0 {
            self.backend.slow_paths().hit(SlowPath::LodBiasInShader);
        }
        if border != 0 {
            self.backend.slow_paths().hit(SlowPath::BorderInShader);
        }
        if clamped != 0 {
            self.backend.slow_paths().hit(SlowPath::MinLodClamp);
        }
        if filtered != 0 {
            self.backend.slow_paths().hit(SlowPath::FilterInShader);
        }
        Ok(Some(Bound {
            formats,
            swizzled,
            layers,
            lod_bias,
            border,
            through,
            clamped,
            filtered,
            offsets,
            early,
        }))
    }

    /// Refuses typed views bound to a vertex stage run before a geometry or domain stage in
    /// formats other than those its program was made with: such programs are not converted
    /// again for them.
    pub(super) fn vertex_formats(&self, pid: PipelineId) -> Result<(), Error> {
        let Some(r) = &self.pipelines[pid.0 as usize].render else {
            return Ok(());
        };
        if r.vertex_formats.is_some() {
            return Ok(());
        }
        let made = slots(&r.typed_key);
        for &(visibility, slot, format) in &self.bound_formats {
            if visibility == bind::VERTEX && made.iter().any(|m| m.0 == slot && m.1 != format) {
                return Err(Error::NotImplemented(
                    "typed views of other formats in a vertex shader",
                ));
            }
        }
        Ok(())
    }

    /// The pipeline's program with its `visibility` stage converted from `code` with `key`.
    fn formats_program(
        &mut self,
        pid: PipelineId,
        visibility: u32,
        code: &[u8],
        key: VariantKey,
    ) -> Result<(B::Program, Helpers), Error> {
        if visibility == 0 {
            let cs = self.convert(code, ShaderStage::Compute, key)?;
            let shader = self.backend.create_shader(&cs.modules[0])?;
            let program = self.backend.create_program(&Program::Compute(&shader));
            self.backend.destroy(Object::Shader(shader));
            return Ok((program?, Helpers::of(&cs)));
        }
        let ps = self.convert(code, ShaderStage::Fragment, key)?;
        let vertex = self.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .expect("a graphics pipeline")
            .vertex
            .clone();
        let vertex = self.backend.create_shader(&vertex)?;
        let fragment = match self.backend.create_shader(&ps.modules[0]) {
            Ok(f) => f,
            Err(e) => {
                self.backend.destroy(Object::Shader(vertex));
                return Err(e.into());
            }
        };
        let program = self.backend.create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        });
        self.backend.destroy(Object::Shader(vertex));
        self.backend.destroy(Object::Shader(fragment));
        Ok((program?, Helpers::of(&ps)))
    }
}
