// Written by tools/api-crate-generator from Khronos's gl.xml (and ANGLE's gl_angle_ext.xml for ANGLE's extensions), pinned in src/khronos/write.rs. Do not edit.
//! Every command of the set as a safe method: the checks of its parameters' kinds, then one call of its entry point.

use core::ffi::c_void;

use wcontainers_opengl_es::constants;
use wcontainers_opengl_es::{CompressedPixelUnpack, PixelPack, PixelUnpack};

use crate::{Fence, OpenGlEsContext, runtime};

/// The library's entry points, each null when the library gives none.
pub(crate) struct Entries {
    pub active_texture: Option<unsafe extern "system" fn(u32)>,
    pub attach_shader: Option<unsafe extern "system" fn(u32, u32)>,
    pub begin_query: Option<unsafe extern "system" fn(u32, u32)>,
    pub begin_transform_feedback: Option<unsafe extern "system" fn(u32)>,
    pub bind_buffer: Option<unsafe extern "system" fn(u32, u32)>,
    pub bind_buffer_base: Option<unsafe extern "system" fn(u32, u32, u32)>,
    pub bind_buffer_range: Option<unsafe extern "system" fn(u32, u32, u32, isize, isize)>,
    pub bind_framebuffer: Option<unsafe extern "system" fn(u32, u32)>,
    pub bind_renderbuffer: Option<unsafe extern "system" fn(u32, u32)>,
    pub bind_sampler: Option<unsafe extern "system" fn(u32, u32)>,
    pub bind_texture: Option<unsafe extern "system" fn(u32, u32)>,
    pub bind_vertex_array: Option<unsafe extern "system" fn(u32)>,
    pub blend_color: Option<unsafe extern "system" fn(f32, f32, f32, f32)>,
    pub blend_equation_separate: Option<unsafe extern "system" fn(u32, u32)>,
    pub blend_func_separate: Option<unsafe extern "system" fn(u32, u32, u32, u32)>,
    pub blit_framebuffer: Option<unsafe extern "system" fn(i32, i32, i32, i32, i32, i32, i32, i32, u32, u32)>,
    pub buffer_data: Option<unsafe extern "system" fn(u32, isize, *const c_void, u32)>,
    pub buffer_sub_data: Option<unsafe extern "system" fn(u32, isize, isize, *const c_void)>,
    pub check_framebuffer_status: Option<unsafe extern "system" fn(u32) -> u32>,
    pub clear_bufferfi: Option<unsafe extern "system" fn(u32, i32, f32, i32)>,
    pub clear_bufferfv: Option<unsafe extern "system" fn(u32, i32, *const f32)>,
    pub clear_bufferiv: Option<unsafe extern "system" fn(u32, i32, *const i32)>,
    pub clear_bufferuiv: Option<unsafe extern "system" fn(u32, i32, *const u32)>,
    pub client_wait_sync: Option<unsafe extern "system" fn(*mut c_void, u32, u64) -> u32>,
    pub color_mask: Option<unsafe extern "system" fn(u8, u8, u8, u8)>,
    pub compile_shader: Option<unsafe extern "system" fn(u32)>,
    pub compressed_tex_sub_image_2d: Option<unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, u32, i32, *const c_void)>,
    pub compressed_tex_sub_image_3d: Option<unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, i32, i32, u32, i32, *const c_void)>,
    pub copy_buffer_sub_data: Option<unsafe extern "system" fn(u32, u32, isize, isize, isize)>,
    pub copy_tex_sub_image_2d: Option<unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, i32, i32)>,
    pub copy_tex_sub_image_3d: Option<unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, i32, i32, i32)>,
    pub create_program: Option<unsafe extern "system" fn() -> u32>,
    pub create_shader: Option<unsafe extern "system" fn(u32) -> u32>,
    pub cull_face: Option<unsafe extern "system" fn(u32)>,
    pub delete_buffers: Option<unsafe extern "system" fn(i32, *const u32)>,
    pub delete_framebuffers: Option<unsafe extern "system" fn(i32, *const u32)>,
    pub delete_program: Option<unsafe extern "system" fn(u32)>,
    pub delete_queries: Option<unsafe extern "system" fn(i32, *const u32)>,
    pub delete_renderbuffers: Option<unsafe extern "system" fn(i32, *const u32)>,
    pub delete_samplers: Option<unsafe extern "system" fn(i32, *const u32)>,
    pub delete_shader: Option<unsafe extern "system" fn(u32)>,
    pub delete_sync: Option<unsafe extern "system" fn(*mut c_void)>,
    pub delete_textures: Option<unsafe extern "system" fn(i32, *const u32)>,
    pub delete_vertex_arrays: Option<unsafe extern "system" fn(i32, *const u32)>,
    pub depth_func: Option<unsafe extern "system" fn(u32)>,
    pub depth_mask: Option<unsafe extern "system" fn(u8)>,
    pub depth_rangef: Option<unsafe extern "system" fn(f32, f32)>,
    pub detach_shader: Option<unsafe extern "system" fn(u32, u32)>,
    pub disable: Option<unsafe extern "system" fn(u32)>,
    pub disable_vertex_attrib_array: Option<unsafe extern "system" fn(u32)>,
    pub draw_arrays: Option<unsafe extern "system" fn(u32, i32, i32)>,
    pub draw_arrays_instanced: Option<unsafe extern "system" fn(u32, i32, i32, i32)>,
    pub draw_buffers: Option<unsafe extern "system" fn(i32, *const u32)>,
    pub draw_elements_instanced: Option<unsafe extern "system" fn(u32, i32, u32, *const c_void, i32)>,
    pub enable: Option<unsafe extern "system" fn(u32)>,
    pub enable_vertex_attrib_array: Option<unsafe extern "system" fn(u32)>,
    pub end_query: Option<unsafe extern "system" fn(u32)>,
    pub end_transform_feedback: Option<unsafe extern "system" fn()>,
    pub fence_sync: Option<unsafe extern "system" fn(u32, u32) -> *mut c_void>,
    pub finish: Option<unsafe extern "system" fn()>,
    pub flush: Option<unsafe extern "system" fn()>,
    pub framebuffer_renderbuffer: Option<unsafe extern "system" fn(u32, u32, u32, u32)>,
    pub framebuffer_texture_2d: Option<unsafe extern "system" fn(u32, u32, u32, u32, i32)>,
    pub framebuffer_texture_layer: Option<unsafe extern "system" fn(u32, u32, u32, i32, i32)>,
    pub front_face: Option<unsafe extern "system" fn(u32)>,
    pub gen_buffers: Option<unsafe extern "system" fn(i32, *mut u32)>,
    pub gen_framebuffers: Option<unsafe extern "system" fn(i32, *mut u32)>,
    pub gen_queries: Option<unsafe extern "system" fn(i32, *mut u32)>,
    pub gen_renderbuffers: Option<unsafe extern "system" fn(i32, *mut u32)>,
    pub gen_samplers: Option<unsafe extern "system" fn(i32, *mut u32)>,
    pub gen_textures: Option<unsafe extern "system" fn(i32, *mut u32)>,
    pub gen_vertex_arrays: Option<unsafe extern "system" fn(i32, *mut u32)>,
    pub generate_mipmap: Option<unsafe extern "system" fn(u32)>,
    pub get_booleanv: Option<unsafe extern "system" fn(u32, *mut u8)>,
    pub get_error: Option<unsafe extern "system" fn() -> u32>,
    pub get_floatv: Option<unsafe extern "system" fn(u32, *mut f32)>,
    pub get_integer64v: Option<unsafe extern "system" fn(u32, *mut i64)>,
    pub get_integerv: Option<unsafe extern "system" fn(u32, *mut i32)>,
    pub get_internalformativ: Option<unsafe extern "system" fn(u32, u32, u32, i32, *mut i32)>,
    pub get_program_info_log: Option<unsafe extern "system" fn(u32, i32, *mut i32, *mut u8)>,
    pub get_programiv: Option<unsafe extern "system" fn(u32, u32, *mut i32)>,
    pub get_query_objectuiv: Option<unsafe extern "system" fn(u32, u32, *mut u32)>,
    pub get_shader_info_log: Option<unsafe extern "system" fn(u32, i32, *mut i32, *mut u8)>,
    pub get_shaderiv: Option<unsafe extern "system" fn(u32, u32, *mut i32)>,
    pub get_string: Option<unsafe extern "system" fn(u32) -> *const u8>,
    pub get_stringi: Option<unsafe extern "system" fn(u32, u32) -> *const u8>,
    pub get_synciv: Option<unsafe extern "system" fn(*mut c_void, u32, i32, *mut i32, *mut i32)>,
    pub get_uniform_block_index: Option<unsafe extern "system" fn(u32, *const u8) -> u32>,
    pub get_uniform_location: Option<unsafe extern "system" fn(u32, *const u8) -> i32>,
    pub get_vertex_attribiv: Option<unsafe extern "system" fn(u32, u32, *mut i32)>,
    pub invalidate_framebuffer: Option<unsafe extern "system" fn(u32, i32, *const u32)>,
    pub link_program: Option<unsafe extern "system" fn(u32)>,
    pub pixel_storei: Option<unsafe extern "system" fn(u32, i32)>,
    pub polygon_offset: Option<unsafe extern "system" fn(f32, f32)>,
    pub read_pixels: Option<unsafe extern "system" fn(i32, i32, i32, i32, u32, u32, *mut c_void)>,
    pub renderbuffer_storage: Option<unsafe extern "system" fn(u32, u32, i32, i32)>,
    pub renderbuffer_storage_multisample: Option<unsafe extern "system" fn(u32, i32, u32, i32, i32)>,
    pub sampler_parameterf: Option<unsafe extern "system" fn(u32, u32, f32)>,
    pub sampler_parameteri: Option<unsafe extern "system" fn(u32, u32, i32)>,
    pub scissor: Option<unsafe extern "system" fn(i32, i32, i32, i32)>,
    pub shader_source: Option<unsafe extern "system" fn(u32, i32, *const *const u8, *const i32)>,
    pub stencil_func: Option<unsafe extern "system" fn(u32, i32, u32)>,
    pub stencil_func_separate: Option<unsafe extern "system" fn(u32, u32, i32, u32)>,
    pub stencil_mask: Option<unsafe extern "system" fn(u32)>,
    pub stencil_op: Option<unsafe extern "system" fn(u32, u32, u32)>,
    pub stencil_op_separate: Option<unsafe extern "system" fn(u32, u32, u32, u32)>,
    pub tex_parameteri: Option<unsafe extern "system" fn(u32, u32, i32)>,
    pub tex_storage_2d: Option<unsafe extern "system" fn(u32, i32, u32, i32, i32)>,
    pub tex_storage_3d: Option<unsafe extern "system" fn(u32, i32, u32, i32, i32, i32)>,
    pub tex_sub_image_2d: Option<unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, u32, u32, *const c_void)>,
    pub tex_sub_image_3d: Option<unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, i32, i32, u32, u32, *const c_void)>,
    pub transform_feedback_varyings: Option<unsafe extern "system" fn(u32, i32, *const *const u8, u32)>,
    pub uniform1f: Option<unsafe extern "system" fn(i32, f32)>,
    pub uniform1fv: Option<unsafe extern "system" fn(i32, i32, *const f32)>,
    pub uniform1i: Option<unsafe extern "system" fn(i32, i32)>,
    pub uniform1iv: Option<unsafe extern "system" fn(i32, i32, *const i32)>,
    pub uniform1ui: Option<unsafe extern "system" fn(i32, u32)>,
    pub uniform1uiv: Option<unsafe extern "system" fn(i32, i32, *const u32)>,
    pub uniform2f: Option<unsafe extern "system" fn(i32, f32, f32)>,
    pub uniform2fv: Option<unsafe extern "system" fn(i32, i32, *const f32)>,
    pub uniform2iv: Option<unsafe extern "system" fn(i32, i32, *const i32)>,
    pub uniform2uiv: Option<unsafe extern "system" fn(i32, i32, *const u32)>,
    pub uniform3fv: Option<unsafe extern "system" fn(i32, i32, *const f32)>,
    pub uniform3i: Option<unsafe extern "system" fn(i32, i32, i32, i32)>,
    pub uniform3iv: Option<unsafe extern "system" fn(i32, i32, *const i32)>,
    pub uniform3uiv: Option<unsafe extern "system" fn(i32, i32, *const u32)>,
    pub uniform4fv: Option<unsafe extern "system" fn(i32, i32, *const f32)>,
    pub uniform4iv: Option<unsafe extern "system" fn(i32, i32, *const i32)>,
    pub uniform4uiv: Option<unsafe extern "system" fn(i32, i32, *const u32)>,
    pub uniform_block_binding: Option<unsafe extern "system" fn(u32, u32, u32)>,
    pub uniform_matrix2fv: Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>,
    pub uniform_matrix2x3fv: Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>,
    pub uniform_matrix2x4fv: Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>,
    pub uniform_matrix3fv: Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>,
    pub uniform_matrix3x2fv: Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>,
    pub uniform_matrix3x4fv: Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>,
    pub uniform_matrix4fv: Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>,
    pub uniform_matrix4x2fv: Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>,
    pub uniform_matrix4x3fv: Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>,
    pub use_program: Option<unsafe extern "system" fn(u32)>,
    pub vertex_attrib_divisor: Option<unsafe extern "system" fn(u32, u32)>,
    pub vertex_attrib_i_pointer: Option<unsafe extern "system" fn(u32, i32, u32, i32, *const c_void)>,
    pub vertex_attrib_pointer: Option<unsafe extern "system" fn(u32, i32, u32, u8, i32, *const c_void)>,
    pub viewport: Option<unsafe extern "system" fn(i32, i32, i32, i32)>,
    pub blend_equation_separatei_oes: Option<unsafe extern "system" fn(u32, u32, u32)>,
    pub blend_func_separatei_oes: Option<unsafe extern "system" fn(u32, u32, u32, u32, u32)>,
    pub clip_control_ext: Option<unsafe extern "system" fn(u32, u32)>,
    pub color_maski_oes: Option<unsafe extern "system" fn(u32, u8, u8, u8, u8)>,
    pub disablei_oes: Option<unsafe extern "system" fn(u32, u32)>,
    pub enablei_oes: Option<unsafe extern "system" fn(u32, u32)>,
    pub get_query_objectui64v_ext: Option<unsafe extern "system" fn(u32, u32, *mut u64)>,
    pub get_queryiv_ext: Option<unsafe extern "system" fn(u32, u32, *mut i32)>,
    pub polygon_mode_angle: Option<unsafe extern "system" fn(u32, u32)>,
    pub polygon_offset_clamp_ext: Option<unsafe extern "system" fn(f32, f32, f32)>,
    pub provoking_vertex_angle: Option<unsafe extern "system" fn(u32)>,
    pub query_counter_ext: Option<unsafe extern "system" fn(u32, u32)>,
    pub map_buffer_range: Option<unsafe extern "system" fn(u32, isize, isize, u32) -> *mut c_void>,
    pub unmap_buffer: Option<unsafe extern "system" fn(u32) -> u8>,
}

