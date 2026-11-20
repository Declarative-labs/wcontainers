//! NIR from the pinned Mesa, owned, with the steps the routes take on it (through `bridge/bridge.c`).

use crate::ConvertError;
use crate::bind::{self, KINDS};
#[cfg(feature = "standalone")]
use std::ffi::{CStr, CString, c_char};
#[cfg(feature = "standalone")]
use std::sync::Once;
use wcontainers_gpu::{ReflectedBinding, ShaderStage};

/// The program stages the routes take (`enum wc_stage`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Vertex,
    Fragment,
    Compute,
}

impl Stage {
    /// Every stage, in the bridge's order (`WC_STAGES`).
    pub const ALL: &[Stage] = &[Stage::Vertex, Stage::Fragment, Stage::Compute];

    pub const fn shader_stage(self) -> ShaderStage {
        match self {
            Stage::Vertex => ShaderStage::Vertex,
            Stage::Fragment => ShaderStage::Fragment,
            Stage::Compute => ShaderStage::Compute,
        }
    }
}

#[repr(C)]
struct SlotRange {
    first: u32,
    slots: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Binding {
    kind: u32,
    set: u32,
    binding: u32,
    count: u32,
    new_binding: u32,
    stride: u32,
}

#[repr(C)]
struct Bindings {
    data: *mut Binding,
    count: usize,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
enum BindResult {
    Ok,
    BeyondLimit,
    Unbounded,
    OutOfMemory,
}

#[repr(C)]
pub(crate) struct NirShader {
    _opaque: [u8; 0],
}

#[repr(C)]
struct Bytes {
    data: *mut u8,
    size: usize,
}

/// What a name of a GLSL program, or a state value, is in one stage (`enum wc_glsl_name_kind`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
pub enum GlslNameKind {
    /// A default-block uniform: `at` its first word in uniform buffer 0, `count` its words.
    Uniform,
    /// A subroutine uniform by its API name, as `Uniform`: each element's word holds the index of
    /// the subroutine chosen for it.
    SubroutineUniform,
    /// `STATE_FB_WPOS_Y_TRANSFORM`'s four words at `at`.
    FragCoordYTransform,
    /// `STATE_FB_PNTC_Y_TRANSFORM`'s four words at `at`.
    PointCoordYTransform,
    /// Any other state value: a matrix's rows as `matrix.<matrix>.<form>.<index>.<first
    /// row>.<last row>` (form `plain`, `inverse`, `transpose` or `invtrans`, as Mesa's state
    /// tokens), anything else as `state.` and Mesa's five tokens.
    OtherState,
    /// `at` the program binding, `count` the bindings.
    UniformBlock,
    StorageBlock,
    Sampler,
    Image,
    /// An atomic counter buffer, which the driver gives the program as a storage block: `at` its
    /// program binding, `element` its OpenGL binding point.
    AtomicCounterBuffer,
    /// `at` the location in the common form, `count` the locations.
    Input,
    Output,
}

/// A stage value's interpolation qualifier (`enum wc_glsl_interpolation`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
pub enum Interpolation {
    Unqualified,
    Smooth,
    Flat,
    NoPerspective,
    Other,
}

/// One name of a GLSL program in one stage and what the OpenGL driver's state reaches it by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlslName {
    pub kind: GlslNameKind,
    pub name: String,
    /// A stage value's GLSL type.
    pub ty: Option<String>,
    pub interpolation: Interpolation,
    pub at: u32,
    pub count: u32,
    /// The first of the value's slots the name places: a vertex attribute read in part (some
    /// columns of a matrix) has a name a run of the slots read.
    pub element: u32,
}

/// One stage of a linked GLSL program.
pub struct GlslStage {
    pub stage: Stage,
    pub nir: Nir,
    pub names: Vec<GlslName>,
}

#[cfg(feature = "standalone")]
#[repr(C)]
struct RawGlslName {
    kind: GlslNameKind,
    name: *mut c_char,
    ty: *mut c_char,
    interpolation: Interpolation,
    at: u32,
    count: u32,
    element: u32,
}

#[cfg(feature = "standalone")]
#[repr(C)]
struct RawGlslNames {
    data: *mut RawGlslName,
    count: usize,
}

#[cfg(feature = "standalone")]
#[repr(C)]
struct GlslProgram {
    stages: [*mut NirShader; Stage::ALL.len()],
    names: [RawGlslNames; Stage::ALL.len()],
    log: *mut c_char,
}

#[cfg(feature = "standalone")]
/// A string the bridge owns, copied.
fn owned_string(text: *const c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    // SAFETY: the bridge's strings are NUL-terminated until the program is freed.
    unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(feature = "standalone")]
#[repr(C)]
struct Words {
    data: *mut u32,
    count: usize,
}

unsafe extern "C" {
    fn wc_nir_to_common_form(
        nir: *mut NirShader,
        set: u32,
        ranges: *const SlotRange,
        out: *mut Bindings,
    ) -> BindResult;
    fn wc_bindings_free(bindings: *mut Bindings);
    fn wc_nir_alpha_to_one(nir: *mut NirShader);
    fn wc_nir_serialize(nir: *const NirShader, out: *mut Bytes) -> bool;
    fn wc_nir_free(nir: *mut NirShader);
    fn wc_bytes_free(bytes: *mut Bytes);
}

#[cfg(feature = "standalone")]
unsafe extern "C" {
    fn wc_init();
    fn wc_spirv_to_nir(
        words: *const u32,
        count: usize,
        stage: Stage,
        entry_point: *const c_char,
    ) -> *mut NirShader;
    fn wc_glsl_to_nir(sources: *const *const c_char, out: *mut GlslProgram) -> bool;
    fn wc_glsl_program_free(program: *mut GlslProgram);
    fn wc_nir_to_spirv(nir: *mut NirShader, out: *mut Words) -> bool;
    fn wc_words_free(words: *mut Words);
}

/// One shader's NIR, freed on drop.
pub struct Nir(*mut NirShader);

#[cfg(feature = "standalone")]
/// Mesa's type tables, held for the life of the process (Mesa counts their references).
fn initialised() {
    static INIT: Once = Once::new();
    // SAFETY: wc_init only takes a reference on Mesa's process-wide type tables.
    INIT.call_once(|| unsafe { wc_init() });
}

impl Nir {
    pub(crate) fn raw(&mut self) -> *mut NirShader {
        self.0
    }

