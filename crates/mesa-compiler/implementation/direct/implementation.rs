// Written by tools/api-crate-generator from the Mesa compiler bridge's bridge.h and flat.h (crates/nir/bridge). Do not edit.
//! Mesa's compiler through our C bridge (crates/nir/bridge): every bridge function a safe method of
//! wcontainers-mesa-compiler's trait, each shader a value of `Shader` that owns one NIR shader and gives it back when
//! dropped, and every structure the bridge fills copied out whole into owned values before the bridge's free.
//!
//! What the marks rely on: the bridge's functions have the types bridge.h and flat.h declare; a function reads and
//! writes only what its parameters name, as far as the count beside each says (or the header's array length); the
//! arrays and texts it fills are its own until the matching free, which takes a zeroed structure as nothing; a shader
//! it returns or fills is a nir_shader it made, given back once by wc_nir_free; Mesa's instruction names are static.
//! The bridge's C is linked by the crate that builds it (crates/nir's build today; see the status for where it moves).

use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;

use wcontainers_mesa_compiler::*;

/// The compiler: Mesa's type tables taken once for the process.
#[derive(Clone, Copy, Debug)]
pub struct Bridge {
    _private: (),
}

/// Whether the type tables were taken.
static STARTED: std::sync::Once = std::sync::Once::new();

impl Bridge {
    fn started() -> Bridge {
        STARTED.call_once(|| {
            #[expect(unsafe_code, reason = "bridge.h: wc_init takes nothing and takes a reference on Mesa's type tables")]
            // SAFETY: no pointer is passed; called once for the process.
            unsafe { wc_init() };
        });
        Bridge { _private: () }
    }
}

/// One NIR shader the bridge made, given back when dropped. Not `Send`: no fact of the bridge's is relied on for a
/// shader moved between threads (NEEDED B1 leaves it to this crate; it may be offered once measured).
#[derive(Debug)]
pub struct Shader {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl Shader {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<Shader> {
        NonNull::new(nir).map(|nir| Shader { nir, _thread: PhantomData })
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_free gives back a shader the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_nir_free(self.nir.as_ptr()) };
    }
}

/// Copies the `count` elements the bridge holds at `data`; none when it is null or the count does not fit.
fn array<T: Copy, C: TryInto<usize>>(data: *mut T, count: C) -> Vec<T> {
    let Ok(count) = count.try_into() else { return Vec::new() };
    if data.is_null() || count == 0 {
        return Vec::new();
    }
    #[expect(unsafe_code, reason = "bridge.h and flat.h: each array the bridge fills holds the count beside it, and is the bridge's until its free")]
    // SAFETY: not null; `count` elements of T, the bridge's, unchanged until the free that follows the copy.
    let elements = unsafe { core::slice::from_raw_parts(data, count) };
    elements.to_vec()
}

/// Copies a zero-ended text the bridge owns, or Mesa keeps; None for null.
fn text(data: *mut u8) -> Option<Vec<u8>> {
    if data.is_null() {
        return None;
    }
    #[expect(unsafe_code, reason = "bridge.h and flat.h: every text the bridge fills is zero-ended (its own until its free, or Mesa's static instruction names)")]
    // SAFETY: not null and zero-ended, unchanged until the free that follows the copy.
    let text = unsafe { core::ffi::CStr::from_ptr(data.cast()) };
    Some(text.to_bytes().to_vec())
}

// ----- The bridge's structures as it lays them out. -----

/// `struct wc_flat_type` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct FlatTypeMirror {
    pub(crate) kind: u32,
    pub(crate) scalar: u32,
    pub(crate) bits: u32,
    pub(crate) components: u32,
    pub(crate) columns: u32,
    pub(crate) length: u32,
    pub(crate) element: u32,
    pub(crate) stride: u32,
    pub(crate) row_major: u32,
    pub(crate) dim: u32,
    pub(crate) arrayed: u32,
    pub(crate) shadow: u32,
    pub(crate) multisampled: u32,
    pub(crate) sampled: u32,
}

impl FlatTypeMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> FlatType {
        FlatType { kind: self.kind, scalar: self.scalar, bits: self.bits, components: self.components, columns: self.columns, length: self.length, element: self.element, stride: self.stride, row_major: self.row_major, dim: self.dim, arrayed: self.arrayed, shadow: self.shadow, multisampled: self.multisampled, sampled: self.sampled }
    }
}

