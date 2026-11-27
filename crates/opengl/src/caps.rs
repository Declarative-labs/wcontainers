//! The screen's caps (`pipe_screen.caps`, `shader_caps` and `compute_caps`) on the Gallium trait,
//! from the interface's caps: its limits over Mesa's defaults, and what the driver does and does not
//! do.

use wcontainers_gallium::{
    Caps as GalliumCaps, ComputeCaps, ShaderCaps, values::MesaShaderStage,
    values::PointSizeLowerMode,
};
use wcontainers_gpu::{
    caps::Caps, caps::Features, caps::FormatUse, format::Format, reflect::SLOT_KINDS,
    reflect::SlotKind, reflect::TEXEL_BUFFER_TEXTURE_WIDTH,
};
use wcontainers_shaders::stage::STREAM_BUFFERS;

use crate::pipe::*;

/// Levels of a full chain on a side of `size` texels.
fn levels(size: u32) -> u32 {
    size.ilog2() + 1
}

/// The last level any texture the screen reports can have: a level of detail past it selects
/// that level, as one at it does.
pub fn deepest_level(ours: &Caps) -> u32 {
    let limits = &ours.limits;
    levels(
        limits
            .max_texture_2d
            .max(limits.max_texture_cube)
            .max(limits.max_texture_3d),
    ) - 1
}

/// The largest level-of-detail bias: a sampler's is added in the program, and a sample's bias is
/// clamped to it by WGSL (<https://www.w3.org/TR/WGSL/#texturesamplebias>) and Direct3D
/// (`D3D11_MIP_LOD_BIAS_MAX`).
const MAX_LOD_BIAS: f32 = 15.99;

/// The GLSL versions st/mesa's GLSL compiler takes for this driver (the core profile's, the
/// compatibility profile's) and OpenGL ES's: the versions whose programs the NIR route converts
/// and whose state the context draws with. OpenGL ES's is 3.20's, which turns on
/// `EXT_gpu_shader5`.
const GLSL_VERSION: u32 = 330;
const GLSL_COMPATIBILITY_VERSION: u32 = 130;
const ESSL_VERSION: u32 = 320;

/// OpenGL 3.1's GLSL version, the core profile's where the passed values are fewer than OpenGL
/// 3.2 requires.
const GLSL_3_1_VERSION: u32 = 140;

/// The least components OpenGL 3.2 lets a fragment program take in and a geometry program pass on
/// (<https://registry.khronos.org/OpenGL-Refpages/gl4/html/glGet.xhtml#:~:text=GL_MAX_FRAGMENT_INPUT_COMPONENTS>).
const GL_3_2_PASSED_COMPONENTS: u32 = 128;

/// The core profile's GLSL version: 3.30's where the passed values meet OpenGL 3.2's minimums,
/// otherwise OpenGL 3.1's, so that no version is reported whose minimums are not met.
pub(crate) fn glsl_version(limits: &wcontainers_gpu::caps::Limits) -> u32 {
    if passed_values(limits) * naga::VectorSize::Quad as u32 >= GL_3_2_PASSED_COMPONENTS {
        GLSL_VERSION
    } else {
        GLSL_3_1_VERSION
    }
}

/// A geometry program's limits, the least `OES_geometry_shader` allows, which are Mesa's own
/// (`main/config.h`): the stage helper sizes its records by the program's own declarations.
const GEOMETRY_EMITTED_VERTICES: u32 = MAX_GEOMETRY_OUTPUT_VERTICES;
const GEOMETRY_OUTPUT_COMPONENTS: u32 = MAX_GEOMETRY_TOTAL_OUTPUT_COMPONENTS;
const GEOMETRY_INVOCATIONS: u32 = MAX_GEOMETRY_SHADER_INVOCATIONS;

/// The values a tessellation control program writes once a patch, the least OpenGL 4.0 and
/// `OES_tessellation_shader` allow (120 components): the stage helper sizes its patch records by
/// the program's own declarations.
const PATCH_VARYINGS: u32 = 30;