impl Entries {
    /// Asks the library for every entry point by name.
    pub(crate) fn load(load: impl Fn(&core::ffi::CStr) -> *const c_void) -> Entries {
        Entries {
            active_texture: {
                let address = load(c"glActiveTexture");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glActiveTexture is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            attach_shader: {
                let address = load(c"glAttachShader");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glAttachShader is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32)>>(address) }
            },
            begin_query: {
                let address = load(c"glBeginQuery");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBeginQuery is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32)>>(address) }
            },
            begin_transform_feedback: {
                let address = load(c"glBeginTransformFeedback");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBeginTransformFeedback is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            bind_buffer: {
                let address = load(c"glBindBuffer");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBindBuffer is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32)>>(address) }
            },
            bind_buffer_base: {
                let address = load(c"glBindBufferBase");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBindBufferBase is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, u32)>>(address) }
            },
            bind_buffer_range: {
                let address = load(c"glBindBufferRange");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBindBufferRange is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, u32, isize, isize)>>(address) }
            },
            bind_framebuffer: {
                let address = load(c"glBindFramebuffer");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBindFramebuffer is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32)>>(address) }
            },
            bind_renderbuffer: {
                let address = load(c"glBindRenderbuffer");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBindRenderbuffer is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32)>>(address) }
            },
            bind_sampler: {
                let address = load(c"glBindSampler");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBindSampler is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32)>>(address) }
            },
            bind_texture: {
                let address = load(c"glBindTexture");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBindTexture is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32)>>(address) }
            },
            bind_vertex_array: {
                let address = load(c"glBindVertexArray");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBindVertexArray is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            blend_color: {
                let address = load(c"glBlendColor");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBlendColor is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(f32, f32, f32, f32)>>(address) }
            },
            blend_equation_separate: {
                let address = load(c"glBlendEquationSeparate");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBlendEquationSeparate is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32)>>(address) }
            },
            blend_func_separate: {
                let address = load(c"glBlendFuncSeparate");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBlendFuncSeparate is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, u32, u32)>>(address) }
            },
            blit_framebuffer: {
                let address = load(c"glBlitFramebuffer");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBlitFramebuffer is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, i32, i32, i32, i32, i32, i32, u32, u32)>>(address) }
            },
            buffer_data: {
                let address = load(c"glBufferData");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBufferData is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, isize, *const c_void, u32)>>(address) }
            },
            buffer_sub_data: {
                let address = load(c"glBufferSubData");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBufferSubData is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, isize, isize, *const c_void)>>(address) }
            },
            check_framebuffer_status: {
                let address = load(c"glCheckFramebufferStatus");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glCheckFramebufferStatus is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32) -> u32>>(address) }
            },
            clear_bufferfi: {
                let address = load(c"glClearBufferfi");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glClearBufferfi is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, f32, i32)>>(address) }
            },
            clear_bufferfv: {
                let address = load(c"glClearBufferfv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glClearBufferfv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, *const f32)>>(address) }
            },
            clear_bufferiv: {
                let address = load(c"glClearBufferiv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glClearBufferiv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, *const i32)>>(address) }
            },
            clear_bufferuiv: {
                let address = load(c"glClearBufferuiv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glClearBufferuiv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, *const u32)>>(address) }
            },
            client_wait_sync: {
                let address = load(c"glClientWaitSync");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glClientWaitSync is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*mut c_void, u32, u64) -> u32>>(address) }
            },
            color_mask: {
                let address = load(c"glColorMask");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glColorMask is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u8, u8, u8, u8)>>(address) }
            },
            compile_shader: {
                let address = load(c"glCompileShader");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glCompileShader is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            compressed_tex_sub_image_2d: {
                let address = load(c"glCompressedTexSubImage2D");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glCompressedTexSubImage2D is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, u32, i32, *const c_void)>>(address) }
            },
            compressed_tex_sub_image_3d: {
                let address = load(c"glCompressedTexSubImage3D");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glCompressedTexSubImage3D is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, i32, i32, u32, i32, *const c_void)>>(address) }
            },
            copy_buffer_sub_data: {
                let address = load(c"glCopyBufferSubData");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glCopyBufferSubData is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, isize, isize, isize)>>(address) }
            },
            copy_tex_sub_image_2d: {
                let address = load(c"glCopyTexSubImage2D");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glCopyTexSubImage2D is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, i32, i32)>>(address) }
            },
            copy_tex_sub_image_3d: {
                let address = load(c"glCopyTexSubImage3D");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glCopyTexSubImage3D is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, i32, i32, i32)>>(address) }
            },
            create_program: {
                let address = load(c"glCreateProgram");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glCreateProgram is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn() -> u32>>(address) }
            },
            create_shader: {
                let address = load(c"glCreateShader");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glCreateShader is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32) -> u32>>(address) }
            },
            cull_face: {
                let address = load(c"glCullFace");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glCullFace is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            delete_buffers: {
                let address = load(c"glDeleteBuffers");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDeleteBuffers is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, *const u32)>>(address) }
            },
            delete_framebuffers: {
                let address = load(c"glDeleteFramebuffers");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDeleteFramebuffers is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, *const u32)>>(address) }
            },
            delete_program: {
                let address = load(c"glDeleteProgram");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDeleteProgram is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            delete_queries: {
                let address = load(c"glDeleteQueries");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDeleteQueries is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, *const u32)>>(address) }
            },
            delete_renderbuffers: {
                let address = load(c"glDeleteRenderbuffers");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDeleteRenderbuffers is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, *const u32)>>(address) }
            },
            delete_samplers: {
                let address = load(c"glDeleteSamplers");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDeleteSamplers is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, *const u32)>>(address) }
            },
            delete_shader: {
                let address = load(c"glDeleteShader");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDeleteShader is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            delete_sync: {
                let address = load(c"glDeleteSync");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDeleteSync is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*mut c_void)>>(address) }
            },
            delete_textures: {
                let address = load(c"glDeleteTextures");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDeleteTextures is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, *const u32)>>(address) }
            },
            delete_vertex_arrays: {
                let address = load(c"glDeleteVertexArrays");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDeleteVertexArrays is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, *const u32)>>(address) }
            },
            depth_func: {
                let address = load(c"glDepthFunc");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDepthFunc is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            depth_mask: {
                let address = load(c"glDepthMask");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDepthMask is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u8)>>(address) }
            },
            depth_rangef: {
                let address = load(c"glDepthRangef");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDepthRangef is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(f32, f32)>>(address) }
            },
            detach_shader: {
                let address = load(c"glDetachShader");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDetachShader is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32)>>(address) }
            },
            disable: {
                let address = load(c"glDisable");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDisable is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            disable_vertex_attrib_array: {
                let address = load(c"glDisableVertexAttribArray");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDisableVertexAttribArray is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            draw_arrays: {
                let address = load(c"glDrawArrays");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDrawArrays is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, i32)>>(address) }
            },
            draw_arrays_instanced: {
                let address = load(c"glDrawArraysInstanced");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDrawArraysInstanced is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, i32, i32)>>(address) }
            },
            draw_buffers: {
                let address = load(c"glDrawBuffers");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDrawBuffers is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, *const u32)>>(address) }
            },
            draw_elements_instanced: {
                let address = load(c"glDrawElementsInstanced");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDrawElementsInstanced is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, u32, *const c_void, i32)>>(address) }
            },
            enable: {
                let address = load(c"glEnable");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glEnable is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            enable_vertex_attrib_array: {
                let address = load(c"glEnableVertexAttribArray");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glEnableVertexAttribArray is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            end_query: {
                let address = load(c"glEndQuery");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glEndQuery is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            end_transform_feedback: {
                let address = load(c"glEndTransformFeedback");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glEndTransformFeedback is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn()>>(address) }
            },
            fence_sync: {
                let address = load(c"glFenceSync");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glFenceSync is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32) -> *mut c_void>>(address) }
            },
            finish: {
                let address = load(c"glFinish");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glFinish is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn()>>(address) }
            },
            flush: {
                let address = load(c"glFlush");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glFlush is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn()>>(address) }
            },
            framebuffer_renderbuffer: {
                let address = load(c"glFramebufferRenderbuffer");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glFramebufferRenderbuffer is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, u32, u32)>>(address) }
            },
            framebuffer_texture_2d: {
                let address = load(c"glFramebufferTexture2D");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glFramebufferTexture2D is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, u32, u32, i32)>>(address) }
            },
            framebuffer_texture_layer: {
                let address = load(c"glFramebufferTextureLayer");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glFramebufferTextureLayer is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, u32, i32, i32)>>(address) }
            },
            front_face: {
                let address = load(c"glFrontFace");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glFrontFace is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            gen_buffers: {
                let address = load(c"glGenBuffers");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGenBuffers is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, *mut u32)>>(address) }
            },
            gen_framebuffers: {
                let address = load(c"glGenFramebuffers");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGenFramebuffers is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, *mut u32)>>(address) }
            },
            gen_queries: {
                let address = load(c"glGenQueries");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGenQueries is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, *mut u32)>>(address) }
            },
            gen_renderbuffers: {
                let address = load(c"glGenRenderbuffers");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGenRenderbuffers is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, *mut u32)>>(address) }
            },
            gen_samplers: {
                let address = load(c"glGenSamplers");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGenSamplers is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, *mut u32)>>(address) }
            },
            gen_textures: {
                let address = load(c"glGenTextures");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGenTextures is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, *mut u32)>>(address) }
            },
            gen_vertex_arrays: {
                let address = load(c"glGenVertexArrays");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGenVertexArrays is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, *mut u32)>>(address) }
            },
            generate_mipmap: {
                let address = load(c"glGenerateMipmap");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGenerateMipmap is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            get_booleanv: {
                let address = load(c"glGetBooleanv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetBooleanv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, *mut u8)>>(address) }
            },
            get_error: {
                let address = load(c"glGetError");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetError is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn() -> u32>>(address) }
            },
            get_floatv: {
                let address = load(c"glGetFloatv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetFloatv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, *mut f32)>>(address) }
            },
            get_integer64v: {
                let address = load(c"glGetInteger64v");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetInteger64v is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, *mut i64)>>(address) }
            },
            get_integerv: {
                let address = load(c"glGetIntegerv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetIntegerv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, *mut i32)>>(address) }
            },
            get_internalformativ: {
                let address = load(c"glGetInternalformativ");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetInternalformativ is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, u32, i32, *mut i32)>>(address) }
            },
            get_program_info_log: {
                let address = load(c"glGetProgramInfoLog");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetProgramInfoLog is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, *mut i32, *mut u8)>>(address) }
            },
            get_programiv: {
                let address = load(c"glGetProgramiv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetProgramiv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, *mut i32)>>(address) }
            },
            get_query_objectuiv: {
                let address = load(c"glGetQueryObjectuiv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetQueryObjectuiv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, *mut u32)>>(address) }
            },
            get_shader_info_log: {
                let address = load(c"glGetShaderInfoLog");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetShaderInfoLog is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, *mut i32, *mut u8)>>(address) }
            },
            get_shaderiv: {
                let address = load(c"glGetShaderiv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetShaderiv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, *mut i32)>>(address) }
            },
            get_string: {
                let address = load(c"glGetString");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetString is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32) -> *const u8>>(address) }
            },
            get_stringi: {
                let address = load(c"glGetStringi");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetStringi is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32) -> *const u8>>(address) }
            },
            get_synciv: {
                let address = load(c"glGetSynciv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetSynciv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*mut c_void, u32, i32, *mut i32, *mut i32)>>(address) }
            },
            get_uniform_block_index: {
                let address = load(c"glGetUniformBlockIndex");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetUniformBlockIndex is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, *const u8) -> u32>>(address) }
            },
            get_uniform_location: {
                let address = load(c"glGetUniformLocation");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetUniformLocation is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, *const u8) -> i32>>(address) }
            },
            get_vertex_attribiv: {
                let address = load(c"glGetVertexAttribiv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetVertexAttribiv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, *mut i32)>>(address) }
            },
            invalidate_framebuffer: {
                let address = load(c"glInvalidateFramebuffer");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glInvalidateFramebuffer is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, *const u32)>>(address) }
            },
            link_program: {
                let address = load(c"glLinkProgram");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glLinkProgram is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            pixel_storei: {
                let address = load(c"glPixelStorei");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glPixelStorei is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32)>>(address) }
            },
            polygon_offset: {
                let address = load(c"glPolygonOffset");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glPolygonOffset is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(f32, f32)>>(address) }
            },
            read_pixels: {
                let address = load(c"glReadPixels");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glReadPixels is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, i32, i32, u32, u32, *mut c_void)>>(address) }
            },
            renderbuffer_storage: {
                let address = load(c"glRenderbufferStorage");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glRenderbufferStorage is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, i32, i32)>>(address) }
            },
            renderbuffer_storage_multisample: {
                let address = load(c"glRenderbufferStorageMultisample");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glRenderbufferStorageMultisample is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, u32, i32, i32)>>(address) }
            },
            sampler_parameterf: {
                let address = load(c"glSamplerParameterf");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glSamplerParameterf is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, f32)>>(address) }
            },
            sampler_parameteri: {
                let address = load(c"glSamplerParameteri");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glSamplerParameteri is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, i32)>>(address) }
            },
            scissor: {
                let address = load(c"glScissor");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glScissor is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, i32, i32)>>(address) }
            },
            shader_source: {
                let address = load(c"glShaderSource");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glShaderSource is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, *const *const u8, *const i32)>>(address) }
            },
            stencil_func: {
                let address = load(c"glStencilFunc");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glStencilFunc is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, u32)>>(address) }
            },
            stencil_func_separate: {
                let address = load(c"glStencilFuncSeparate");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glStencilFuncSeparate is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, i32, u32)>>(address) }
            },
            stencil_mask: {
                let address = load(c"glStencilMask");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glStencilMask is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            stencil_op: {
                let address = load(c"glStencilOp");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glStencilOp is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, u32)>>(address) }
            },
            stencil_op_separate: {
                let address = load(c"glStencilOpSeparate");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glStencilOpSeparate is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, u32, u32)>>(address) }
            },
            tex_parameteri: {
                let address = load(c"glTexParameteri");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glTexParameteri is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, i32)>>(address) }
            },
            tex_storage_2d: {
                let address = load(c"glTexStorage2D");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glTexStorage2D is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, u32, i32, i32)>>(address) }
            },
            tex_storage_3d: {
                let address = load(c"glTexStorage3D");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glTexStorage3D is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, u32, i32, i32, i32)>>(address) }
            },
            tex_sub_image_2d: {
                let address = load(c"glTexSubImage2D");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glTexSubImage2D is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, u32, u32, *const c_void)>>(address) }
            },
            tex_sub_image_3d: {
                let address = load(c"glTexSubImage3D");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glTexSubImage3D is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, i32, i32, u32, u32, *const c_void)>>(address) }
            },
            transform_feedback_varyings: {
                let address = load(c"glTransformFeedbackVaryings");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glTransformFeedbackVaryings is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, *const *const u8, u32)>>(address) }
            },
            uniform1f: {
                let address = load(c"glUniform1f");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform1f is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, f32)>>(address) }
            },
            uniform1fv: {
                let address = load(c"glUniform1fv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform1fv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, *const f32)>>(address) }
            },
            uniform1i: {
                let address = load(c"glUniform1i");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform1i is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32)>>(address) }
            },
            uniform1iv: {
                let address = load(c"glUniform1iv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform1iv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, *const i32)>>(address) }
            },
            uniform1ui: {
                let address = load(c"glUniform1ui");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform1ui is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, u32)>>(address) }
            },
            uniform1uiv: {
                let address = load(c"glUniform1uiv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform1uiv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, *const u32)>>(address) }
            },
            uniform2f: {
                let address = load(c"glUniform2f");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform2f is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, f32, f32)>>(address) }
            },
            uniform2fv: {
                let address = load(c"glUniform2fv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform2fv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, *const f32)>>(address) }
            },
            uniform2iv: {
                let address = load(c"glUniform2iv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform2iv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, *const i32)>>(address) }
            },
            uniform2uiv: {
                let address = load(c"glUniform2uiv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform2uiv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, *const u32)>>(address) }
            },
            uniform3fv: {
                let address = load(c"glUniform3fv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform3fv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, *const f32)>>(address) }
            },
            uniform3i: {
                let address = load(c"glUniform3i");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform3i is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, i32, i32)>>(address) }
            },
            uniform3iv: {
                let address = load(c"glUniform3iv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform3iv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, *const i32)>>(address) }
            },
            uniform3uiv: {
                let address = load(c"glUniform3uiv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform3uiv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, *const u32)>>(address) }
            },
            uniform4fv: {
                let address = load(c"glUniform4fv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform4fv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, *const f32)>>(address) }
            },
            uniform4iv: {
                let address = load(c"glUniform4iv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform4iv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, *const i32)>>(address) }
            },
            uniform4uiv: {
                let address = load(c"glUniform4uiv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniform4uiv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, *const u32)>>(address) }
            },
            uniform_block_binding: {
                let address = load(c"glUniformBlockBinding");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniformBlockBinding is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, u32)>>(address) }
            },
            uniform_matrix2fv: {
                let address = load(c"glUniformMatrix2fv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniformMatrix2fv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>>(address) }
            },
            uniform_matrix2x3fv: {
                let address = load(c"glUniformMatrix2x3fv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniformMatrix2x3fv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>>(address) }
            },
            uniform_matrix2x4fv: {
                let address = load(c"glUniformMatrix2x4fv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniformMatrix2x4fv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>>(address) }
            },
            uniform_matrix3fv: {
                let address = load(c"glUniformMatrix3fv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniformMatrix3fv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>>(address) }
            },
            uniform_matrix3x2fv: {
                let address = load(c"glUniformMatrix3x2fv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniformMatrix3x2fv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>>(address) }
            },
            uniform_matrix3x4fv: {
                let address = load(c"glUniformMatrix3x4fv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniformMatrix3x4fv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>>(address) }
            },
            uniform_matrix4fv: {
                let address = load(c"glUniformMatrix4fv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniformMatrix4fv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>>(address) }
            },
            uniform_matrix4x2fv: {
                let address = load(c"glUniformMatrix4x2fv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniformMatrix4x2fv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>>(address) }
            },
            uniform_matrix4x3fv: {
                let address = load(c"glUniformMatrix4x3fv");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUniformMatrix4x3fv is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, u8, *const f32)>>(address) }
            },
            use_program: {
                let address = load(c"glUseProgram");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUseProgram is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            vertex_attrib_divisor: {
                let address = load(c"glVertexAttribDivisor");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glVertexAttribDivisor is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32)>>(address) }
            },
            vertex_attrib_i_pointer: {
                let address = load(c"glVertexAttribIPointer");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glVertexAttribIPointer is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, u32, i32, *const c_void)>>(address) }
            },
            vertex_attrib_pointer: {
                let address = load(c"glVertexAttribPointer");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glVertexAttribPointer is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, i32, u32, u8, i32, *const c_void)>>(address) }
            },
            viewport: {
                let address = load(c"glViewport");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glViewport is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(i32, i32, i32, i32)>>(address) }
            },
            blend_equation_separatei_oes: {
                let address = load(c"glBlendEquationSeparateiOES");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBlendEquationSeparateiOES is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, u32)>>(address) }
            },
            blend_func_separatei_oes: {
                let address = load(c"glBlendFuncSeparateiOES");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glBlendFuncSeparateiOES is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, u32, u32, u32)>>(address) }
            },
            clip_control_ext: {
                let address = load(c"glClipControlEXT");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glClipControlEXT is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32)>>(address) }
            },
            color_maski_oes: {
                let address = load(c"glColorMaskiOES");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glColorMaskiOES is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u8, u8, u8, u8)>>(address) }
            },
            disablei_oes: {
                let address = load(c"glDisableiOES");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glDisableiOES is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32)>>(address) }
            },
            enablei_oes: {
                let address = load(c"glEnableiOES");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glEnableiOES is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32)>>(address) }
            },
            get_query_objectui64v_ext: {
                let address = load(c"glGetQueryObjectui64vEXT");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetQueryObjectui64vEXT is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, *mut u64)>>(address) }
            },
            get_queryiv_ext: {
                let address = load(c"glGetQueryivEXT");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glGetQueryivEXT is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32, *mut i32)>>(address) }
            },
            polygon_mode_angle: {
                let address = load(c"glPolygonModeANGLE");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glPolygonModeANGLE is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32)>>(address) }
            },
            polygon_offset_clamp_ext: {
                let address = load(c"glPolygonOffsetClampEXT");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glPolygonOffsetClampEXT is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(f32, f32, f32)>>(address) }
            },
            provoking_vertex_angle: {
                let address = load(c"glProvokingVertexANGLE");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glProvokingVertexANGLE is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32)>>(address) }
            },
            query_counter_ext: {
                let address = load(c"glQueryCounterEXT");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glQueryCounterEXT is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, u32)>>(address) }
            },
            map_buffer_range: {
                let address = load(c"glMapBufferRange");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glMapBufferRange is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32, isize, isize, u32) -> *mut c_void>>(address) }
            },
            unmap_buffer: {
                let address = load(c"glUnmapBuffer");
                #[expect(unsafe_code, reason = "gl.xml: the address the library gives for glUnmapBuffer is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32) -> u8>>(address) }
            },
        }
    }
}

