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
}

impl Recording {
    /// A screen recording into a new log.
    pub fn new() -> Recording {
        Recording::default()
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
    }

    fn shader_caps(&self, stage: values::MesaShaderStage, caps: &mut ShaderCaps) {
        self.log.record(format!("shader_caps stage {stage:?} caps {caps:?}"));
    }

    fn compute_caps(&self, caps: &mut ComputeCaps) {
        self.log.record(format!("compute_caps caps {caps:?}"));
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

    fn buffer_subdata(&self, arg1: &Arc<<Self::Screen as Gallium>::Resource>, usage: values::MapFlags, offset: u32, size: u32, data: &[u8]) {
        self.log.record(format!("buffer_subdata arg1 {arg1:?} usage {usage:?} offset {offset:?} size {size:?} data {data:?}"));
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