/// The vertex inputs a pass-through vertex program of the stage helper takes beside the values a
/// program passes on: the position, and the point coordinate of points drawn as squares
/// (`PointsAsQuads`).
const PASS_THROUGH_INPUTS: u32 = 2;

/// The values a vertex, geometry or evaluation program may pass on: what the host passes between
/// stages, and what the stage helper's pass-through vertex program, which draws a geometry or
/// evaluation program's output and points as squares, takes as vertex inputs beside its own.
fn passed_values(limits: &wcontainers_gpu::caps::Limits) -> u32 {
    limits
        .inter_stage_slots
        .min(limits.vertex_attributes.saturating_sub(PASS_THROUGH_INPUTS))
}

/// Whether points drawn as squares can pass `gl_PointCoord` at location `at`: between stages, and
/// to the pass-through vertex program beside its position.
pub(crate) fn point_coordinate_passes(limits: &wcontainers_gpu::caps::Limits, at: u32) -> bool {
    at < limits.inter_stage_slots && at + PASS_THROUGH_INPUTS <= limits.vertex_attributes
}

/// Stream output's limits: the stage helper's (every vertex output written to its records, into
/// its four buffers), on back ends with compute work; none without.
fn stream_output(ours: &Caps) -> (u32, u32) {
    if ours.features.contains(Features::COMPUTE) {
        (
            STREAM_BUFFERS as u32,
            ours.limits.inter_stage_slots * naga::VectorSize::Quad as u32,
        )
    } else {
        (0, 0)
    }
}

/// The largest point the screen reports: the back end's, or where it draws points of one size,
/// the largest target's side, as each point is then drawn as a square (`PointsAsQuads`), which
/// covers any target whole at that size.
pub(crate) fn largest_point_size(caps: &Caps) -> f32 {
    if caps.features.contains(Features::POINT_SIZE) {
        caps.limits.max_point_size
    } else {
        caps.limits.max_texture_2d as f32
    }
}

/// The uniform blocks a stage reports, every one of them a Gallium slot and a host buffer: one of
/// the host's buffers is left for a converted program's helper constants, and one of Gallium's
/// slots, the first past these, for the blocks the driver binds itself (a geometry program's
/// squares, `draw::geometry_squares_for`).
pub(crate) fn uniform_blocks(caps: &Caps) -> u32 {
    (caps.limits.uniform_buffers_per_stage - 1).min(PIPE_MAX_CONSTANT_BUFFERS - 1)
}