impl wcontainers_opengl_es::OpenGlEs for OpenGlEsContext {
    type Fence = Fence;

    fn active_texture(&self, texture: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.active_texture else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glActiveTexture takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(texture) };
    }

    fn attach_shader(&self, program: u32, shader: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.attach_shader else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glAttachShader takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(program, shader) };
    }

    fn begin_query(&self, target: u32, id: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.begin_query else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glBeginQuery takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, id) };
    }

    fn begin_transform_feedback(&self, primitive_mode: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.begin_transform_feedback else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glBeginTransformFeedback takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(primitive_mode) };
    }

    fn bind_buffer(&self, target: u32, buffer: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.bind_buffer else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glBindBuffer takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, buffer) };
    }

    fn bind_buffer_base(&self, target: u32, index: u32, buffer: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.bind_buffer_base else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glBindBufferBase takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, index, buffer) };
    }

    fn bind_buffer_range(&self, target: u32, index: u32, buffer: u32, offset: i64, size: i64) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.bind_buffer_range else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Ok(offset) = isize::try_from(offset) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        let Ok(size) = isize::try_from(size) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glBindBufferRange takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, index, buffer, offset, size) };
    }

    fn bind_framebuffer(&self, target: u32, framebuffer: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.bind_framebuffer else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glBindFramebuffer takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, framebuffer) };
    }

    fn bind_renderbuffer(&self, target: u32, renderbuffer: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.bind_renderbuffer else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glBindRenderbuffer takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, renderbuffer) };
    }

    fn bind_sampler(&self, unit: u32, sampler: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.bind_sampler else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glBindSampler takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(unit, sampler) };
    }

    fn bind_texture(&self, target: u32, texture: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.bind_texture else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glBindTexture takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, texture) };
    }

    fn bind_vertex_array(&self, array: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.bind_vertex_array else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glBindVertexArray takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(array) };
    }

    fn blend_color(&self, red: f32, green: f32, blue: f32, alpha: f32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.blend_color else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glBlendColor takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(red, green, blue, alpha) };
    }

    fn blend_equation_separate(&self, mode_rgb: u32, mode_alpha: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.blend_equation_separate else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glBlendEquationSeparate takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(mode_rgb, mode_alpha) };
    }

    fn blend_func_separate(&self, sfactor_rgb: u32, dfactor_rgb: u32, sfactor_alpha: u32, dfactor_alpha: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.blend_func_separate else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glBlendFuncSeparate takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(sfactor_rgb, dfactor_rgb, sfactor_alpha, dfactor_alpha) };
    }

    fn blit_framebuffer(&self, src_x0: i32, src_y0: i32, src_x1: i32, src_y1: i32, dst_x0: i32, dst_y0: i32, dst_x1: i32, dst_y1: i32, mask: u32, filter: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.blit_framebuffer else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glBlitFramebuffer takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(src_x0, src_y0, src_x1, src_y1, dst_x0, dst_y0, dst_x1, dst_y1, mask, filter) };
    }

    fn buffer_data(&self, target: u32, size: i64, data: Option<&[u8]>, usage: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.buffer_data else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Ok(size) = isize::try_from(size) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        if data.is_some_and(|bytes| usize::try_from(size).is_ok_and(|size| bytes.len() < size)) { self.refuse(constants::INVALID_VALUE); return Default::default(); }
        #[expect(unsafe_code, reason = "gl.xml: glBufferData takes the C types the field declares; data, when there, holds at least size bytes")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, size, data.map_or(core::ptr::null(), |bytes| bytes.as_ptr().cast()), usage) };
    }

    fn buffer_sub_data(&self, target: u32, offset: i64, data: &[u8]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.buffer_sub_data else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Ok(offset) = isize::try_from(offset) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        let Some(size) = runtime::counted::<isize>(data.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glBufferSubData takes the C types the field declares; size is data's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, offset, size, data.as_ptr().cast()) };
    }

    fn check_framebuffer_status(&self, target: u32) -> u32 {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.check_framebuffer_status else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glCheckFramebufferStatus takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(target) };
        result
    }

    fn clear_bufferfi(&self, buffer: u32, drawbuffer: i32, depth: f32, stencil: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.clear_bufferfi else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glClearBufferfi takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(buffer, drawbuffer, depth, stencil) };
    }

    fn clear_bufferfv(&self, buffer: u32, drawbuffer: i32, value: &[f32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.clear_bufferfv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let mut value_room = [f32::default(); 4];
        for (to, from) in value_room.iter_mut().zip(value) { *to = *from; }
        #[expect(unsafe_code, reason = "gl.xml: glClearBufferfv takes the C types the field declares; value_room holds 4 values, the most the command reads")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(buffer, drawbuffer, value_room.as_ptr()) };
    }

    fn clear_bufferiv(&self, buffer: u32, drawbuffer: i32, value: &[i32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.clear_bufferiv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let mut value_room = [i32::default(); 4];
        for (to, from) in value_room.iter_mut().zip(value) { *to = *from; }
        #[expect(unsafe_code, reason = "gl.xml: glClearBufferiv takes the C types the field declares; value_room holds 4 values, the most the command reads")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(buffer, drawbuffer, value_room.as_ptr()) };
    }

    fn clear_bufferuiv(&self, buffer: u32, drawbuffer: i32, value: &[u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.clear_bufferuiv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let mut value_room = [u32::default(); 4];
        for (to, from) in value_room.iter_mut().zip(value) { *to = *from; }
        #[expect(unsafe_code, reason = "gl.xml: glClearBufferuiv takes the C types the field declares; value_room holds 4 values, the most the command reads")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(buffer, drawbuffer, value_room.as_ptr()) };
    }

    fn client_wait_sync(&self, sync: &Fence, flags: u32, timeout: u64) -> u32 {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.client_wait_sync else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(sync_address) = self.fence_address(sync) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glClientWaitSync takes the C types the field declares; sync is a fence this context made and has not deleted")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(sync_address, flags, timeout) };
        result
    }

    fn color_mask(&self, red: bool, green: bool, blue: bool, alpha: bool) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.color_mask else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glColorMask takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(u8::from(red), u8::from(green), u8::from(blue), u8::from(alpha)) };
    }

    fn compile_shader(&self, shader: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.compile_shader else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glCompileShader takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(shader) };
    }

    fn compressed_tex_sub_image_2d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, width: i32, height: i32, format: u32, data: CompressedPixelUnpack<'_>) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.compressed_tex_sub_image_2d else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some((data, data_size)) = self.compressed_pointer(data) else { return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glCompressedTexSubImage2D takes the C types the field declares; data is the caller's bytes of data_size, or an offset into the bound unpack buffer")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, level, xoffset, yoffset, width, height, format, data_size, data) };
    }

    fn compressed_tex_sub_image_3d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, zoffset: i32, width: i32, height: i32, depth: i32, format: u32, data: CompressedPixelUnpack<'_>) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.compressed_tex_sub_image_3d else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some((data, data_size)) = self.compressed_pointer(data) else { return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glCompressedTexSubImage3D takes the C types the field declares; data is the caller's bytes of data_size, or an offset into the bound unpack buffer")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, level, xoffset, yoffset, zoffset, width, height, depth, format, data_size, data) };
    }

    fn copy_buffer_sub_data(&self, read_target: u32, write_target: u32, read_offset: i64, write_offset: i64, size: i64) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.copy_buffer_sub_data else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Ok(read_offset) = isize::try_from(read_offset) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        let Ok(write_offset) = isize::try_from(write_offset) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        let Ok(size) = isize::try_from(size) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glCopyBufferSubData takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(read_target, write_target, read_offset, write_offset, size) };
    }

    fn copy_tex_sub_image_2d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, x: i32, y: i32, width: i32, height: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.copy_tex_sub_image_2d else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glCopyTexSubImage2D takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, level, xoffset, yoffset, x, y, width, height) };
    }

    fn copy_tex_sub_image_3d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, zoffset: i32, x: i32, y: i32, width: i32, height: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.copy_tex_sub_image_3d else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glCopyTexSubImage3D takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, level, xoffset, yoffset, zoffset, x, y, width, height) };
    }

    fn create_program(&self) -> u32 {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.create_program else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glCreateProgram takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry() };
        result
    }

    fn create_shader(&self, type_: u32) -> u32 {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.create_shader else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glCreateShader takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(type_) };
        result
    }

    fn cull_face(&self, mode: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.cull_face else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glCullFace takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(mode) };
    }

    fn delete_buffers(&self, buffers: &[u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.delete_buffers else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(n) = runtime::counted::<i32>(buffers.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDeleteBuffers takes the C types the field declares; n is buffers's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(n, buffers.as_ptr().cast()) };
    }

    fn delete_framebuffers(&self, framebuffers: &[u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.delete_framebuffers else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(n) = runtime::counted::<i32>(framebuffers.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDeleteFramebuffers takes the C types the field declares; n is framebuffers's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(n, framebuffers.as_ptr().cast()) };
    }

    fn delete_program(&self, program: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.delete_program else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDeleteProgram takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(program) };
    }

    fn delete_queries(&self, ids: &[u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.delete_queries else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(n) = runtime::counted::<i32>(ids.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDeleteQueries takes the C types the field declares; n is ids's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(n, ids.as_ptr().cast()) };
    }

    fn delete_renderbuffers(&self, renderbuffers: &[u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.delete_renderbuffers else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(n) = runtime::counted::<i32>(renderbuffers.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDeleteRenderbuffers takes the C types the field declares; n is renderbuffers's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(n, renderbuffers.as_ptr().cast()) };
    }

    fn delete_samplers(&self, samplers: &[u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.delete_samplers else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(samplers.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDeleteSamplers takes the C types the field declares; count is samplers's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(count, samplers.as_ptr().cast()) };
    }

    fn delete_shader(&self, shader: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.delete_shader else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDeleteShader takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(shader) };
    }

    fn delete_sync(&self, sync: &Fence) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.delete_sync else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(sync_address) = self.fence_address(sync) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDeleteSync takes the C types the field declares; sync is a fence this context made and has not deleted")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(sync_address) };
        self.forget_fence(sync);
    }

    fn delete_textures(&self, textures: &[u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.delete_textures else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(n) = runtime::counted::<i32>(textures.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDeleteTextures takes the C types the field declares; n is textures's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(n, textures.as_ptr().cast()) };
    }

    fn delete_vertex_arrays(&self, arrays: &[u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.delete_vertex_arrays else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(n) = runtime::counted::<i32>(arrays.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDeleteVertexArrays takes the C types the field declares; n is arrays's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(n, arrays.as_ptr().cast()) };
    }

    fn depth_func(&self, func: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.depth_func else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDepthFunc takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(func) };
    }

    fn depth_mask(&self, flag: bool) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.depth_mask else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDepthMask takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(u8::from(flag)) };
    }

    fn depth_rangef(&self, n: f32, f: f32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.depth_rangef else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDepthRangef takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(n, f) };
    }

    fn detach_shader(&self, program: u32, shader: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.detach_shader else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDetachShader takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(program, shader) };
    }

    fn disable(&self, cap: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.disable else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDisable takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(cap) };
    }

    fn disable_vertex_attrib_array(&self, index: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.disable_vertex_attrib_array else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDisableVertexAttribArray takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(index) };
    }

    fn draw_arrays(&self, mode: u32, first: i32, count: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.draw_arrays else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        if !self.draw_ready() { self.refuse(constants::INVALID_OPERATION); return Default::default(); }
        #[expect(unsafe_code, reason = "gl.xml: glDrawArrays takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(mode, first, count) };
    }

    fn draw_arrays_instanced(&self, mode: u32, first: i32, count: i32, instancecount: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.draw_arrays_instanced else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        if !self.draw_ready() { self.refuse(constants::INVALID_OPERATION); return Default::default(); }
        #[expect(unsafe_code, reason = "gl.xml: glDrawArraysInstanced takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(mode, first, count, instancecount) };
    }

    fn draw_buffers(&self, bufs: &[u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.draw_buffers else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(n) = runtime::counted::<i32>(bufs.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDrawBuffers takes the C types the field declares; n is bufs's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(n, bufs.as_ptr().cast()) };
    }

    fn draw_elements_instanced(&self, mode: u32, count: i32, type_: u32, indices: u64, instancecount: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.draw_elements_instanced else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        if !self.draw_ready() { self.refuse(constants::INVALID_OPERATION); return Default::default(); }
        let Some(indices) = self.offset_into(constants::ELEMENT_ARRAY_BUFFER_BINDING, indices) else { return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glDrawElementsInstanced takes the C types the field declares; indices is an offset into the bound buffer, never read as an address")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(mode, count, type_, indices, instancecount) };
    }

    fn enable(&self, cap: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.enable else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glEnable takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(cap) };
    }

    fn enable_vertex_attrib_array(&self, index: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.enable_vertex_attrib_array else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glEnableVertexAttribArray takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(index) };
    }

    fn end_query(&self, target: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.end_query else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glEndQuery takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target) };
    }

    fn end_transform_feedback(&self) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.end_transform_feedback else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glEndTransformFeedback takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry() };
    }

    fn fence_sync(&self, condition: u32, flags: u32) -> Option<Self::Fence> {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.fence_sync else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glFenceSync takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(condition, flags) };
        self.keep_fence(result)
    }

    fn finish(&self) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.finish else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glFinish takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry() };
    }

    fn flush(&self) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.flush else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glFlush takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry() };
    }

    fn framebuffer_renderbuffer(&self, target: u32, attachment: u32, renderbuffertarget: u32, renderbuffer: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.framebuffer_renderbuffer else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glFramebufferRenderbuffer takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, attachment, renderbuffertarget, renderbuffer) };
    }

    fn framebuffer_texture_2d(&self, target: u32, attachment: u32, textarget: u32, texture: u32, level: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.framebuffer_texture_2d else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glFramebufferTexture2D takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, attachment, textarget, texture, level) };
    }

    fn framebuffer_texture_layer(&self, target: u32, attachment: u32, texture: u32, level: i32, layer: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.framebuffer_texture_layer else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glFramebufferTextureLayer takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, attachment, texture, level, layer) };
    }

    fn front_face(&self, mode: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.front_face else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glFrontFace takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(mode) };
    }

    fn gen_buffers(&self, buffers: &mut [u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.gen_buffers else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(n) = runtime::counted::<i32>(buffers.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glGenBuffers takes the C types the field declares; n is buffers's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(n, buffers.as_mut_ptr()) };
    }

    fn gen_framebuffers(&self, framebuffers: &mut [u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.gen_framebuffers else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(n) = runtime::counted::<i32>(framebuffers.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glGenFramebuffers takes the C types the field declares; n is framebuffers's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(n, framebuffers.as_mut_ptr()) };
    }

    fn gen_queries(&self, ids: &mut [u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.gen_queries else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(n) = runtime::counted::<i32>(ids.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glGenQueries takes the C types the field declares; n is ids's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(n, ids.as_mut_ptr()) };
    }

    fn gen_renderbuffers(&self, renderbuffers: &mut [u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.gen_renderbuffers else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(n) = runtime::counted::<i32>(renderbuffers.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glGenRenderbuffers takes the C types the field declares; n is renderbuffers's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(n, renderbuffers.as_mut_ptr()) };
    }

    fn gen_samplers(&self, samplers: &mut [u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.gen_samplers else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(samplers.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glGenSamplers takes the C types the field declares; count is samplers's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(count, samplers.as_mut_ptr()) };
    }

    fn gen_textures(&self, textures: &mut [u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.gen_textures else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(n) = runtime::counted::<i32>(textures.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glGenTextures takes the C types the field declares; n is textures's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(n, textures.as_mut_ptr()) };
    }

    fn gen_vertex_arrays(&self, arrays: &mut [u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.gen_vertex_arrays else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(n) = runtime::counted::<i32>(arrays.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glGenVertexArrays takes the C types the field declares; n is arrays's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(n, arrays.as_mut_ptr()) };
    }

    fn generate_mipmap(&self, target: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.generate_mipmap else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glGenerateMipmap takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target) };
    }

    fn get_booleanv(&self, pname: u32, data: &mut [bool]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_booleanv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        if [constants::COMPRESSED_TEXTURE_FORMATS, constants::PROGRAM_BINARY_FORMATS, constants::SHADER_BINARY_FORMATS].contains(&pname) { self.refuse(constants::INVALID_ENUM); return Default::default(); }
        let mut data_room = [u8::default(); 64];
        #[expect(unsafe_code, reason = "gl.xml: glGetBooleanv takes the C types the field declares; data_room holds 64 values, more than any pname writes")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(pname, data_room.as_mut_ptr()) };
        for (to, from) in data.iter_mut().zip(data_room) { *to = from != 0; }
    }

    fn get_error(&self) -> u32 {
        if let Some(error) = self.take_refusal() { return error; }
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_error else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glGetError takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry() };
        result
    }

    fn get_floatv(&self, pname: u32, data: &mut [f32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_floatv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        if [constants::COMPRESSED_TEXTURE_FORMATS, constants::PROGRAM_BINARY_FORMATS, constants::SHADER_BINARY_FORMATS].contains(&pname) { self.refuse(constants::INVALID_ENUM); return Default::default(); }
        let mut data_room = [f32::default(); 64];
        #[expect(unsafe_code, reason = "gl.xml: glGetFloatv takes the C types the field declares; data_room holds 64 values, more than any pname writes")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(pname, data_room.as_mut_ptr()) };
        for (to, from) in data.iter_mut().zip(data_room) { *to = from; }
    }

    fn get_integer64v(&self, pname: u32, data: &mut [i64]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_integer64v else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        if [constants::COMPRESSED_TEXTURE_FORMATS, constants::PROGRAM_BINARY_FORMATS, constants::SHADER_BINARY_FORMATS].contains(&pname) { self.refuse(constants::INVALID_ENUM); return Default::default(); }
        let mut data_room = [i64::default(); 64];
        #[expect(unsafe_code, reason = "gl.xml: glGetInteger64v takes the C types the field declares; data_room holds 64 values, more than any pname writes")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(pname, data_room.as_mut_ptr()) };
        for (to, from) in data.iter_mut().zip(data_room) { *to = from; }
    }

    fn get_integerv(&self, pname: u32, data: &mut [i32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_integerv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        if [constants::COMPRESSED_TEXTURE_FORMATS, constants::PROGRAM_BINARY_FORMATS, constants::SHADER_BINARY_FORMATS].contains(&pname) { self.refuse(constants::INVALID_ENUM); return Default::default(); }
        let mut data_room = [i32::default(); 64];
        #[expect(unsafe_code, reason = "gl.xml: glGetIntegerv takes the C types the field declares; data_room holds 64 values, more than any pname writes")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(pname, data_room.as_mut_ptr()) };
        for (to, from) in data.iter_mut().zip(data_room) { *to = from; }
    }

    fn get_internalformativ(&self, target: u32, internalformat: u32, pname: u32, params: &mut [i32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_internalformativ else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(params.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glGetInternalformativ takes the C types the field declares; count is params's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, internalformat, pname, count, params.as_mut_ptr()) };
    }

    fn get_program_info_log(&self, program: u32, info_log: &mut [u8]) -> i32 {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_program_info_log else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(buf_size) = runtime::counted::<i32>(info_log.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        let mut length: i32 = Default::default();
        #[expect(unsafe_code, reason = "gl.xml: glGetProgramInfoLog takes the C types the field declares; buf_size is info_log's length; length is one i32 of ours")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(program, buf_size, &raw mut length, info_log.as_mut_ptr()) };
        length
    }

    fn get_programiv(&self, program: u32, pname: u32, params: &mut [i32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_programiv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let mut params_room = [i32::default(); 16];
        #[expect(unsafe_code, reason = "gl.xml: glGetProgramiv takes the C types the field declares; params_room holds 16 values, more than any pname writes")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(program, pname, params_room.as_mut_ptr()) };
        for (to, from) in params.iter_mut().zip(params_room) { *to = from; }
    }

    fn get_query_objectuiv(&self, id: u32, pname: u32, params: &mut [u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_query_objectuiv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let mut params_room = [u32::default(); 16];
        #[expect(unsafe_code, reason = "gl.xml: glGetQueryObjectuiv takes the C types the field declares; params_room holds 16 values, more than any pname writes")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(id, pname, params_room.as_mut_ptr()) };
        for (to, from) in params.iter_mut().zip(params_room) { *to = from; }
    }

    fn get_shader_info_log(&self, shader: u32, info_log: &mut [u8]) -> i32 {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_shader_info_log else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(buf_size) = runtime::counted::<i32>(info_log.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        let mut length: i32 = Default::default();
        #[expect(unsafe_code, reason = "gl.xml: glGetShaderInfoLog takes the C types the field declares; buf_size is info_log's length; length is one i32 of ours")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(shader, buf_size, &raw mut length, info_log.as_mut_ptr()) };
        length
    }

    fn get_shaderiv(&self, shader: u32, pname: u32, params: &mut [i32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_shaderiv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let mut params_room = [i32::default(); 16];
        #[expect(unsafe_code, reason = "gl.xml: glGetShaderiv takes the C types the field declares; params_room holds 16 values, more than any pname writes")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(shader, pname, params_room.as_mut_ptr()) };
        for (to, from) in params.iter_mut().zip(params_room) { *to = from; }
    }

    fn get_string(&self, name: u32) -> Option<Vec<u8>> {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_string else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glGetString takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(name) };
        runtime::copied_text(result)
    }

    fn get_stringi(&self, name: u32, index: u32) -> Option<Vec<u8>> {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_stringi else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glGetStringi takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(name, index) };
        runtime::copied_text(result)
    }

    fn get_synciv(&self, sync: &Fence, pname: u32, values: &mut [i32]) -> i32 {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_synciv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(sync_address) = self.fence_address(sync) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(values.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        let mut length: i32 = Default::default();
        #[expect(unsafe_code, reason = "gl.xml: glGetSynciv takes the C types the field declares; sync is a fence this context made and has not deleted; count is values's length; length is one i32 of ours")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(sync_address, pname, count, &raw mut length, values.as_mut_ptr()) };
        length
    }

    fn get_uniform_block_index(&self, program: u32, uniform_block_name: &core::ffi::CStr) -> u32 {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_uniform_block_index else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glGetUniformBlockIndex takes the C types the field declares; uniform_block_name is zero-ended")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(program, uniform_block_name.as_ptr().cast()) };
        result
    }

    fn get_uniform_location(&self, program: u32, name: &core::ffi::CStr) -> i32 {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_uniform_location else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glGetUniformLocation takes the C types the field declares; name is zero-ended")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(program, name.as_ptr().cast()) };
        result
    }

    fn get_vertex_attribiv(&self, index: u32, pname: u32, params: &mut [i32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.get_vertex_attribiv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let mut params_room = [i32::default(); 16];
        #[expect(unsafe_code, reason = "gl.xml: glGetVertexAttribiv takes the C types the field declares; params_room holds 16 values, more than any pname writes")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(index, pname, params_room.as_mut_ptr()) };
        for (to, from) in params.iter_mut().zip(params_room) { *to = from; }
    }

    fn invalidate_framebuffer(&self, target: u32, attachments: &[u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.invalidate_framebuffer else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(num_attachments) = runtime::counted::<i32>(attachments.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glInvalidateFramebuffer takes the C types the field declares; num_attachments is attachments's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, num_attachments, attachments.as_ptr().cast()) };
    }

    fn link_program(&self, program: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.link_program else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glLinkProgram takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(program) };
    }

    fn pixel_storei(&self, pname: u32, param: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.pixel_storei else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glPixelStorei takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(pname, param) };
    }

    fn polygon_offset(&self, factor: f32, units: f32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.polygon_offset else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glPolygonOffset takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(factor, units) };
    }

    fn read_pixels(&self, x: i32, y: i32, width: i32, height: i32, format: u32, type_: u32, pixels: PixelPack<'_>) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.read_pixels else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(pixels) = self.pack_pointer(pixels, width, height, format, type_) else { return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glReadPixels takes the C types the field declares; pixels is the caller's bytes, as many as the pack state writes, or an offset into the bound pack buffer")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(x, y, width, height, format, type_, pixels) };
    }

    fn renderbuffer_storage(&self, target: u32, internalformat: u32, width: i32, height: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.renderbuffer_storage else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glRenderbufferStorage takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, internalformat, width, height) };
    }

    fn renderbuffer_storage_multisample(&self, target: u32, samples: i32, internalformat: u32, width: i32, height: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.renderbuffer_storage_multisample else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glRenderbufferStorageMultisample takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, samples, internalformat, width, height) };
    }

    fn sampler_parameterf(&self, sampler: u32, pname: u32, param: f32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.sampler_parameterf else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glSamplerParameterf takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(sampler, pname, param) };
    }

    fn sampler_parameteri(&self, sampler: u32, pname: u32, param: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.sampler_parameteri else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glSamplerParameteri takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(sampler, pname, param) };
    }

    fn scissor(&self, x: i32, y: i32, width: i32, height: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.scissor else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glScissor takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(x, y, width, height) };
    }

    fn shader_source(&self, shader: u32, string: &[&[u8]]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.shader_source else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(string.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        let string_pointers: Vec<*const u8> = string.iter().map(|text| text.as_ptr()).collect();
        let Some(string_lengths) = string.iter().map(|text| i32::try_from(text.len()).ok()).collect::<Option<Vec<i32>>>() else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glShaderSource takes the C types the field declares; count is string's length; each of string_pointers points at as many bytes as string_lengths says")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(shader, count, string_pointers.as_ptr(), string_lengths.as_ptr()) };
    }

    fn stencil_func(&self, func: u32, ref_: i32, mask: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.stencil_func else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glStencilFunc takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(func, ref_, mask) };
    }

    fn stencil_func_separate(&self, face: u32, func: u32, ref_: i32, mask: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.stencil_func_separate else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glStencilFuncSeparate takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(face, func, ref_, mask) };
    }

    fn stencil_mask(&self, mask: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.stencil_mask else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glStencilMask takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(mask) };
    }

    fn stencil_op(&self, fail: u32, zfail: u32, zpass: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.stencil_op else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glStencilOp takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(fail, zfail, zpass) };
    }

    fn stencil_op_separate(&self, face: u32, sfail: u32, dpfail: u32, dppass: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.stencil_op_separate else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glStencilOpSeparate takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(face, sfail, dpfail, dppass) };
    }

    fn tex_parameteri(&self, target: u32, pname: u32, param: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.tex_parameteri else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glTexParameteri takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, pname, param) };
    }

    fn tex_storage_2d(&self, target: u32, levels: i32, internalformat: u32, width: i32, height: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.tex_storage_2d else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glTexStorage2D takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, levels, internalformat, width, height) };
    }

    fn tex_storage_3d(&self, target: u32, levels: i32, internalformat: u32, width: i32, height: i32, depth: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.tex_storage_3d else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glTexStorage3D takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, levels, internalformat, width, height, depth) };
    }

    fn tex_sub_image_2d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, width: i32, height: i32, format: u32, type_: u32, pixels: PixelUnpack<'_>) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.tex_sub_image_2d else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(pixels) = self.unpack_pointer(pixels, width, height, None, format, type_) else { return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glTexSubImage2D takes the C types the field declares; pixels is the caller's bytes, as many as the unpack state reads, or an offset into the bound unpack buffer")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, level, xoffset, yoffset, width, height, format, type_, pixels) };
    }

    fn tex_sub_image_3d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, zoffset: i32, width: i32, height: i32, depth: i32, format: u32, type_: u32, pixels: PixelUnpack<'_>) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.tex_sub_image_3d else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(pixels) = self.unpack_pointer(pixels, width, height, Some(depth), format, type_) else { return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glTexSubImage3D takes the C types the field declares; pixels is the caller's bytes, as many as the unpack state reads, or an offset into the bound unpack buffer")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, level, xoffset, yoffset, zoffset, width, height, depth, format, type_, pixels) };
    }

    fn transform_feedback_varyings(&self, program: u32, varyings: &[&core::ffi::CStr], buffer_mode: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.transform_feedback_varyings else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(varyings.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        let varyings_pointers: Vec<*const u8> = varyings.iter().map(|text| text.as_ptr().cast::<u8>()).collect();
        #[expect(unsafe_code, reason = "gl.xml: glTransformFeedbackVaryings takes the C types the field declares; count is varyings's length; each of varyings_pointers is zero-ended")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(program, count, varyings_pointers.as_ptr(), buffer_mode) };
    }

    fn uniform1f(&self, location: i32, v0: f32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform1f else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform1f takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, v0) };
    }

    fn uniform1fv(&self, location: i32, value: &[f32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform1fv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform1fv takes the C types the field declares; count is value's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, value.as_ptr().cast()) };
    }

    fn uniform1i(&self, location: i32, v0: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform1i else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform1i takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, v0) };
    }

    fn uniform1iv(&self, location: i32, value: &[i32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform1iv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform1iv takes the C types the field declares; count is value's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, value.as_ptr().cast()) };
    }

    fn uniform1ui(&self, location: i32, v0: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform1ui else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform1ui takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, v0) };
    }

    fn uniform1uiv(&self, location: i32, value: &[u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform1uiv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 1) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform1uiv takes the C types the field declares; count is value's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, value.as_ptr().cast()) };
    }

    fn uniform2f(&self, location: i32, v0: f32, v1: f32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform2f else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform2f takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, v0, v1) };
    }

    fn uniform2fv(&self, location: i32, value: &[f32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform2fv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 2) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform2fv takes the C types the field declares; count is value's length over 2")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, value.as_ptr().cast()) };
    }

    fn uniform2iv(&self, location: i32, value: &[i32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform2iv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 2) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform2iv takes the C types the field declares; count is value's length over 2")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, value.as_ptr().cast()) };
    }

    fn uniform2uiv(&self, location: i32, value: &[u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform2uiv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 2) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform2uiv takes the C types the field declares; count is value's length over 2")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, value.as_ptr().cast()) };
    }

    fn uniform3fv(&self, location: i32, value: &[f32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform3fv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 3) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform3fv takes the C types the field declares; count is value's length over 3")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, value.as_ptr().cast()) };
    }

    fn uniform3i(&self, location: i32, v0: i32, v1: i32, v2: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform3i else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform3i takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, v0, v1, v2) };
    }

    fn uniform3iv(&self, location: i32, value: &[i32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform3iv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 3) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform3iv takes the C types the field declares; count is value's length over 3")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, value.as_ptr().cast()) };
    }

    fn uniform3uiv(&self, location: i32, value: &[u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform3uiv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 3) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform3uiv takes the C types the field declares; count is value's length over 3")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, value.as_ptr().cast()) };
    }

    fn uniform4fv(&self, location: i32, value: &[f32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform4fv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 4) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform4fv takes the C types the field declares; count is value's length over 4")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, value.as_ptr().cast()) };
    }

    fn uniform4iv(&self, location: i32, value: &[i32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform4iv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 4) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform4iv takes the C types the field declares; count is value's length over 4")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, value.as_ptr().cast()) };
    }

    fn uniform4uiv(&self, location: i32, value: &[u32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform4uiv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 4) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniform4uiv takes the C types the field declares; count is value's length over 4")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, value.as_ptr().cast()) };
    }

    fn uniform_block_binding(&self, program: u32, uniform_block_index: u32, uniform_block_binding: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform_block_binding else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniformBlockBinding takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(program, uniform_block_index, uniform_block_binding) };
    }

    fn uniform_matrix2fv(&self, location: i32, transpose: bool, value: &[f32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform_matrix2fv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 4) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniformMatrix2fv takes the C types the field declares; count is value's length over 4")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, u8::from(transpose), value.as_ptr().cast()) };
    }

    fn uniform_matrix2x3fv(&self, location: i32, transpose: bool, value: &[f32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform_matrix2x3fv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 6) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniformMatrix2x3fv takes the C types the field declares; count is value's length over 6")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, u8::from(transpose), value.as_ptr().cast()) };
    }

    fn uniform_matrix2x4fv(&self, location: i32, transpose: bool, value: &[f32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform_matrix2x4fv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 8) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniformMatrix2x4fv takes the C types the field declares; count is value's length over 8")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, u8::from(transpose), value.as_ptr().cast()) };
    }

    fn uniform_matrix3fv(&self, location: i32, transpose: bool, value: &[f32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform_matrix3fv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 9) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniformMatrix3fv takes the C types the field declares; count is value's length over 9")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, u8::from(transpose), value.as_ptr().cast()) };
    }

    fn uniform_matrix3x2fv(&self, location: i32, transpose: bool, value: &[f32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform_matrix3x2fv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 6) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniformMatrix3x2fv takes the C types the field declares; count is value's length over 6")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, u8::from(transpose), value.as_ptr().cast()) };
    }

    fn uniform_matrix3x4fv(&self, location: i32, transpose: bool, value: &[f32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform_matrix3x4fv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 12) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniformMatrix3x4fv takes the C types the field declares; count is value's length over 12")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, u8::from(transpose), value.as_ptr().cast()) };
    }

    fn uniform_matrix4fv(&self, location: i32, transpose: bool, value: &[f32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform_matrix4fv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 16) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniformMatrix4fv takes the C types the field declares; count is value's length over 16")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, u8::from(transpose), value.as_ptr().cast()) };
    }

    fn uniform_matrix4x2fv(&self, location: i32, transpose: bool, value: &[f32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform_matrix4x2fv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 8) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniformMatrix4x2fv takes the C types the field declares; count is value's length over 8")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, u8::from(transpose), value.as_ptr().cast()) };
    }

    fn uniform_matrix4x3fv(&self, location: i32, transpose: bool, value: &[f32]) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.uniform_matrix4x3fv else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(count) = runtime::counted::<i32>(value.len(), 12) else { self.refuse(constants::INVALID_VALUE); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUniformMatrix4x3fv takes the C types the field declares; count is value's length over 12")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(location, count, u8::from(transpose), value.as_ptr().cast()) };
    }

    fn use_program(&self, program: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.use_program else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glUseProgram takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(program) };
    }

    fn vertex_attrib_divisor(&self, index: u32, divisor: u32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.vertex_attrib_divisor else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glVertexAttribDivisor takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(index, divisor) };
    }

    fn vertex_attrib_i_pointer(&self, index: u32, size: i32, type_: u32, stride: i32, pointer: u64) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.vertex_attrib_i_pointer else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(pointer) = self.offset_into(constants::ARRAY_BUFFER_BINDING, pointer) else { return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glVertexAttribIPointer takes the C types the field declares; pointer is an offset into the bound buffer, never read as an address")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(index, size, type_, stride, pointer) };
    }

    fn vertex_attrib_pointer(&self, index: u32, size: i32, type_: u32, normalized: bool, stride: i32, pointer: u64) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.vertex_attrib_pointer else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        let Some(pointer) = self.offset_into(constants::ARRAY_BUFFER_BINDING, pointer) else { return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glVertexAttribPointer takes the C types the field declares; pointer is an offset into the bound buffer, never read as an address")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(index, size, type_, u8::from(normalized), stride, pointer) };
    }

    fn viewport(&self, x: i32, y: i32, width: i32, height: i32) {
        let Some(entries) = self.enter() else { return Default::default(); };
        let Some(entry) = entries.viewport else { self.refuse(constants::INVALID_OPERATION); return Default::default(); };
        #[expect(unsafe_code, reason = "gl.xml: glViewport takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(x, y, width, height) };
    }

    fn blend_equation_separatei_oes(&self, buf: u32, mode_rgb: u32, mode_alpha: u32) -> Option<()> {
        let Some(entries) = self.enter() else { return Default::default(); };
        if !self.has_extension(&[c"GL_OES_draw_buffers_indexed"]) { return None; }
        let entry = entries.blend_equation_separatei_oes?;
        #[expect(unsafe_code, reason = "gl.xml: glBlendEquationSeparateiOES takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(buf, mode_rgb, mode_alpha) };
        Some(())
    }

    fn blend_func_separatei_oes(&self, buf: u32, src_rgb: u32, dst_rgb: u32, src_alpha: u32, dst_alpha: u32) -> Option<()> {
        let Some(entries) = self.enter() else { return Default::default(); };
        if !self.has_extension(&[c"GL_OES_draw_buffers_indexed"]) { return None; }
        let entry = entries.blend_func_separatei_oes?;
        #[expect(unsafe_code, reason = "gl.xml: glBlendFuncSeparateiOES takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(buf, src_rgb, dst_rgb, src_alpha, dst_alpha) };
        Some(())
    }

    fn clip_control_ext(&self, origin: u32, depth: u32) -> Option<()> {
        let Some(entries) = self.enter() else { return Default::default(); };
        if !self.has_extension(&[c"GL_EXT_clip_control"]) { return None; }
        let entry = entries.clip_control_ext?;
        #[expect(unsafe_code, reason = "gl.xml: glClipControlEXT takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(origin, depth) };
        Some(())
    }

    fn color_maski_oes(&self, index: u32, r: bool, g: bool, b: bool, a: bool) -> Option<()> {
        let Some(entries) = self.enter() else { return Default::default(); };
        if !self.has_extension(&[c"GL_OES_draw_buffers_indexed"]) { return None; }
        let entry = entries.color_maski_oes?;
        #[expect(unsafe_code, reason = "gl.xml: glColorMaskiOES takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(index, u8::from(r), u8::from(g), u8::from(b), u8::from(a)) };
        Some(())
    }

    fn disablei_oes(&self, target: u32, index: u32) -> Option<()> {
        let Some(entries) = self.enter() else { return Default::default(); };
        if !self.has_extension(&[c"GL_OES_draw_buffers_indexed", c"GL_OES_viewport_array"]) { return None; }
        let entry = entries.disablei_oes?;
        #[expect(unsafe_code, reason = "gl.xml: glDisableiOES takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, index) };
        Some(())
    }

    fn enablei_oes(&self, target: u32, index: u32) -> Option<()> {
        let Some(entries) = self.enter() else { return Default::default(); };
        if !self.has_extension(&[c"GL_OES_draw_buffers_indexed", c"GL_OES_viewport_array"]) { return None; }
        let entry = entries.enablei_oes?;
        #[expect(unsafe_code, reason = "gl.xml: glEnableiOES takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, index) };
        Some(())
    }

    fn get_query_objectui64v_ext(&self, id: u32, pname: u32, params: &mut [u64]) -> Option<()> {
        let Some(entries) = self.enter() else { return Default::default(); };
        if !self.has_extension(&[c"GL_EXT_disjoint_timer_query"]) { return None; }
        let entry = entries.get_query_objectui64v_ext?;
        let mut params_room = [u64::default(); 16];
        #[expect(unsafe_code, reason = "gl.xml: glGetQueryObjectui64vEXT takes the C types the field declares; params_room holds 16 values, more than any pname writes")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(id, pname, params_room.as_mut_ptr()) };
        for (to, from) in params.iter_mut().zip(params_room) { *to = from; }
        Some(())
    }

    fn get_queryiv_ext(&self, target: u32, pname: u32, params: &mut [i32]) -> Option<()> {
        let Some(entries) = self.enter() else { return Default::default(); };
        if !self.has_extension(&[c"GL_EXT_disjoint_timer_query", c"GL_EXT_occlusion_query_boolean"]) { return None; }
        let entry = entries.get_queryiv_ext?;
        let mut params_room = [i32::default(); 16];
        #[expect(unsafe_code, reason = "gl.xml: glGetQueryivEXT takes the C types the field declares; params_room holds 16 values, more than any pname writes")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(target, pname, params_room.as_mut_ptr()) };
        for (to, from) in params.iter_mut().zip(params_room) { *to = from; }
        Some(())
    }

    fn polygon_mode_angle(&self, face: u32, mode: u32) -> Option<()> {
        let Some(entries) = self.enter() else { return Default::default(); };
        if !self.has_extension(&[c"GL_ANGLE_polygon_mode"]) { return None; }
        let entry = entries.polygon_mode_angle?;
        #[expect(unsafe_code, reason = "gl.xml: glPolygonModeANGLE takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(face, mode) };
        Some(())
    }

    fn polygon_offset_clamp_ext(&self, factor: f32, units: f32, clamp: f32) -> Option<()> {
        let Some(entries) = self.enter() else { return Default::default(); };
        if !self.has_extension(&[c"GL_EXT_polygon_offset_clamp"]) { return None; }
        let entry = entries.polygon_offset_clamp_ext?;
        #[expect(unsafe_code, reason = "gl.xml: glPolygonOffsetClampEXT takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(factor, units, clamp) };
        Some(())
    }

    fn provoking_vertex_angle(&self, provoke_mode: u32) -> Option<()> {
        let Some(entries) = self.enter() else { return Default::default(); };
        if !self.has_extension(&[c"GL_ANGLE_provoking_vertex"]) { return None; }
        let entry = entries.provoking_vertex_angle?;
        #[expect(unsafe_code, reason = "gl.xml: glProvokingVertexANGLE takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(provoke_mode) };
        Some(())
    }

    fn query_counter_ext(&self, id: u32, target: u32) -> Option<()> {
        let Some(entries) = self.enter() else { return Default::default(); };
        if !self.has_extension(&[c"GL_EXT_disjoint_timer_query"]) { return None; }
        let entry = entries.query_counter_ext?;
        #[expect(unsafe_code, reason = "gl.xml: glQueryCounterEXT takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        unsafe { entry(id, target) };
        Some(())
    }

    fn map_buffer_range_read(&self, target: u32, offset: i64, data: &mut [u8]) -> bool {
        self.map_buffer_range_read_inside(target, offset, data)
    }
}
