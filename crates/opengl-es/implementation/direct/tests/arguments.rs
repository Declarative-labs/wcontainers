// Written by tools/api-crate-generator from Khronos's gl.xml (and ANGLE's gl_angle_ext.xml for ANGLE's extensions), pinned in src/khronos/write.rs. Do not edit.
//! Every argument of every OpenGL ES method reaches the library as the method was given it: the generated stand-in
//! library (fake/fake_egl.c), built here with the C compiler (WCONTAINERS_CC, else `cc`), writes each call's arguments
//! to the file FAKE_LOG names (a number as C has it, a float to 9 digits, an address as `p` or `null`, an offset into a
//! bound buffer as its number), and each method is called once with arguments of its own, its line compared with what
//! the registry says the call passes (each value as given, each count its slice's length over its multiplier, each
//! pointer one of the caller's or ours). The calls run in a process of their own (this test run again), which the
//! log's variable is set for.
#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::process::Command;

use wcontainers_egl::Egl;
use wcontainers_egl::constants as egl;
use wcontainers_egl_direct::Library;
use wcontainers_opengl_es::constants;
use wcontainers_opengl_es::{CompressedPixelUnpack, OpenGlEs, PixelPack, PixelUnpack};
use wcontainers_opengl_es_direct::OpenGlEsContext;

/// The variable naming the stand-in's log, set for the process that makes the calls.
const LOG_VARIABLE: &str = "FAKE_LOG";
/// The variable naming the folder the stand-in was built into.
const FOLDER_VARIABLE: &str = "WCONTAINERS_FAKE_FOLDER";
/// The bytes every pixel transfer and optional byte range is given: more than any call here reaches.
const ROOM: usize = 8192;

/// The stand-in library built from fake/fake_egl.c, as libEGL.so.1 in a folder of this test's.
fn built() -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("fake-egl");
    std::fs::create_dir_all(&folder).expect("the stand-in's folder made");
    let compiler = std::env::var_os("WCONTAINERS_CC").unwrap_or_else(|| "cc".into());
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("fake").join("fake_egl.c");
    let status = Command::new(compiler).args(["-O1", "-std=c11", "-shared", "-fPIC", "-o"]).arg(folder.join("libEGL.so.1")).arg(&source).arg("-ldl").status().expect("the C compiler runs");
    assert!(status.success(), "the stand-in builds from {}", source.display());
    folder
}

/// The calls checked so far, the log's lines read, and what differed.
struct Calls {
    log: PathBuf,
    read: usize,
    checked: usize,
    failures: Vec<String>,
}

impl Calls {
    /// The last line `command` logged since the last check must be `expected`.
    fn expect(&mut self, command: &str, expected: &str) {
        let text = std::fs::read_to_string(&self.log).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let new = lines.get(self.read..).unwrap_or_default();
        let spaced = format!("{command} ");
        let received = new.iter().rev().find(|line| **line == command || line.starts_with(&spaced)).copied();
        if received != Some(expected) {
            self.failures.push(format!("{command}: the library received {received:?}, expected {expected:?}"));
        }
        self.read = lines.len();
        self.checked += 1;
    }
}

