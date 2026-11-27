// Written by tools/api-crate-generator from the Mesa compiler bridge's bridge.h and flat.h (crates/mesa-compiler/implementation/direct/bridge). Do not edit.
//! Mesa's compiler through our C bridge (crates/mesa-compiler/implementation/direct/bridge): every bridge function a safe method of
//! wcontainers-mesa-compiler's trait, each shader a value of the type of its place in the bridge's typed chain, owning
//! one NIR shader and giving it back when dropped, and every structure the bridge fills copied out whole into owned
//! values before the bridge's free.
//!
//! What the marks rely on: the bridge's functions have the types bridge.h and flat.h declare; a function reads and
//! writes only what its parameters name, as far as the count beside each says (or the header's array length); the
//! arrays and texts it fills are its own until the matching free, which takes a zeroed structure as nothing; a shader
//! it returns or fills is a shader it made, of the type it names, given back once by that type's free; a function
//! taking a shader named `taken` gives it back itself; Mesa's instruction names are static. The bridge's C is built
//! and linked by this crate's build.rs.

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
        FlatSrcMirror { def: value.def, swizzle: value.swizzle.each_ref().map(|element| *element) }
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

/// `struct wc_spirv_common_form` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct SpirvCommonFormMirror {
    pub(crate) shader: *mut c_void,
    pub(crate) bindings: BindingsMirror,
    pub(crate) result: u32,
}

impl Default for SpirvCommonFormMirror {
    fn default() -> SpirvCommonFormMirror {
        SpirvCommonFormMirror { shader: core::ptr::null_mut(), bindings: BindingsMirror::default(), result: 0 }
    }
}

impl SpirvCommonFormMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> SpirvCommonForm<SpirvCommon> {
        SpirvCommonForm { shader: SpirvCommon::owning(core::mem::replace(&mut self.shader, core::ptr::null_mut())), bindings: self.bindings.owned(), result: self.result }
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
    pub(crate) stages: [*mut c_void; 6],
    pub(crate) names: [GlslNamesMirror; 6],
    pub(crate) log: *mut u8,
}

impl Default for GlslProgramMirror {
    fn default() -> GlslProgramMirror {
        GlslProgramMirror { stages: [core::ptr::null_mut(); 6], names: [GlslNamesMirror::default(); 6], log: core::ptr::null_mut() }
    }
}

impl GlslProgramMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> GlslProgram<ReceivedVertex, ReceivedFragment, ReceivedCompute, ReceivedGeometry, ReceivedTessControl, ReceivedTessEvaluation> {
        GlslProgram { vertex: ReceivedVertex::owning(core::mem::replace(&mut self.stages[0], core::ptr::null_mut())), fragment: ReceivedFragment::owning(core::mem::replace(&mut self.stages[1], core::ptr::null_mut())), compute: ReceivedCompute::owning(core::mem::replace(&mut self.stages[2], core::ptr::null_mut())), geometry: ReceivedGeometry::owning(core::mem::replace(&mut self.stages[3], core::ptr::null_mut())), tess_control: ReceivedTessControl::owning(core::mem::replace(&mut self.stages[4], core::ptr::null_mut())), tess_evaluation: ReceivedTessEvaluation::owning(core::mem::replace(&mut self.stages[5], core::ptr::null_mut())), names: self.names.each_mut().map(|element| element.owned()), log: text(self.log) }
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

/// `struct wc_prepared_form` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct PreparedFormMirror {
    pub(crate) shader: *mut c_void,
    pub(crate) key: BytesMirror,
    pub(crate) streamed: *mut StreamOutputMirror,
    pub(crate) streamed_count: u32,
    pub(crate) strides: [u32; 4],
    pub(crate) streams_elsewhere: u8,
    pub(crate) reads_sample_positions: u8,
    pub(crate) writes_sample_mask: u8,
    pub(crate) fetches_target: u8,
    pub(crate) formatless_images: u64,
    pub(crate) integer_sampled_units: u64,
    pub(crate) named_layer: u32,
    pub(crate) passes_through: u8,
    pub(crate) pass_through: [*mut c_void; 5],
    pub(crate) generic_end: u32,
    pub(crate) geometry_vertices_out: u32,
    pub(crate) control_points_out: u32,
    pub(crate) makes_points: u8,
}

impl Default for PreparedFormMirror {
    fn default() -> PreparedFormMirror {
        PreparedFormMirror { shader: core::ptr::null_mut(), key: BytesMirror::default(), streamed: core::ptr::null_mut(), streamed_count: 0, strides: [0; 4], streams_elsewhere: 0, reads_sample_positions: 0, writes_sample_mask: 0, fetches_target: 0, formatless_images: 0, integer_sampled_units: 0, named_layer: 0, passes_through: 0, pass_through: [core::ptr::null_mut(); 5], generic_end: 0, geometry_vertices_out: 0, control_points_out: 0, makes_points: 0 }
    }
}

