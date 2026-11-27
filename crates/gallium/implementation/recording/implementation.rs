// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit.
//! A Gallium implementation that records every call: each method writes its name and its arguments as it
//! received them into the shared `Log`, and every object asked for is a plain `Object` with a number of its own.
//! For the serving glue's tests, which read the log to see what reached the implementation.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use wcontainers_gallium::*;

/// The bytes of every mapping the recording implementation lends.
const MAPPED_BYTES: usize = 64;

/// What the implementation received, in order, and the numbers of the objects it made.
#[derive(Debug, Default)]
pub struct Log {
    calls: Mutex<Vec<String>>,
    made: AtomicUsize,
}

impl Log {
    /// Records one call.
    pub fn record(&self, call: String) {
        self.calls.lock().unwrap_or_else(PoisonError::into_inner).push(call);
    }

    /// Every call recorded so far.
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// A new object of a kind.
    pub fn made(&self, kind: &'static str) -> Object {
        Object { kind, number: self.made.fetch_add(1, Ordering::Relaxed) }
    }
}

/// Every object the recording implementation makes, of any kind but the context.
#[derive(Debug, PartialEq, Eq)]
pub struct Object {
    /// Its kind, as the trait names it.
    pub kind: &'static str,
    /// Its number, in the order made.
    pub number: usize,
}

/// The recording screen.
#[derive(Debug, Default)]
pub struct Recording {
    /// What every call received.
    pub log: Arc<Log>,
    /// The seed of the capabilities it sets (`filled_caps` and the others), or none: Mesa's defaults kept.
    pub caps_seed: Option<usize>,
}

impl Recording {
    /// A screen recording into a new log, keeping Mesa's default capabilities.
    pub fn new() -> Recording {
        Recording::default()
    }

    /// A screen recording into a new log, setting every field of every capability to the seed's own value.
    pub fn filling_caps(seed: usize) -> Recording {
        Recording { log: Arc::default(), caps_seed: Some(seed) }
    }
}

/// A recording context, recording into its screen's log.
#[derive(Debug)]
pub struct RecordingContext {
    /// The screen's log.
    pub log: Arc<Log>,
}

impl Gallium for Recording {
    type Compiler = wcontainers_mesa_compiler_direct::Bridge;
    type Context = RecordingContext;
    type Resource = Object;
    type Fence = Object;
    type Blend = Object;
    type DepthStencilAlpha = Object;
    type Rasterizer = Object;
    type Sampler = Object;
    type VertexElements = Object;
    type VertexShader = Object;
    type FragmentShader = Object;
    type GeometryShader = Object;
    type TessControlShader = Object;
    type TessEvaluationShader = Object;
    type ComputeShader = Object;
    type SamplerView = Object;
    type StreamOutputTarget = Object;
    type Query = Object;
    type ActiveQuery = Object;
    type EndOnlyQuery = Object;
    type Mapping = Object;

    fn context_create(&self, priv_: u64, flags: u32) -> Option<Self::Context> {
        self.log.record(format!("context_create priv_ {priv_:?} flags {flags:?}"));
        Some(RecordingContext { log: Arc::clone(&self.log) })
    }

    fn destroy(&self) {
        self.log.record("destroy".to_string());
    }

    fn fence_finish(&self, ctx: Option<&Self::Context>, fence: &Arc<Self::Fence>, timeout: u64) -> bool {
        self.log.record(format!("fence_finish ctx {ctx:?} fence {fence:?} timeout {timeout:?}"));
        true
    }

    fn flush_frontbuffer(&self, ctx: Option<&Self::Context>, resource: &Arc<Self::Resource>, level: u32, layer: u32, winsys_drawable_handle: u64, subbox: Option<&PipeBox>) {
        self.log.record(format!("flush_frontbuffer ctx {ctx:?} resource {resource:?} level {level:?} layer {layer:?} winsys_drawable_handle {winsys_drawable_handle:?} subbox {subbox:?}"));
    }

    fn get_device_vendor(&self) -> Vec<u8> {
        self.log.record("get_device_vendor".to_string());
        b"recording".to_vec()
    }

    fn get_name(&self) -> Vec<u8> {
        self.log.record("get_name".to_string());
        b"recording".to_vec()
    }

    fn get_vendor(&self) -> Vec<u8> {
        self.log.record("get_vendor".to_string());
        b"recording".to_vec()
    }

    fn is_format_supported(&self, format: values::Format, target: values::TextureTarget, sample_count: u32, storage_sample_count: u32, bindings: u32) -> bool {
        self.log.record(format!("is_format_supported format {format:?} target {target:?} sample_count {sample_count:?} storage_sample_count {storage_sample_count:?} bindings {bindings:?}"));
        true
    }

    fn resource_create(&self, templat: &ResourceTemplate) -> Option<Self::Resource> {
        self.log.record(format!("resource_create templat {templat:?}"));
        Some(self.log.made("Resource"))
    }

    fn caps(&self, caps: &mut Caps) {
        self.log.record(format!("caps caps {caps:?}"));
        if let Some(seed) = self.caps_seed {
            *caps = filled_caps(seed);
        }
    }

    fn shader_caps(&self, stage: values::MesaShaderStage, caps: &mut ShaderCaps) {
        self.log.record(format!("shader_caps stage {stage:?} caps {caps:?}"));
        if let Some(seed) = self.caps_seed {
            *caps = filled_shader_caps(seed, usize::try_from(stage.0).unwrap_or(usize::MAX));
        }
    }

    fn compute_caps(&self, caps: &mut ComputeCaps) {
        self.log.record(format!("compute_caps caps {caps:?}"));
        if let Some(seed) = self.caps_seed {
            *caps = filled_compute_caps(seed);
        }
    }

}

impl GalliumContext for RecordingContext {
    type Screen = Recording;

    fn begin_query(&self, q: <Self::Screen as Gallium>::Query) -> Result<<Self::Screen as Gallium>::ActiveQuery, <Self::Screen as Gallium>::Query> {
        self.log.record(format!("begin_query q {q:?}"));
        Ok(q)
    }

    fn bind_blend_state(&self, arg1: Option<&<Self::Screen as Gallium>::Blend>) {
        self.log.record(format!("bind_blend_state arg1 {arg1:?}"));
    }

    fn bind_compute_state(&self, arg1: Option<&<Self::Screen as Gallium>::ComputeShader>) {
        self.log.record(format!("bind_compute_state arg1 {arg1:?}"));
    }

    fn bind_depth_stencil_alpha_state(&self, arg1: Option<&<Self::Screen as Gallium>::DepthStencilAlpha>) {
        self.log.record(format!("bind_depth_stencil_alpha_state arg1 {arg1:?}"));
    }

    fn bind_fs_state(&self, arg1: Option<&<Self::Screen as Gallium>::FragmentShader>) {
        self.log.record(format!("bind_fs_state arg1 {arg1:?}"));
    }

    fn bind_gs_state(&self, arg1: Option<&<Self::Screen as Gallium>::GeometryShader>) {
        self.log.record(format!("bind_gs_state arg1 {arg1:?}"));
    }

    fn bind_rasterizer_state(&self, arg1: Option<&<Self::Screen as Gallium>::Rasterizer>) {
        self.log.record(format!("bind_rasterizer_state arg1 {arg1:?}"));
    }

    fn bind_sampler_states(&self, shader: values::MesaShaderStage, start_slot: u32, samplers: &[Option<&<Self::Screen as Gallium>::Sampler>]) {
        self.log.record(format!("bind_sampler_states shader {shader:?} start_slot {start_slot:?} samplers {samplers:?}"));
    }

    fn bind_tcs_state(&self, arg1: Option<&<Self::Screen as Gallium>::TessControlShader>) {
        self.log.record(format!("bind_tcs_state arg1 {arg1:?}"));
    }

    fn bind_tes_state(&self, arg1: Option<&<Self::Screen as Gallium>::TessEvaluationShader>) {
        self.log.record(format!("bind_tes_state arg1 {arg1:?}"));
    }

    fn bind_vertex_elements_state(&self, arg1: Option<&<Self::Screen as Gallium>::VertexElements>) {
        self.log.record(format!("bind_vertex_elements_state arg1 {arg1:?}"));
    }

    fn bind_vs_state(&self, arg1: Option<&<Self::Screen as Gallium>::VertexShader>) {
        self.log.record(format!("bind_vs_state arg1 {arg1:?}"));
    }

    fn blit(&self, info: &BlitInfo<'_, <Self::Screen as Gallium>::Resource>) {
        self.log.record(format!("blit info {info:?}"));
    }

