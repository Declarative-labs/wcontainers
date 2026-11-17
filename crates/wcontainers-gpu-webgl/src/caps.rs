//! The capability list: what WebGL 2 and the context's extensions give, as far as this back end
//! serves it. A feature WebGL 2 has whose route is not built yet is declared absent until it is.

use glow::HasContext;
use std::collections::BTreeSet;
use wcontainers_gpu::{Caps, Features, Format, FormatCaps, FormatUse, Limits};

use crate::format::{self, FormatExtensions};

pub fn caps(gl: &glow::Context, ext: &BTreeSet<&'static str>) -> Caps {
    // SAFETY: plain queries of the current context.
    let get = |name| unsafe { gl.get_parameter_i32(name) }.max(0) as u32;
    let range = |name| {
        let mut r = [1.0f32; 2];
        // SAFETY: a plain query of a two-float range.
        unsafe { gl.get_parameter_f32_slice(name, &mut r) };
        r[1]
    };
    let mut features = Features::READ_BACK_NOW
        // GL's provoking vertex is the last; its rule for every topology is Direct3D 9's and OpenGL's.
        | Features::PROVOKING_VERTEX_LAST
        // GL's own clip depth; with EXT_clip_control set per raster block.
        | Features::CLIP_DEPTH_MINUS_ONE
        | Features::INDEX_U8
        // polygonOffset takes float units.
        | Features::FRACTIONAL_DEPTH_BIAS
        | Features::BLEND_CONSTANT_ALPHA;
    features.set(
        Features::CLIP_DEPTH_ZERO_ONE,
        ext.contains("EXT_clip_control"),
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
        max_point_size: range(glow::ALIASED_POINT_SIZE_RANGE),
        max_line_width: range(glow::ALIASED_LINE_WIDTH_RANGE),
        max_anisotropy: if anisotropic {
            // SAFETY: a plain query the extension defines.
            unsafe { gl.get_parameter_f32(glow::MAX_TEXTURE_MAX_ANISOTROPY_EXT) as u16 }
        } else {
            1
        },
        capture_buffers: 0,
    };
    let format_ext = FormatExtensions {
        color_buffer_float: ext.contains("EXT_color_buffer_float"),
        float_linear: ext.contains("OES_texture_float_linear"),
        float_blend: ext.contains("EXT_float_blend"),
    };
    let mut formats = FormatCaps {
        uses: [FormatUse::empty(); Format::COUNT],
        // Multisampled targets (renderbuffers) are not built yet: one sample only.
        samples: [0; Format::COUNT],
    };
    for i in 0..Format::COUNT {
        // SAFETY: every index below COUNT is a Format, which is repr(u16) and fieldless.
        let format: Format = unsafe { core::mem::transmute(i as u16) };
        formats.uses[i] = format::uses(format, format_ext);
        if !formats.uses[i].is_empty() {
            formats.samples[i] = 1;
        }
    }
    Caps {
        adapter: Default::default(),
        features,
        limits,
        formats,
    }
}