impl PreparedFormMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned<Prepared>(&mut self, owning_prepared: fn(*mut c_void) -> Option<Prepared>) -> PreparedForm<Prepared, Made> {
        PreparedForm { shader: owning_prepared(core::mem::replace(&mut self.shader, core::ptr::null_mut())), key: self.key.owned(), streamed: array(self.streamed, self.streamed_count).iter_mut().map(|element| element.owned()).collect(), strides: self.strides.each_mut().map(|element| *element), streams_elsewhere: self.streams_elsewhere != 0, reads_sample_positions: self.reads_sample_positions != 0, writes_sample_mask: self.writes_sample_mask != 0, fetches_target: self.fetches_target != 0, formatless_images: self.formatless_images, integer_sampled_units: self.integer_sampled_units, named_layer: self.named_layer, passes_through: self.passes_through != 0, pass_through: self.pass_through.each_mut().map(|element| Made::owning(core::mem::replace(element, core::ptr::null_mut()))), generic_end: self.generic_end, geometry_vertices_out: self.geometry_vertices_out, control_points_out: self.control_points_out, makes_points: self.makes_points != 0 }
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

/// `struct wc_variant_settings` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct VariantSettingsMirror {
    pub(crate) transform: PositionTransformMirror,
    pub(crate) key_transform: PositionTransformMirror,
    pub(crate) fetch_unit: u32,
    pub(crate) fetch_multisampled: u8,
    pub(crate) single_sampled: u8,
    pub(crate) samples: u32,
    pub(crate) set: u32,
    pub(crate) ranges: [SlotRangeMirror; 4],
}

impl From<&VariantSettings> for VariantSettingsMirror {
    fn from(value: &VariantSettings) -> VariantSettingsMirror {
        VariantSettingsMirror { transform: PositionTransformMirror::from(&value.transform), key_transform: PositionTransformMirror::from(&value.key_transform), fetch_unit: value.fetch_unit, fetch_multisampled: u8::from(value.fetch_multisampled), single_sampled: u8::from(value.single_sampled), samples: value.samples, set: value.set, ranges: value.ranges.each_ref().map(SlotRangeMirror::from) }
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
        TextureSwizzleMirror { unit: value.unit, swizzle: value.swizzle.each_ref().map(|element| *element) }
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
        NearestSamplerMirror { unit: value.unit, wrap: value.wrap.each_ref().map(|element| *element), mipmapped: u8::from(value.mipmapped), min_lod: value.min_lod, max_lod: value.max_lod, border: value.border.each_ref().map(|element| *element) }
    }
}

/// `struct wc_variant_form` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct VariantFormMirror {
    pub(crate) shader: *mut c_void,
    pub(crate) result: u32,
    pub(crate) key: BytesMirror,
    pub(crate) bindings: BindingsMirror,
}

impl Default for VariantFormMirror {
    fn default() -> VariantFormMirror {
        VariantFormMirror { shader: core::ptr::null_mut(), result: 0, key: BytesMirror::default(), bindings: BindingsMirror::default() }
    }
}

impl VariantFormMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> VariantForm<Variant> {
        VariantForm { shader: Variant::owning(core::mem::replace(&mut self.shader, core::ptr::null_mut())), result: self.result, key: self.key.owned(), bindings: self.bindings.owned() }
    }
}

/// `struct wc_squares` as the bridge lays it out.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct SquaresMirror {
    pub(crate) kind: u32,
    pub(crate) sized: u8,
    pub(crate) before: *mut c_void,
    pub(crate) geometry: *mut c_void,
}

impl Default for SquaresMirror {
    fn default() -> SquaresMirror {
        SquaresMirror { kind: 0, sized: 0, before: core::ptr::null_mut(), geometry: core::ptr::null_mut() }
    }
}

impl SquaresMirror {
    /// The values read out (the bridge's arrays and texts copied, its shaders taken).
    pub(crate) fn owned(&mut self) -> Squares<Made> {
        Squares { kind: self.kind, sized: self.sized != 0, before: Made::owning(core::mem::replace(&mut self.before, core::ptr::null_mut())), geometry: Made::owning(core::mem::replace(&mut self.geometry, core::ptr::null_mut())) }
    }
}

// ----- The bridge's functions. -----

#[expect(unsafe_code, reason = "bridge.h declares these functions; each is reached only through the method that marks its call")]
unsafe extern "C" {
    fn wc_flat_free(flat: *mut FlatMirror);
    fn wc_init();
    fn wc_spirv_read(words: *const u32, count: usize, stage: u32, entry_point: *const core::ffi::c_char, alpha_to_one: u8) -> *mut c_void;
    fn wc_spirv_shader_key(shader: *const c_void, out: *mut BytesMirror) -> u8;
    fn wc_spirv_to_common_form(taken: *mut c_void, set: u32, ranges: *const SlotRangeMirror, out: *mut SpirvCommonFormMirror);
    fn wc_spirv_common_form_free(form: *mut SpirvCommonFormMirror);
    fn wc_spirv_common_flatten(taken: *mut c_void, out: *mut FlatMirror) -> u8;
    fn wc_glsl_to_nir(sources: *const *const core::ffi::c_char, out: *mut GlslProgramMirror) -> u8;
    fn wc_glsl_program_free(program: *mut GlslProgramMirror);
    fn wc_received_prepare(taken: *mut c_void, out: *mut PreparedFormMirror);
    fn wc_prepared_form_free(form: *mut PreparedFormMirror);
    fn wc_prepared_variant(shader: *const c_void, settings: VariantSettingsMirror, swizzles: *const TextureSwizzleMirror, swizzle_count: usize, samplers: *const NearestSamplerMirror, sampler_count: usize, out: *mut VariantFormMirror);
    fn wc_variant_form_free(form: *mut VariantFormMirror);
    fn wc_prepared_points_as_squares(vs: *const c_void, at: u32, largest: f32, out: *mut SquaresMirror);
    fn wc_prepared_geometry_points_as_squares(gs: *const c_void, at: u32, largest: f32, block: u32, out: *mut SquaresMirror);
    fn wc_squares_free(squares: *mut SquaresMirror);
    fn wc_made_key(shader: *const c_void, out: *mut BytesMirror) -> u8;
    fn wc_made_named_layer_location(shader: *const c_void) -> u32;
    fn wc_made_variant(shader: *const c_void, settings: VariantSettingsMirror, swizzles: *const TextureSwizzleMirror, swizzle_count: usize, samplers: *const NearestSamplerMirror, sampler_count: usize, out: *mut VariantFormMirror);
    fn wc_variant_flatten(taken: *mut c_void, points_in: u32, out: *mut FlatMirror) -> u8;
    fn wc_spirv_shader_free(shader: *mut c_void);
    fn wc_spirv_common_free(shader: *mut c_void);
    fn wc_received_free(shader: *mut c_void);
    fn wc_prepared_free(shader: *mut c_void);
    fn wc_made_free(shader: *mut c_void);
    fn wc_variant_free(shader: *mut c_void);
    fn wc_bytes_free(bytes: *mut BytesMirror);
}