    /// Takes the NIR a Mesa driver was handed (a Gallium shader state's `ir.nir`), freed on drop.
    ///
    /// # Safety
    /// `nir` is a live `nir_shader` the caller owns and does not use again.
    pub unsafe fn from_raw(nir: *mut std::ffi::c_void) -> Nir {
        Nir(nir.cast())
    }

    #[cfg(feature = "standalone")]
    /// The NIR of one entry point of a Vulkan SPIR-V module.
    pub fn from_spirv(words: &[u32], stage: Stage, entry_point: &str) -> Result<Nir, ConvertError> {
        initialised();
        let entry = CString::new(entry_point).map_err(|_| ConvertError::Mesa(SPIRV_TO_NIR))?;
        // SAFETY: the words and the name outlive the call; Mesa copies what it keeps.
        let nir = unsafe { wc_spirv_to_nir(words.as_ptr(), words.len(), stage, entry.as_ptr()) };
        if nir.is_null() {
            return Err(ConvertError::Mesa(SPIRV_TO_NIR));
        }
        Ok(Nir(nir))
    }

    #[cfg(feature = "standalone")]
    /// The stages of one GLSL program as the OpenGL driver receives them: compiled and linked by
    /// Mesa's GLSL compiler, then st/mesa's steps for the driver's capabilities (`bridge/glsl.cpp`),
    /// each buffer an array of words; with each stage's names. One entry a source, in
    /// `Stage::ALL`'s order.
    pub fn from_glsl(sources: &[(Stage, &str)]) -> Result<Vec<GlslStage>, ConvertError> {
        initialised();
        let owned = sources
            .iter()
            .map(|(stage, text)| Ok((*stage, CString::new(*text)?)))
            .collect::<Result<Vec<_>, std::ffi::NulError>>()
            .map_err(|e| ConvertError::Glsl(e.to_string()))?;
        let pointers: Vec<*const c_char> = Stage::ALL
            .iter()
            .map(|stage| {
                owned
                    .iter()
                    .find(|(s, _)| s == stage)
                    .map_or(std::ptr::null(), |(_, text)| text.as_ptr())
            })
            .collect();
        let mut program = GlslProgram {
            stages: [std::ptr::null_mut(); Stage::ALL.len()],
            names: std::array::from_fn(|_| RawGlslNames {
                data: std::ptr::null_mut(),
                count: 0,
            }),
            log: std::ptr::null_mut(),
        };
        // SAFETY: one source pointer a stage (or null), each alive for the call; the bridge fills
        // `program` and Mesa copies what it keeps.
        let linked = unsafe { wc_glsl_to_nir(pointers.as_ptr(), &mut program) };
        let log = owned_string(program.log);
        let stages = Stage::ALL
            .iter()
            .zip(program.stages.iter_mut().zip(&program.names))
            .filter(|(_, (nir, _))| !nir.is_null())
            .map(|(stage, (nir, names))| {
                let names = if names.data.is_null() {
                    &[][..]
                } else {
                    // SAFETY: the bridge's array of `count` names, alive until the program is freed.
                    unsafe { std::slice::from_raw_parts(names.data, names.count) }
                };
                GlslStage {
                    stage: *stage,
                    nir: Nir(std::mem::replace(nir, std::ptr::null_mut())),
                    names: names
                        .iter()
                        .map(|n| GlslName {
                            kind: n.kind,
                            name: owned_string(n.name),
                            ty: (!n.ty.is_null()).then(|| owned_string(n.ty)),
                            interpolation: n.interpolation,
                            at: n.at,
                            count: n.count,
                            element: n.element,
                        })
                        .collect(),
                }
            })
            .collect();
        // SAFETY: program came from the bridge; the shaders taken above are no longer in it.
        unsafe { wc_glsl_program_free(&mut program) };
        if linked {
            Ok(stages)
        } else {
            Err(ConvertError::Glsl(log))
        }
    }