impl From<&FlatType> for FlatTypeMirror {
    fn from(value: &FlatType) -> FlatTypeMirror {
        FlatTypeMirror { kind: value.kind, scalar: value.scalar, bits: value.bits, components: value.components, columns: value.columns, length: value.length, element: value.element, stride: value.stride, row_major: value.row_major, dim: value.dim, arrayed: value.arrayed, shadow: value.shadow, multisampled: value.multisampled, sampled: value.sampled }
    }
}

/// `struct wc_flat_field` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct FlatFieldMirror {
    pub(crate) type_: u32,
    pub(crate) offset: i32,
}

impl FlatFieldMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> FlatField {
        FlatField { type_: self.type_, offset: self.offset }
    }
}

impl From<&FlatField> for FlatFieldMirror {
    fn from(value: &FlatField) -> FlatFieldMirror {
        FlatFieldMirror { type_: value.type_, offset: value.offset }
    }
}

/// `struct wc_flat_var` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct FlatVarMirror {
    pub(crate) mode: u32,
    pub(crate) type_: u32,
    pub(crate) builtin: u32,
    pub(crate) location: u32,
    pub(crate) set: u32,
    pub(crate) binding: u32,
    pub(crate) interpolation: u32,
    pub(crate) centroid: u32,
    pub(crate) sample: u32,
    pub(crate) readable: u32,
    pub(crate) writable: u32,
    pub(crate) format: u32,
    pub(crate) initialised: u32,
    pub(crate) stream: u32,
    pub(crate) blend_source: u32,
    pub(crate) patch: u32,
    pub(crate) invariant: u32,
}

impl FlatVarMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> FlatVar {
        FlatVar { mode: self.mode, type_: self.type_, builtin: self.builtin, location: self.location, set: self.set, binding: self.binding, interpolation: self.interpolation, centroid: self.centroid, sample: self.sample, readable: self.readable, writable: self.writable, format: self.format, initialised: self.initialised, stream: self.stream, blend_source: self.blend_source, patch: self.patch, invariant: self.invariant }
    }
}

impl From<&FlatVar> for FlatVarMirror {
    fn from(value: &FlatVar) -> FlatVarMirror {
        FlatVarMirror { mode: value.mode, type_: value.type_, builtin: value.builtin, location: value.location, set: value.set, binding: value.binding, interpolation: value.interpolation, centroid: value.centroid, sample: value.sample, readable: value.readable, writable: value.writable, format: value.format, initialised: value.initialised, stream: value.stream, blend_source: value.blend_source, patch: value.patch, invariant: value.invariant }
    }
}

/// `struct wc_flat_src` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct FlatSrcMirror {
    pub(crate) def: u32,
    pub(crate) swizzle: [u8; 4],
}

impl FlatSrcMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> FlatSrc {
        FlatSrc { def: self.def, swizzle: self.swizzle.each_mut().map(|element| *element) }
    }
}

impl From<&FlatSrc> for FlatSrcMirror {
    fn from(value: &FlatSrc) -> FlatSrcMirror {
        FlatSrcMirror { def: value.def, swizzle: value.swizzle.map(|element| element) }
    }
}

/// `struct wc_flat_instr` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct FlatInstrMirror {
    pub(crate) kind: u32,
    pub(crate) op: u32,
    pub(crate) def: u32,
    pub(crate) components: u32,
    pub(crate) bits: u32,
    pub(crate) sources: u32,
    pub(crate) src: [FlatSrcMirror; 5],
    pub(crate) value: [u32; 4],
    pub(crate) type_: u32,
    pub(crate) tex: [u32; 10],
    pub(crate) dim: u32,
    pub(crate) arrayed: u32,
    pub(crate) shadow: u32,
    pub(crate) component: u32,
    pub(crate) name: *mut u8,
}

impl Default for FlatInstrMirror {
    fn default() -> FlatInstrMirror {
        FlatInstrMirror { kind: 0, op: 0, def: 0, components: 0, bits: 0, sources: 0, src: [FlatSrcMirror::default(); 5], value: [0; 4], type_: 0, tex: [0; 10], dim: 0, arrayed: 0, shadow: 0, component: 0, name: core::ptr::null_mut() }
    }
}

impl FlatInstrMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> FlatInstr {
        FlatInstr { kind: self.kind, op: self.op, def: self.def, components: self.components, bits: self.bits, sources: self.sources, src: self.src.each_mut().map(|element| element.owned()), value: self.value.each_mut().map(|element| *element), type_: self.type_, tex: self.tex.each_mut().map(|element| *element), dim: self.dim, arrayed: self.arrayed, shadow: self.shadow, component: self.component, name: text(self.name) }
    }
}