/// A `struct wc_spirv_shader` the bridge made, given back by `wc_spirv_shader_free` when dropped. Not `Send`: no fact of the
/// bridge's is relied on for a shader moved between threads.
#[derive(Debug)]
pub struct SpirvShader {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl SpirvShader {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<SpirvShader> {
        NonNull::new(nir).map(|nir| SpirvShader { nir, _thread: PhantomData })
    }

    /// The shader, given to a bridge function that takes it (and gives it back itself): not dropped here.
    fn into_raw(self) -> *mut c_void {
        let kept = core::mem::ManuallyDrop::new(self);
        kept.nir.as_ptr()
    }
}

impl Drop for SpirvShader {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_spirv_shader_free gives back a wc_spirv_shader the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_spirv_shader_free(self.nir.as_ptr()) };
    }
}

/// A `struct wc_spirv_common` the bridge made, given back by `wc_spirv_common_free` when dropped. Not `Send`: no fact of the
/// bridge's is relied on for a shader moved between threads.
#[derive(Debug)]
pub struct SpirvCommon {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl SpirvCommon {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<SpirvCommon> {
        NonNull::new(nir).map(|nir| SpirvCommon { nir, _thread: PhantomData })
    }

    /// The shader, given to a bridge function that takes it (and gives it back itself): not dropped here.
    fn into_raw(self) -> *mut c_void {
        let kept = core::mem::ManuallyDrop::new(self);
        kept.nir.as_ptr()
    }
}

impl Drop for SpirvCommon {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_spirv_common_free gives back a wc_spirv_common the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_spirv_common_free(self.nir.as_ptr()) };
    }
}

/// A `struct wc_received` of the vertex stage the bridge made, given back by `wc_received_free` when dropped. Not `Send`: no fact of the
/// bridge's is relied on for a shader moved between threads.
#[derive(Debug)]
pub struct ReceivedVertex {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl ReceivedVertex {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<ReceivedVertex> {
        NonNull::new(nir).map(|nir| ReceivedVertex { nir, _thread: PhantomData })
    }

    /// The shader, given to a bridge function that takes it (and gives it back itself): not dropped here.
    fn into_raw(self) -> *mut c_void {
        let kept = core::mem::ManuallyDrop::new(self);
        kept.nir.as_ptr()
    }
}

impl Drop for ReceivedVertex {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_received_free gives back a wc_received the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_received_free(self.nir.as_ptr()) };
    }
}

/// A `struct wc_received` of the fragment stage the bridge made, given back by `wc_received_free` when dropped. Not `Send`: no fact of the
/// bridge's is relied on for a shader moved between threads.
#[derive(Debug)]
pub struct ReceivedFragment {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl ReceivedFragment {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<ReceivedFragment> {
        NonNull::new(nir).map(|nir| ReceivedFragment { nir, _thread: PhantomData })
    }

    /// The shader, given to a bridge function that takes it (and gives it back itself): not dropped here.
    fn into_raw(self) -> *mut c_void {
        let kept = core::mem::ManuallyDrop::new(self);
        kept.nir.as_ptr()
    }
}

impl Drop for ReceivedFragment {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_received_free gives back a wc_received the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_received_free(self.nir.as_ptr()) };
    }
}

/// A `struct wc_received` of the compute stage the bridge made, given back by `wc_received_free` when dropped. Not `Send`: no fact of the
/// bridge's is relied on for a shader moved between threads.
#[derive(Debug)]
pub struct ReceivedCompute {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl ReceivedCompute {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<ReceivedCompute> {
        NonNull::new(nir).map(|nir| ReceivedCompute { nir, _thread: PhantomData })
    }

    /// The shader, given to a bridge function that takes it (and gives it back itself): not dropped here.
    fn into_raw(self) -> *mut c_void {
        let kept = core::mem::ManuallyDrop::new(self);
        kept.nir.as_ptr()
    }
}

impl Drop for ReceivedCompute {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_received_free gives back a wc_received the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_received_free(self.nir.as_ptr()) };
    }
}