#[test]
fn every_argument_reaches_the_library() {
    let Some(log) = std::env::var_os(LOG_VARIABLE).map(PathBuf::from) else {
        let folder = built();
        let log = folder.join("calls.log");
        let binary = std::env::current_exe().expect("the test binary");
        let output = Command::new(binary)
            .args(["--exact", "every_argument_reaches_the_library", "--nocapture", "--test-threads=1"])
            .env(LOG_VARIABLE, &log)
            .env(FOLDER_VARIABLE, &folder)
            .output()
            .expect("the calls' process runs");
        let text = String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr);
        println!("{}", text.find("ARGUMENTS ").and_then(|at| text[at..].lines().next()).unwrap_or("ARGUMENTS: no count"));
        assert!(output.status.success(), "the calls' process failed: {text}");
        return;
    };
    let folder = PathBuf::from(std::env::var_os(FOLDER_VARIABLE).expect("the stand-in's folder named"));
    let library = Library::load(Some(&folder)).expect("the stand-in loads");
    let display = library.get_display().expect("the default display");
    library.initialize(&display).expect("the display initializes");
    assert!(library.bind_api(egl::OPENGL_ES_API.cast_unsigned()));
    let mut configs = vec![None; 1];
    let wanted = [egl::SURFACE_TYPE, egl::PBUFFER_BIT, egl::RENDERABLE_TYPE, egl::OPENGL_ES3_BIT, egl::NONE];
    library.choose_config(&display, &wanted, &mut configs).expect("eglChooseConfig succeeds");
    let config = configs[0].clone().expect("a configuration");
    let context = library.create_context(&display, &config, None, &[egl::CONTEXT_MAJOR_VERSION, 3, egl::NONE]).expect("a context");
    let surface = library.create_pbuffer_surface(&display, &config, &[egl::WIDTH, 1, egl::HEIGHT, 1, egl::NONE]).expect("a surface");
    let gl = OpenGlEsContext::owning(library, display, context, Some(surface.clone()), Some(surface)).expect("the context binds");
    // A buffer bound at each target a method takes an offset into (an offset with none bound is refused).
    let mut names = [0_u32; 2];
    gl.gen_buffers(&mut names);
    gl.bind_buffer(constants::ARRAY_BUFFER, names[0]);
    gl.bind_buffer(constants::ELEMENT_ARRAY_BUFFER, names[1]);
    let mut calls = Calls { log, read: 0, checked: 0, failures: Vec::new() };
    every_method(&gl, &mut calls);
    println!("ARGUMENTS {} methods checked, {} received otherwise", calls.checked, calls.failures.len());
    assert!(calls.failures.is_empty(), "{}", calls.failures.join("\n"));
}

/// A fence this context made, for a method that takes one.
fn fence(gl: &OpenGlEsContext) -> <OpenGlEsContext as OpenGlEs>::Fence {
    gl.fence_sync(constants::SYNC_GPU_COMMANDS_COMPLETE, 0).expect("a fence")
}