    fn buffer_map(&self, resource: &Arc<<Self::Screen as Gallium>::Resource>, level: u32, usage: values::MapFlags, arg4: &PipeBox) -> Option<Mapped<<Self::Screen as Gallium>::Mapping>> {
        self.log.record(format!("buffer_map resource {resource:?} level {level:?} usage {usage:?} arg4 {arg4:?}"));
        Some(Mapped { mapping: self.log.made("Mapping"), bytes: vec![0; MAPPED_BYTES], stride: 0, layer_stride: 0 })
    }

    fn buffer_subdata(&self, arg1: &Arc<<Self::Screen as Gallium>::Resource>, usage: values::MapFlags, offset: u32, data: &[u8]) {
        self.log.record(format!("buffer_subdata arg1 {arg1:?} usage {usage:?} offset {offset:?} data {data:?}"));
    }

    fn buffer_unmap(&self, transfer: <Self::Screen as Gallium>::Mapping, written: &[u8]) {
        self.log.record(format!("buffer_unmap transfer {transfer:?} written {written:?}"));
    }

    fn clear(&self, buffers: u32, color_clear_mask: u32, stencil_clear_mask: u8, scissor_state: Option<&ScissorState>, color: &[u32; 4], depth: f64, stencil: u32) {
        self.log.record(format!("clear buffers {buffers:?} color_clear_mask {color_clear_mask:?} stencil_clear_mask {stencil_clear_mask:?} scissor_state {scissor_state:?} color {color:?} depth {depth:?} stencil {stencil:?}"));
    }

    fn clear_depth_stencil(&self, dst: &Surface<'_, <Self::Screen as Gallium>::Resource>, clear_flags: u32, depth: f64, stencil: u32, dstx: u32, dsty: u32, width: u32, height: u32, render_condition_enabled: bool) {
        self.log.record(format!("clear_depth_stencil dst {dst:?} clear_flags {clear_flags:?} depth {depth:?} stencil {stencil:?} dstx {dstx:?} dsty {dsty:?} width {width:?} height {height:?} render_condition_enabled {render_condition_enabled:?}"));
    }

    fn clear_render_target(&self, dst: &Surface<'_, <Self::Screen as Gallium>::Resource>, color: &[u32; 4], dstx: u32, dsty: u32, width: u32, height: u32, render_condition_enabled: bool) {
        self.log.record(format!("clear_render_target dst {dst:?} color {color:?} dstx {dstx:?} dsty {dsty:?} width {width:?} height {height:?} render_condition_enabled {render_condition_enabled:?}"));
    }

    fn create_blend_state(&self, arg1: &BlendState) -> Option<<Self::Screen as Gallium>::Blend> {
        self.log.record(format!("create_blend_state arg1 {arg1:?}"));
        Some(self.log.made("Blend"))
    }

    fn create_compute_state(&self, program: <<Self::Screen as Gallium>::Compiler as wcontainers_mesa_compiler::MesaCompiler>::ReceivedCompute, arg1: &ComputeState) -> Option<<Self::Screen as Gallium>::ComputeShader> {
        let _ = &program;
        self.log.record(format!("create_compute_state program <program> arg1 {arg1:?}"));
        Some(self.log.made("ComputeShader"))
    }

    fn create_depth_stencil_alpha_state(&self, arg1: &DepthStencilAlphaState) -> Option<<Self::Screen as Gallium>::DepthStencilAlpha> {
        self.log.record(format!("create_depth_stencil_alpha_state arg1 {arg1:?}"));
        Some(self.log.made("DepthStencilAlpha"))
    }

    fn create_fs_state(&self, program: <<Self::Screen as Gallium>::Compiler as wcontainers_mesa_compiler::MesaCompiler>::ReceivedFragment, arg1: &ShaderState) -> Option<<Self::Screen as Gallium>::FragmentShader> {
        let _ = &program;
        self.log.record(format!("create_fs_state program <program> arg1 {arg1:?}"));
        Some(self.log.made("FragmentShader"))
    }

    fn create_gs_state(&self, program: <<Self::Screen as Gallium>::Compiler as wcontainers_mesa_compiler::MesaCompiler>::ReceivedGeometry, arg1: &ShaderState) -> Option<<Self::Screen as Gallium>::GeometryShader> {
        let _ = &program;
        self.log.record(format!("create_gs_state program <program> arg1 {arg1:?}"));
        Some(self.log.made("GeometryShader"))
    }

    fn create_query(&self, query_type: values::QueryType, index: u32) -> Option<<Self::Screen as Gallium>::Query> {
        self.log.record(format!("create_query query_type {query_type:?} index {index:?}"));
        Some(self.log.made("Query"))
    }

    fn create_query_end_only_query(&self, query_type: values::QueryType, index: u32) -> Option<<Self::Screen as Gallium>::EndOnlyQuery> {
        self.log.record(format!("create_query_end_only_query query_type {query_type:?} index {index:?}"));
        Some(self.log.made("EndOnlyQuery"))
    }

    fn create_rasterizer_state(&self, arg1: &RasterizerState) -> Option<<Self::Screen as Gallium>::Rasterizer> {
        self.log.record(format!("create_rasterizer_state arg1 {arg1:?}"));
        Some(self.log.made("Rasterizer"))
    }

    fn create_sampler_state(&self, arg1: &SamplerState) -> Option<<Self::Screen as Gallium>::Sampler> {
        self.log.record(format!("create_sampler_state arg1 {arg1:?}"));
        Some(self.log.made("Sampler"))
    }

    fn create_sampler_view(&self, texture: &Arc<<Self::Screen as Gallium>::Resource>, templat: &SamplerViewTemplate) -> Option<<Self::Screen as Gallium>::SamplerView> {
        self.log.record(format!("create_sampler_view texture {texture:?} templat {templat:?}"));
        Some(self.log.made("SamplerView"))
    }

    fn create_stream_output_target(&self, arg1: &Arc<<Self::Screen as Gallium>::Resource>, buffer_offset: u32, buffer_size: u32) -> Option<<Self::Screen as Gallium>::StreamOutputTarget> {
        self.log.record(format!("create_stream_output_target arg1 {arg1:?} buffer_offset {buffer_offset:?} buffer_size {buffer_size:?}"));
        Some(self.log.made("StreamOutputTarget"))
    }

    fn create_tcs_state(&self, program: <<Self::Screen as Gallium>::Compiler as wcontainers_mesa_compiler::MesaCompiler>::ReceivedTessControl, arg1: &ShaderState) -> Option<<Self::Screen as Gallium>::TessControlShader> {
        let _ = &program;
        self.log.record(format!("create_tcs_state program <program> arg1 {arg1:?}"));
        Some(self.log.made("TessControlShader"))
    }

    fn create_tes_state(&self, program: <<Self::Screen as Gallium>::Compiler as wcontainers_mesa_compiler::MesaCompiler>::ReceivedTessEvaluation, arg1: &ShaderState) -> Option<<Self::Screen as Gallium>::TessEvaluationShader> {
        let _ = &program;
        self.log.record(format!("create_tes_state program <program> arg1 {arg1:?}"));
        Some(self.log.made("TessEvaluationShader"))
    }

    fn create_vertex_elements_state(&self, arg2: &[VertexElement]) -> Option<<Self::Screen as Gallium>::VertexElements> {
        self.log.record(format!("create_vertex_elements_state arg2 {arg2:?}"));
        Some(self.log.made("VertexElements"))
    }

    fn create_vs_state(&self, program: <<Self::Screen as Gallium>::Compiler as wcontainers_mesa_compiler::MesaCompiler>::ReceivedVertex, arg1: &ShaderState) -> Option<<Self::Screen as Gallium>::VertexShader> {
        let _ = &program;
        self.log.record(format!("create_vs_state program <program> arg1 {arg1:?}"));
        Some(self.log.made("VertexShader"))
    }

    fn delete_blend_state(&self, arg1: <Self::Screen as Gallium>::Blend) {
        self.log.record(format!("delete_blend_state arg1 {arg1:?}"));
    }

    fn delete_compute_state(&self, arg1: <Self::Screen as Gallium>::ComputeShader) {
        self.log.record(format!("delete_compute_state arg1 {arg1:?}"));
    }

    fn delete_depth_stencil_alpha_state(&self, arg1: <Self::Screen as Gallium>::DepthStencilAlpha) {
        self.log.record(format!("delete_depth_stencil_alpha_state arg1 {arg1:?}"));
    }

    fn delete_fs_state(&self, arg1: <Self::Screen as Gallium>::FragmentShader) {
        self.log.record(format!("delete_fs_state arg1 {arg1:?}"));
    }

    fn delete_gs_state(&self, arg1: <Self::Screen as Gallium>::GeometryShader) {
        self.log.record(format!("delete_gs_state arg1 {arg1:?}"));
    }

