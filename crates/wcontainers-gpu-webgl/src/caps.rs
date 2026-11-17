//! The capability list: what WebGL 2 and the context's extensions give, as far as this back end
//! serves it. A feature WebGL 2 has whose route is not built yet is declared absent until it is.

use glow::HasContext;
use std::collections::BTreeSet;
use wcontainers_gpu::{
    AdapterInfo, Caps, Features, Format, FormatCaps, FormatKind, FormatUse, Limits,
};

use crate::format::{self, FormatExtensions};

pub fn caps(
    gl: &glow::Context,
    platform: &crate::platform::Platform,
    ext: &BTreeSet<&'static str>,
    renderer: &str,
) -> Caps {
    // SAFETY: plain queries of the current context.
    let get = |name| unsafe { gl.get_parameter_i32(name) }.max(0) as u32;
    let mut features = Features::READ_BACK_NOW
        // GL's provoking vertex is the last; its rule for every topology is Direct3D 9's and OpenGL's.
        | Features::PROVOKING_VERTEX_LAST
        // TRIANGLE_FAN and LINE_LOOP are WebGL 2 topologies.
        | Features::FAN_AND_LOOP
        // GL's own clip depth; with EXT_clip_control set per raster block.
        | Features::CLIP_DEPTH_MINUS_ONE
        | Features::INDEX_U8
        // Transform feedback is GLES 3.0 core.
        | Features::TRANSFORM_FEEDBACK
        // vertexAttribDivisor takes any rate.
        | Features::INSTANCE_STEP_RATE
        // FRACTIONAL_DEPTH_BIAS: polygonOffset takes float units, but GL may round them; set by the
        // back end's probe.
        | Features::BLEND_CONSTANT_ALPHA;
    features.set(
        Features::CLIP_DEPTH_ZERO_ONE,
        ext.contains("EXT_clip_control"),
    );
    features.set(
        Features::PROVOKING_VERTEX_FIRST,
        ext.contains("WEBGL_provoking_vertex"),
    );
    features.set(
        Features::BLOCK_COMPRESSION,
        [
            "WEBGL_compressed_texture_s3tc",
            "WEBGL_compressed_texture_s3tc_srgb",
            "EXT_texture_compression_rgtc",
            "EXT_texture_compression_bptc",
        ]
        .iter()
        .all(|e| ext.contains(e)),
    );
    let anisotropic = ext.contains("EXT_texture_filter_anisotropic");
    let units_vertex = get(glow::MAX_VERTEX_TEXTURE_IMAGE_UNITS);
    let units_fragment = get(glow::MAX_TEXTURE_IMAGE_UNITS);
    let blocks = get(glow::MAX_VERTEX_UNIFORM_BLOCKS).min(get(glow::MAX_FRAGMENT_UNIFORM_BLOCKS));
    let targets = get(glow::MAX_DRAW_BUFFERS).min(get(glow::MAX_COLOR_ATTACHMENTS));
    let uniform_alignment = get(glow::UNIFORM_BUFFER_OFFSET_ALIGNMENT);
    let limits = Limits {
        max_texture_2d: get(glow::MAX_TEXTURE_SIZE),
        max_texture_cube: get(glow::MAX_CUBE_MAP_TEXTURE_SIZE),
        max_texture_3d: get(glow::MAX_3D_TEXTURE_SIZE),
        max_texture_layers: get(glow::MAX_ARRAY_TEXTURE_LAYERS),
        // WebGL passes buffer sizes as GLsizeiptr, which glow takes as i32.
        max_buffer_size: i32::MAX as u64,
        max_uniform_buffer_size: get(glow::MAX_UNIFORM_BLOCK_SIZE),
        max_storage_buffer_size: 0,
        uniform_buffers_per_stage: blocks,
        textures_per_stage: units_vertex.min(units_fragment),
        // Combined samplers: one unit per texture and sampler pair.
        samplers_per_stage: units_vertex.min(units_fragment),
        storage_buffers_per_stage: 0,
        storage_textures_per_stage: 0,
        vertex_streams: get(glow::MAX_VERTEX_ATTRIBS),
        vertex_attributes: get(glow::MAX_VERTEX_ATTRIBS),
        // WebGL's vertexAttribPointer takes strides up to 255.
        max_vertex_stride: 255,
        color_targets: targets,
        color_bytes_per_sample: targets * 16,
        inter_stage_slots: get(glow::MAX_VARYING_VECTORS),
        min_uniform_offset_alignment: uniform_alignment,
        min_storage_offset_alignment: uniform_alignment,
        buffer_copy_alignment: 1,
        // Immediates as plain uniforms are not built yet.
        immediate_bytes: 0,
        compute_workgroup_storage: 0,
        compute_invocations: 0,
        compute_size: [0; 3],
        max_dispatch: 0,
        // Programs write gl_PointSize 1 (POINT_SIZE is absent).
        max_point_size: 1.0,
        // Lines are drawn 1 wide (WIDE_LINES is absent), whatever range GL has.
        max_line_width: 1.0,
        max_anisotropy: if anisotropic {
            // SAFETY: a plain query the extension defines.
            unsafe { gl.get_parameter_f32(glow::MAX_TEXTURE_MAX_ANISOTROPY_EXT) as u16 }
        } else {
            1
        },
        capture_buffers: get(glow::MAX_TRANSFORM_FEEDBACK_SEPARATE_ATTRIBS),
    };
    let format_ext = FormatExtensions::found(ext);
    let mut formats = FormatCaps {
        uses: [FormatUse::empty(); Format::COUNT],
        samples: [0; Format::COUNT],
    };
    for i in 0..Format::COUNT {
        // SAFETY: every index below COUNT is a Format, which is repr(u16) and fieldless.
        let format: Format = unsafe { core::mem::transmute(i as u16) };
        formats.uses[i] = format::uses(format, format_ext);
        if formats.uses[i].is_empty() {
            continue;
        }
        formats.samples[i] = 1;
        let targets = FormatUse::RENDER | FormatUse::DEPTH_STENCIL;
        // WebGL's getInternalformatParameter refuses the signed normalised formats even with
        // EXT_render_snorm, so they take one sample.
        let snorm = format::Class::Norm { render: false };
        if let (true, Some(g)) = (
            formats.uses[i].intersects(targets),
            format::gl_format(format).filter(|g| g.class != snorm),
        ) {
            formats.samples[i] |= platform
                .sample_counts(gl, g.internal)
                .into_iter()
                .filter(|c| (2..=128).contains(c) && (*c as u32).is_power_of_two())
                .fold(0, |bits, c| bits | c as u8);
            // Blits average the samples of colour formats that are not integers.
            if formats.samples[i] > 1 && format.kind() == FormatKind::Float {
                formats.uses[i] |= FormatUse::RESOLVE;
            }
        }
    }
    // WebGL names the GPU only in its renderer string; the software renderers say so in it.
    let software = ["SwiftShader", "llvmpipe", "Microsoft Basic Render Driver"]
        .iter()
        .any(|s| renderer.contains(s));
    Caps {
        adapter: AdapterInfo {
            name: renderer.into(),
            vendor: 0,
            device: 0,
            software,
        },
        features,
        limits,
        formats,
    }
}