/// `struct wc_flat_geometry` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct FlatGeometryMirror {
    pub(crate) input: u32,
    pub(crate) output: u32,
    pub(crate) vertices_in: u32,
    pub(crate) max_vertices: u32,
    pub(crate) invocations: u32,
    pub(crate) streams: u32,
}

impl FlatGeometryMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> FlatGeometry {
        FlatGeometry { input: self.input, output: self.output, vertices_in: self.vertices_in, max_vertices: self.max_vertices, invocations: self.invocations, streams: self.streams }
    }
}

impl From<&FlatGeometry> for FlatGeometryMirror {
    fn from(value: &FlatGeometry) -> FlatGeometryMirror {
        FlatGeometryMirror { input: value.input, output: value.output, vertices_in: value.vertices_in, max_vertices: value.max_vertices, invocations: value.invocations, streams: value.streams }
    }
}

/// `struct wc_flat_tessellation` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct FlatTessellationMirror {
    pub(crate) points_in: u32,
    pub(crate) points_out: u32,
    pub(crate) domain: u32,
    pub(crate) spacing: u32,
    pub(crate) counter_clockwise: u32,
    pub(crate) point_mode: u32,
}

impl FlatTessellationMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> FlatTessellation {
        FlatTessellation { points_in: self.points_in, points_out: self.points_out, domain: self.domain, spacing: self.spacing, counter_clockwise: self.counter_clockwise, point_mode: self.point_mode }
    }
}

impl From<&FlatTessellation> for FlatTessellationMirror {
    fn from(value: &FlatTessellation) -> FlatTessellationMirror {
        FlatTessellationMirror { points_in: value.points_in, points_out: value.points_out, domain: value.domain, spacing: value.spacing, counter_clockwise: value.counter_clockwise, point_mode: value.point_mode }
    }
}

/// `struct wc_flat` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct FlatMirror {
    pub(crate) stage: u32,
    pub(crate) workgroup_size: [u32; 3],
    pub(crate) early_fragment_tests: u32,
    pub(crate) values: u32,
    pub(crate) types: *mut FlatTypeMirror,
    pub(crate) type_count: u32,
    pub(crate) fields: *mut FlatFieldMirror,
    pub(crate) field_count: u32,
    pub(crate) vars: *mut FlatVarMirror,
    pub(crate) var_count: u32,
    pub(crate) initialisers: *mut u32,
    pub(crate) initialiser_count: u32,
    pub(crate) instrs: *mut FlatInstrMirror,
    pub(crate) instr_count: u32,
    pub(crate) geometry: FlatGeometryMirror,
    pub(crate) tessellation: FlatTessellationMirror,
}

impl Default for FlatMirror {
    fn default() -> FlatMirror {
        FlatMirror { stage: 0, workgroup_size: [0; 3], early_fragment_tests: 0, values: 0, types: core::ptr::null_mut(), type_count: 0, fields: core::ptr::null_mut(), field_count: 0, vars: core::ptr::null_mut(), var_count: 0, initialisers: core::ptr::null_mut(), initialiser_count: 0, instrs: core::ptr::null_mut(), instr_count: 0, geometry: FlatGeometryMirror::default(), tessellation: FlatTessellationMirror::default() }
    }
}

impl FlatMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> Flat {
        Flat { stage: self.stage, workgroup_size: self.workgroup_size.each_mut().map(|element| *element), early_fragment_tests: self.early_fragment_tests, values: self.values, types: array(self.types, self.type_count).iter_mut().map(|element| element.owned()).collect(), fields: array(self.fields, self.field_count).iter_mut().map(|element| element.owned()).collect(), vars: array(self.vars, self.var_count).iter_mut().map(|element| element.owned()).collect(), initialisers: array(self.initialisers, self.initialiser_count), instrs: array(self.instrs, self.instr_count).iter_mut().map(|element| element.owned()).collect(), geometry: self.geometry.owned(), tessellation: self.tessellation.owned() }
    }
}

/// `struct wc_glsl_name` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct GlslNameMirror {
    pub(crate) kind: u32,
    pub(crate) name: *mut u8,
    pub(crate) type_: *mut u8,
    pub(crate) interpolation: u32,
    pub(crate) at: u32,
    pub(crate) count: u32,
    pub(crate) element: u32,
}

