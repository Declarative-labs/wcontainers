// Written by tools/api-crate-generator from Khronos's gl.xml (and ANGLE's gl_angle_ext.xml for ANGLE's extensions), pinned in src/khronos/write.rs. Do not edit.
//! OpenGL ES 3, the Khronos graphics API the WebGL route draws through natively (ANGLE, Mesa), as a safe trait.

pub mod constants;

/// Pixels a call reads: the caller's bytes, or an offset into the pixel unpack buffer bound.
#[derive(Clone, Copy, Debug)]
pub enum PixelUnpack<'a> {
    Bytes(&'a [u8]),
    Offset(u64),
}

/// Pixels a call writes: the caller's bytes, or an offset into the pixel pack buffer bound.
#[derive(Debug)]
pub enum PixelPack<'a> {
    Bytes(&'a mut [u8]),
    Offset(u64),
}

/// Compressed pixels a call reads: the caller's bytes, or a size at an offset into the pixel unpack buffer bound.
#[derive(Clone, Copy, Debug)]
pub enum CompressedPixelUnpack<'a> {
    Bytes(&'a [u8]),
    Offset { offset: u64, size: i32 },
}

/// An OpenGL ES context: each command this set carries as a safe method on the context.
#[allow(reviewable::explicit_shared_error, reason = "the API reports its errors through get_error, as its registry declares")]
pub trait OpenGlEs: Sized {
    /// A handle of the implementation's own, made only by its methods.
    type Fence: Clone + PartialEq + core::fmt::Debug;

    /// Calls [`glActiveTexture`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn active_texture(&self, texture: u32);

    /// Calls [`glAttachShader`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn attach_shader(&self, program: u32, shader: u32);

    /// Calls [`glBeginQuery`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn begin_query(&self, target: u32, id: u32);

    /// Calls [`glBeginTransformFeedback`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn begin_transform_feedback(&self, primitive_mode: u32);

    /// Calls [`glBindBuffer`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn bind_buffer(&self, target: u32, buffer: u32);

    /// Calls [`glBindBufferBase`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn bind_buffer_base(&self, target: u32, index: u32, buffer: u32);

    /// Calls [`glBindBufferRange`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn bind_buffer_range(&self, target: u32, index: u32, buffer: u32, offset: i64, size: i64);

    /// Calls [`glBindFramebuffer`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn bind_framebuffer(&self, target: u32, framebuffer: u32);

    /// Calls [`glBindRenderbuffer`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn bind_renderbuffer(&self, target: u32, renderbuffer: u32);

    /// Calls [`glBindSampler`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn bind_sampler(&self, unit: u32, sampler: u32);

    /// Calls [`glBindTexture`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn bind_texture(&self, target: u32, texture: u32);

    /// Calls [`glBindVertexArray`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn bind_vertex_array(&self, array: u32);

    /// Calls [`glBlendColor`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn blend_color(&self, red: f32, green: f32, blue: f32, alpha: f32);

    /// Calls [`glBlendEquationSeparate`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn blend_equation_separate(&self, mode_rgb: u32, mode_alpha: u32);

    /// Calls [`glBlendFuncSeparate`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn blend_func_separate(&self, sfactor_rgb: u32, dfactor_rgb: u32, sfactor_alpha: u32, dfactor_alpha: u32);

    /// Calls [`glBlitFramebuffer`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    #[expect(clippy::too_many_arguments, reason = "the command's parameters, as the registry declares them")]
    fn blit_framebuffer(&self, src_x0: i32, src_y0: i32, src_x1: i32, src_y1: i32, dst_x0: i32, dst_y0: i32, dst_x1: i32, dst_y1: i32, mask: u32, filter: u32);

    /// Calls [`glBufferData`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn buffer_data(&self, target: u32, size: i64, data: Option<&[u8]>, usage: u32);

    /// Calls [`glBufferSubData`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn buffer_sub_data(&self, target: u32, offset: i64, data: &[u8]);

    /// Calls [`glCheckFramebufferStatus`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn check_framebuffer_status(&self, target: u32) -> u32;

    /// Calls [`glClearBufferfi`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn clear_bufferfi(&self, buffer: u32, drawbuffer: i32, depth: f32, stencil: i32);

