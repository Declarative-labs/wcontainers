//! Typed unordered access views of textures: WebGPU's storage textures name their format in the
//! program, Direct3D's only in the bound view. A pixel or compute shader declaring such views is
//! converted at pipeline creation with each one's format taken as the 32-bit single-channel
//! format of its declared kind (the only formats Direct3D guarantees typed loads of), and for
//! bound views of other formats the program is converted again with theirs, at the first draw or
//! dispatch binding them, and kept per pipeline (`VariantKey::storage_formats`).

use super::*;
use crate::dxbc::{Kind, typed_texture_views};

/// Programs made for a set of bound formats, by register; `None` where the conversion failed.
type Variant<B> = (Vec<(u32, Format)>, Option<<B as Backend>::Program>);

/// A pipeline's program stage whose storage texture formats come from the bound views.
pub(super) struct Formats<B: Backend> {
    /// The API stage (`D3D12_SHADER_VISIBILITY`, 0 for compute) and its bytecode.
    visibility: u32,
    code: Vec<u8>,
    /// The key it was converted with, `storage_formats` aside.
    key: VariantKey,
    /// The formats the pipeline's own program was made with, by register.
    made: Vec<(u32, Format)>,
    /// Programs made for other bound formats.
    variants: Vec<Variant<B>>,
}

/// The formats a shader's typed texture views are first converted with, by register; empty for
/// DXIL and for shaders declaring none.
pub(super) fn first_formats(code: &[u8]) -> Vec<(u32, Format)> {
    let mut formats: Vec<(u32, Format)> = typed_texture_views(code)
        .unwrap_or_default()
        .into_iter()
        .map(|(register, kind)| {
            let format = match kind {
                Kind::Float => Format::R32Float,
                Kind::Uint => Format::R32Uint,
                Kind::Sint => Format::R32Sint,
            };
            (register, format)
        })
        .collect();
    formats.sort_by_key(|f| f.0);
    formats
}

impl<B: Backend> Formats<B> {
    /// The record of a stage converted with `key`, whose `storage_formats` it made the program
    /// with; `None` when it has no such views.
    pub(super) fn of(visibility: u32, code: &[u8], key: &VariantKey) -> Option<Box<Self>> {
        if key.storage_formats.is_empty() {
            return None;
        }
        Some(Box::new(Formats {
            visibility,
            code: code.to_vec(),
            key: VariantKey {
                storage_formats: Vec::new(),
                ..key.clone()
            },
            made: key.storage_formats.clone(),
            variants: Vec::new(),
        }))
    }

    pub(super) fn destroy(self, backend: &mut B) {
        for (_, p) in self.variants {
            if let Some(p) = p {
                backend.destroy(Object::Program(p));
            }
        }
    }
}

impl<B: Backend> Device<B> {
    /// The program to draw or dispatch with for the storage textures the last `binding_sets`
    /// bound: `None` for the pipeline's own, else the one converted for their formats.
    /// `vertex_variant` says the draw uses a vertex program other than the pipeline's own,
    /// which such programs are not made with.
    pub(super) fn bound_program(
        &mut self,
        pid: PipelineId,
        vertex_variant: bool,
    ) -> Result<Option<B::Program>, Error> {
        let Some(f) = &self.pipelines[pid.0 as usize].formats else {
            return Ok(None);
        };
        let mut bound: Vec<(u32, Format)> = self
            .bound_storage
            .iter()
            .filter(|b| b.0 == f.visibility)
            .map(|b| (b.1, b.2))
            .collect();
        bound.sort_by_key(|b| b.0);
        if bound == f.made {
            return Ok(None);
        }
        if vertex_variant {
            return Err(Error::NotImplemented(
                "a vertex program fetching its inputs with storage textures of other formats",
            ));
        }
        let refused = Error::NotImplemented("storage textures of formats the shader cannot take");
        if let Some((_, p)) = f.variants.iter().find(|v| v.0 == bound) {
            return p.clone().map(Some).ok_or(refused);
        }
        let (visibility, code) = (f.visibility, f.code.clone());
        let key = VariantKey {
            storage_formats: bound.clone(),
            ..f.key.clone()
        };
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

    /// The pipeline's program with its `visibility` stage converted from `code` with `key`.
    fn formats_program(
        &mut self,
        pid: PipelineId,
        visibility: u32,
        code: &[u8],
        key: VariantKey,
    ) -> Result<B::Program, Error> {
        if visibility == 0 {
            let cs = self.convert(code, ShaderStage::Compute, key)?;
            let shader = self.backend.create_shader(&cs.modules[0])?;
            let program = self.backend.create_program(&Program::Compute(&shader));
            self.backend.destroy(Object::Shader(shader));
            return Ok(program?);
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
        Ok(program?)
    }
}