impl Default for GlslNameMirror {
    fn default() -> GlslNameMirror {
        GlslNameMirror { kind: 0, name: core::ptr::null_mut(), type_: core::ptr::null_mut(), interpolation: 0, at: 0, count: 0, element: 0 }
    }
}

impl GlslNameMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> GlslName {
        GlslName { kind: self.kind, name: text(self.name), type_: text(self.type_), interpolation: self.interpolation, at: self.at, count: self.count, element: self.element }
    }
}

/// `struct wc_glsl_names` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct GlslNamesMirror {
    pub(crate) data: *mut GlslNameMirror,
    pub(crate) count: usize,
}

impl Default for GlslNamesMirror {
    fn default() -> GlslNamesMirror {
        GlslNamesMirror { data: core::ptr::null_mut(), count: 0 }
    }
}

impl GlslNamesMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> Vec<GlslName> {
        array(self.data, self.count).iter_mut().map(|element| element.owned()).collect()
    }
}

/// `struct wc_glsl_program` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct GlslProgramMirror {
    pub(crate) stages: [*mut c_void; 3],
    pub(crate) names: [GlslNamesMirror; 3],
    pub(crate) log: *mut u8,
}

impl Default for GlslProgramMirror {
    fn default() -> GlslProgramMirror {
        GlslProgramMirror { stages: [core::ptr::null_mut(); 3], names: [GlslNamesMirror::default(); 3], log: core::ptr::null_mut() }
    }
}

impl GlslProgramMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> GlslProgram<Shader> {
        GlslProgram { stages: self.stages.each_mut().map(|element| Shader::owning(core::mem::replace(element, core::ptr::null_mut()))), names: self.names.each_mut().map(|element| element.owned()), log: text(self.log) }
    }
}

/// `struct wc_slot_range` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct SlotRangeMirror {
    pub(crate) first: u32,
    pub(crate) slots: u32,
}

impl From<&SlotRange> for SlotRangeMirror {
    fn from(value: &SlotRange) -> SlotRangeMirror {
        SlotRangeMirror { first: value.first, slots: value.slots }
    }
}

/// `struct wc_binding` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct BindingMirror {
    pub(crate) kind: u32,
    pub(crate) set: u32,
    pub(crate) binding: u32,
    pub(crate) count: u32,
    pub(crate) new_binding: u32,
    pub(crate) stride: u32,
}

impl BindingMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> Binding {
        Binding { kind: self.kind, set: self.set, binding: self.binding, count: self.count, new_binding: self.new_binding, stride: self.stride }
    }
}

impl From<&Binding> for BindingMirror {
    fn from(value: &Binding) -> BindingMirror {
        BindingMirror { kind: value.kind, set: value.set, binding: value.binding, count: value.count, new_binding: value.new_binding, stride: value.stride }
    }
}

/// `struct wc_bindings` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct BindingsMirror {
    pub(crate) data: *mut BindingMirror,
    pub(crate) count: usize,
}

impl Default for BindingsMirror {
    fn default() -> BindingsMirror {
        BindingsMirror { data: core::ptr::null_mut(), count: 0 }
    }
}

impl BindingsMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> Vec<Binding> {
        array(self.data, self.count).iter_mut().map(|element| element.owned()).collect()
    }
}

/// `struct wc_bytes` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct BytesMirror {
    pub(crate) data: *mut u8,
    pub(crate) size: usize,
}

impl Default for BytesMirror {
    fn default() -> BytesMirror {
        BytesMirror { data: core::ptr::null_mut(), size: 0 }
    }
}

impl BytesMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> Vec<u8> {
        array(self.data, self.size)
    }
}

/// `struct wc_words` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct WordsMirror {
    pub(crate) data: *mut u32,
    pub(crate) count: usize,
}

impl Default for WordsMirror {
    fn default() -> WordsMirror {
        WordsMirror { data: core::ptr::null_mut(), count: 0 }
    }
}

impl WordsMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> Vec<u32> {
        array(self.data, self.count)
    }
}

/// `struct wc_position_transform` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct PositionTransformMirror {
    pub(crate) flip_y: u8,
    pub(crate) half_z: u8,
    pub(crate) reverse_z: u8,
    pub(crate) fixed_point_size: u8,
}

impl From<&PositionTransform> for PositionTransformMirror {
    fn from(value: &PositionTransform) -> PositionTransformMirror {
        PositionTransformMirror { flip_y: u8::from(value.flip_y), half_z: u8::from(value.half_z), reverse_z: u8::from(value.reverse_z), fixed_point_size: u8::from(value.fixed_point_size) }
    }
}