/// A `struct wc_received` of the geometry stage the bridge made, given back by `wc_received_free` when dropped. Not `Send`: no fact of the
/// bridge's is relied on for a shader moved between threads.
#[derive(Debug)]
pub struct ReceivedGeometry {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl ReceivedGeometry {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<ReceivedGeometry> {
        NonNull::new(nir).map(|nir| ReceivedGeometry { nir, _thread: PhantomData })
    }

    /// The shader, given to a bridge function that takes it (and gives it back itself): not dropped here.
    fn into_raw(self) -> *mut c_void {
        let kept = core::mem::ManuallyDrop::new(self);
        kept.nir.as_ptr()
    }
}

impl Drop for ReceivedGeometry {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_received_free gives back a wc_received the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_received_free(self.nir.as_ptr()) };
    }
}

/// A `struct wc_received` of the tess control stage the bridge made, given back by `wc_received_free` when dropped. Not `Send`: no fact of the
/// bridge's is relied on for a shader moved between threads.
#[derive(Debug)]
pub struct ReceivedTessControl {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl ReceivedTessControl {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<ReceivedTessControl> {
        NonNull::new(nir).map(|nir| ReceivedTessControl { nir, _thread: PhantomData })
    }

    /// The shader, given to a bridge function that takes it (and gives it back itself): not dropped here.
    fn into_raw(self) -> *mut c_void {
        let kept = core::mem::ManuallyDrop::new(self);
        kept.nir.as_ptr()
    }
}

impl Drop for ReceivedTessControl {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_received_free gives back a wc_received the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_received_free(self.nir.as_ptr()) };
    }
}

/// A `struct wc_received` of the tess evaluation stage the bridge made, given back by `wc_received_free` when dropped. Not `Send`: no fact of the
/// bridge's is relied on for a shader moved between threads.
#[derive(Debug)]
pub struct ReceivedTessEvaluation {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl ReceivedTessEvaluation {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<ReceivedTessEvaluation> {
        NonNull::new(nir).map(|nir| ReceivedTessEvaluation { nir, _thread: PhantomData })
    }

    /// The shader, given to a bridge function that takes it (and gives it back itself): not dropped here.
    fn into_raw(self) -> *mut c_void {
        let kept = core::mem::ManuallyDrop::new(self);
        kept.nir.as_ptr()
    }
}

impl Drop for ReceivedTessEvaluation {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_received_free gives back a wc_received the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_received_free(self.nir.as_ptr()) };
    }
}

/// A `struct wc_prepared` of the vertex stage the bridge made, given back by `wc_prepared_free` when dropped. Not `Send`: no fact of the
/// bridge's is relied on for a shader moved between threads.
#[derive(Debug)]
pub struct PreparedVertex {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl PreparedVertex {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<PreparedVertex> {
        NonNull::new(nir).map(|nir| PreparedVertex { nir, _thread: PhantomData })
    }
}

impl Drop for PreparedVertex {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_free gives back a wc_prepared the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_prepared_free(self.nir.as_ptr()) };
    }
}

/// A `struct wc_prepared` of the fragment stage the bridge made, given back by `wc_prepared_free` when dropped. Not `Send`: no fact of the
/// bridge's is relied on for a shader moved between threads.
#[derive(Debug)]
pub struct PreparedFragment {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl PreparedFragment {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<PreparedFragment> {
        NonNull::new(nir).map(|nir| PreparedFragment { nir, _thread: PhantomData })
    }
}

impl Drop for PreparedFragment {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_free gives back a wc_prepared the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_prepared_free(self.nir.as_ptr()) };
    }
}

/// A `struct wc_prepared` of the compute stage the bridge made, given back by `wc_prepared_free` when dropped. Not `Send`: no fact of the
/// bridge's is relied on for a shader moved between threads.
#[derive(Debug)]
pub struct PreparedCompute {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl PreparedCompute {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<PreparedCompute> {
        NonNull::new(nir).map(|nir| PreparedCompute { nir, _thread: PhantomData })
    }
}

impl Drop for PreparedCompute {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_free gives back a wc_prepared the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_prepared_free(self.nir.as_ptr()) };
    }
}

/// A `struct wc_prepared` of the geometry stage the bridge made, given back by `wc_prepared_free` when dropped. Not `Send`: no fact of the
/// bridge's is relied on for a shader moved between threads.
#[derive(Debug)]
pub struct PreparedGeometry {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl PreparedGeometry {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<PreparedGeometry> {
        NonNull::new(nir).map(|nir| PreparedGeometry { nir, _thread: PhantomData })
    }
}

impl Drop for PreparedGeometry {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_free gives back a wc_prepared the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_prepared_free(self.nir.as_ptr()) };
    }
}

/// A `struct wc_prepared` of the tess control stage the bridge made, given back by `wc_prepared_free` when dropped. Not `Send`: no fact of the
/// bridge's is relied on for a shader moved between threads.
#[derive(Debug)]
pub struct PreparedTessControl {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl PreparedTessControl {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<PreparedTessControl> {
        NonNull::new(nir).map(|nir| PreparedTessControl { nir, _thread: PhantomData })
    }
}

impl Drop for PreparedTessControl {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_free gives back a wc_prepared the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_prepared_free(self.nir.as_ptr()) };
    }
}