    fn delete_rasterizer_state(&self, arg1: <Self::Screen as Gallium>::Rasterizer) {
        self.log.record(format!("delete_rasterizer_state arg1 {arg1:?}"));
    }

    fn delete_sampler_state(&self, arg1: <Self::Screen as Gallium>::Sampler) {
        self.log.record(format!("delete_sampler_state arg1 {arg1:?}"));
    }

    fn delete_tcs_state(&self, arg1: <Self::Screen as Gallium>::TessControlShader) {
        self.log.record(format!("delete_tcs_state arg1 {arg1:?}"));
    }

    fn delete_tes_state(&self, arg1: <Self::Screen as Gallium>::TessEvaluationShader) {
        self.log.record(format!("delete_tes_state arg1 {arg1:?}"));
    }

    fn delete_vertex_elements_state(&self, arg1: <Self::Screen as Gallium>::VertexElements) {
        self.log.record(format!("delete_vertex_elements_state arg1 {arg1:?}"));
    }

    fn delete_vs_state(&self, arg1: <Self::Screen as Gallium>::VertexShader) {
        self.log.record(format!("delete_vs_state arg1 {arg1:?}"));
    }

    fn destroy(&self) {
        self.log.record("destroy".to_string());
    }

    fn destroy_query(&self, q: <Self::Screen as Gallium>::Query) {
        self.log.record(format!("destroy_query q {q:?}"));
    }

    fn destroy_query_end_only_query(&self, q: <Self::Screen as Gallium>::EndOnlyQuery) {
        self.log.record(format!("destroy_query_end_only_query q {q:?}"));
    }

    fn destroy_query_active_query(&self, q: <Self::Screen as Gallium>::ActiveQuery) {
        self.log.record(format!("destroy_query_active_query q {q:?}"));
    }

    fn draw_vbo(&self, info: &DrawInfo<'_, <Self::Screen as Gallium>::Resource>, drawid_offset: u32, indirect: Option<&DrawIndirectInfo<'_, <Self::Screen as Gallium>::Resource, <Self::Screen as Gallium>::StreamOutputTarget>>, draws: &[DrawStartCountBias]) {
        self.log.record(format!("draw_vbo info {info:?} drawid_offset {drawid_offset:?} indirect {indirect:?} draws {draws:?}"));
    }

    fn end_query(&self, q: <Self::Screen as Gallium>::ActiveQuery) -> Result<<Self::Screen as Gallium>::Query, <Self::Screen as Gallium>::ActiveQuery> {
        self.log.record(format!("end_query q {q:?}"));
        Ok(q)
    }

    fn end_query_end_only_query(&self, q: &<Self::Screen as Gallium>::EndOnlyQuery) -> bool {
        self.log.record(format!("end_query_end_only_query q {q:?}"));
        true
    }

    fn flush(&self, flags: values::FlushFlags) -> Option<<Self::Screen as Gallium>::Fence> {
        self.log.record(format!("flush flags {flags:?}"));
        Some(self.log.made("Fence"))
    }

    fn flush_resource(&self, resource: &Arc<<Self::Screen as Gallium>::Resource>) {
        self.log.record(format!("flush_resource resource {resource:?}"));
    }

    fn generate_mipmap(&self, resource: &Arc<<Self::Screen as Gallium>::Resource>, format: values::Format, base_level: u32, last_level: u32, first_layer: u32, last_layer: u32) -> bool {
        self.log.record(format!("generate_mipmap resource {resource:?} format {format:?} base_level {base_level:?} last_level {last_level:?} first_layer {first_layer:?} last_layer {last_layer:?}"));
        true
    }

    fn get_device_reset_status(&self) -> values::ResetStatus {
        self.log.record("get_device_reset_status".to_string());
        values::ResetStatus(0)
    }

    fn get_query_result(&self, q: &<Self::Screen as Gallium>::Query, wait: bool, result: &mut [u8]) -> bool {
        self.log.record(format!("get_query_result q {q:?} wait {wait:?} result {result:?}"));
        true
    }

    fn get_query_result_end_only_query(&self, q: &<Self::Screen as Gallium>::EndOnlyQuery, wait: bool, result: &mut [u8]) -> bool {
        self.log.record(format!("get_query_result_end_only_query q {q:?} wait {wait:?} result {result:?}"));
        true
    }

    fn get_timestamp(&self) -> u64 {
        self.log.record("get_timestamp".to_string());
        0
    }

    fn launch_grid(&self, info: &GridInfo<'_, <Self::Screen as Gallium>::Resource>) {
        self.log.record(format!("launch_grid info {info:?}"));
    }

    fn memory_barrier(&self, flags: u32) {
        self.log.record(format!("memory_barrier flags {flags:?}"));
    }

    fn render_condition(&self, query: Option<&<Self::Screen as Gallium>::Query>, condition: bool, mode: values::RenderCondFlag) {
        self.log.record(format!("render_condition query {query:?} condition {condition:?} mode {mode:?}"));
    }

    fn render_condition_end_only_query(&self, query: &<Self::Screen as Gallium>::EndOnlyQuery, condition: bool, mode: values::RenderCondFlag) {
        self.log.record(format!("render_condition_end_only_query query {query:?} condition {condition:?} mode {mode:?}"));
    }

    fn resource_copy_region(&self, dst: &Arc<<Self::Screen as Gallium>::Resource>, dst_level: u32, dstx: u32, dsty: u32, dstz: u32, src: &Arc<<Self::Screen as Gallium>::Resource>, src_level: u32, src_box: &PipeBox) {
        self.log.record(format!("resource_copy_region dst {dst:?} dst_level {dst_level:?} dstx {dstx:?} dsty {dsty:?} dstz {dstz:?} src {src:?} src_level {src_level:?} src_box {src_box:?}"));
    }

    fn set_active_query_state(&self, enable: bool) {
        self.log.record(format!("set_active_query_state enable {enable:?}"));
    }

    fn set_blend_color(&self, arg1: &BlendColor) {
        self.log.record(format!("set_blend_color arg1 {arg1:?}"));
    }

    fn set_clip_state(&self, arg1: &ClipState) {
        self.log.record(format!("set_clip_state arg1 {arg1:?}"));
    }

    fn set_constant_buffer(&self, shader: values::MesaShaderStage, index: u32, buf: Option<&ConstantBuffer<'_, <Self::Screen as Gallium>::Resource>>) {
        self.log.record(format!("set_constant_buffer shader {shader:?} index {index:?} buf {buf:?}"));
    }

    fn set_framebuffer_state(&self, arg1: &FramebufferState<'_, <Self::Screen as Gallium>::Resource>) {
        self.log.record(format!("set_framebuffer_state arg1 {arg1:?}"));
    }

    fn set_patch_vertices(&self, patch_vertices: u8) {
        self.log.record(format!("set_patch_vertices patch_vertices {patch_vertices:?}"));
    }

    fn set_polygon_stipple(&self, arg1: &PolyStipple) {
        self.log.record(format!("set_polygon_stipple arg1 {arg1:?}"));
    }

    fn set_sample_mask(&self, sample_mask: u32) {
        self.log.record(format!("set_sample_mask sample_mask {sample_mask:?}"));
    }

    fn set_sampler_views(&self, shader: values::MesaShaderStage, start_slot: u32, num_views: u32, unbind_num_trailing_slots: u32, views: Option<&[Option<&Arc<<Self::Screen as Gallium>::SamplerView>>]>) {
        self.log.record(format!("set_sampler_views shader {shader:?} start_slot {start_slot:?} num_views {num_views:?} unbind_num_trailing_slots {unbind_num_trailing_slots:?} views {views:?}"));
    }

    fn set_scissor_states(&self, start_slot: u32, arg3: &[ScissorState]) {
        self.log.record(format!("set_scissor_states start_slot {start_slot:?} arg3 {arg3:?}"));
    }