/// Gallium's screen caps: its defaults, with the interface's limits over them and what the
/// context does and does not do.
pub fn fill(caps: &mut GalliumCaps, ours: &Caps) {
    let limits = &ours.limits;
    caps.max_texture_2d_size = limits.max_texture_2d;
    caps.max_texture_cube_levels = levels(limits.max_texture_cube);
    caps.max_texture_3d_levels = levels(limits.max_texture_3d);
    caps.max_texture_array_layers = limits.max_texture_layers;
    caps.max_render_targets = limits.color_targets;
    caps.max_vertex_buffers = limits.vertex_streams;
    caps.max_vertex_attrib_stride = limits.max_vertex_stride;
    caps.max_point_size = largest_point_size(ours);
    caps.max_texture_lod_bias = MAX_LOD_BIAS;
    caps.max_line_width = limits.max_line_width;
    caps.max_line_width_aa = limits.max_line_width;
    // Without wide lines the back end draws one width, to which OpenGL clamps every width.
    if !ours.features.contains(Features::WIDE_LINES) {
        caps.min_line_width = limits.max_line_width;
        caps.min_line_width_aa = limits.max_line_width;
    }
    caps.max_texture_anisotropy = f32::from(limits.max_anisotropy);
    // Samplers take their anisotropy (`state::create_sampler_state`); the back end applies it to
    // fully linear filtering only, as OpenGL lets an implementation.
    caps.anisotropic_filter = limits.max_anisotropy > 1;
    caps.constant_buffer_offset_alignment = limits.min_uniform_offset_alignment;
    caps.max_constant_buffer_size = limits.max_uniform_buffer_size;
    caps.max_varyings = passed_values(limits);
    caps.glsl_feature_level = glsl_version(limits);
    caps.glsl_feature_level_compatibility = GLSL_COMPATIBILITY_VERSION;
    caps.essl_feature_level = ESSL_VERSION;
    let filters = |format| ours.formats.uses[format as usize].contains(FormatUse::FILTER);
    caps.texture_half_float_linear = filters(Format::Rgba16Float);
    caps.texture_float_linear = filters(Format::Rgba32Float);
    caps.texture_shadow_map = true;
    // Strips restart at the largest index on the host; lists, where OpenGL drops the primitive
    // a restart cuts, and strips drawn without restart are rewritten (`RestartRewrite`).
    caps.primitive_restart_fixed_index = true;
    // Restart at any other index is rewritten too.
    caps.primitive_restart = true;
    // Views' swizzles are applied in the programs that read them (`SwizzleInShader`).
    caps.texture_swizzle = true;
    caps.depth_clip_disable = ours.features.contains(Features::DEPTH_CLAMP);
    caps.polygon_offset_clamp = ours.features.contains(Features::DEPTH_BIAS_CLAMP);
    caps.max_dual_source_render_targets =
        u32::from(ours.features.contains(Features::DUAL_SOURCE_BLEND));
    let timestamps = ours.features.contains(Features::TIMESTAMPS);
    caps.query_timestamp = timestamps;
    caps.query_time_elapsed = timestamps;
    // Texel buffers, read through 2D textures the views are copied into
    // (`TexelBufferAsTexture`), rows of `TEXEL_BUFFER_TEXTURE_WIDTH` texels.
    caps.texture_buffer_objects = true;
    caps.max_texel_buffer_elements =
        TEXEL_BUFFER_TEXTURE_WIDTH.saturating_mul(limits.max_texture_2d);
    (
        caps.max_stream_output_buffers,
        caps.max_stream_output_interleaved_components,
    ) = stream_output(ours);
    // Geometry programs, run by the stage helper as compute work.
    if ours.features.contains(Features::COMPUTE) {
        caps.max_geometry_output_vertices = GEOMETRY_EMITTED_VERTICES;
        caps.max_geometry_total_output_components = GEOMETRY_OUTPUT_COMPONENTS;
        caps.max_gs_invocations = GEOMETRY_INVOCATIONS;
        caps.max_shader_patch_varyings = PATCH_VARYINGS;
        // An evaluation program reads the levels as patch inputs, which the stage helper fills.
        caps.glsl_tess_levels_as_inputs = true;
    }
    // Compute work, storage buffers and images (in compute programs), indirect draws (their
    // arguments read back), multisampled textures, gathers, stencil texturing and framebuffers
    // without attachments: OpenGL ES 3.1's.
    caps.compute = ours.features.contains(Features::COMPUTE);
    caps.shader_buffer_offset_alignment = limits.min_storage_offset_alignment;
    caps.max_shader_buffer_size = u32::try_from(limits.max_storage_buffer_size).unwrap_or(u32::MAX);
    caps.draw_indirect = true;
    // Stores to images declared without a format, in the bound image's format: a program is
    // converted for the formats of the images bound where it declares none (`image_formats`).
    caps.image_store_formatted = true;
    caps.texture_multisample = ours.features.contains(Features::MULTISAMPLE_READ)
        && ours.features.contains(Features::SAMPLE_MASK);
    // Programs run per sample: st/mesa makes the per-sample variant of a fragment program itself,
    // and the NIR route takes inputs at an offset or a sample and the sample positions from the
    // values the host interpolates (`interpolate_elsewhere`).
    caps.sample_shading = ours.features.contains(Features::SAMPLE_SHADING);
    // The first target's texels read in fragment programs, from a copy of it each draw bound at
    // `FETCHED_UNIT` (multisampled where the target is), as st/mesa makes advanced blending:
    // `KHR_blend_equation_advanced`, not its coherent form.
    caps.fbfetch = u32::from(ours.features.contains(Features::MULTISAMPLE_READ));
    caps.max_texture_gather_components = naga::VectorSize::Quad as u32;
    caps.framebuffer_no_attachment = true;
    caps.npot_textures = true;
    caps.blend_equation_separate = true;
    let independent = ours.features.contains(Features::INDEPENDENT_BLEND);
    caps.indep_blend_enable = independent;
    caps.indep_blend_func = independent;
    // Attachments of different sizes are fitted by the pass tracker.
    caps.mixed_framebuffer_sizes = true;
    // The host's cube maps are always seamless, as OpenGL ES's are; the interface's views include
    // cube arrays.
    caps.seamless_cube_map = true;
    caps.cube_map_array = true;
    // The interface copies between formats of one texel or block size, a block-compressed one and
    // a plain one among them (`copy_texture`); st/mesa's copies between other formats of one size
    // are blits the driver makes as such copies (`blit::bit_copy`).
    caps.copy_between_compressed_and_plain_formats = true;
    // Every back end bounds a program's buffer accesses: one past a binding's end reads zero or
    // a value inside it and writes nothing outside it.
    caps.robust_buffer_access_behavior = true;
    // A context one of whose calls was refused reports itself reset by its own fault
    // (`state::guest_call`).
    caps.device_reset_status_query = true;
    caps.fragment_shader_texture_lod = true;
    // The base instance is always zero (no `start_instance`), so the instance index is OpenGL's.
    caps.vs_instanceid = true;
    // Occlusion predicates (every back end answers whether any sample passed); an exact count is
    // refused by name where the back end lacks it.
    caps.occlusion_query = true;
    // Draws and clears under a render condition wait for its answer (`PredicationWait`).
    caps.conditional_render = true;
    // Steps other than one a back end lacks are fetched in the vertex program.
    caps.vertex_element_instance_divisor = true;
    // Maps are copies the driver writes back, never the device's own memory.
    caps.buffer_map_persistent_coherent = false;
    caps.allow_mapped_buffers_during_execution = false;
    caps.texture_transfer_modes = 0;
    // The host's fragment coordinates, and depth clip spaces of both kinds (the vertex program
    // is rewritten for OpenGL's).
    caps.fs_coord_origin_upper_left = true;
    caps.fs_coord_pixel_center_half_integer = true;
    // A fragment program's face is the host's built-in (a system value), not an input.
    caps.fs_face_is_integer_sysval = true;
    caps.clip_halfz = true;
    // Uniforms packed in words into buffer 0, as the NIR route's options take them.
    caps.packed_uniforms = true;
    // Textures and samplers stay variables the program names, which the NIR route binds by
    // (Mesa's own programs, made from TGSI, otherwise name only a unit).
    caps.nir_samplers_as_deref = true;
    // OpenGL's fixed-function steps the host lacks, made by st/mesa in the programs.
    caps.flatshade = false;
    caps.alpha_test = false;
    caps.two_sided_color = false;
    caps.point_size_fixed = PointSizeLowerMode::PIPE_POINT_SIZE_LOWER_ALWAYS;
    caps.clip_planes = 0;
    caps.texrect = false;
    caps.gl_clamp = false;
}