/// A `struct wc_prepared` of the tess evaluation stage the bridge made, given back by `wc_prepared_free` when dropped. Not `Send`: no fact of the
/// bridge's is relied on for a shader moved between threads.
#[derive(Debug)]
pub struct PreparedTessEvaluation {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl PreparedTessEvaluation {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<PreparedTessEvaluation> {
        NonNull::new(nir).map(|nir| PreparedTessEvaluation { nir, _thread: PhantomData })
    }
}

impl Drop for PreparedTessEvaluation {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_free gives back a wc_prepared the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_prepared_free(self.nir.as_ptr()) };
    }
}

/// A `struct wc_made` the bridge made, given back by `wc_made_free` when dropped. Not `Send`: no fact of the
/// bridge's is relied on for a shader moved between threads.
#[derive(Debug)]
pub struct Made {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl Made {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<Made> {
        NonNull::new(nir).map(|nir| Made { nir, _thread: PhantomData })
    }
}

impl Drop for Made {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_made_free gives back a wc_made the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_made_free(self.nir.as_ptr()) };
    }
}

/// A `struct wc_variant` the bridge made, given back by `wc_variant_free` when dropped. Not `Send`: no fact of the
/// bridge's is relied on for a shader moved between threads.
#[derive(Debug)]
pub struct Variant {
    nir: NonNull<c_void>,
    _thread: PhantomData<*const ()>,
}

impl Variant {
    /// Takes a shader the bridge made; None for null.
    fn owning(nir: *mut c_void) -> Option<Variant> {
        NonNull::new(nir).map(|nir| Variant { nir, _thread: PhantomData })
    }

    /// The shader, given to a bridge function that takes it (and gives it back itself): not dropped here.
    fn into_raw(self) -> *mut c_void {
        let kept = core::mem::ManuallyDrop::new(self);
        kept.nir.as_ptr()
    }
}

impl Drop for Variant {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "bridge.h: wc_variant_free gives back a wc_variant the bridge made")]
        // SAFETY: the shader is ours alone (made by the bridge, never shared), given back once, here.
        unsafe { wc_variant_free(self.nir.as_ptr()) };
    }
}


impl wcontainers_mesa_compiler::MesaCompiler for Bridge {
    type SpirvShader = SpirvShader;
    type SpirvCommon = SpirvCommon;
    type ReceivedVertex = ReceivedVertex;
    type ReceivedFragment = ReceivedFragment;
    type ReceivedCompute = ReceivedCompute;
    type ReceivedGeometry = ReceivedGeometry;
    type ReceivedTessControl = ReceivedTessControl;
    type ReceivedTessEvaluation = ReceivedTessEvaluation;
    type PreparedVertex = PreparedVertex;
    type PreparedFragment = PreparedFragment;
    type PreparedCompute = PreparedCompute;
    type PreparedGeometry = PreparedGeometry;
    type PreparedTessControl = PreparedTessControl;
    type PreparedTessEvaluation = PreparedTessEvaluation;
    type Made = Made;
    type Variant = Variant;

    fn new() -> Bridge {
        Bridge::started()
    }

    fn spirv_read(&self, words: &[u32], stage: u32, entry_point: &core::ffi::CStr, alpha_to_one: bool) -> Option<SpirvShader> {
        if stage >= 6 { return Default::default(); }
        #[expect(unsafe_code, reason = "bridge.h: wc_spirv_read takes the types declared there; words holds as many words as the count beside it; count is words's length; stage is one of enum wc_stage's 6 values; entry_point is zero-ended")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_spirv_read(words.as_ptr(), words.len(), stage, entry_point.as_ptr(), u8::from(alpha_to_one)) };
        SpirvShader::owning(result)
    }

