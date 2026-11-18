//! Typed views whose format only the bound view names: WebGPU's storage textures name their
//! format in the program, and typed buffers read through storage buffers decode theirs there
//! (`TexelBufferAsStorage`). A shader declaring such views is converted at pipeline creation with
//! each one's format taken as the 32-bit single-channel format of its declared kind (the only
//! formats Direct3D guarantees typed loads of), and for bound views of other formats the program
//! is converted again with theirs, at the first draw or dispatch binding them, and kept per
//! pipeline (`VariantKey::storage_formats`, `VariantKey::texel_formats`).

use super::*;
use crate::dxbc::{Kind, typed_views};
use pull::Helpers;

/// A typed view's slot: a storage slot (else a texture slot) and its register.
pub(super) type Slot = (bool, u32);

/// A program made for a set of bound formats with its helper layout; `None` where the
/// conversion failed.
type Variant<B> = (
    Vec<(Slot, Format)>,
    Option<(<B as Backend>::Program, Helpers)>,
);

/// A pipeline's program stage whose typed view formats come from the bound views.
pub(super) struct Formats<B: Backend> {
    /// The API stage (`D3D12_SHADER_VISIBILITY`, 0 for compute) and its bytecode.
    visibility: u32,
    code: Vec<u8>,
    /// The key it was converted with, the formats aside.
    key: VariantKey,
    /// The formats the pipeline's own program was made with, by slot.
    made: Vec<(Slot, Format)>,
    /// Programs made for other bound formats.
    variants: Vec<Variant<B>>,
}

/// The key fields a shader's typed views are first converted with: their formats by slot, and
/// `route` where it declares typed buffers. Default for DXIL and for shaders declaring none.
pub(super) fn typed_key(code: &[u8], route: TexelBufferRoute) -> VariantKey {
    let mut key = VariantKey::default();
    for v in typed_views(code).unwrap_or_default() {
        let format = match v.kind {
            Kind::Float => Format::R32Float,
            Kind::Uint => Format::R32Uint,
            Kind::Sint => Format::R32Sint,
        };
        match v.storage {
            true => key.storage_formats.push((v.register, format)),
            false => key.texel_formats.push((v.register, format)),
        }
        if v.buffer {
            key.texel_buffers = route;
        }
    }
    key.storage_formats.sort_by_key(|f| f.0);
    key.texel_formats.sort_by_key(|f| f.0);
    key
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
    /// The record of a stage converted with `key`, whose formats it made the program with;
    /// `None` when it has no such views.
    pub(super) fn of(visibility: u32, code: &[u8], key: &VariantKey) -> Option<Box<Self>> {
        let made = slots(key);
        if made.is_empty() {
            return None;
        }
        Some(Box::new(Formats {
            visibility,
            code: code.to_vec(),
            key: VariantKey {
                storage_formats: Vec::new(),
                texel_formats: Vec::new(),
                ..key.clone()
            },
            made,
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

impl<B: Backend> Device<B> {
    /// The program to draw or dispatch with for the typed views the last `binding_sets` bound,
    /// with its helper layout: `None` for the pipeline's own, else the one converted for their
    /// formats. Views bound with no format (null views) keep the program's. `vertex_variant` says
    /// the draw uses a vertex program other than the pipeline's own, which such programs are not
    /// made with.
    pub(super) fn bound_program(
        &mut self,
        pid: PipelineId,
        vertex_variant: bool,
    ) -> Result<Option<(B::Program, Helpers)>, Error> {
        let Some(f) = &self.pipelines[pid.0 as usize].formats else {
            return Ok(None);
        };
        let mut bound = f.made.clone();
        for &(visibility, slot, format) in &self.bound_formats {
            if visibility == f.visibility
                && let Some(b) = bound.iter_mut().find(|b| b.0 == slot)
            {
                b.1 = format;
            }
        }
        if bound == f.made {
            return Ok(None);
        }
        if vertex_variant {
            return Err(Error::NotImplemented(
                "a vertex program fetching its inputs with typed views of other formats",
            ));
        }
        let refused = Error::NotImplemented("typed views of formats the shader cannot take");
        if let Some((_, p)) = f.variants.iter().find(|v| v.0 == bound) {
            return p.clone().map(Some).ok_or(refused);
        }
        let (visibility, code) = (f.visibility, f.code.clone());
        let mut key = f.key.clone();
        for &((storage, register), format) in &bound {
            match storage {
                true => key.storage_formats.push((register, format)),
                false => key.texel_formats.push((register, format)),
            }
        }
        // A program made at the draw, as a state combination first met there.
        self.backend.slow_paths().hit(SlowPath::PipelineMiss);
        let made = self.formats_program(pid, visibility, &code, key);
        let f = self.pipelines[pid.0 as usize]
            .formats
            .as_mut()
            .expect("formats");
        f.variants.push((bound, made.as_ref().ok().cloned()));
        made.map(Some)
    }

    /// Refuses typed views bound to the vertex stage in formats other than those its program
    /// was made with: vertex programs are not converted again for them.
    pub(super) fn vertex_formats(&self, pid: PipelineId) -> Result<(), Error> {
        let Some(r) = &self.pipelines[pid.0 as usize].render else {
            return Ok(());
        };
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