    /// Calls [`glClearBufferfv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn clear_bufferfv(&self, buffer: u32, drawbuffer: i32, value: &[f32]);

    /// Calls [`glClearBufferiv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn clear_bufferiv(&self, buffer: u32, drawbuffer: i32, value: &[i32]);

    /// Calls [`glClearBufferuiv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn clear_bufferuiv(&self, buffer: u32, drawbuffer: i32, value: &[u32]);

    /// Calls [`glClientWaitSync`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn client_wait_sync(&self, sync: &Self::Fence, flags: u32, timeout: u64) -> u32;

    /// Calls [`glColorMask`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn color_mask(&self, red: bool, green: bool, blue: bool, alpha: bool);

    /// Calls [`glCompileShader`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn compile_shader(&self, shader: u32);

    /// Calls [`glCompressedTexSubImage2D`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); bytes only with no pixel buffer bound and long enough, an offset only with one.
    #[expect(clippy::too_many_arguments, reason = "the command's parameters, as the registry declares them")]
    fn compressed_tex_sub_image_2d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, width: i32, height: i32, format: u32, data: CompressedPixelUnpack<'_>);

    /// Calls [`glCompressedTexSubImage3D`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); bytes only with no pixel buffer bound and long enough, an offset only with one.
    #[expect(clippy::too_many_arguments, reason = "the command's parameters, as the registry declares them")]
    fn compressed_tex_sub_image_3d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, zoffset: i32, width: i32, height: i32, depth: i32, format: u32, data: CompressedPixelUnpack<'_>);

    /// Calls [`glCopyBufferSubData`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn copy_buffer_sub_data(&self, read_target: u32, write_target: u32, read_offset: i64, write_offset: i64, size: i64);