/// `struct wc_texture_swizzle` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct TextureSwizzleMirror {
    pub(crate) unit: u32,
    pub(crate) swizzle: [u8; 4],
}

impl From<&TextureSwizzle> for TextureSwizzleMirror {
    fn from(value: &TextureSwizzle) -> TextureSwizzleMirror {
        TextureSwizzleMirror { unit: value.unit, swizzle: value.swizzle.map(|element| element) }
    }
}

/// `struct wc_nearest_sampler` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct NearestSamplerMirror {
    pub(crate) unit: u32,
    pub(crate) wrap: [u8; 3],
    pub(crate) mipmapped: u8,
    pub(crate) min_lod: f32,
    pub(crate) max_lod: f32,
    pub(crate) border: [u32; 4],
}

impl From<&NearestSampler> for NearestSamplerMirror {
    fn from(value: &NearestSampler) -> NearestSamplerMirror {
        NearestSamplerMirror { unit: value.unit, wrap: value.wrap.map(|element| element), mipmapped: u8::from(value.mipmapped), min_lod: value.min_lod, max_lod: value.max_lod, border: value.border.map(|element| element) }
    }
}

/// `struct wc_stream_output` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct StreamOutputMirror {
    pub(crate) buffer: u32,
    pub(crate) offset: u32,
    pub(crate) location: u32,
    pub(crate) position: u8,
    pub(crate) first_component: u8,
    pub(crate) components: u8,
}

impl StreamOutputMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> StreamOutput {
        StreamOutput { buffer: self.buffer, offset: self.offset, location: self.location, position: self.position != 0, first_component: self.first_component, components: self.components }
    }
}

impl From<&StreamOutput> for StreamOutputMirror {
    fn from(value: &StreamOutput) -> StreamOutputMirror {
        StreamOutputMirror { buffer: value.buffer, offset: value.offset, location: value.location, position: u8::from(value.position), first_component: value.first_component, components: value.components }
    }
}

// ----- The bridge's functions. -----

#[expect(unsafe_code, reason = "bridge.h declares these functions; each is reached only through the method that marks its call")]
unsafe extern "C" {
    fn wc_nir_flatten(nir: *mut c_void, points_in: u32, out: *mut FlatMirror) -> u8;
    fn wc_flat_free(flat: *mut FlatMirror);
    fn wc_init();
    fn wc_spirv_to_nir(words: *const u32, count: usize, stage: u32, entry_point: *const core::ffi::c_char) -> *mut c_void;
    fn wc_glsl_to_nir(sources: *const *const core::ffi::c_char, out: *mut GlslProgramMirror) -> u8;
    fn wc_glsl_program_free(program: *mut GlslProgramMirror);
    fn wc_nir_to_common_form(nir: *mut c_void, set: u32, ranges: *const SlotRangeMirror, out: *mut BindingsMirror) -> u32;
    fn wc_nir_lower_io(nir: *mut c_void);
    fn wc_nir_alpha_to_one(nir: *mut c_void);
    fn wc_nir_serialize(nir: *const c_void, out: *mut BytesMirror) -> u8;
    fn wc_nir_to_spirv(nir: *mut c_void, out: *mut WordsMirror) -> u8;
    fn wc_nir_clone(nir: *const c_void) -> *mut c_void;
    fn wc_nir_number_vertex_elements(nir: *mut c_void) -> u64;
    fn wc_nir_geometry_vertices_out(nir: *const c_void) -> u32;
    fn wc_nir_named_layer_location(nir: *const c_void) -> u32;
    fn wc_nir_control_points_out(nir: *const c_void) -> u32;
    fn wc_nir_makes_points(nir: *const c_void) -> u8;
    fn wc_nir_fragcolor_to_buffers(nir: *mut c_void, buffers: u32);
    fn wc_nir_split_resource_arrays(nir: *mut c_void);
    fn wc_nir_textures_1d_as_2d(nir: *mut c_void);
    fn wc_nir_drop_name(nir: *mut c_void);
    fn wc_nir_uniform_blocks_as_words(nir: *mut c_void);
    fn wc_nir_transform_position(nir: *mut c_void, transform: PositionTransformMirror);
    fn wc_nir_reads_sample_positions(nir: *mut c_void) -> u8;
    fn wc_nir_interpolate_elsewhere(nir: *mut c_void, samples: u32) -> u8;
    fn wc_nir_fit_sample_masks(nir: *mut c_void, single_sampled: u8);
    fn wc_nir_writes_sample_mask(nir: *mut c_void) -> u8;
    fn wc_nir_fetches_target(nir: *mut c_void) -> u8;
    fn wc_nir_formatless_images(nir: *mut c_void) -> u64;
    fn wc_nir_fetch_from_texture(nir: *mut c_void, unit: u32, multisampled: u8);
    fn wc_nir_swizzle_textures(nir: *mut c_void, swizzles: *const TextureSwizzleMirror, count: usize);
    fn wc_nir_nearest_integer_textures(nir: *mut c_void, samplers: *const NearestSamplerMirror, count: usize) -> u8;
    fn wc_nir_integer_sampled_units(nir: *mut c_void) -> u64;
    fn wc_nir_stream_point_size_at_a_location(nir: *mut c_void);
    fn wc_nir_generic_end(nir: *const c_void) -> u32;
    fn wc_nir_point_size_also_at(nir: *mut c_void, at: u32) -> u8;
    fn wc_nir_point_size_at_most(nir: *const c_void, size: f32) -> u8;
    fn wc_nir_point_square(vs: *const c_void, at: u32, largest: f32) -> *mut c_void;
    fn wc_nir_geometry_squares(gs: *const c_void, at: u32, largest: f32, block: u32) -> *mut c_void;
    fn wc_nir_storage_pass_through(last: *mut c_void, made: *mut *mut c_void) -> u8;
    fn wc_nir_stream_outputs(nir: *const c_void, outputs: *mut StreamOutputMirror, capacity: u32, strides: *mut u32, buffers: u32) -> u32;
    fn wc_nir_free(nir: *mut c_void);
    fn wc_bytes_free(bytes: *mut BytesMirror);
    fn wc_words_free(words: *mut WordsMirror);
    fn wc_bindings_free(bindings: *mut BindingsMirror);
}