/// Each method called once with arguments of its own, the line the library logged for it checked.
fn every_method(gl: &OpenGlEsContext, calls: &mut Calls) {
    {
        gl.active_texture(1001);
        calls.expect("glActiveTexture", "glActiveTexture 1001");
    }
    {
        gl.attach_shader(1101, 1102);
        calls.expect("glAttachShader", "glAttachShader 1101 1102");
    }
    {
        gl.begin_query(1201, 1202);
        calls.expect("glBeginQuery", "glBeginQuery 1201 1202");
    }
    {
        gl.begin_transform_feedback(1301);
        calls.expect("glBeginTransformFeedback", "glBeginTransformFeedback 1301");
    }
    {
        gl.bind_buffer(1401, 1402);
        calls.expect("glBindBuffer", "glBindBuffer 1401 1402");
    }
    {
        gl.bind_buffer_base(1501, 1502, 1503);
        calls.expect("glBindBufferBase", "glBindBufferBase 1501 1502 1503");
    }
    {
        gl.bind_buffer_range(1601, 1602, 1603, 1604, 1605);
        calls.expect("glBindBufferRange", "glBindBufferRange 1601 1602 1603 1604 1605");
    }
    {
        gl.bind_framebuffer(1701, 1702);
        calls.expect("glBindFramebuffer", "glBindFramebuffer 1701 1702");
    }
    {
        gl.bind_renderbuffer(1801, 1802);
        calls.expect("glBindRenderbuffer", "glBindRenderbuffer 1801 1802");
    }
    {
        gl.bind_sampler(1901, 1902);
        calls.expect("glBindSampler", "glBindSampler 1901 1902");
    }
    {
        gl.bind_texture(2001, 2002);
        calls.expect("glBindTexture", "glBindTexture 2001 2002");
    }
    {
        gl.bind_vertex_array(2101);
        calls.expect("glBindVertexArray", "glBindVertexArray 2101");
    }
    {
        gl.blend_color(2201.5, 2202.5, 2203.5, 2204.5);
        calls.expect("glBlendColor", "glBlendColor 2201.5 2202.5 2203.5 2204.5");
    }
    {
        gl.blend_equation_separate(2301, 2302);
        calls.expect("glBlendEquationSeparate", "glBlendEquationSeparate 2301 2302");
    }
    {
        gl.blend_func_separate(2401, 2402, 2403, 2404);
        calls.expect("glBlendFuncSeparate", "glBlendFuncSeparate 2401 2402 2403 2404");
    }
    {
        gl.blit_framebuffer(2501, 2502, 2503, 2504, 2505, 2506, 2507, 2508, 2509, 2510);
        calls.expect("glBlitFramebuffer", "glBlitFramebuffer 2501 2502 2503 2504 2505 2506 2507 2508 2509 2510");
    }
    {
        let data = vec![0_u8; ROOM];
        gl.buffer_data(2601, 2602, Some(data.as_slice()), 2604);
        calls.expect("glBufferData", "glBufferData 2601 2602 p 2604");
    }
    {
        let data = vec![0_u8; 2];
        gl.buffer_sub_data(2701, 2702, &data);
        calls.expect("glBufferSubData", "glBufferSubData 2701 2702 2 p");
    }
    {
        let _ = gl.check_framebuffer_status(2801);
        calls.expect("glCheckFramebufferStatus", "glCheckFramebufferStatus 2801");
    }
    {
        gl.clear_bufferfi(2901, 2902, 2903.5, 2904);
        calls.expect("glClearBufferfi", "glClearBufferfi 2901 2902 2903.5 2904");
    }
    {
        let value = vec![0.0_f32; 1];
        gl.clear_bufferfv(3001, 3002, &value);
        calls.expect("glClearBufferfv", "glClearBufferfv 3001 3002 p");
    }
    {
        let value = vec![0_i32; 1];
        gl.clear_bufferiv(3101, 3102, &value);
        calls.expect("glClearBufferiv", "glClearBufferiv 3101 3102 p");
    }
    {
        let value = vec![0_u32; 1];
        gl.clear_bufferuiv(3201, 3202, &value);
        calls.expect("glClearBufferuiv", "glClearBufferuiv 3201 3202 p");
    }
    {
        let sync = fence(gl);
        let _ = gl.client_wait_sync(&sync, 3302, 3303);
        calls.expect("glClientWaitSync", "glClientWaitSync p 3302 3303");
    }
    {
        gl.color_mask(true, true, true, true);
        calls.expect("glColorMask", "glColorMask 1 1 1 1");
    }
    {
        gl.compile_shader(3501);
        calls.expect("glCompileShader", "glCompileShader 3501");
    }
    {
        let data = vec![0_u8; 8];
        gl.compressed_tex_sub_image_2d(3601, 3602, 3603, 3604, 3605, 3606, 3607, CompressedPixelUnpack::Bytes(&data));
        calls.expect("glCompressedTexSubImage2D", "glCompressedTexSubImage2D 3601 3602 3603 3604 3605 3606 3607 8 p");
    }
    {
        let data = vec![0_u8; 8];
        gl.compressed_tex_sub_image_3d(3701, 3702, 3703, 3704, 3705, 3706, 3707, 3708, 3709, CompressedPixelUnpack::Bytes(&data));
        calls.expect("glCompressedTexSubImage3D", "glCompressedTexSubImage3D 3701 3702 3703 3704 3705 3706 3707 3708 3709 8 p");
    }
    {
        gl.copy_buffer_sub_data(3801, 3802, 3803, 3804, 3805);
        calls.expect("glCopyBufferSubData", "glCopyBufferSubData 3801 3802 3803 3804 3805");
    }
    {
        gl.copy_tex_sub_image_2d(3901, 3902, 3903, 3904, 3905, 3906, 3907, 3908);
        calls.expect("glCopyTexSubImage2D", "glCopyTexSubImage2D 3901 3902 3903 3904 3905 3906 3907 3908");
    }
    {
        gl.copy_tex_sub_image_3d(4001, 4002, 4003, 4004, 4005, 4006, 4007, 4008, 4009);
        calls.expect("glCopyTexSubImage3D", "glCopyTexSubImage3D 4001 4002 4003 4004 4005 4006 4007 4008 4009");
    }
    {
        let _ = gl.create_program();
        calls.expect("glCreateProgram", "glCreateProgram");
    }
    {
        let _ = gl.create_shader(4201);
        calls.expect("glCreateShader", "glCreateShader 4201");
    }
    {
        gl.cull_face(4301);
        calls.expect("glCullFace", "glCullFace 4301");
    }
    {
        let buffers = vec![0_u32; 2];
        gl.delete_buffers(&buffers);
        calls.expect("glDeleteBuffers", "glDeleteBuffers 2 p");
    }
    {
        let framebuffers = vec![0_u32; 2];
        gl.delete_framebuffers(&framebuffers);
        calls.expect("glDeleteFramebuffers", "glDeleteFramebuffers 2 p");
    }
    {
        gl.delete_program(4601);
        calls.expect("glDeleteProgram", "glDeleteProgram 4601");
    }
    {
        let ids = vec![0_u32; 2];
        gl.delete_queries(&ids);
        calls.expect("glDeleteQueries", "glDeleteQueries 2 p");
    }
    {
        let renderbuffers = vec![0_u32; 2];
        gl.delete_renderbuffers(&renderbuffers);
        calls.expect("glDeleteRenderbuffers", "glDeleteRenderbuffers 2 p");
    }
    {
        let samplers = vec![0_u32; 2];
        gl.delete_samplers(&samplers);
        calls.expect("glDeleteSamplers", "glDeleteSamplers 2 p");
    }
    {
        gl.delete_shader(5001);
        calls.expect("glDeleteShader", "glDeleteShader 5001");
    }
    {
        let sync = fence(gl);
        gl.delete_sync(&sync);
        calls.expect("glDeleteSync", "glDeleteSync p");
    }
    {
        let textures = vec![0_u32; 2];
        gl.delete_textures(&textures);
        calls.expect("glDeleteTextures", "glDeleteTextures 2 p");
    }
    {
        let arrays = vec![0_u32; 2];
        gl.delete_vertex_arrays(&arrays);
        calls.expect("glDeleteVertexArrays", "glDeleteVertexArrays 2 p");
    }
    {
        gl.depth_func(5401);
        calls.expect("glDepthFunc", "glDepthFunc 5401");
    }
    {
        gl.depth_mask(true);
        calls.expect("glDepthMask", "glDepthMask 1");
    }
    {
        gl.depth_rangef(5601.5, 5602.5);
        calls.expect("glDepthRangef", "glDepthRangef 5601.5 5602.5");
    }
    {
        gl.detach_shader(5701, 5702);
        calls.expect("glDetachShader", "glDetachShader 5701 5702");
    }
    {
        gl.disable(5801);
        calls.expect("glDisable", "glDisable 5801");
    }
    {
        gl.disable_vertex_attrib_array(5901);
        calls.expect("glDisableVertexAttribArray", "glDisableVertexAttribArray 5901");
    }
    {
        gl.draw_arrays(6001, 6002, 6003);
        calls.expect("glDrawArrays", "glDrawArrays 6001 6002 6003");
    }
    {
        gl.draw_arrays_instanced(6101, 6102, 6103, 6104);
        calls.expect("glDrawArraysInstanced", "glDrawArraysInstanced 6101 6102 6103 6104");
    }
    {
        let bufs = vec![0_u32; 2];
        gl.draw_buffers(&bufs);
        calls.expect("glDrawBuffers", "glDrawBuffers 2 p");
    }
    {
        gl.draw_elements_instanced(6301, 6302, 6303, 16, 6305);
        calls.expect("glDrawElementsInstanced", "glDrawElementsInstanced 6301 6302 6303 16 6305");
    }
    {
        gl.enable(6401);
        calls.expect("glEnable", "glEnable 6401");
    }
    {
        gl.enable_vertex_attrib_array(6501);
        calls.expect("glEnableVertexAttribArray", "glEnableVertexAttribArray 6501");
    }
    {
        gl.end_query(6601);
        calls.expect("glEndQuery", "glEndQuery 6601");
    }
    {
        gl.end_transform_feedback();
        calls.expect("glEndTransformFeedback", "glEndTransformFeedback");
    }
    {
        let _ = gl.fence_sync(6801, 6802);
        calls.expect("glFenceSync", "glFenceSync 6801 6802");
    }
    {
        gl.finish();
        calls.expect("glFinish", "glFinish");
    }
    {
        gl.flush();
        calls.expect("glFlush", "glFlush");
    }
    {
        gl.framebuffer_renderbuffer(1101, 1102, 1103, 1104);
        calls.expect("glFramebufferRenderbuffer", "glFramebufferRenderbuffer 1101 1102 1103 1104");
    }
    {
        gl.framebuffer_texture_2d(1201, 1202, 1203, 1204, 1205);
        calls.expect("glFramebufferTexture2D", "glFramebufferTexture2D 1201 1202 1203 1204 1205");
    }
    {
        gl.framebuffer_texture_layer(1301, 1302, 1303, 1304, 1305);
        calls.expect("glFramebufferTextureLayer", "glFramebufferTextureLayer 1301 1302 1303 1304 1305");
    }
    {
        gl.front_face(1401);
        calls.expect("glFrontFace", "glFrontFace 1401");
    }
    {
        let mut buffers = vec![0_u32; 2];
        gl.gen_buffers(&mut buffers);
        calls.expect("glGenBuffers", "glGenBuffers 2 p");
    }
    {
        let mut framebuffers = vec![0_u32; 2];
        gl.gen_framebuffers(&mut framebuffers);
        calls.expect("glGenFramebuffers", "glGenFramebuffers 2 p");
    }
    {
        let mut ids = vec![0_u32; 2];
        gl.gen_queries(&mut ids);
        calls.expect("glGenQueries", "glGenQueries 2 p");
    }
    {
        let mut renderbuffers = vec![0_u32; 2];
        gl.gen_renderbuffers(&mut renderbuffers);
        calls.expect("glGenRenderbuffers", "glGenRenderbuffers 2 p");
    }
    {
        let mut samplers = vec![0_u32; 2];
        gl.gen_samplers(&mut samplers);
        calls.expect("glGenSamplers", "glGenSamplers 2 p");
    }
    {
        let mut textures = vec![0_u32; 2];
        gl.gen_textures(&mut textures);
        calls.expect("glGenTextures", "glGenTextures 2 p");
    }
    {
        let mut arrays = vec![0_u32; 2];
        gl.gen_vertex_arrays(&mut arrays);
        calls.expect("glGenVertexArrays", "glGenVertexArrays 2 p");
    }
    {
        gl.generate_mipmap(2201);
        calls.expect("glGenerateMipmap", "glGenerateMipmap 2201");
    }
    {
        let mut data = vec![false; 1];
        gl.get_booleanv(2301, &mut data);
        calls.expect("glGetBooleanv", "glGetBooleanv 2301 p");
    }
    {
        let _ = gl.get_error();
        let _ = gl.get_error();
        calls.expect("glGetError", "glGetError");
    }
    {
        let mut data = vec![0.0_f32; 1];
        gl.get_floatv(2501, &mut data);
        calls.expect("glGetFloatv", "glGetFloatv 2501 p");
    }
    {
        let mut data = vec![0_i64; 1];
        gl.get_integer64v(2601, &mut data);
        calls.expect("glGetInteger64v", "glGetInteger64v 2601 p");
    }
    {
        let mut data = vec![0_i32; 1];
        gl.get_integerv(2701, &mut data);
        calls.expect("glGetIntegerv", "glGetIntegerv 2701 p");
    }
    {
        let mut params = vec![0_i32; 2];
        gl.get_internalformativ(2801, 2802, 2803, &mut params);
        calls.expect("glGetInternalformativ", "glGetInternalformativ 2801 2802 2803 2 p");
    }
    {
        let mut info_log = vec![0_u8; 2];
        let _ = gl.get_program_info_log(2901, &mut info_log);
        calls.expect("glGetProgramInfoLog", "glGetProgramInfoLog 2901 2 p p");
    }
    {
        let mut params = vec![0_i32; 1];
        gl.get_programiv(3001, 3002, &mut params);
        calls.expect("glGetProgramiv", "glGetProgramiv 3001 3002 p");
    }
    {
        let mut params = vec![0_u32; 1];
        gl.get_query_objectuiv(3101, 3102, &mut params);
        calls.expect("glGetQueryObjectuiv", "glGetQueryObjectuiv 3101 3102 p");
    }
    {
        let mut info_log = vec![0_u8; 2];
        let _ = gl.get_shader_info_log(3201, &mut info_log);
        calls.expect("glGetShaderInfoLog", "glGetShaderInfoLog 3201 2 p p");
    }
    {
        let mut params = vec![0_i32; 1];
        gl.get_shaderiv(3301, 3302, &mut params);
        calls.expect("glGetShaderiv", "glGetShaderiv 3301 3302 p");
    }
    {
        let _ = gl.get_string(3401);
        calls.expect("glGetString", "glGetString 3401");
    }
    {
        let _ = gl.get_stringi(3501, 3502);
        calls.expect("glGetStringi", "glGetStringi 3501 3502");
    }
    {
        let sync = fence(gl);
        let mut values = vec![0_i32; 2];
        let _ = gl.get_synciv(&sync, 3602, &mut values);
        calls.expect("glGetSynciv", "glGetSynciv p 3602 2 p p");
    }
    {
        let _ = gl.get_uniform_block_index(3701, c"name");
        calls.expect("glGetUniformBlockIndex", "glGetUniformBlockIndex 3701 p");
    }
    {
        let _ = gl.get_uniform_location(3801, c"name");
        calls.expect("glGetUniformLocation", "glGetUniformLocation 3801 p");
    }
    {
        let mut params = vec![0_i32; 1];
        gl.get_vertex_attribiv(3901, 3902, &mut params);
        calls.expect("glGetVertexAttribiv", "glGetVertexAttribiv 3901 3902 p");
    }
    {
        let attachments = vec![0_u32; 2];
        gl.invalidate_framebuffer(4001, &attachments);
        calls.expect("glInvalidateFramebuffer", "glInvalidateFramebuffer 4001 2 p");
    }
    {
        gl.link_program(4101);
        calls.expect("glLinkProgram", "glLinkProgram 4101");
    }
    {
        gl.pixel_storei(4201, 4202);
        calls.expect("glPixelStorei", "glPixelStorei 4201 4202");
    }
    {
        gl.polygon_offset(4301.5, 4302.5);
        calls.expect("glPolygonOffset", "glPolygonOffset 4301.5 4302.5");
    }
    {
        let mut pixels = vec![0_u8; ROOM];
        gl.read_pixels(4401, 4402, 2, 2, constants::RGBA, constants::UNSIGNED_BYTE, PixelPack::Bytes(&mut pixels));
        calls.expect("glReadPixels", "glReadPixels 4401 4402 2 2 6408 5121 p");
    }
    {
        gl.renderbuffer_storage(4501, 4502, 4503, 4504);
        calls.expect("glRenderbufferStorage", "glRenderbufferStorage 4501 4502 4503 4504");
    }
    {
        gl.renderbuffer_storage_multisample(4601, 4602, 4603, 4604, 4605);
        calls.expect("glRenderbufferStorageMultisample", "glRenderbufferStorageMultisample 4601 4602 4603 4604 4605");
    }
    {
        gl.sampler_parameterf(4701, 4702, 4703.5);
        calls.expect("glSamplerParameterf", "glSamplerParameterf 4701 4702 4703.5");
    }
    {
        gl.sampler_parameteri(4801, 4802, 4803);
        calls.expect("glSamplerParameteri", "glSamplerParameteri 4801 4802 4803");
    }
    {
        gl.scissor(4901, 4902, 4903, 4904);
        calls.expect("glScissor", "glScissor 4901 4902 4903 4904");
    }
    {
        gl.shader_source(5001, &[b"void main() {}".as_slice(), b"void main() {}".as_slice()]);
        calls.expect("glShaderSource", "glShaderSource 5001 2 p p");
    }
    {
        gl.stencil_func(5101, 5102, 5103);
        calls.expect("glStencilFunc", "glStencilFunc 5101 5102 5103");
    }
    {
        gl.stencil_func_separate(5201, 5202, 5203, 5204);
        calls.expect("glStencilFuncSeparate", "glStencilFuncSeparate 5201 5202 5203 5204");
    }
    {
        gl.stencil_mask(5301);
        calls.expect("glStencilMask", "glStencilMask 5301");
    }
    {
        gl.stencil_op(5401, 5402, 5403);
        calls.expect("glStencilOp", "glStencilOp 5401 5402 5403");
    }
    {
        gl.stencil_op_separate(5501, 5502, 5503, 5504);
        calls.expect("glStencilOpSeparate", "glStencilOpSeparate 5501 5502 5503 5504");
    }
    {
        gl.tex_parameteri(5601, 5602, 5603);
        calls.expect("glTexParameteri", "glTexParameteri 5601 5602 5603");
    }
    {
        gl.tex_storage_2d(5701, 5702, 5703, 5704, 5705);
        calls.expect("glTexStorage2D", "glTexStorage2D 5701 5702 5703 5704 5705");
    }
    {
        gl.tex_storage_3d(5801, 5802, 5803, 5804, 5805, 5806);
        calls.expect("glTexStorage3D", "glTexStorage3D 5801 5802 5803 5804 5805 5806");
    }
    {
        let pixels = vec![0_u8; ROOM];
        gl.tex_sub_image_2d(5901, 5902, 5903, 5904, 2, 2, constants::RGBA, constants::UNSIGNED_BYTE, PixelUnpack::Bytes(&pixels));
        calls.expect("glTexSubImage2D", "glTexSubImage2D 5901 5902 5903 5904 2 2 6408 5121 p");
    }
    {
        let pixels = vec![0_u8; ROOM];
        gl.tex_sub_image_3d(6001, 6002, 6003, 6004, 6005, 2, 2, 1, constants::RGBA, constants::UNSIGNED_BYTE, PixelUnpack::Bytes(&pixels));
        calls.expect("glTexSubImage3D", "glTexSubImage3D 6001 6002 6003 6004 6005 2 2 1 6408 5121 p");
    }
    {
        gl.transform_feedback_varyings(6101, &[c"name", c"name"], 6104);
        calls.expect("glTransformFeedbackVaryings", "glTransformFeedbackVaryings 6101 2 p 6104");
    }
    {
        gl.uniform1f(6201, 6202.5);
        calls.expect("glUniform1f", "glUniform1f 6201 6202.5");
    }
    {
        let value = vec![0.0_f32; 2];
        gl.uniform1fv(6301, &value);
        calls.expect("glUniform1fv", "glUniform1fv 6301 2 p");
    }
    {
        gl.uniform1i(6401, 6402);
        calls.expect("glUniform1i", "glUniform1i 6401 6402");
    }
    {
        let value = vec![0_i32; 2];
        gl.uniform1iv(6501, &value);
        calls.expect("glUniform1iv", "glUniform1iv 6501 2 p");
    }
    {
        gl.uniform1ui(6601, 6602);
        calls.expect("glUniform1ui", "glUniform1ui 6601 6602");
    }
    {
        let value = vec![0_u32; 2];
        gl.uniform1uiv(6701, &value);
        calls.expect("glUniform1uiv", "glUniform1uiv 6701 2 p");
    }
    {
        gl.uniform2f(6801, 6802.5, 6803.5);
        calls.expect("glUniform2f", "glUniform2f 6801 6802.5 6803.5");
    }
    {
        let value = vec![0.0_f32; 4];
        gl.uniform2fv(6901, &value);
        calls.expect("glUniform2fv", "glUniform2fv 6901 2 p");
    }
    {
        let value = vec![0_i32; 4];
        gl.uniform2iv(1001, &value);
        calls.expect("glUniform2iv", "glUniform2iv 1001 2 p");
    }
    {
        let value = vec![0_u32; 4];
        gl.uniform2uiv(1101, &value);
        calls.expect("glUniform2uiv", "glUniform2uiv 1101 2 p");
    }
    {
        let value = vec![0.0_f32; 6];
        gl.uniform3fv(1201, &value);
        calls.expect("glUniform3fv", "glUniform3fv 1201 2 p");
    }
    {
        gl.uniform3i(1301, 1302, 1303, 1304);
        calls.expect("glUniform3i", "glUniform3i 1301 1302 1303 1304");
    }
    {
        let value = vec![0_i32; 6];
        gl.uniform3iv(1401, &value);
        calls.expect("glUniform3iv", "glUniform3iv 1401 2 p");
    }
    {
        let value = vec![0_u32; 6];
        gl.uniform3uiv(1501, &value);
        calls.expect("glUniform3uiv", "glUniform3uiv 1501 2 p");
    }
    {
        let value = vec![0.0_f32; 8];
        gl.uniform4fv(1601, &value);
        calls.expect("glUniform4fv", "glUniform4fv 1601 2 p");
    }
    {
        let value = vec![0_i32; 8];
        gl.uniform4iv(1701, &value);
        calls.expect("glUniform4iv", "glUniform4iv 1701 2 p");
    }
    {
        let value = vec![0_u32; 8];
        gl.uniform4uiv(1801, &value);
        calls.expect("glUniform4uiv", "glUniform4uiv 1801 2 p");
    }
    {
        gl.uniform_block_binding(1901, 1902, 1903);
        calls.expect("glUniformBlockBinding", "glUniformBlockBinding 1901 1902 1903");
    }
    {
        let value = vec![0.0_f32; 8];
        gl.uniform_matrix2fv(2001, true, &value);
        calls.expect("glUniformMatrix2fv", "glUniformMatrix2fv 2001 2 1 p");
    }
    {
        let value = vec![0.0_f32; 12];
        gl.uniform_matrix2x3fv(2101, true, &value);
        calls.expect("glUniformMatrix2x3fv", "glUniformMatrix2x3fv 2101 2 1 p");
    }
    {
        let value = vec![0.0_f32; 16];
        gl.uniform_matrix2x4fv(2201, true, &value);
        calls.expect("glUniformMatrix2x4fv", "glUniformMatrix2x4fv 2201 2 1 p");
    }
    {
        let value = vec![0.0_f32; 18];
        gl.uniform_matrix3fv(2301, true, &value);
        calls.expect("glUniformMatrix3fv", "glUniformMatrix3fv 2301 2 1 p");
    }
    {
        let value = vec![0.0_f32; 12];
        gl.uniform_matrix3x2fv(2401, true, &value);
        calls.expect("glUniformMatrix3x2fv", "glUniformMatrix3x2fv 2401 2 1 p");
    }
    {
        let value = vec![0.0_f32; 24];
        gl.uniform_matrix3x4fv(2501, true, &value);
        calls.expect("glUniformMatrix3x4fv", "glUniformMatrix3x4fv 2501 2 1 p");
    }
    {
        let value = vec![0.0_f32; 32];
        gl.uniform_matrix4fv(2601, true, &value);
        calls.expect("glUniformMatrix4fv", "glUniformMatrix4fv 2601 2 1 p");
    }
    {
        let value = vec![0.0_f32; 16];
        gl.uniform_matrix4x2fv(2701, true, &value);
        calls.expect("glUniformMatrix4x2fv", "glUniformMatrix4x2fv 2701 2 1 p");
    }
    {
        let value = vec![0.0_f32; 24];
        gl.uniform_matrix4x3fv(2801, true, &value);
        calls.expect("glUniformMatrix4x3fv", "glUniformMatrix4x3fv 2801 2 1 p");
    }
    {
        gl.use_program(2901);
        calls.expect("glUseProgram", "glUseProgram 2901");
    }
    {
        gl.vertex_attrib_divisor(3001, 3002);
        calls.expect("glVertexAttribDivisor", "glVertexAttribDivisor 3001 3002");
    }
    {
        gl.vertex_attrib_i_pointer(3101, 3102, 3103, 3104, 16);
        calls.expect("glVertexAttribIPointer", "glVertexAttribIPointer 3101 3102 3103 3104 16");
    }
    {
        gl.vertex_attrib_pointer(3201, 3202, 3203, true, 3205, 16);
        calls.expect("glVertexAttribPointer", "glVertexAttribPointer 3201 3202 3203 1 3205 16");
    }
    {
        gl.viewport(3301, 3302, 3303, 3304);
        calls.expect("glViewport", "glViewport 3301 3302 3303 3304");
    }
    {
        let _ = gl.blend_equation_separatei_oes(3401, 3402, 3403);
        calls.expect("glBlendEquationSeparateiOES", "glBlendEquationSeparateiOES 3401 3402 3403");
    }
    {
        let _ = gl.blend_func_separatei_oes(3501, 3502, 3503, 3504, 3505);
        calls.expect("glBlendFuncSeparateiOES", "glBlendFuncSeparateiOES 3501 3502 3503 3504 3505");
    }
    {
        let _ = gl.clip_control_ext(3601, 3602);
        calls.expect("glClipControlEXT", "glClipControlEXT 3601 3602");
    }
    {
        let _ = gl.color_maski_oes(3701, true, true, true, true);
        calls.expect("glColorMaskiOES", "glColorMaskiOES 3701 1 1 1 1");
    }
    {
        let _ = gl.disablei_oes(3801, 3802);
        calls.expect("glDisableiOES", "glDisableiOES 3801 3802");
    }
    {
        let _ = gl.enablei_oes(3901, 3902);
        calls.expect("glEnableiOES", "glEnableiOES 3901 3902");
    }
    {
        let mut params = vec![0_u64; 1];
        let _ = gl.get_query_objectui64v_ext(4001, 4002, &mut params);
        calls.expect("glGetQueryObjectui64vEXT", "glGetQueryObjectui64vEXT 4001 4002 p");
    }
    {
        let mut params = vec![0_i32; 1];
        let _ = gl.get_queryiv_ext(4101, 4102, &mut params);
        calls.expect("glGetQueryivEXT", "glGetQueryivEXT 4101 4102 p");
    }
    {
        let _ = gl.polygon_mode_angle(4201, 4202);
        calls.expect("glPolygonModeANGLE", "glPolygonModeANGLE 4201 4202");
    }
    {
        let _ = gl.polygon_offset_clamp_ext(4301.5, 4302.5, 4303.5);
        calls.expect("glPolygonOffsetClampEXT", "glPolygonOffsetClampEXT 4301.5 4302.5 4303.5");
    }
    {
        let _ = gl.provoking_vertex_angle(4401);
        calls.expect("glProvokingVertexANGLE", "glProvokingVertexANGLE 4401");
    }
    {
        let _ = gl.query_counter_ext(4501, 4502);
        calls.expect("glQueryCounterEXT", "glQueryCounterEXT 4501 4502");
    }
}