    /// Calls [`glCopyTexSubImage2D`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    #[expect(clippy::too_many_arguments, reason = "the command's parameters, as the registry declares them")]
    fn copy_tex_sub_image_2d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, x: i32, y: i32, width: i32, height: i32);

    /// Calls [`glCopyTexSubImage3D`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    #[expect(clippy::too_many_arguments, reason = "the command's parameters, as the registry declares them")]
    fn copy_tex_sub_image_3d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, zoffset: i32, x: i32, y: i32, width: i32, height: i32);

    /// Calls [`glCreateProgram`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn create_program(&self) -> u32;

    /// Calls [`glCreateShader`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn create_shader(&self, type_: u32) -> u32;

    /// Calls [`glCullFace`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn cull_face(&self, mode: u32);

    /// Calls [`glDeleteBuffers`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn delete_buffers(&self, buffers: &[u32]);

    /// Calls [`glDeleteFramebuffers`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn delete_framebuffers(&self, framebuffers: &[u32]);

    /// Calls [`glDeleteProgram`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn delete_program(&self, program: u32);

    /// Calls [`glDeleteQueries`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn delete_queries(&self, ids: &[u32]);

    /// Calls [`glDeleteRenderbuffers`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn delete_renderbuffers(&self, renderbuffers: &[u32]);

    /// Calls [`glDeleteSamplers`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn delete_samplers(&self, samplers: &[u32]);

    /// Calls [`glDeleteShader`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn delete_shader(&self, shader: u32);

    /// Calls [`glDeleteSync`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn delete_sync(&self, sync: &Self::Fence);

    /// Calls [`glDeleteTextures`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn delete_textures(&self, textures: &[u32]);

    /// Calls [`glDeleteVertexArrays`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn delete_vertex_arrays(&self, arrays: &[u32]);

    /// Calls [`glDepthFunc`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn depth_func(&self, func: u32);

    /// Calls [`glDepthMask`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn depth_mask(&self, flag: bool);

    /// Calls [`glDepthRangef`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn depth_rangef(&self, n: f32, f: f32);

    /// Calls [`glDetachShader`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn detach_shader(&self, program: u32, shader: u32);

    /// Calls [`glDisable`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn disable(&self, cap: u32);

    /// Calls [`glDisableVertexAttribArray`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn disable_vertex_attrib_array(&self, index: u32);

    /// Calls [`glDrawArrays`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); refused while an enabled vertex attribute has no buffer.
    fn draw_arrays(&self, mode: u32, first: i32, count: i32);

    /// Calls [`glDrawArraysInstanced`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); refused while an enabled vertex attribute has no buffer.
    fn draw_arrays_instanced(&self, mode: u32, first: i32, count: i32, instancecount: i32);

    /// Calls [`glDrawBuffers`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn draw_buffers(&self, bufs: &[u32]);

    /// Calls [`glDrawElementsInstanced`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); refused with no element array buffer bound; refused while an enabled vertex attribute has no buffer.
    fn draw_elements_instanced(&self, mode: u32, count: i32, type_: u32, indices: u64, instancecount: i32);

    /// Calls [`glEnable`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn enable(&self, cap: u32);

    /// Calls [`glEnableVertexAttribArray`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn enable_vertex_attrib_array(&self, index: u32);

    /// Calls [`glEndQuery`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn end_query(&self, target: u32);

    /// Calls [`glEndTransformFeedback`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn end_transform_feedback(&self);

    /// Calls [`glFenceSync`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn fence_sync(&self, condition: u32, flags: u32) -> Option<Self::Fence>;

    /// Calls [`glFinish`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn finish(&self);

    /// Calls [`glFlush`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn flush(&self);

    /// Calls [`glFramebufferRenderbuffer`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn framebuffer_renderbuffer(&self, target: u32, attachment: u32, renderbuffertarget: u32, renderbuffer: u32);

    /// Calls [`glFramebufferTexture2D`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn framebuffer_texture_2d(&self, target: u32, attachment: u32, textarget: u32, texture: u32, level: i32);

    /// Calls [`glFramebufferTextureLayer`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn framebuffer_texture_layer(&self, target: u32, attachment: u32, texture: u32, level: i32, layer: i32);

    /// Calls [`glFrontFace`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn front_face(&self, mode: u32);

    /// Calls [`glGenBuffers`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn gen_buffers(&self, buffers: &mut [u32]);

    /// Calls [`glGenFramebuffers`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn gen_framebuffers(&self, framebuffers: &mut [u32]);

    /// Calls [`glGenQueries`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn gen_queries(&self, ids: &mut [u32]);

    /// Calls [`glGenRenderbuffers`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn gen_renderbuffers(&self, renderbuffers: &mut [u32]);

    /// Calls [`glGenSamplers`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn gen_samplers(&self, samplers: &mut [u32]);

    /// Calls [`glGenTextures`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn gen_textures(&self, textures: &mut [u32]);

    /// Calls [`glGenVertexArrays`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn gen_vertex_arrays(&self, arrays: &mut [u32]);

    /// Calls [`glGenerateMipmap`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn generate_mipmap(&self, target: u32);

    /// Calls [`glGetBooleanv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); refuses GL_COMPRESSED_TEXTURE_FORMATS, GL_PROGRAM_BINARY_FORMATS, GL_SHADER_BINARY_FORMATS.
    fn get_booleanv(&self, pname: u32, data: &mut [bool]);

    /// Calls [`glGetError`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn get_error(&self) -> u32;

    /// Calls [`glGetFloatv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); refuses GL_COMPRESSED_TEXTURE_FORMATS, GL_PROGRAM_BINARY_FORMATS, GL_SHADER_BINARY_FORMATS.
    fn get_floatv(&self, pname: u32, data: &mut [f32]);

    /// Calls [`glGetInteger64v`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); refuses GL_COMPRESSED_TEXTURE_FORMATS, GL_PROGRAM_BINARY_FORMATS, GL_SHADER_BINARY_FORMATS.
    fn get_integer64v(&self, pname: u32, data: &mut [i64]);

    /// Calls [`glGetIntegerv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); refuses GL_COMPRESSED_TEXTURE_FORMATS, GL_PROGRAM_BINARY_FORMATS, GL_SHADER_BINARY_FORMATS.
    fn get_integerv(&self, pname: u32, data: &mut [i32]);

    /// Calls [`glGetInternalformativ`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn get_internalformativ(&self, target: u32, internalformat: u32, pname: u32, params: &mut [i32]);

    /// Calls [`glGetProgramInfoLog`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn get_program_info_log(&self, program: u32, info_log: &mut [u8]) -> i32;

    /// Calls [`glGetProgramiv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn get_programiv(&self, program: u32, pname: u32, params: &mut [i32]);

    /// Calls [`glGetQueryObjectuiv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn get_query_objectuiv(&self, id: u32, pname: u32, params: &mut [u32]);

    /// Calls [`glGetShaderInfoLog`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn get_shader_info_log(&self, shader: u32, info_log: &mut [u8]) -> i32;

    /// Calls [`glGetShaderiv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn get_shaderiv(&self, shader: u32, pname: u32, params: &mut [i32]);

    /// Calls [`glGetString`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn get_string(&self, name: u32) -> Option<Vec<u8>>;

    /// Calls [`glGetStringi`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn get_stringi(&self, name: u32, index: u32) -> Option<Vec<u8>>;

    /// Calls [`glGetSynciv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn get_synciv(&self, sync: &Self::Fence, pname: u32, values: &mut [i32]) -> i32;

    /// Calls [`glGetUniformBlockIndex`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn get_uniform_block_index(&self, program: u32, uniform_block_name: &core::ffi::CStr) -> u32;

    /// Calls [`glGetUniformLocation`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn get_uniform_location(&self, program: u32, name: &core::ffi::CStr) -> i32;

    /// Calls [`glGetVertexAttribiv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn get_vertex_attribiv(&self, index: u32, pname: u32, params: &mut [i32]);

    /// Calls [`glInvalidateFramebuffer`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn invalidate_framebuffer(&self, target: u32, attachments: &[u32]);

    /// Calls [`glLinkProgram`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn link_program(&self, program: u32);

    /// Calls [`glPixelStorei`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn pixel_storei(&self, pname: u32, param: i32);

    /// Calls [`glPolygonOffset`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn polygon_offset(&self, factor: f32, units: f32);

    /// Calls [`glReadPixels`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); bytes only with no pixel buffer bound and long enough, an offset only with one.
    #[expect(clippy::too_many_arguments, reason = "the command's parameters, as the registry declares them")]
    fn read_pixels(&self, x: i32, y: i32, width: i32, height: i32, format: u32, type_: u32, pixels: PixelPack<'_>);

    /// Calls [`glRenderbufferStorage`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn renderbuffer_storage(&self, target: u32, internalformat: u32, width: i32, height: i32);

    /// Calls [`glRenderbufferStorageMultisample`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn renderbuffer_storage_multisample(&self, target: u32, samples: i32, internalformat: u32, width: i32, height: i32);

    /// Calls [`glSamplerParameterf`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn sampler_parameterf(&self, sampler: u32, pname: u32, param: f32);

    /// Calls [`glSamplerParameteri`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn sampler_parameteri(&self, sampler: u32, pname: u32, param: i32);

    /// Calls [`glScissor`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn scissor(&self, x: i32, y: i32, width: i32, height: i32);

    /// Calls [`glShaderSource`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn shader_source(&self, shader: u32, string: &[&[u8]]);

    /// Calls [`glStencilFunc`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn stencil_func(&self, func: u32, ref_: i32, mask: u32);

    /// Calls [`glStencilFuncSeparate`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn stencil_func_separate(&self, face: u32, func: u32, ref_: i32, mask: u32);

    /// Calls [`glStencilMask`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn stencil_mask(&self, mask: u32);

    /// Calls [`glStencilOp`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn stencil_op(&self, fail: u32, zfail: u32, zpass: u32);

    /// Calls [`glStencilOpSeparate`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn stencil_op_separate(&self, face: u32, sfail: u32, dpfail: u32, dppass: u32);

    /// Calls [`glTexParameteri`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn tex_parameteri(&self, target: u32, pname: u32, param: i32);

    /// Calls [`glTexStorage2D`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn tex_storage_2d(&self, target: u32, levels: i32, internalformat: u32, width: i32, height: i32);

    /// Calls [`glTexStorage3D`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn tex_storage_3d(&self, target: u32, levels: i32, internalformat: u32, width: i32, height: i32, depth: i32);

    /// Calls [`glTexSubImage2D`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); bytes only with no pixel buffer bound and long enough, an offset only with one.
    #[expect(clippy::too_many_arguments, reason = "the command's parameters, as the registry declares them")]
    fn tex_sub_image_2d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, width: i32, height: i32, format: u32, type_: u32, pixels: PixelUnpack<'_>);

    /// Calls [`glTexSubImage3D`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); bytes only with no pixel buffer bound and long enough, an offset only with one.
    #[expect(clippy::too_many_arguments, reason = "the command's parameters, as the registry declares them")]
    fn tex_sub_image_3d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, zoffset: i32, width: i32, height: i32, depth: i32, format: u32, type_: u32, pixels: PixelUnpack<'_>);

    /// Calls [`glTransformFeedbackVaryings`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn transform_feedback_varyings(&self, program: u32, varyings: &[&core::ffi::CStr], buffer_mode: u32);

    /// Calls [`glUniform1f`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform1f(&self, location: i32, v0: f32);

    /// Calls [`glUniform1fv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform1fv(&self, location: i32, value: &[f32]);

    /// Calls [`glUniform1i`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform1i(&self, location: i32, v0: i32);

    /// Calls [`glUniform1iv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform1iv(&self, location: i32, value: &[i32]);

    /// Calls [`glUniform1ui`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform1ui(&self, location: i32, v0: u32);

    /// Calls [`glUniform1uiv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform1uiv(&self, location: i32, value: &[u32]);

    /// Calls [`glUniform2f`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform2f(&self, location: i32, v0: f32, v1: f32);

    /// Calls [`glUniform2fv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform2fv(&self, location: i32, value: &[f32]);

    /// Calls [`glUniform2iv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform2iv(&self, location: i32, value: &[i32]);

    /// Calls [`glUniform2uiv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform2uiv(&self, location: i32, value: &[u32]);

    /// Calls [`glUniform3fv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform3fv(&self, location: i32, value: &[f32]);

    /// Calls [`glUniform3i`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform3i(&self, location: i32, v0: i32, v1: i32, v2: i32);

    /// Calls [`glUniform3iv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform3iv(&self, location: i32, value: &[i32]);

    /// Calls [`glUniform3uiv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform3uiv(&self, location: i32, value: &[u32]);

    /// Calls [`glUniform4fv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform4fv(&self, location: i32, value: &[f32]);

    /// Calls [`glUniform4iv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform4iv(&self, location: i32, value: &[i32]);

    /// Calls [`glUniform4uiv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform4uiv(&self, location: i32, value: &[u32]);

    /// Calls [`glUniformBlockBinding`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform_block_binding(&self, program: u32, uniform_block_index: u32, uniform_block_binding: u32);

    /// Calls [`glUniformMatrix2fv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform_matrix2fv(&self, location: i32, transpose: bool, value: &[f32]);

    /// Calls [`glUniformMatrix2x3fv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform_matrix2x3fv(&self, location: i32, transpose: bool, value: &[f32]);

    /// Calls [`glUniformMatrix2x4fv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform_matrix2x4fv(&self, location: i32, transpose: bool, value: &[f32]);

    /// Calls [`glUniformMatrix3fv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform_matrix3fv(&self, location: i32, transpose: bool, value: &[f32]);

    /// Calls [`glUniformMatrix3x2fv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform_matrix3x2fv(&self, location: i32, transpose: bool, value: &[f32]);

    /// Calls [`glUniformMatrix3x4fv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform_matrix3x4fv(&self, location: i32, transpose: bool, value: &[f32]);

    /// Calls [`glUniformMatrix4fv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform_matrix4fv(&self, location: i32, transpose: bool, value: &[f32]);

    /// Calls [`glUniformMatrix4x2fv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform_matrix4x2fv(&self, location: i32, transpose: bool, value: &[f32]);

    /// Calls [`glUniformMatrix4x3fv`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn uniform_matrix4x3fv(&self, location: i32, transpose: bool, value: &[f32]);

    /// Calls [`glUseProgram`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn use_program(&self, program: u32);

    /// Calls [`glVertexAttribDivisor`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn vertex_attrib_divisor(&self, index: u32, divisor: u32);

    /// Calls [`glVertexAttribIPointer`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); refused with no array buffer bound.
    fn vertex_attrib_i_pointer(&self, index: u32, size: i32, type_: u32, stride: i32, pointer: u64);

    /// Calls [`glVertexAttribPointer`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); refused with no array buffer bound.
    fn vertex_attrib_pointer(&self, index: u32, size: i32, type_: u32, normalized: bool, stride: i32, pointer: u64);

    /// Calls [`glViewport`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml).
    fn viewport(&self, x: i32, y: i32, width: i32, height: i32);

    /// Calls [`glBlendEquationSeparateiOES`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); None where the context names none of GL_OES_draw_buffers_indexed.
    fn blend_equation_separatei_oes(&self, buf: u32, mode_rgb: u32, mode_alpha: u32) -> Option<()>;

    /// Calls [`glBlendFuncSeparateiOES`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); None where the context names none of GL_OES_draw_buffers_indexed.
    fn blend_func_separatei_oes(&self, buf: u32, src_rgb: u32, dst_rgb: u32, src_alpha: u32, dst_alpha: u32) -> Option<()>;

    /// Calls [`glClipControlEXT`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); None where the context names none of GL_EXT_clip_control.
    fn clip_control_ext(&self, origin: u32, depth: u32) -> Option<()>;

    /// Calls [`glColorMaskiOES`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); None where the context names none of GL_OES_draw_buffers_indexed.
    fn color_maski_oes(&self, index: u32, r: bool, g: bool, b: bool, a: bool) -> Option<()>;

    /// Calls [`glDisableiOES`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); None where the context names none of GL_OES_draw_buffers_indexed, GL_OES_viewport_array.
    fn disablei_oes(&self, target: u32, index: u32) -> Option<()>;

    /// Calls [`glEnableiOES`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); None where the context names none of GL_OES_draw_buffers_indexed, GL_OES_viewport_array.
    fn enablei_oes(&self, target: u32, index: u32) -> Option<()>;

    /// Calls [`glGetQueryObjectui64vEXT`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); None where the context names none of GL_EXT_disjoint_timer_query.
    fn get_query_objectui64v_ext(&self, id: u32, pname: u32, params: &mut [u64]) -> Option<()>;

    /// Calls [`glGetQueryivEXT`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); None where the context names none of GL_EXT_disjoint_timer_query, GL_EXT_occlusion_query_boolean.
    fn get_queryiv_ext(&self, target: u32, pname: u32, params: &mut [i32]) -> Option<()>;

    /// Calls [`glPolygonModeANGLE`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); None where the context names none of GL_ANGLE_polygon_mode.
    fn polygon_mode_angle(&self, face: u32, mode: u32) -> Option<()>;

    /// Calls [`glPolygonOffsetClampEXT`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); None where the context names none of GL_EXT_polygon_offset_clamp.
    fn polygon_offset_clamp_ext(&self, factor: f32, units: f32, clamp: f32) -> Option<()>;

    /// Calls [`glProvokingVertexANGLE`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); None where the context names none of GL_ANGLE_provoking_vertex.
    fn provoking_vertex_angle(&self, provoke_mode: u32) -> Option<()>;

    /// Calls [`glQueryCounterEXT`](https://github.com/KhronosGroup/OpenGL-Registry/blob/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml); None where the context names none of GL_EXT_disjoint_timer_query.
    fn query_counter_ext(&self, id: u32, target: u32) -> Option<()>;

    /// glMapBufferRange returns memory lent until glUnmapBuffer; the method maps the range for reading only, copies it into the caller's bytes and unmaps, so nothing is lent past the call.
    fn map_buffer_range_read(&self, target: u32, offset: i64, data: &mut [u8]) -> bool;
}

/// An OpenGL ES context reached through EGL: made over an EGL context and the surfaces it draws to and reads
/// from, and made current on the calling thread before each call when it is not.
pub trait OnEgl<E: wcontainers_egl::Egl>: OpenGlEs {
    fn new(egl: &E, display: &E::Display, context: &E::Context, draw: Option<&E::Surface>, read: Option<&E::Surface>) -> Option<Self>;
}