impl wcontainers_mesa_compiler::MesaCompiler for Bridge {
    type Shader = Shader;

    fn new() -> Bridge {
        Bridge::started()
    }

    fn flatten(&self, nir: &mut Shader, points_in: u32) -> Option<Flat> {
        let mut out = FlatMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_flatten takes the types declared there; nir is a shader of ours, changed by the call alone; out is one zeroed struct wc_flat of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_flatten(nir.nir.as_ptr(), points_in, &raw mut out) };
        let result = result != 0;
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_flat_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_flat_free(&raw mut out) };
        result.then_some(copied)
    }

    fn spirv_to_nir(&self, words: &[u32], stage: u32, entry_point: &core::ffi::CStr) -> Option<Shader> {
        if stage >= 3 { return Default::default(); }
        #[expect(unsafe_code, reason = "bridge.h: wc_spirv_to_nir takes the types declared there; words holds as many words as the count beside it; count is words's length; stage is one of enum wc_stage's 3 values; entry_point is zero-ended")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_spirv_to_nir(words.as_ptr(), words.len(), stage, entry_point.as_ptr()) };
        Shader::owning(result)
    }

    fn glsl_to_nir(&self, sources: [Option<&core::ffi::CStr>; 3]) -> Result<GlslProgram<Shader>, Vec<u8>> {
        let sources: [*const core::ffi::c_char; 3] = sources.map(|text| text.map_or(core::ptr::null(), core::ffi::CStr::as_ptr));
        let mut out = GlslProgramMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_glsl_to_nir takes the types declared there; sources is 3 texts of ours, each zero-ended or null; out is one zeroed struct wc_glsl_program of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_glsl_to_nir(sources.as_ptr(), &raw mut out) };
        let result = result != 0;
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_glsl_program_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_glsl_program_free(&raw mut out) };
        if result { Ok(copied) } else { Err(copied.log.unwrap_or_default()) }
    }

    fn to_common_form(&self, nir: &mut Shader, set: u32, ranges: &[SlotRange; 4]) -> Result<Vec<Binding>, u32> {
        let ranges: [SlotRangeMirror; 4] = ranges.each_ref().map(SlotRangeMirror::from);
        let mut out = BindingsMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_to_common_form takes the types declared there; nir is a shader of ours, changed by the call alone; ranges is 4 elements of ours, as the header's array length says; out is one zeroed struct wc_bindings of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_to_common_form(nir.nir.as_ptr(), set, ranges.as_ptr(), &raw mut out) };
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_bindings_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_bindings_free(&raw mut out) };
        if result == 0 { Ok(copied) } else { Err(result) }
    }

    fn lower_io(&self, nir: &mut Shader) {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_lower_io takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_lower_io(nir.nir.as_ptr()) };
    }

    fn alpha_to_one(&self, nir: &mut Shader) {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_alpha_to_one takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_alpha_to_one(nir.nir.as_ptr()) };
    }

    fn serialize(&self, nir: &Shader) -> Option<Vec<u8>> {
        let mut out = BytesMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_serialize takes the types declared there; nir is a shader of ours, read by the call alone; out is one zeroed struct wc_bytes of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_serialize(nir.nir.as_ptr(), &raw mut out) };
        let result = result != 0;
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_bytes_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_bytes_free(&raw mut out) };
        result.then_some(copied)
    }

    fn to_spirv(&self, nir: &mut Shader) -> Option<Vec<u32>> {
        let mut out = WordsMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_to_spirv takes the types declared there; nir is a shader of ours, changed by the call alone; out is one zeroed struct wc_words of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_to_spirv(nir.nir.as_ptr(), &raw mut out) };
        let result = result != 0;
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_words_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_words_free(&raw mut out) };
        result.then_some(copied)
    }

    fn cloned(&self, nir: &Shader) -> Option<Shader> {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_clone takes the types declared there; nir is a shader of ours, read by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_clone(nir.nir.as_ptr()) };
        Shader::owning(result)
    }

    fn number_vertex_elements(&self, nir: &mut Shader) -> u64 {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_number_vertex_elements takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_number_vertex_elements(nir.nir.as_ptr()) }
    }

    fn geometry_vertices_out(&self, nir: &Shader) -> u32 {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_geometry_vertices_out takes the types declared there; nir is a shader of ours, read by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_geometry_vertices_out(nir.nir.as_ptr()) }
    }

    fn named_layer_location(&self, nir: &Shader) -> u32 {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_named_layer_location takes the types declared there; nir is a shader of ours, read by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_named_layer_location(nir.nir.as_ptr()) }
    }

    fn control_points_out(&self, nir: &Shader) -> u32 {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_control_points_out takes the types declared there; nir is a shader of ours, read by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_control_points_out(nir.nir.as_ptr()) }
    }

    fn makes_points(&self, nir: &Shader) -> bool {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_makes_points takes the types declared there; nir is a shader of ours, read by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_makes_points(nir.nir.as_ptr()) };
        result != 0
    }

    fn fragcolor_to_buffers(&self, nir: &mut Shader, buffers: u32) {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_fragcolor_to_buffers takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_fragcolor_to_buffers(nir.nir.as_ptr(), buffers) };
    }

    fn split_resource_arrays(&self, nir: &mut Shader) {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_split_resource_arrays takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_split_resource_arrays(nir.nir.as_ptr()) };
    }

    fn textures_1d_as_2d(&self, nir: &mut Shader) {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_textures_1d_as_2d takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_textures_1d_as_2d(nir.nir.as_ptr()) };
    }

    fn drop_name(&self, nir: &mut Shader) {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_drop_name takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_drop_name(nir.nir.as_ptr()) };
    }

    fn uniform_blocks_as_words(&self, nir: &mut Shader) {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_uniform_blocks_as_words takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_uniform_blocks_as_words(nir.nir.as_ptr()) };
    }

    fn transform_position(&self, nir: &mut Shader, transform: PositionTransform) {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_transform_position takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_transform_position(nir.nir.as_ptr(), PositionTransformMirror::from(&transform)) };
    }

    fn reads_sample_positions(&self, nir: &mut Shader) -> bool {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_reads_sample_positions takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_reads_sample_positions(nir.nir.as_ptr()) };
        result != 0
    }

    fn interpolate_elsewhere(&self, nir: &mut Shader, samples: u32) -> bool {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_interpolate_elsewhere takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_interpolate_elsewhere(nir.nir.as_ptr(), samples) };
        result != 0
    }

    fn fit_sample_masks(&self, nir: &mut Shader, single_sampled: bool) {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_fit_sample_masks takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_fit_sample_masks(nir.nir.as_ptr(), u8::from(single_sampled)) };
    }

    fn writes_sample_mask(&self, nir: &mut Shader) -> bool {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_writes_sample_mask takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_writes_sample_mask(nir.nir.as_ptr()) };
        result != 0
    }

    fn fetches_target(&self, nir: &mut Shader) -> bool {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_fetches_target takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_fetches_target(nir.nir.as_ptr()) };
        result != 0
    }

    fn formatless_images(&self, nir: &mut Shader) -> u64 {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_formatless_images takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_formatless_images(nir.nir.as_ptr()) }
    }

    fn fetch_from_texture(&self, nir: &mut Shader, unit: u32, multisampled: bool) {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_fetch_from_texture takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_fetch_from_texture(nir.nir.as_ptr(), unit, u8::from(multisampled)) };
    }

    fn swizzle_textures(&self, nir: &mut Shader, swizzles: &[TextureSwizzle]) {
        let swizzles: Vec<TextureSwizzleMirror> = swizzles.iter().map(TextureSwizzleMirror::from).collect();
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_swizzle_textures takes the types declared there; nir is a shader of ours, changed by the call alone; swizzles holds as many elements as the count beside it, in our memory; count is swizzles's length")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_swizzle_textures(nir.nir.as_ptr(), swizzles.as_ptr(), swizzles.len()) };
    }

    fn nearest_integer_textures(&self, nir: &mut Shader, samplers: &[NearestSampler]) -> bool {
        let samplers: Vec<NearestSamplerMirror> = samplers.iter().map(NearestSamplerMirror::from).collect();
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_nearest_integer_textures takes the types declared there; nir is a shader of ours, changed by the call alone; samplers holds as many elements as the count beside it, in our memory; count is samplers's length")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_nearest_integer_textures(nir.nir.as_ptr(), samplers.as_ptr(), samplers.len()) };
        result != 0
    }

    fn integer_sampled_units(&self, nir: &mut Shader) -> u64 {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_integer_sampled_units takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_integer_sampled_units(nir.nir.as_ptr()) }
    }

    fn stream_point_size_at_a_location(&self, nir: &mut Shader) {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_stream_point_size_at_a_location takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_stream_point_size_at_a_location(nir.nir.as_ptr()) };
    }

    fn generic_end(&self, nir: &Shader) -> u32 {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_generic_end takes the types declared there; nir is a shader of ours, read by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_nir_generic_end(nir.nir.as_ptr()) }
    }

    fn point_size_also_at(&self, nir: &mut Shader, at: u32) -> bool {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_point_size_also_at takes the types declared there; nir is a shader of ours, changed by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_point_size_also_at(nir.nir.as_ptr(), at) };
        result != 0
    }

    fn point_size_at_most(&self, nir: &Shader, size: f32) -> bool {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_point_size_at_most takes the types declared there; nir is a shader of ours, read by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_point_size_at_most(nir.nir.as_ptr(), size) };
        result != 0
    }

    fn point_square(&self, vs: &Shader, at: u32, largest: f32) -> Option<Shader> {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_point_square takes the types declared there; vs is a shader of ours, read by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_point_square(vs.nir.as_ptr(), at, largest) };
        Shader::owning(result)
    }

    fn geometry_squares(&self, gs: &Shader, at: u32, largest: f32, block: u32) -> Option<Shader> {
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_geometry_squares takes the types declared there; gs is a shader of ours, read by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_geometry_squares(gs.nir.as_ptr(), at, largest, block) };
        Shader::owning(result)
    }

    fn storage_pass_through(&self, last: &mut Shader) -> Option<[Option<Shader>; 5]> {
        let mut made = [core::ptr::null_mut::<c_void>(); 5];
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_storage_pass_through takes the types declared there; last is a shader of ours, changed by the call alone; made is room for 5 shaders, ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_storage_pass_through(last.nir.as_ptr(), made.as_mut_ptr()) };
        let result = result != 0;
        let made = made.map(Shader::owning);
        result.then_some(made)
    }

    fn stream_outputs(&self, nir: &Shader, outputs: &mut [StreamOutput], strides: &mut [u32]) -> u32 {
        let mut outputs_room: Vec<StreamOutputMirror> = vec![StreamOutputMirror::default(); outputs.len()];
        let Ok(capacity) = u32::try_from(outputs.len()) else { return Default::default(); };
        let Ok(buffers) = u32::try_from(strides.len()) else { return Default::default(); };
        #[expect(unsafe_code, reason = "bridge.h: wc_nir_stream_outputs takes the types declared there; nir is a shader of ours, read by the call alone; outputs_room holds as many elements as the capacity beside it, in our memory; capacity is outputs's length; strides holds as many words as the count beside it; buffers is strides's length")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_nir_stream_outputs(nir.nir.as_ptr(), outputs_room.as_mut_ptr(), capacity, strides.as_mut_ptr(), buffers) };
        for (to, from) in outputs.iter_mut().zip(outputs_room.iter_mut()) { *to = from.owned(); }
        result
    }

}