    /// Gives the program of `stage` the common form's meaning (sampled array layers rounded to
    /// integers) and its binding rule ([`crate::bind`]), splitting combined image samplers, and gives
    /// each resource's reflection.
    pub fn to_common_form(&mut self, stage: Stage) -> Result<Vec<ReflectedBinding>, ConvertError> {
        let ranges = KINDS.map(|k| SlotRange {
            first: k.first_binding(),
            slots: k.slots(),
        });
        let set = bind::stage_set(stage.shader_stage());
        let mut out = Bindings {
            data: std::ptr::null_mut(),
            count: 0,
        };
        // SAFETY: self.0 is a live shader, ranges holds one range a kind, and the bridge fills `out`.
        let result = unsafe { wc_nir_to_common_form(self.0, set, ranges.as_ptr(), &mut out) };
        let made: Vec<ReflectedBinding> = if out.data.is_null() {
            Vec::new()
        } else {
            // SAFETY: data holds count entries until wc_bindings_free.
            unsafe { std::slice::from_raw_parts(out.data, out.count) }
                .iter()
                .map(|b| ReflectedBinding {
                    kind: KINDS[b.kind as usize],
                    space: b.set,
                    register: b.binding,
                    count: b.count,
                    set,
                    binding: b.new_binding,
                    added: None,
                    stride: b.stride,
                })
                .collect()
        };
        // SAFETY: out came from the bridge and is freed once.
        unsafe { wc_bindings_free(&mut out) };
        match result {
            BindResult::Ok => Ok(made),
            BindResult::BeyondLimit => Err(ConvertError::Bindings(bind::BEYOND_LIMIT)),
            BindResult::Unbounded => Err(ConvertError::Bindings(bind::UNBOUNDED)),
            BindResult::OutOfMemory => Err(ConvertError::Bindings(bind::OUT_OF_MEMORY)),
        }
    }