    fn set_shader_buffers(&self, shader: values::MesaShaderStage, start_slot: u32, count: u32, buffers: Option<&[ShaderBuffer<'_, <Self::Screen as Gallium>::Resource>]>, writable_bitmask: u32) {
        self.log.record(format!("set_shader_buffers shader {shader:?} start_slot {start_slot:?} count {count:?} buffers {buffers:?} writable_bitmask {writable_bitmask:?}"));
    }

    fn set_shader_images(&self, shader: values::MesaShaderStage, start_slot: u32, count: u32, unbind_num_trailing_slots: u32, images: Option<&[ImageView<'_, <Self::Screen as Gallium>::Resource>]>) {
        self.log.record(format!("set_shader_images shader {shader:?} start_slot {start_slot:?} count {count:?} unbind_num_trailing_slots {unbind_num_trailing_slots:?} images {images:?}"));
    }

    fn set_stencil_ref(&self, ref_: StencilRef) {
        self.log.record(format!("set_stencil_ref ref_ {ref_:?}"));
    }

    fn set_stream_output_targets(&self, targets: &[Option<&Arc<<Self::Screen as Gallium>::StreamOutputTarget>>], offsets: &[u32], output_prim: values::MesaPrim) {
        self.log.record(format!("set_stream_output_targets targets {targets:?} offsets {offsets:?} output_prim {output_prim:?}"));
    }

    fn set_vertex_buffers(&self, arg2: &[VertexBuffer<'_, <Self::Screen as Gallium>::Resource>]) {
        self.log.record(format!("set_vertex_buffers arg2 {arg2:?}"));
    }

    fn set_viewport_states(&self, start_slot: u32, arg3: &[ViewportState]) {
        self.log.record(format!("set_viewport_states start_slot {start_slot:?} arg3 {arg3:?}"));
    }

    fn texture_barrier(&self, flags: u32) {
        self.log.record(format!("texture_barrier flags {flags:?}"));
    }

    fn texture_subdata(&self, arg1: &Arc<<Self::Screen as Gallium>::Resource>, level: u32, usage: values::MapFlags, arg4: &PipeBox, data: &[u8], stride: u32, layer_stride: u64) {
        self.log.record(format!("texture_subdata arg1 {arg1:?} level {level:?} usage {usage:?} arg4 {arg4:?} data {data:?} stride {stride:?} layer_stride {layer_stride:?}"));
    }

    fn helper_transfer_map(&self, prsc: &Arc<<Self::Screen as Gallium>::Resource>, level: u32, usage: values::MapFlags, box_: &PipeBox) -> Option<Mapped<<Self::Screen as Gallium>::Mapping>> {
        self.log.record(format!("helper_transfer_map prsc {prsc:?} level {level:?} usage {usage:?} box_ {box_:?}"));
        Some(Mapped { mapping: self.log.made("Mapping"), bytes: vec![0; MAPPED_BYTES], stride: 0, layer_stride: 0 })
    }

    fn helper_transfer_unmap(&self, ptrans: <Self::Screen as Gallium>::Mapping, written: &[u8]) {
        self.log.record(format!("helper_transfer_unmap ptrans {ptrans:?} written {written:?}"));
    }

    fn helper_transfer_flush_region(&self, ptrans: &<Self::Screen as Gallium>::Mapping, written: &[u8], box_: &PipeBox) {
        self.log.record(format!("helper_transfer_flush_region ptrans {ptrans:?} written {written:?} box_ {box_:?}"));
    }

}

/// The seeds `filling_caps` takes.
pub const CAPS_SEEDS: usize = 2;

/// Every field of a stage's `ShaderCaps` set to a value of its own, by seed (the direct crate's caller.c holds Mesa's
/// screen to the same values); Mesa's defaults past the seeds and the stages.
pub fn filled_shader_caps(seed: usize, stage: usize) -> ShaderCaps {
    match (seed, stage) {
        (0, 0) => ShaderCaps { max_instructions: 1001, max_alu_instructions: 1002, max_tex_instructions: 1003, max_tex_indirections: 1004, max_control_flow_depth: 1005, max_inputs: 1006, max_outputs: 1007, max_const_buffer0_size: 1008, max_const_buffers: 1009, max_temps: 1010, max_texture_samplers: 1011, max_sampler_views: 1012, max_shader_buffers: 1013, max_shader_images: 1014, max_hw_atomic_counters: 1015, max_hw_atomic_counter_buffers: 1016, supported_irs: 1017, cont_supported: true, indirect_temp_addr: true, indirect_const_addr: false, subroutines: false, integers: true, int64_atomics: true, fp16: false, fp16_derivatives: false, fp16_const_buffers: true, fp16_no_denorms: true, int16: false, glsl_16bit_consts: false, glsl_16bit_load_dst: true, tgsi_sqrt_supported: true, tgsi_any_inout_decl_range: false },
        (0, 1) => ShaderCaps { max_instructions: 1033, max_alu_instructions: 1034, max_tex_instructions: 1035, max_tex_indirections: 1036, max_control_flow_depth: 1037, max_inputs: 1038, max_outputs: 1039, max_const_buffer0_size: 1040, max_const_buffers: 1041, max_temps: 1042, max_texture_samplers: 1043, max_sampler_views: 1044, max_shader_buffers: 1045, max_shader_images: 1046, max_hw_atomic_counters: 1047, max_hw_atomic_counter_buffers: 1048, supported_irs: 1049, cont_supported: true, indirect_temp_addr: true, indirect_const_addr: false, subroutines: false, integers: true, int64_atomics: true, fp16: false, fp16_derivatives: false, fp16_const_buffers: true, fp16_no_denorms: true, int16: false, glsl_16bit_consts: false, glsl_16bit_load_dst: true, tgsi_sqrt_supported: true, tgsi_any_inout_decl_range: false },
        (0, 2) => ShaderCaps { max_instructions: 1065, max_alu_instructions: 1066, max_tex_instructions: 1067, max_tex_indirections: 1068, max_control_flow_depth: 1069, max_inputs: 1070, max_outputs: 1071, max_const_buffer0_size: 1072, max_const_buffers: 1073, max_temps: 1074, max_texture_samplers: 1075, max_sampler_views: 1076, max_shader_buffers: 1077, max_shader_images: 1078, max_hw_atomic_counters: 1079, max_hw_atomic_counter_buffers: 1080, supported_irs: 1081, cont_supported: true, indirect_temp_addr: true, indirect_const_addr: false, subroutines: false, integers: true, int64_atomics: true, fp16: false, fp16_derivatives: false, fp16_const_buffers: true, fp16_no_denorms: true, int16: false, glsl_16bit_consts: false, glsl_16bit_load_dst: true, tgsi_sqrt_supported: true, tgsi_any_inout_decl_range: false },
        (0, 3) => ShaderCaps { max_instructions: 1097, max_alu_instructions: 1098, max_tex_instructions: 1099, max_tex_indirections: 1100, max_control_flow_depth: 1101, max_inputs: 1102, max_outputs: 1103, max_const_buffer0_size: 1104, max_const_buffers: 1105, max_temps: 1106, max_texture_samplers: 1107, max_sampler_views: 1108, max_shader_buffers: 1109, max_shader_images: 1110, max_hw_atomic_counters: 1111, max_hw_atomic_counter_buffers: 1112, supported_irs: 1113, cont_supported: true, indirect_temp_addr: true, indirect_const_addr: false, subroutines: false, integers: true, int64_atomics: true, fp16: false, fp16_derivatives: false, fp16_const_buffers: true, fp16_no_denorms: true, int16: false, glsl_16bit_consts: false, glsl_16bit_load_dst: true, tgsi_sqrt_supported: true, tgsi_any_inout_decl_range: false },
        (0, 4) => ShaderCaps { max_instructions: 1129, max_alu_instructions: 1130, max_tex_instructions: 1131, max_tex_indirections: 1132, max_control_flow_depth: 1133, max_inputs: 1134, max_outputs: 1135, max_const_buffer0_size: 1136, max_const_buffers: 1137, max_temps: 1138, max_texture_samplers: 1139, max_sampler_views: 1140, max_shader_buffers: 1141, max_shader_images: 1142, max_hw_atomic_counters: 1143, max_hw_atomic_counter_buffers: 1144, supported_irs: 1145, cont_supported: true, indirect_temp_addr: true, indirect_const_addr: false, subroutines: false, integers: true, int64_atomics: true, fp16: false, fp16_derivatives: false, fp16_const_buffers: true, fp16_no_denorms: true, int16: false, glsl_16bit_consts: false, glsl_16bit_load_dst: true, tgsi_sqrt_supported: true, tgsi_any_inout_decl_range: false },
        (0, 5) => ShaderCaps { max_instructions: 1161, max_alu_instructions: 1162, max_tex_instructions: 1163, max_tex_indirections: 1164, max_control_flow_depth: 1165, max_inputs: 1166, max_outputs: 1167, max_const_buffer0_size: 1168, max_const_buffers: 1169, max_temps: 1170, max_texture_samplers: 1171, max_sampler_views: 1172, max_shader_buffers: 1173, max_shader_images: 1174, max_hw_atomic_counters: 1175, max_hw_atomic_counter_buffers: 1176, supported_irs: 1177, cont_supported: true, indirect_temp_addr: true, indirect_const_addr: false, subroutines: false, integers: true, int64_atomics: true, fp16: false, fp16_derivatives: false, fp16_const_buffers: true, fp16_no_denorms: true, int16: false, glsl_16bit_consts: false, glsl_16bit_load_dst: true, tgsi_sqrt_supported: true, tgsi_any_inout_decl_range: false },
        (0, 6) => ShaderCaps { max_instructions: 1193, max_alu_instructions: 1194, max_tex_instructions: 1195, max_tex_indirections: 1196, max_control_flow_depth: 1197, max_inputs: 1198, max_outputs: 1199, max_const_buffer0_size: 1200, max_const_buffers: 1201, max_temps: 1202, max_texture_samplers: 1203, max_sampler_views: 1204, max_shader_buffers: 1205, max_shader_images: 1206, max_hw_atomic_counters: 1207, max_hw_atomic_counter_buffers: 1208, supported_irs: 1209, cont_supported: true, indirect_temp_addr: true, indirect_const_addr: false, subroutines: false, integers: true, int64_atomics: true, fp16: false, fp16_derivatives: false, fp16_const_buffers: true, fp16_no_denorms: true, int16: false, glsl_16bit_consts: false, glsl_16bit_load_dst: true, tgsi_sqrt_supported: true, tgsi_any_inout_decl_range: false },
        (0, 7) => ShaderCaps { max_instructions: 1225, max_alu_instructions: 1226, max_tex_instructions: 1227, max_tex_indirections: 1228, max_control_flow_depth: 1229, max_inputs: 1230, max_outputs: 1231, max_const_buffer0_size: 1232, max_const_buffers: 1233, max_temps: 1234, max_texture_samplers: 1235, max_sampler_views: 1236, max_shader_buffers: 1237, max_shader_images: 1238, max_hw_atomic_counters: 1239, max_hw_atomic_counter_buffers: 1240, supported_irs: 1241, cont_supported: true, indirect_temp_addr: true, indirect_const_addr: false, subroutines: false, integers: true, int64_atomics: true, fp16: false, fp16_derivatives: false, fp16_const_buffers: true, fp16_no_denorms: true, int16: false, glsl_16bit_consts: false, glsl_16bit_load_dst: true, tgsi_sqrt_supported: true, tgsi_any_inout_decl_range: false },
        (1, 0) => ShaderCaps { max_instructions: 1003, max_alu_instructions: 1004, max_tex_instructions: 1005, max_tex_indirections: 1006, max_control_flow_depth: 1007, max_inputs: 1008, max_outputs: 1009, max_const_buffer0_size: 1010, max_const_buffers: 1011, max_temps: 1012, max_texture_samplers: 1013, max_sampler_views: 1014, max_shader_buffers: 1015, max_shader_images: 1016, max_hw_atomic_counters: 1017, max_hw_atomic_counter_buffers: 1018, supported_irs: 1019, cont_supported: false, indirect_temp_addr: false, indirect_const_addr: true, subroutines: true, integers: false, int64_atomics: false, fp16: true, fp16_derivatives: true, fp16_const_buffers: false, fp16_no_denorms: false, int16: true, glsl_16bit_consts: true, glsl_16bit_load_dst: false, tgsi_sqrt_supported: false, tgsi_any_inout_decl_range: true },
        (1, 1) => ShaderCaps { max_instructions: 1035, max_alu_instructions: 1036, max_tex_instructions: 1037, max_tex_indirections: 1038, max_control_flow_depth: 1039, max_inputs: 1040, max_outputs: 1041, max_const_buffer0_size: 1042, max_const_buffers: 1043, max_temps: 1044, max_texture_samplers: 1045, max_sampler_views: 1046, max_shader_buffers: 1047, max_shader_images: 1048, max_hw_atomic_counters: 1049, max_hw_atomic_counter_buffers: 1050, supported_irs: 1051, cont_supported: false, indirect_temp_addr: false, indirect_const_addr: true, subroutines: true, integers: false, int64_atomics: false, fp16: true, fp16_derivatives: true, fp16_const_buffers: false, fp16_no_denorms: false, int16: true, glsl_16bit_consts: true, glsl_16bit_load_dst: false, tgsi_sqrt_supported: false, tgsi_any_inout_decl_range: true },
        (1, 2) => ShaderCaps { max_instructions: 1067, max_alu_instructions: 1068, max_tex_instructions: 1069, max_tex_indirections: 1070, max_control_flow_depth: 1071, max_inputs: 1072, max_outputs: 1073, max_const_buffer0_size: 1074, max_const_buffers: 1075, max_temps: 1076, max_texture_samplers: 1077, max_sampler_views: 1078, max_shader_buffers: 1079, max_shader_images: 1080, max_hw_atomic_counters: 1081, max_hw_atomic_counter_buffers: 1082, supported_irs: 1083, cont_supported: false, indirect_temp_addr: false, indirect_const_addr: true, subroutines: true, integers: false, int64_atomics: false, fp16: true, fp16_derivatives: true, fp16_const_buffers: false, fp16_no_denorms: false, int16: true, glsl_16bit_consts: true, glsl_16bit_load_dst: false, tgsi_sqrt_supported: false, tgsi_any_inout_decl_range: true },
        (1, 3) => ShaderCaps { max_instructions: 1099, max_alu_instructions: 1100, max_tex_instructions: 1101, max_tex_indirections: 1102, max_control_flow_depth: 1103, max_inputs: 1104, max_outputs: 1105, max_const_buffer0_size: 1106, max_const_buffers: 1107, max_temps: 1108, max_texture_samplers: 1109, max_sampler_views: 1110, max_shader_buffers: 1111, max_shader_images: 1112, max_hw_atomic_counters: 1113, max_hw_atomic_counter_buffers: 1114, supported_irs: 1115, cont_supported: false, indirect_temp_addr: false, indirect_const_addr: true, subroutines: true, integers: false, int64_atomics: false, fp16: true, fp16_derivatives: true, fp16_const_buffers: false, fp16_no_denorms: false, int16: true, glsl_16bit_consts: true, glsl_16bit_load_dst: false, tgsi_sqrt_supported: false, tgsi_any_inout_decl_range: true },
        (1, 4) => ShaderCaps { max_instructions: 1131, max_alu_instructions: 1132, max_tex_instructions: 1133, max_tex_indirections: 1134, max_control_flow_depth: 1135, max_inputs: 1136, max_outputs: 1137, max_const_buffer0_size: 1138, max_const_buffers: 1139, max_temps: 1140, max_texture_samplers: 1141, max_sampler_views: 1142, max_shader_buffers: 1143, max_shader_images: 1144, max_hw_atomic_counters: 1145, max_hw_atomic_counter_buffers: 1146, supported_irs: 1147, cont_supported: false, indirect_temp_addr: false, indirect_const_addr: true, subroutines: true, integers: false, int64_atomics: false, fp16: true, fp16_derivatives: true, fp16_const_buffers: false, fp16_no_denorms: false, int16: true, glsl_16bit_consts: true, glsl_16bit_load_dst: false, tgsi_sqrt_supported: false, tgsi_any_inout_decl_range: true },
        (1, 5) => ShaderCaps { max_instructions: 1163, max_alu_instructions: 1164, max_tex_instructions: 1165, max_tex_indirections: 1166, max_control_flow_depth: 1167, max_inputs: 1168, max_outputs: 1169, max_const_buffer0_size: 1170, max_const_buffers: 1171, max_temps: 1172, max_texture_samplers: 1173, max_sampler_views: 1174, max_shader_buffers: 1175, max_shader_images: 1176, max_hw_atomic_counters: 1177, max_hw_atomic_counter_buffers: 1178, supported_irs: 1179, cont_supported: false, indirect_temp_addr: false, indirect_const_addr: true, subroutines: true, integers: false, int64_atomics: false, fp16: true, fp16_derivatives: true, fp16_const_buffers: false, fp16_no_denorms: false, int16: true, glsl_16bit_consts: true, glsl_16bit_load_dst: false, tgsi_sqrt_supported: false, tgsi_any_inout_decl_range: true },
        (1, 6) => ShaderCaps { max_instructions: 1195, max_alu_instructions: 1196, max_tex_instructions: 1197, max_tex_indirections: 1198, max_control_flow_depth: 1199, max_inputs: 1200, max_outputs: 1201, max_const_buffer0_size: 1202, max_const_buffers: 1203, max_temps: 1204, max_texture_samplers: 1205, max_sampler_views: 1206, max_shader_buffers: 1207, max_shader_images: 1208, max_hw_atomic_counters: 1209, max_hw_atomic_counter_buffers: 1210, supported_irs: 1211, cont_supported: false, indirect_temp_addr: false, indirect_const_addr: true, subroutines: true, integers: false, int64_atomics: false, fp16: true, fp16_derivatives: true, fp16_const_buffers: false, fp16_no_denorms: false, int16: true, glsl_16bit_consts: true, glsl_16bit_load_dst: false, tgsi_sqrt_supported: false, tgsi_any_inout_decl_range: true },
        (1, 7) => ShaderCaps { max_instructions: 1227, max_alu_instructions: 1228, max_tex_instructions: 1229, max_tex_indirections: 1230, max_control_flow_depth: 1231, max_inputs: 1232, max_outputs: 1233, max_const_buffer0_size: 1234, max_const_buffers: 1235, max_temps: 1236, max_texture_samplers: 1237, max_sampler_views: 1238, max_shader_buffers: 1239, max_shader_images: 1240, max_hw_atomic_counters: 1241, max_hw_atomic_counter_buffers: 1242, supported_irs: 1243, cont_supported: false, indirect_temp_addr: false, indirect_const_addr: true, subroutines: true, integers: false, int64_atomics: false, fp16: true, fp16_derivatives: true, fp16_const_buffers: false, fp16_no_denorms: false, int16: true, glsl_16bit_consts: true, glsl_16bit_load_dst: false, tgsi_sqrt_supported: false, tgsi_any_inout_decl_range: true },
        _ => ShaderCaps::default(),
    }
}

/// Every field of `Caps` set to a value of its own, by seed; Mesa's defaults past the seeds.
pub fn filled_caps(seed: usize) -> Caps {
    match seed {
        0 => Caps { graphics: false, npot_textures: true, anisotropic_filter: true, occlusion_query: false, query_time_elapsed: false, texture_shadow_map: true, texture_swizzle: true, texture_mirror_clamp: false, blend_equation_separate: false, primitive_restart: true, primitive_restart_fixed_index: true, indep_blend_enable: false, indep_blend_func: false, fs_coord_origin_upper_left: true, fs_coord_origin_lower_left: true, fs_coord_pixel_center_half_integer: false, fs_coord_pixel_center_integer: false, depth_clip_disable: true, depth_clip_disable_separate: true, depth_clamp_enable: false, shader_stencil_export: false, vs_instanceid: true, vertex_element_instance_divisor: true, fragment_color_clamped: false, mixed_colorbuffer_formats: false, seamless_cube_map: true, seamless_cube_map_per_texture: true, conditional_render: false, texture_barrier: false, stream_output_pause_resume: true, tgsi_can_compact_constants: true, vertex_color_unclamped: false, vertex_color_clamped: false, quads_follow_provoking_vertex_convention: true, user_vertex_buffers: true, compute: false, start_instance: false, query_timestamp: true, texture_multisample: true, cube_map_array: false, texture_buffer_objects: false, buffer_sampler_view_rgba_only: true, tgsi_texcoord: true, query_pipeline_statistics: false, mixed_framebuffer_sizes: false, vs_layer_viewport: true, texture_gather_sm5: true, buffer_map_persistent_coherent: false, fake_sw_msaa: false, texture_query_lod: true, sample_shading: true, texture_gather_offsets: false, vs_window_space_position: false, draw_indirect: true, fs_fine_derivative: true, uma: false, conditional_render_inverted: false, sampler_view_target: true, clip_halfz: true, polygon_offset_clamp: false, multisample_z_resolve: false, resource_from_user_memory: true, resource_from_user_memory_compute_only: true, device_reset_status_query: false, texture_float_linear: false, texture_half_float_linear: true, depth_bounds_test: true, native_fp32_depth: false, texture_query_samples: false, force_persample_interp: true, shareable_shaders: true, copy_between_compressed_and_plain_formats: false, clear_scissored: false, clear_masked: true, draw_parameters: true, shader_pack_half_float: false, multi_draw_indirect: false, multi_draw_indirect_params: true, multi_draw_indirect_partial_stride: true, fs_position_is_sysval: false, fs_point_is_sysval: false, fs_face_is_integer_sysval: true, invalidate_buffer: true, generate_mipmap: false, string_marker: false, surface_no_compress: true, surface_reinterpret_blocks: true, compressed_surface_reinterpret_blocks_layered: false, query_buffer_object: false, query_memory_info: true, framebuffer_no_attachment: true, robust_buffer_access_behavior: false, cull_distance: false, shader_group_vote: true, shader_array_components: true, stream_output_interleave_buffers: false, native_fence_fd: false, glsl_tess_levels_as_inputs: true, legacy_math_rules: true, fp16: false, doubles: false, int64: true, shader_clock: true, shader_realtime_clock: false, polygon_mode_fill_rectangle: false, shader_ballot: true, tes_layer_viewport: true, can_bind_const_buffer_as_vertex: false, allow_mapped_buffers_during_execution: false, post_depth_coverage: true, bindless_texture: true, nir_samplers_as_deref: false, query_so_overflow: false, memobj: true, load_constbuf: true, tile_raster_order: false, signed_vertex_buffer_offset: false, fence_signal: true, packed_uniforms: true, conservative_raster_post_snap_triangles: false, conservative_raster_post_snap_points_lines: false, conservative_raster_pre_snap_triangles: true, conservative_raster_pre_snap_points_lines: true, conservative_raster_post_depth_coverage: false, conservative_raster_inner_coverage: false, programmable_sample_locations: true, texture_mirror_clamp_to_edge: true, surface_sample_count: false, image_atomic_float_add: false, query_pipeline_statistics_single: true, dest_surface_srgb_control: true, compute_grid_info_last_block: false, compute_shader_derivatives: false, image_load_formatted: true, image_store_formatted: true, throttle: false, cl_gl_sharing: false, prefer_compute_for_multimedia: true, fragment_shader_interlock: true, fbfetch_coherent: false, atomic_float_minmax: false, fragment_shader_texture_lod: true, fragment_shader_derivatives: true, fragment_shader_depth: false, texture_shadow_lod: false, shader_samples_identical: true, image_atomic_inc_wrap: true, prefer_imm_arrays_as_constbuf: false, gl_spirv: false, gl_spirv_variable_pointers: true, demote_to_helper_invocation: true, tgsi_tg4_component_in_swizzle: false, flatshade: false, alpha_test: true, two_sided_color: true, opencl_integer_functions: false, integer_multiply_32x16: false, frontend_noop: true, nir_images_as_deref: true, packed_stream_output: false, viewport_transform_lowered: false, psiz_clamped: true, viewport_swizzle: true, system_svm: false, viewport_mask: false, alpha_to_coverage_dither_control: true, map_unsynchronized_thread_safe: true, nir_atomics_as_deref: false, no_clip_on_copy_tex: false, shader_atomic_int64: true, device_protected_surface: true, prefer_real_buffer_in_constbuf0: false, gl_clamp: false, texrect: true, sampler_reduction_minmax: true, sampler_reduction_minmax_arb: false, allow_dynamic_vao_fastpath: false, emulate_nonfixed_primitive_restart: true, prefer_back_buffer_reuse: true, draw_vertex_state: false, prefer_pot_aligned_varyings: false, sparse_texture_full_array_cube_mipmaps: true, query_sparse_texture_residency: true, clamp_sparse_texture_lod: false, allow_draw_out_of_order: false, hardware_gl_select: true, dithering: true, fbfetch_zs: false, timeline_semaphore_import: false, device_protected_context: true, allow_glthread_buffer_subdata_opt: true, null_textures: false, astc_void_extents_need_denorm_flush: false, validate_all_dirty_states: true, has_const_bw: true, performance_monitor: false, texture_sampler_independent: false, astc_decode_mode: true, shader_subgroup_quad_all_stages: true, call_finalize_nir_in_linker: false, mesh_shader: false, representative_fragment_test: true, prefer_persp: true, blit_3d: false, glsl_bindless_handles_are_32bit: false, polygon_stipple: true, disable_custom_border_color_caching: true, needs_split_planar_samplers: false, accelerated: 1465, min_texel_offset: 1466, max_texel_offset: 1467, min_texture_gather_offset: 1468, max_texture_gather_offset: 1469, max_dual_source_render_targets: 1470, max_render_targets: 1471, max_texture_2d_size: 1472, max_texture_3d_levels: 1473, max_texture_cube_levels: 1474, max_stream_output_buffers: 1475, max_texture_array_layers: 1476, max_stream_output_separate_components: 1477, max_stream_output_interleaved_components: 1478, glsl_feature_level: 1479, glsl_feature_level_compatibility: 1480, essl_feature_level: 1481, constant_buffer_offset_alignment: 1482, timer_resolution: 1483, min_map_buffer_alignment: 1484, texture_buffer_offset_alignment: 1485, linear_image_pitch_alignment: 1486, linear_image_base_address_alignment: 1487, texture_transfer_modes: 1488, texture_border_color_quirk: 1489, max_texel_buffer_elements: 1490, max_viewports: 1491, max_geometry_output_vertices: 1492, max_geometry_total_output_components: 1493, max_texture_gather_components: 1494, max_vertex_streams: 1495, vendor_id: 1496, device_id: 1497, video_memory: 1498, max_vertex_attrib_stride: 1499, max_shader_patch_varyings: 1500, shader_buffer_offset_alignment: 1501, pci_group: 1502, pci_bus: 1503, pci_device: 1504, pci_function: 1505, max_window_rectangles: 1506, viewport_subpixel_bits: 1507, rasterizer_subpixel_bits: 1508, mixed_color_depth_bits: 1509, fbfetch: 1510, blend_equation_advanced: 1511, sparse_buffer_page_size: 1512, max_combined_shader_output_resources: 1513, framebuffer_msaa_constraints: 1514, context_priority_mask: 1515, constbuf0_flags: 1516, max_conservative_raster_subpixel_precision_bias: 1517, max_gs_invocations: 1518, max_shader_buffer_size: 1519, max_combined_shader_buffers: 1520, max_combined_image_uniforms: 1521, max_combined_hw_atomic_counters: 1522, max_combined_hw_atomic_counter_buffers: 1523, max_texture_upload_memory_budget: 1524, max_vertex_element_src_offset: 1525, max_varyings: 1526, ignored_output_varyings: [1527, 1528, 1529, 1530, 1531, 1532, 1533, 1534], dmabuf: 1535, clip_planes: 1536, max_vertex_buffers: 1537, gl_begin_end_buffer_size: 1538, glsl_zero_init: 1539, max_texture_mb: 1540, supported_prim_modes: 1541, supported_prim_modes_with_restart: 1542, max_sparse_texture_size: 1543, max_sparse_3d_texture_size: 1544, max_sparse_array_texture_layers: 1545, max_constant_buffer_size: 1546, query_timestamp_bits: 1547, shader_subgroup_size: 1548, shader_subgroup_supported_stages: 1549, shader_subgroup_supported_features: 1550, shader_pixel_local_storage_size: 1551, shader_pixel_local_storage_fast_size: 1552, multiview: 1553, max_label_length: 1554, max_timeline_semaphore_difference: 1555, map_client_pointer: 1556, min_vma: 1557, max_vma: 1558, hw_clear_buffer_sizes: 1559, device_type: values::DeviceType(1560), vertex_input_alignment: values::VertexInputAlignment(1561), endianness: values::Endian(1562), point_size_fixed: values::PointSizeLowerMode(1563), min_line_width: 1564.25, min_line_width_aa: 1565.25, max_line_width: 1566.25, max_line_width_aa: 1567.25, line_width_granularity: 1568.25, min_point_size: 1569.25, min_point_size_aa: 1570.25, max_point_size: 1571.25, max_point_size_aa: 1572.25, point_size_granularity: 1573.25, max_texture_anisotropy: 1574.25, max_texture_lod_bias: 1575.25, min_conservative_raster_dilate: 1576.25, max_conservative_raster_dilate: 1577.25, conservative_raster_dilate_granularity: 1578.25, mesh: MeshCaps { max_task_work_group_total_count: 1579, max_mesh_work_group_total_count: 1580, max_mesh_work_group_invocations: 1581, max_task_work_group_invocations: 1582, max_task_payload_size: 1583, max_task_shared_memory_size: 1584, max_mesh_shared_memory_size: 1585, max_task_payload_and_shared_memory_size: 1586, max_mesh_payload_and_shared_memory_size: 1587, max_mesh_output_memory_size: 1588, max_mesh_payload_and_output_memory_size: 1589, max_mesh_output_vertices: 1590, max_mesh_output_primitives: 1591, max_mesh_output_components: 1592, max_mesh_output_layers: 1593, max_mesh_multiview_view_count: 1594, mesh_output_per_vertex_granularity: 1595, mesh_output_per_primitive_granularity: 1596, max_preferred_task_work_group_invocations: 1597, max_preferred_mesh_work_group_invocations: 1598, mesh_prefers_local_invocation_vertex_output: true, mesh_prefers_local_invocation_primitive_output: false, mesh_prefers_compact_vertex_output: false, mesh_prefers_compact_primitive_output: true, max_task_work_group_count: [1603, 1604, 1605], max_mesh_work_group_count: [1606, 1607, 1608], max_task_work_group_size: [1609, 1610, 1611], max_mesh_work_group_size: [1612, 1613, 1614], pipeline_statistic_queries: true } },
        1 => Caps { graphics: true, npot_textures: false, anisotropic_filter: false, occlusion_query: true, query_time_elapsed: true, texture_shadow_map: false, texture_swizzle: false, texture_mirror_clamp: true, blend_equation_separate: true, primitive_restart: false, primitive_restart_fixed_index: false, indep_blend_enable: true, indep_blend_func: true, fs_coord_origin_upper_left: false, fs_coord_origin_lower_left: false, fs_coord_pixel_center_half_integer: true, fs_coord_pixel_center_integer: true, depth_clip_disable: false, depth_clip_disable_separate: false, depth_clamp_enable: true, shader_stencil_export: true, vs_instanceid: false, vertex_element_instance_divisor: false, fragment_color_clamped: true, mixed_colorbuffer_formats: true, seamless_cube_map: false, seamless_cube_map_per_texture: false, conditional_render: true, texture_barrier: true, stream_output_pause_resume: false, tgsi_can_compact_constants: false, vertex_color_unclamped: true, vertex_color_clamped: true, quads_follow_provoking_vertex_convention: false, user_vertex_buffers: false, compute: true, start_instance: true, query_timestamp: false, texture_multisample: false, cube_map_array: true, texture_buffer_objects: true, buffer_sampler_view_rgba_only: false, tgsi_texcoord: false, query_pipeline_statistics: true, mixed_framebuffer_sizes: true, vs_layer_viewport: false, texture_gather_sm5: false, buffer_map_persistent_coherent: true, fake_sw_msaa: true, texture_query_lod: false, sample_shading: false, texture_gather_offsets: true, vs_window_space_position: true, draw_indirect: false, fs_fine_derivative: false, uma: true, conditional_render_inverted: true, sampler_view_target: false, clip_halfz: false, polygon_offset_clamp: true, multisample_z_resolve: true, resource_from_user_memory: false, resource_from_user_memory_compute_only: false, device_reset_status_query: true, texture_float_linear: true, texture_half_float_linear: false, depth_bounds_test: false, native_fp32_depth: true, texture_query_samples: true, force_persample_interp: false, shareable_shaders: false, copy_between_compressed_and_plain_formats: true, clear_scissored: true, clear_masked: false, draw_parameters: false, shader_pack_half_float: true, multi_draw_indirect: true, multi_draw_indirect_params: false, multi_draw_indirect_partial_stride: false, fs_position_is_sysval: true, fs_point_is_sysval: true, fs_face_is_integer_sysval: false, invalidate_buffer: false, generate_mipmap: true, string_marker: true, surface_no_compress: false, surface_reinterpret_blocks: false, compressed_surface_reinterpret_blocks_layered: true, query_buffer_object: true, query_memory_info: false, framebuffer_no_attachment: false, robust_buffer_access_behavior: true, cull_distance: true, shader_group_vote: false, shader_array_components: false, stream_output_interleave_buffers: true, native_fence_fd: true, glsl_tess_levels_as_inputs: false, legacy_math_rules: false, fp16: true, doubles: true, int64: false, shader_clock: false, shader_realtime_clock: true, polygon_mode_fill_rectangle: true, shader_ballot: false, tes_layer_viewport: false, can_bind_const_buffer_as_vertex: true, allow_mapped_buffers_during_execution: true, post_depth_coverage: false, bindless_texture: false, nir_samplers_as_deref: true, query_so_overflow: true, memobj: false, load_constbuf: false, tile_raster_order: true, signed_vertex_buffer_offset: true, fence_signal: false, packed_uniforms: false, conservative_raster_post_snap_triangles: true, conservative_raster_post_snap_points_lines: true, conservative_raster_pre_snap_triangles: false, conservative_raster_pre_snap_points_lines: false, conservative_raster_post_depth_coverage: true, conservative_raster_inner_coverage: true, programmable_sample_locations: false, texture_mirror_clamp_to_edge: false, surface_sample_count: true, image_atomic_float_add: true, query_pipeline_statistics_single: false, dest_surface_srgb_control: false, compute_grid_info_last_block: true, compute_shader_derivatives: true, image_load_formatted: false, image_store_formatted: false, throttle: true, cl_gl_sharing: true, prefer_compute_for_multimedia: false, fragment_shader_interlock: false, fbfetch_coherent: true, atomic_float_minmax: true, fragment_shader_texture_lod: false, fragment_shader_derivatives: false, fragment_shader_depth: true, texture_shadow_lod: true, shader_samples_identical: false, image_atomic_inc_wrap: false, prefer_imm_arrays_as_constbuf: true, gl_spirv: true, gl_spirv_variable_pointers: false, demote_to_helper_invocation: false, tgsi_tg4_component_in_swizzle: true, flatshade: true, alpha_test: false, two_sided_color: false, opencl_integer_functions: true, integer_multiply_32x16: true, frontend_noop: false, nir_images_as_deref: false, packed_stream_output: true, viewport_transform_lowered: true, psiz_clamped: false, viewport_swizzle: false, system_svm: true, viewport_mask: true, alpha_to_coverage_dither_control: false, map_unsynchronized_thread_safe: false, nir_atomics_as_deref: true, no_clip_on_copy_tex: true, shader_atomic_int64: false, device_protected_surface: false, prefer_real_buffer_in_constbuf0: true, gl_clamp: true, texrect: false, sampler_reduction_minmax: false, sampler_reduction_minmax_arb: true, allow_dynamic_vao_fastpath: true, emulate_nonfixed_primitive_restart: false, prefer_back_buffer_reuse: false, draw_vertex_state: true, prefer_pot_aligned_varyings: true, sparse_texture_full_array_cube_mipmaps: false, query_sparse_texture_residency: false, clamp_sparse_texture_lod: true, allow_draw_out_of_order: true, hardware_gl_select: false, dithering: false, fbfetch_zs: true, timeline_semaphore_import: true, device_protected_context: false, allow_glthread_buffer_subdata_opt: false, null_textures: true, astc_void_extents_need_denorm_flush: true, validate_all_dirty_states: false, has_const_bw: false, performance_monitor: true, texture_sampler_independent: true, astc_decode_mode: false, shader_subgroup_quad_all_stages: false, call_finalize_nir_in_linker: true, mesh_shader: true, representative_fragment_test: false, prefer_persp: false, blit_3d: true, glsl_bindless_handles_are_32bit: true, polygon_stipple: false, disable_custom_border_color_caching: false, needs_split_planar_samplers: true, accelerated: 1467, min_texel_offset: 1468, max_texel_offset: 1469, min_texture_gather_offset: 1470, max_texture_gather_offset: 1471, max_dual_source_render_targets: 1472, max_render_targets: 1473, max_texture_2d_size: 1474, max_texture_3d_levels: 1475, max_texture_cube_levels: 1476, max_stream_output_buffers: 1477, max_texture_array_layers: 1478, max_stream_output_separate_components: 1479, max_stream_output_interleaved_components: 1480, glsl_feature_level: 1481, glsl_feature_level_compatibility: 1482, essl_feature_level: 1483, constant_buffer_offset_alignment: 1484, timer_resolution: 1485, min_map_buffer_alignment: 1486, texture_buffer_offset_alignment: 1487, linear_image_pitch_alignment: 1488, linear_image_base_address_alignment: 1489, texture_transfer_modes: 1490, texture_border_color_quirk: 1491, max_texel_buffer_elements: 1492, max_viewports: 1493, max_geometry_output_vertices: 1494, max_geometry_total_output_components: 1495, max_texture_gather_components: 1496, max_vertex_streams: 1497, vendor_id: 1498, device_id: 1499, video_memory: 1500, max_vertex_attrib_stride: 1501, max_shader_patch_varyings: 1502, shader_buffer_offset_alignment: 1503, pci_group: 1504, pci_bus: 1505, pci_device: 1506, pci_function: 1507, max_window_rectangles: 1508, viewport_subpixel_bits: 1509, rasterizer_subpixel_bits: 1510, mixed_color_depth_bits: 1511, fbfetch: 1512, blend_equation_advanced: 1513, sparse_buffer_page_size: 1514, max_combined_shader_output_resources: 1515, framebuffer_msaa_constraints: 1516, context_priority_mask: 1517, constbuf0_flags: 1518, max_conservative_raster_subpixel_precision_bias: 1519, max_gs_invocations: 1520, max_shader_buffer_size: 1521, max_combined_shader_buffers: 1522, max_combined_image_uniforms: 1523, max_combined_hw_atomic_counters: 1524, max_combined_hw_atomic_counter_buffers: 1525, max_texture_upload_memory_budget: 1526, max_vertex_element_src_offset: 1527, max_varyings: 1528, ignored_output_varyings: [1529, 1530, 1531, 1532, 1533, 1534, 1535, 1536], dmabuf: 1537, clip_planes: 1538, max_vertex_buffers: 1539, gl_begin_end_buffer_size: 1540, glsl_zero_init: 1541, max_texture_mb: 1542, supported_prim_modes: 1543, supported_prim_modes_with_restart: 1544, max_sparse_texture_size: 1545, max_sparse_3d_texture_size: 1546, max_sparse_array_texture_layers: 1547, max_constant_buffer_size: 1548, query_timestamp_bits: 1549, shader_subgroup_size: 1550, shader_subgroup_supported_stages: 1551, shader_subgroup_supported_features: 1552, shader_pixel_local_storage_size: 1553, shader_pixel_local_storage_fast_size: 1554, multiview: 1555, max_label_length: 1556, max_timeline_semaphore_difference: 1557, map_client_pointer: 1558, min_vma: 1559, max_vma: 1560, hw_clear_buffer_sizes: 1561, device_type: values::DeviceType(1562), vertex_input_alignment: values::VertexInputAlignment(1563), endianness: values::Endian(1564), point_size_fixed: values::PointSizeLowerMode(1565), min_line_width: 1566.25, min_line_width_aa: 1567.25, max_line_width: 1568.25, max_line_width_aa: 1569.25, line_width_granularity: 1570.25, min_point_size: 1571.25, min_point_size_aa: 1572.25, max_point_size: 1573.25, max_point_size_aa: 1574.25, point_size_granularity: 1575.25, max_texture_anisotropy: 1576.25, max_texture_lod_bias: 1577.25, min_conservative_raster_dilate: 1578.25, max_conservative_raster_dilate: 1579.25, conservative_raster_dilate_granularity: 1580.25, mesh: MeshCaps { max_task_work_group_total_count: 1581, max_mesh_work_group_total_count: 1582, max_mesh_work_group_invocations: 1583, max_task_work_group_invocations: 1584, max_task_payload_size: 1585, max_task_shared_memory_size: 1586, max_mesh_shared_memory_size: 1587, max_task_payload_and_shared_memory_size: 1588, max_mesh_payload_and_shared_memory_size: 1589, max_mesh_output_memory_size: 1590, max_mesh_payload_and_output_memory_size: 1591, max_mesh_output_vertices: 1592, max_mesh_output_primitives: 1593, max_mesh_output_components: 1594, max_mesh_output_layers: 1595, max_mesh_multiview_view_count: 1596, mesh_output_per_vertex_granularity: 1597, mesh_output_per_primitive_granularity: 1598, max_preferred_task_work_group_invocations: 1599, max_preferred_mesh_work_group_invocations: 1600, mesh_prefers_local_invocation_vertex_output: false, mesh_prefers_local_invocation_primitive_output: true, mesh_prefers_compact_vertex_output: true, mesh_prefers_compact_primitive_output: false, max_task_work_group_count: [1605, 1606, 1607], max_mesh_work_group_count: [1608, 1609, 1610], max_task_work_group_size: [1611, 1612, 1613], max_mesh_work_group_size: [1614, 1615, 1616], pipeline_statistic_queries: false } },
        _ => Caps::default(),
    }
}

/// Every field of `ComputeCaps` set to a value of its own, by seed; Mesa's defaults past the seeds.
pub fn filled_compute_caps(seed: usize) -> ComputeCaps {
    match seed {
        0 => ComputeCaps { address_bits: 1616, grid_dimension: 1617, max_grid_size: [1618, 1619, 1620], max_block_size: [1621, 1622, 1623], max_threads_per_block: 1624, max_local_size: 1625, max_clock_frequency: 1626, max_compute_units: 1627, max_subgroups: 1628, subgroup_sizes: 1629, max_variable_threads_per_block: 1630, max_mem_alloc_size: 1631, max_global_size: 1632 },
        1 => ComputeCaps { address_bits: 1618, grid_dimension: 1619, max_grid_size: [1620, 1621, 1622], max_block_size: [1623, 1624, 1625], max_threads_per_block: 1626, max_local_size: 1627, max_clock_frequency: 1628, max_compute_units: 1629, max_subgroups: 1630, subgroup_sizes: 1631, max_variable_threads_per_block: 1632, max_mem_alloc_size: 1633, max_global_size: 1634 },
        _ => ComputeCaps::default(),
    }
}
