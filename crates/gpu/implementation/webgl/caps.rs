//! The capability list: what WebGL 2 and the context's extensions give, as far as this back end
//! serves it. A feature WebGL 2 has whose route is not built yet is declared absent until it is.

use std::collections::BTreeSet;
use wcontainers_gpu::{
    caps::AdapterInfo, caps::Caps, caps::Features, caps::FormatCaps, caps::FormatUse, caps::Limits,
    format::FORMAT_COUNT, format::FORMAT_FACTS, format::Format, format::FormatKind,
};

use crate::format::{self, FormatExtensions};
use crate::platform;
use wcontainers_opengl_es::OpenGlEs;
use wcontainers_opengl_es::constants as gles;

/// The format at index `i` of [`FormatCaps`] (its number, which serde's derive reads as the variant's
/// position).
fn format_at(i: usize) -> Format {
    use serde::Deserialize;
    use serde::de::value::{Error, U32Deserializer};
    Format::deserialize(U32Deserializer::<Error>::new(i as u32))
        .expect("every index below FORMAT_COUNT is a format")
}

pub(crate) fn caps(
    gl: &impl OpenGlEs,
    unusable_uniform_blocks: u32,
    ext: &BTreeSet<&'static str>,
    renderer: &str,
) -> Caps {
    let get = |name| platform::integer(gl, name).max(0) as u32;
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
        // gl_PointSize, up to ALIASED_POINT_SIZE_RANGE; programs that write none draw 1.
        | Features::POINT_SIZE
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
    for (name, feature) in [
        ("OES_draw_buffers_indexed", Features::INDEPENDENT_BLEND),
        ("WEBGL_blend_func_extended", Features::DUAL_SOURCE_BLEND),
        // Lines only; WebGL has no point filling.
        ("WEBGL_polygon_mode", Features::POLYGON_MODE),
        ("EXT_depth_clamp", Features::DEPTH_CLAMP),
        ("EXT_polygon_offset_clamp", Features::DEPTH_BIAS_CLAMP),
        ("EXT_texture_mirror_clamp_to_edge", Features::MIRROR_ONCE),
    ] {
        features.set(feature, ext.contains(name));
    }
    // Programs run per sample where they read the sample number, and read and write the mask.
    features.set(
        Features::SAMPLE_SHADING,
        ext.contains(crate::platform::SAMPLE_VARIABLES)
            && ext.contains(crate::platform::MULTISAMPLE_INTERPOLATION),
    );
    // CLIP_DISTANCE and CULL_DISTANCE stay absent although WEBGL_clip_cull_distance is there: with
    // the distances written and enabled, Chrome on the RTX 3080 clipped nothing (not found why).
    let anisotropic = ext.contains("EXT_texture_filter_anisotropic");
    let units_vertex = get(gles::MAX_VERTEX_TEXTURE_IMAGE_UNITS);
    let units_fragment = get(gles::MAX_TEXTURE_IMAGE_UNITS);
    let blocks = get(gles::MAX_VERTEX_UNIFORM_BLOCKS).min(get(gles::MAX_FRAGMENT_UNIFORM_BLOCKS))
        - unusable_uniform_blocks;
    let targets = get(gles::MAX_DRAW_BUFFERS).min(get(gles::MAX_COLOR_ATTACHMENTS));
    let uniform_alignment = get(gles::UNIFORM_BUFFER_OFFSET_ALIGNMENT);
    let limits = Limits {
        max_texture_2d: get(gles::MAX_TEXTURE_SIZE),
        max_texture_cube: get(gles::MAX_CUBE_MAP_TEXTURE_SIZE),
        max_texture_3d: get(gles::MAX_3D_TEXTURE_SIZE),
        max_texture_layers: get(gles::MAX_ARRAY_TEXTURE_LAYERS),
        // WebGL 2 implementations take buffer sizes up to the largest 32-bit signed number.
        max_buffer_size: i32::MAX as u64,
        max_uniform_buffer_size: get(gles::MAX_UNIFORM_BLOCK_SIZE),
        max_storage_buffer_size: 0,
        uniform_buffers_per_stage: blocks,
        textures_per_stage: units_vertex.min(units_fragment),
        // Combined samplers: one unit per texture and sampler pair.
        samplers_per_stage: units_vertex.min(units_fragment),
        storage_buffers_per_stage: 0,
        storage_textures_per_stage: 0,
        vertex_streams: get(gles::MAX_VERTEX_ATTRIBS),
        vertex_attributes: get(gles::MAX_VERTEX_ATTRIBS),
        // WebGL's vertexAttribPointer takes strides up to 255.
        max_vertex_stride: 255,
        color_targets: targets,
        color_bytes_per_sample: targets * 16,
        inter_stage_slots: get(gles::MAX_VARYING_VECTORS),
        min_uniform_offset_alignment: uniform_alignment,
        min_storage_offset_alignment: uniform_alignment,
        buffer_copy_alignment: 1,
        // Immediates are plain uniforms: Direct3D 12's largest root signature (64 words) fits in 16
        // of the 224 uniform vectors WebGL 2 gives a stage at least.
        immediate_bytes: 256,
        compute_workgroup_storage: 0,
        compute_invocations: 0,
        compute_size: [0; 3],
        max_dispatch: 0,
        max_point_size: {
            let mut range = [1.0; 2];
            gl.get_floatv(gles::ALIASED_POINT_SIZE_RANGE, &mut range);
            range[1]
        },
        // Lines are drawn 1 wide (WIDE_LINES is absent), whatever range GL has.
        max_line_width: 1.0,
        max_anisotropy: if anisotropic {
            platform::float(gl, platform::MAX_TEXTURE_MAX_ANISOTROPY_EXT) as u16
        } else {
            1
        },
        capture_buffers: get(gles::MAX_TRANSFORM_FEEDBACK_SEPARATE_ATTRIBS),
        capture_components: get(gles::MAX_TRANSFORM_FEEDBACK_INTERLEAVED_COMPONENTS),
    };
    let format_ext = FormatExtensions::found(ext);
    let mut formats = FormatCaps {
        uses: [FormatUse::empty(); FORMAT_COUNT],
        samples: [0; FORMAT_COUNT],
    };
    for i in 0..FORMAT_COUNT {
        let format = format_at(i);
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
            formats.samples[i] |= platform::sample_counts(gl, g.internal)
                .into_iter()
                .filter(|c| (2..=128).contains(c) && (*c as u32).is_power_of_two())
                .fold(0, |bits, c| bits | c as u8);
            // Blits average the samples of colour formats that are not integers.
            if formats.samples[i] > 1 && FORMAT_FACTS[format as usize].kind == FormatKind::Float {
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