    fn spirv_shader_key(&self, shader: &SpirvShader) -> Option<Vec<u8>> {
        let mut out = BytesMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_spirv_shader_key takes the types declared there; shader is a SpirvShader of ours, of the type the function names, read by the call alone; out is one zeroed struct wc_bytes of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_spirv_shader_key(shader.nir.as_ptr(), &raw mut out) };
        let result = result != 0;
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_bytes_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_bytes_free(&raw mut out) };
        result.then_some(copied)
    }

    fn spirv_to_common_form(&self, taken: SpirvShader, set: u32, ranges: &[SlotRange; 4]) -> SpirvCommonForm<SpirvCommon> {
        let ranges: [SlotRangeMirror; 4] = ranges.each_ref().map(SlotRangeMirror::from);
        let mut out = SpirvCommonFormMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_spirv_to_common_form takes the types declared there; taken is a SpirvShader of ours, of the type the function names, which the bridge takes and gives back itself; ranges is 4 elements of ours, as the header's array length says; out is one zeroed struct wc_spirv_common_form of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_spirv_to_common_form(taken.into_raw(), set, ranges.as_ptr(), &raw mut out) };
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_spirv_common_form_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_spirv_common_form_free(&raw mut out) };
        copied
    }

    fn spirv_common_flatten(&self, taken: SpirvCommon) -> Option<Flat> {
        let mut out = FlatMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_spirv_common_flatten takes the types declared there; taken is a SpirvCommon of ours, of the type the function names, which the bridge takes and gives back itself; out is one zeroed struct wc_flat of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_spirv_common_flatten(taken.into_raw(), &raw mut out) };
        let result = result != 0;
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_flat_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_flat_free(&raw mut out) };
        result.then_some(copied)
    }

    fn glsl_to_nir(&self, sources: [Option<&core::ffi::CStr>; 6]) -> Result<GlslProgram<ReceivedVertex, ReceivedFragment, ReceivedCompute, ReceivedGeometry, ReceivedTessControl, ReceivedTessEvaluation>, Vec<u8>> {
        let sources: [*const core::ffi::c_char; 6] = sources.map(|text| text.map_or(core::ptr::null(), core::ffi::CStr::as_ptr));
        let mut out = GlslProgramMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_glsl_to_nir takes the types declared there; sources is 6 texts of ours, each zero-ended or null; out is one zeroed struct wc_glsl_program of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_glsl_to_nir(sources.as_ptr(), &raw mut out) };
        let result = result != 0;
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_glsl_program_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_glsl_program_free(&raw mut out) };
        if result { Ok(copied) } else { Err(copied.log.unwrap_or_default()) }
    }

    fn received_vertex_prepare(&self, taken: ReceivedVertex) -> PreparedForm<PreparedVertex, Made> {
        let mut out = PreparedFormMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_received_prepare takes the types declared there; taken is a ReceivedVertex of ours, of the type the function names, of the stage its type names (wc_glsl_to_nir fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes), which the bridge takes and gives back itself; out is one zeroed struct wc_prepared_form of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_received_prepare(taken.into_raw(), &raw mut out) };
        let copied = out.owned(PreparedVertex::owning);
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_form_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_prepared_form_free(&raw mut out) };
        copied
    }

    fn received_fragment_prepare(&self, taken: ReceivedFragment) -> PreparedForm<PreparedFragment, Made> {
        let mut out = PreparedFormMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_received_prepare takes the types declared there; taken is a ReceivedFragment of ours, of the type the function names, of the stage its type names (wc_glsl_to_nir fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes), which the bridge takes and gives back itself; out is one zeroed struct wc_prepared_form of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_received_prepare(taken.into_raw(), &raw mut out) };
        let copied = out.owned(PreparedFragment::owning);
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_form_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_prepared_form_free(&raw mut out) };
        copied
    }

    fn received_compute_prepare(&self, taken: ReceivedCompute) -> PreparedForm<PreparedCompute, Made> {
        let mut out = PreparedFormMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_received_prepare takes the types declared there; taken is a ReceivedCompute of ours, of the type the function names, of the stage its type names (wc_glsl_to_nir fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes), which the bridge takes and gives back itself; out is one zeroed struct wc_prepared_form of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_received_prepare(taken.into_raw(), &raw mut out) };
        let copied = out.owned(PreparedCompute::owning);
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_form_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_prepared_form_free(&raw mut out) };
        copied
    }

    fn received_geometry_prepare(&self, taken: ReceivedGeometry) -> PreparedForm<PreparedGeometry, Made> {
        let mut out = PreparedFormMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_received_prepare takes the types declared there; taken is a ReceivedGeometry of ours, of the type the function names, of the stage its type names (wc_glsl_to_nir fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes), which the bridge takes and gives back itself; out is one zeroed struct wc_prepared_form of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_received_prepare(taken.into_raw(), &raw mut out) };
        let copied = out.owned(PreparedGeometry::owning);
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_form_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_prepared_form_free(&raw mut out) };
        copied
    }

    fn received_tess_control_prepare(&self, taken: ReceivedTessControl) -> PreparedForm<PreparedTessControl, Made> {
        let mut out = PreparedFormMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_received_prepare takes the types declared there; taken is a ReceivedTessControl of ours, of the type the function names, of the stage its type names (wc_glsl_to_nir fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes), which the bridge takes and gives back itself; out is one zeroed struct wc_prepared_form of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_received_prepare(taken.into_raw(), &raw mut out) };
        let copied = out.owned(PreparedTessControl::owning);
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_form_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_prepared_form_free(&raw mut out) };
        copied
    }

    fn received_tess_evaluation_prepare(&self, taken: ReceivedTessEvaluation) -> PreparedForm<PreparedTessEvaluation, Made> {
        let mut out = PreparedFormMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_received_prepare takes the types declared there; taken is a ReceivedTessEvaluation of ours, of the type the function names, of the stage its type names (wc_glsl_to_nir fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes), which the bridge takes and gives back itself; out is one zeroed struct wc_prepared_form of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_received_prepare(taken.into_raw(), &raw mut out) };
        let copied = out.owned(PreparedTessEvaluation::owning);
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_form_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_prepared_form_free(&raw mut out) };
        copied
    }

    fn prepared_vertex_variant(&self, shader: &PreparedVertex, settings: VariantSettings, swizzles: &[TextureSwizzle], samplers: &[NearestSampler]) -> VariantForm<Variant> {
        let swizzles: Vec<TextureSwizzleMirror> = swizzles.iter().map(TextureSwizzleMirror::from).collect();
        let samplers: Vec<NearestSamplerMirror> = samplers.iter().map(NearestSamplerMirror::from).collect();
        let mut out = VariantFormMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_variant takes the types declared there; shader is a PreparedVertex of ours, of the type the function names, of the stage its type names (wc_glsl_to_nir fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes), read by the call alone; swizzles holds as many elements as the count beside it, in our memory; swizzle_count is swizzles's length; samplers holds as many elements as the count beside it, in our memory; sampler_count is samplers's length; out is one zeroed struct wc_variant_form of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_prepared_variant(shader.nir.as_ptr(), VariantSettingsMirror::from(&settings), swizzles.as_ptr(), swizzles.len(), samplers.as_ptr(), samplers.len(), &raw mut out) };
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_variant_form_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_variant_form_free(&raw mut out) };
        copied
    }

    fn prepared_fragment_variant(&self, shader: &PreparedFragment, settings: VariantSettings, swizzles: &[TextureSwizzle], samplers: &[NearestSampler]) -> VariantForm<Variant> {
        let swizzles: Vec<TextureSwizzleMirror> = swizzles.iter().map(TextureSwizzleMirror::from).collect();
        let samplers: Vec<NearestSamplerMirror> = samplers.iter().map(NearestSamplerMirror::from).collect();
        let mut out = VariantFormMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_variant takes the types declared there; shader is a PreparedFragment of ours, of the type the function names, of the stage its type names (wc_glsl_to_nir fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes), read by the call alone; swizzles holds as many elements as the count beside it, in our memory; swizzle_count is swizzles's length; samplers holds as many elements as the count beside it, in our memory; sampler_count is samplers's length; out is one zeroed struct wc_variant_form of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_prepared_variant(shader.nir.as_ptr(), VariantSettingsMirror::from(&settings), swizzles.as_ptr(), swizzles.len(), samplers.as_ptr(), samplers.len(), &raw mut out) };
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_variant_form_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_variant_form_free(&raw mut out) };
        copied
    }

    fn prepared_compute_variant(&self, shader: &PreparedCompute, settings: VariantSettings, swizzles: &[TextureSwizzle], samplers: &[NearestSampler]) -> VariantForm<Variant> {
        let swizzles: Vec<TextureSwizzleMirror> = swizzles.iter().map(TextureSwizzleMirror::from).collect();
        let samplers: Vec<NearestSamplerMirror> = samplers.iter().map(NearestSamplerMirror::from).collect();
        let mut out = VariantFormMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_variant takes the types declared there; shader is a PreparedCompute of ours, of the type the function names, of the stage its type names (wc_glsl_to_nir fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes), read by the call alone; swizzles holds as many elements as the count beside it, in our memory; swizzle_count is swizzles's length; samplers holds as many elements as the count beside it, in our memory; sampler_count is samplers's length; out is one zeroed struct wc_variant_form of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_prepared_variant(shader.nir.as_ptr(), VariantSettingsMirror::from(&settings), swizzles.as_ptr(), swizzles.len(), samplers.as_ptr(), samplers.len(), &raw mut out) };
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_variant_form_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_variant_form_free(&raw mut out) };
        copied
    }

    fn prepared_geometry_variant(&self, shader: &PreparedGeometry, settings: VariantSettings, swizzles: &[TextureSwizzle], samplers: &[NearestSampler]) -> VariantForm<Variant> {
        let swizzles: Vec<TextureSwizzleMirror> = swizzles.iter().map(TextureSwizzleMirror::from).collect();
        let samplers: Vec<NearestSamplerMirror> = samplers.iter().map(NearestSamplerMirror::from).collect();
        let mut out = VariantFormMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_variant takes the types declared there; shader is a PreparedGeometry of ours, of the type the function names, of the stage its type names (wc_glsl_to_nir fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes), read by the call alone; swizzles holds as many elements as the count beside it, in our memory; swizzle_count is swizzles's length; samplers holds as many elements as the count beside it, in our memory; sampler_count is samplers's length; out is one zeroed struct wc_variant_form of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_prepared_variant(shader.nir.as_ptr(), VariantSettingsMirror::from(&settings), swizzles.as_ptr(), swizzles.len(), samplers.as_ptr(), samplers.len(), &raw mut out) };
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_variant_form_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_variant_form_free(&raw mut out) };
        copied
    }

    fn prepared_tess_control_variant(&self, shader: &PreparedTessControl, settings: VariantSettings, swizzles: &[TextureSwizzle], samplers: &[NearestSampler]) -> VariantForm<Variant> {
        let swizzles: Vec<TextureSwizzleMirror> = swizzles.iter().map(TextureSwizzleMirror::from).collect();
        let samplers: Vec<NearestSamplerMirror> = samplers.iter().map(NearestSamplerMirror::from).collect();
        let mut out = VariantFormMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_variant takes the types declared there; shader is a PreparedTessControl of ours, of the type the function names, of the stage its type names (wc_glsl_to_nir fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes), read by the call alone; swizzles holds as many elements as the count beside it, in our memory; swizzle_count is swizzles's length; samplers holds as many elements as the count beside it, in our memory; sampler_count is samplers's length; out is one zeroed struct wc_variant_form of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_prepared_variant(shader.nir.as_ptr(), VariantSettingsMirror::from(&settings), swizzles.as_ptr(), swizzles.len(), samplers.as_ptr(), samplers.len(), &raw mut out) };
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_variant_form_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_variant_form_free(&raw mut out) };
        copied
    }

    fn prepared_tess_evaluation_variant(&self, shader: &PreparedTessEvaluation, settings: VariantSettings, swizzles: &[TextureSwizzle], samplers: &[NearestSampler]) -> VariantForm<Variant> {
        let swizzles: Vec<TextureSwizzleMirror> = swizzles.iter().map(TextureSwizzleMirror::from).collect();
        let samplers: Vec<NearestSamplerMirror> = samplers.iter().map(NearestSamplerMirror::from).collect();
        let mut out = VariantFormMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_variant takes the types declared there; shader is a PreparedTessEvaluation of ours, of the type the function names, of the stage its type names (wc_glsl_to_nir fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes), read by the call alone; swizzles holds as many elements as the count beside it, in our memory; swizzle_count is swizzles's length; samplers holds as many elements as the count beside it, in our memory; sampler_count is samplers's length; out is one zeroed struct wc_variant_form of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_prepared_variant(shader.nir.as_ptr(), VariantSettingsMirror::from(&settings), swizzles.as_ptr(), swizzles.len(), samplers.as_ptr(), samplers.len(), &raw mut out) };
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_variant_form_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_variant_form_free(&raw mut out) };
        copied
    }

    fn prepared_vertex_points_as_squares(&self, vs: &PreparedVertex, at: u32, largest: f32) -> Squares<Made> {
        let mut out = SquaresMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_points_as_squares takes the types declared there; vs is a PreparedVertex of ours, of the type the function names, of the stage its type names (wc_glsl_to_nir fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes), read by the call alone; out is one zeroed struct wc_squares of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_prepared_points_as_squares(vs.nir.as_ptr(), at, largest, &raw mut out) };
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_squares_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_squares_free(&raw mut out) };
        copied
    }

    fn prepared_tess_evaluation_points_as_squares(&self, vs: &PreparedTessEvaluation, at: u32, largest: f32) -> Squares<Made> {
        let mut out = SquaresMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_points_as_squares takes the types declared there; vs is a PreparedTessEvaluation of ours, of the type the function names, of the stage its type names (wc_glsl_to_nir fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes), read by the call alone; out is one zeroed struct wc_squares of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_prepared_points_as_squares(vs.nir.as_ptr(), at, largest, &raw mut out) };
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_squares_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_squares_free(&raw mut out) };
        copied
    }

    fn prepared_geometry_points_as_squares(&self, gs: &PreparedGeometry, at: u32, largest: f32, block: u32) -> Squares<Made> {
        let mut out = SquaresMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_prepared_geometry_points_as_squares takes the types declared there; gs is a PreparedGeometry of ours, of the type the function names, of the stage its type names (wc_glsl_to_nir fills stages[s] with a shader of stage s, and wc_received_prepare keeps the stage of the shader it takes), read by the call alone; out is one zeroed struct wc_squares of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_prepared_geometry_points_as_squares(gs.nir.as_ptr(), at, largest, block, &raw mut out) };
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_squares_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_squares_free(&raw mut out) };
        copied
    }

    fn made_key(&self, shader: &Made) -> Option<Vec<u8>> {
        let mut out = BytesMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_made_key takes the types declared there; shader is a Made of ours, of the type the function names, read by the call alone; out is one zeroed struct wc_bytes of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_made_key(shader.nir.as_ptr(), &raw mut out) };
        let result = result != 0;
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_bytes_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_bytes_free(&raw mut out) };
        result.then_some(copied)
    }

    fn made_named_layer_location(&self, shader: &Made) -> u32 {
        #[expect(unsafe_code, reason = "bridge.h: wc_made_named_layer_location takes the types declared there; shader is a Made of ours, of the type the function names, read by the call alone")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_made_named_layer_location(shader.nir.as_ptr()) }
    }

    fn made_variant(&self, shader: &Made, settings: VariantSettings, swizzles: &[TextureSwizzle], samplers: &[NearestSampler]) -> VariantForm<Variant> {
        let swizzles: Vec<TextureSwizzleMirror> = swizzles.iter().map(TextureSwizzleMirror::from).collect();
        let samplers: Vec<NearestSamplerMirror> = samplers.iter().map(NearestSamplerMirror::from).collect();
        let mut out = VariantFormMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_made_variant takes the types declared there; shader is a Made of ours, of the type the function names, read by the call alone; swizzles holds as many elements as the count beside it, in our memory; swizzle_count is swizzles's length; samplers holds as many elements as the count beside it, in our memory; sampler_count is samplers's length; out is one zeroed struct wc_variant_form of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        unsafe { wc_made_variant(shader.nir.as_ptr(), VariantSettingsMirror::from(&settings), swizzles.as_ptr(), swizzles.len(), samplers.as_ptr(), samplers.len(), &raw mut out) };
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_variant_form_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_variant_form_free(&raw mut out) };
        copied
    }

    fn variant_flatten(&self, taken: Variant, points_in: u32) -> Option<Flat> {
        let mut out = FlatMirror::default();
        #[expect(unsafe_code, reason = "bridge.h: wc_variant_flatten takes the types declared there; taken is a Variant of ours, of the type the function names, which the bridge takes and gives back itself; out is one zeroed struct wc_flat of ours")]
        // SAFETY: the bridge's own function, declared as bridge.h declares it; every pointer passed is to our memory or the caller's, as long as the call reads or writes, and lives for the call.
        let result = unsafe { wc_variant_flatten(taken.into_raw(), points_in, &raw mut out) };
        let result = result != 0;
        let copied = out.owned();
        #[expect(unsafe_code, reason = "bridge.h: wc_flat_free gives back what the bridge allocated into out (null pointers are given back as none: free(NULL))")]
        // SAFETY: out is the struct the bridge filled, its shaders taken and set to null by `copied`; given back once.
        unsafe { wc_flat_free(&raw mut out) };
        result.then_some(copied)
    }

}