    /// Vulkan's `alphaToOne` and OpenGL's `GL_SAMPLE_ALPHA_TO_ONE` made in a fragment program (plan
    /// 15.4, `AlphaToOneInShader`): Mesa's alpha to coverage demotes the samples alpha does not
    /// cover, then every colour output's alpha is one. Nothing for another stage.
    pub fn alpha_to_one(&mut self) {
        // SAFETY: self.0 is a live shader.
        unsafe { wc_nir_alpha_to_one(self.0) };
    }

    /// The cache's key of what a route converts from this NIR (plan 15.3): BLAKE3 of NIR's serialised
    /// bytes and the converter's version ([`CONVERTER_VERSION`]). Taken before the route changes the
    /// shader.
    pub fn key(&self) -> Result<[u8; blake3::OUT_LEN], ConvertError> {
        let bytes = self.serialized()?;
        Ok(*blake3::Hasher::new()
            .update(&bytes)
            .update(CONVERTER_VERSION.as_bytes())
            .finalize()
            .as_bytes())
    }

    /// NIR's serialised bytes, the cache's key with the converter's version (plan 15.3).
    pub fn serialized(&self) -> Result<Vec<u8>, ConvertError> {
        let mut bytes = Bytes {
            data: std::ptr::null_mut(),
            size: 0,
        };
        // SAFETY: self.0 is a live shader; the bridge fills `bytes` with memory it allocated.
        if !unsafe { wc_nir_serialize(self.0, &mut bytes) } {
            return Err(ConvertError::Mesa(NIR_SERIALIZE));
        }
        // SAFETY: data holds size bytes until wc_bytes_free.
        let copy = unsafe { std::slice::from_raw_parts(bytes.data, bytes.size) }.to_vec();
        // SAFETY: bytes came from the bridge and is freed once.
        unsafe { wc_bytes_free(&mut bytes) };
        Ok(copy)
    }

    #[cfg(feature = "standalone")]
    /// Candidate A's first step: SPIR-V written by Zink's compiler (the shader is prepared for it
    /// in place, so it is consumed).
    pub fn into_spirv(self) -> Result<Vec<u32>, ConvertError> {
        let mut words = Words {
            data: std::ptr::null_mut(),
            count: 0,
        };
        // SAFETY: self.0 is a live shader; the bridge fills `words` with memory it allocated.
        if !unsafe { wc_nir_to_spirv(self.0, &mut words) } {
            return Err(ConvertError::Mesa(NIR_TO_SPIRV));
        }
        // SAFETY: data holds count words until wc_words_free.
        let copy = unsafe { std::slice::from_raw_parts(words.data, words.count) }.to_vec();
        // SAFETY: words came from the bridge and is freed once.
        unsafe { wc_words_free(&mut words) };
        Ok(copy)
    }
}

impl Drop for Nir {
    fn drop(&mut self) {
        // SAFETY: self.0 came from the bridge and is freed once.
        unsafe { wc_nir_free(self.0) };
    }
}

/// The converter's version in the cache's key: this crate's version, a hash of its sources and of
/// the naga it writes, and the Mesa it links (commit and build options, from `mesa/build`'s
/// record).
pub const CONVERTER_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "+",
    env!("WCONTAINERS_NIR_SOURCES"),
    "+mesa.",
    env!("WCONTAINERS_MESA_MADE_FROM")
);

#[cfg(feature = "standalone")]
/// Lavapipe, Mesa's Vulkan driver on the processor, built with the Mesa this crate links: the
/// reference the corpus run holds the routes to (plan 15.3).
pub const LAVAPIPE: &str = env!("WCONTAINERS_LAVAPIPE");

#[cfg(feature = "standalone")]
/// Mesa's EGL from the same build, through which llvmpipe, Mesa's OpenGL driver on the processor,
/// is reached: the reference for GLSL programs (plan 15.3).
pub const EGL: &str = env!("WCONTAINERS_EGL");

/// The steps named in `ConvertError::Mesa`.
pub const SPIRV_TO_NIR: &str = "spirv_to_nir";
pub const NIR_SERIALIZE: &str = "nir_serialize";
pub const NIR_TO_SPIRV: &str = "nir_to_spirv";