/// The stages the stage helper runs as compute work.
pub const STAGED: &[MesaShaderStage] = &[
    MesaShaderStage::MESA_SHADER_GEOMETRY,
    MesaShaderStage::MESA_SHADER_TESS_CTRL,
    MesaShaderStage::MESA_SHADER_TESS_EVAL,
];

/// The compute grid's limits, the interface's.
pub fn fill_compute(caps: &mut ComputeCaps, ours: &Caps) {
    let limits = &ours.limits;
    caps.address_bits = u32::BITS;
    caps.grid_dimension = limits.compute_size.len() as u32;
    caps.max_grid_size = limits.compute_size.map(|_| limits.max_dispatch);
    caps.max_block_size = limits.compute_size;
    caps.max_threads_per_block = limits.compute_invocations;
    caps.max_local_size = limits.compute_workgroup_storage;
}

/// What a program of one stage may hold. The interface sets no bound on a program's size.
pub fn fill_shader(caps: &mut ShaderCaps, stage: MesaShaderStage, ours: &Caps) {
    let limits = &ours.limits;
    let unbounded = i32::MAX as u32;
    caps.max_instructions = unbounded;
    caps.max_alu_instructions = unbounded;
    caps.max_tex_instructions = unbounded;
    caps.max_tex_indirections = unbounded;
    caps.max_control_flow_depth = unbounded;
    caps.max_temps = unbounded;
    (caps.max_inputs, caps.max_outputs) = match stage {
        MesaShaderStage::MESA_SHADER_VERTEX => (limits.vertex_attributes, passed_values(limits)),
        MesaShaderStage::MESA_SHADER_FRAGMENT => (passed_values(limits), limits.color_targets),
        MesaShaderStage::MESA_SHADER_GEOMETRY
        | MesaShaderStage::MESA_SHADER_TESS_CTRL
        | MesaShaderStage::MESA_SHADER_TESS_EVAL => (passed_values(limits), passed_values(limits)),
        _ => (0, 0),
    };
    // Storage buffers and images in compute programs, within their shared binding range; storage
    // buffers in fragment programs too, which WebGPU reads and writes there (OpenGL's
    // `ARB_shader_storage_buffer_object` asks for them), and images where the back end writes
    // storage there.
    let buffers = limits
        .storage_buffers_per_stage
        .min(PIPE_MAX_SHADER_BUFFERS);
    let images = limits
        .storage_textures_per_stage
        .min(PIPE_MAX_SHADER_IMAGES)
        .min(SLOT_KINDS[SlotKind::Storage as usize].slots - buffers);
    // Geometry and control programs run as compute work in the stage helper (`GeometryAsCompute`,
    // `TessellationAsCompute`), which reads and writes storage as compute programs do; a vertex
    // or evaluation program using storage too, before a pass-through geometry program
    // (`VertexStorageAsCompute`), where the back end runs compute work.
    let staged = ours.features.contains(Features::COMPUTE);
    if matches!(
        stage,
        MesaShaderStage::MESA_SHADER_COMPUTE
            | MesaShaderStage::MESA_SHADER_GEOMETRY
            | MesaShaderStage::MESA_SHADER_TESS_CTRL
            | MesaShaderStage::MESA_SHADER_TESS_EVAL
    ) || (stage == MesaShaderStage::MESA_SHADER_VERTEX && staged)
    {
        caps.max_shader_buffers = buffers;
        caps.max_shader_images = images;
    }
    if stage == MesaShaderStage::MESA_SHADER_FRAGMENT {
        caps.max_shader_buffers = buffers;
        if ours.features.contains(Features::FRAGMENT_STORAGE_WRITE) {
            caps.max_shader_images = images;
        }
    }
    caps.max_const_buffer0_size = limits.max_uniform_buffer_size;
    caps.max_const_buffers = uniform_blocks(ours);
    // Each OpenGL sampler is split into a texture and a sampler at the same binding, each within
    // its kind's range of the stage's bindings.
    let samplers = limits
        .samplers_per_stage
        .min(limits.textures_per_stage)
        .min(SLOT_KINDS[SlotKind::Sampler as usize].slots)
        .min(SLOT_KINDS[SlotKind::Texture as usize].slots)
        .min(PIPE_MAX_SAMPLERS);
    caps.max_texture_samplers = samplers;
    caps.max_sampler_views = samplers;
    caps.supported_irs = 1 << PIPE_SHADER_IR_NIR as u32;
    caps.integers = true;
    caps.cont_supported = true;
    caps.indirect_temp_addr = true;
    caps.indirect_const_addr = true;
}
