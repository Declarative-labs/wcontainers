//! The tests every implementation of wcontainers-opengl-es over wcontainers-egl runs, written against the traits
//! alone: a context made, drawn with and read back; every refusal of the glue's own checks; every method called.

use std::path::Path;

use wcontainers_egl::Egl;
use wcontainers_egl::constants as egl;
use wcontainers_opengl_es::constants as gl;
use wcontainers_opengl_es::{CompressedPixelUnpack, OnEgl, OpenGlEs, PixelPack, PixelUnpack};

/// The values the method calls pick from: edges, and numbers small enough that no draw or store takes long.
const NUMBERS: [i32; 8] = [0, 1, 2, 3, 4, 64, -1, i32::MIN];
const WORDS: [u32; 12] = [0, 1, 2, gl::TEXTURE_2D, gl::ARRAY_BUFFER, gl::RGBA, gl::UNSIGNED_BYTE, gl::FLOAT, gl::TRIANGLES, gl::COLOR, gl::FRAMEBUFFER, u32::MAX];
const SIZES: [i64; 6] = [0, 1, 4, 16, 256, -1];
const FLOATS: [f32; 6] = [0.0, 1.0, -1.0, 0.5, f32::MAX, f32::NAN];
const LENGTHS: [usize; 6] = [0, 1, 3, 4, 16, 1024];

/// The side of the pixel buffer and of the images the scenarios draw and read.
const SIDE: i32 = 4;
/// Bytes of one RGBA8 pixel.
const PIXEL: usize = 4;
/// The pixels of a SIDE by SIDE RGBA8 image.
const IMAGE: usize = (SIDE * SIDE) as usize * PIXEL;
/// One second, the most a scenario waits for its fence.
const SECOND: u64 = 1_000_000_000;

/// An OpenGL ES 3 context's attributes.
const CONTEXT_ES3: [i32; 3] = [egl::CONTEXT_MAJOR_VERSION, 3, egl::NONE];

fn pick<T: Copy>(values: &[T], seed: usize) -> T {
    values[seed.wrapping_mul(0x9E37_79B9) % values.len()]
}

fn length(seed: usize) -> usize {
    pick(&LENGTHS, seed)
}

/// What a scenario holds: EGL's library, its display, the context and its pixel buffer, and the OpenGL ES context.
struct Made<E: Egl, G> {
    egl: E,
    display: E::Display,
    context: E::Context,
    gl: G,
}

fn make<E: Egl, G: OnEgl<E>>(folder: Option<&Path>, attributes: &[i32]) -> Made<E, G> {
    let library = E::load(folder).expect("EGL loads from the folder named");
    let display = library.get_display().expect("the default display");
    let (major, minor) = library.initialize(&display).expect("the display initializes");
    assert!((major, minor) >= (1, 4), "EGL {major}.{minor}");
    assert!(library.bind_api(egl::OPENGL_ES_API.cast_unsigned()));
    let mut configs = vec![None; 8];
    let wanted = [
        egl::SURFACE_TYPE, egl::PBUFFER_BIT, egl::RENDERABLE_TYPE, egl::OPENGL_ES3_BIT, egl::RED_SIZE, 8, egl::GREEN_SIZE, 8, egl::BLUE_SIZE, 8,
        egl::ALPHA_SIZE, 8, egl::NONE,
    ];
    let found = library.choose_config(&display, &wanted, &mut configs).expect("eglChooseConfig succeeds");
    assert!(found > 0, "no configuration for OpenGL ES 3 with a pixel buffer");
    let config = configs[0].clone().expect("the first configuration found");
    assert_eq!(library.get_config_attrib(&display, &config, egl::RED_SIZE), Some(8));
    let context = library.create_context(&display, &config, None, attributes).expect("an OpenGL ES 3 context");
    let surface = library
        .create_pbuffer_surface(&display, &config, &[egl::WIDTH, SIDE, egl::HEIGHT, SIDE, egl::NONE])
        .expect("a pixel buffer surface");
    assert_eq!(library.query_surface(&display, &surface, egl::WIDTH), Some(SIDE));
    let gl = G::new(&library, &display, &context, Some(&surface), Some(&surface)).expect("the context binds");
    Made { egl: library, display, context, gl }
}

fn compiled<G: OpenGlEs>(gl: &G, kind: u32, source: &str) -> u32 {
    let shader = gl.create_shader(kind);
    gl.shader_source(shader, &[source.as_bytes()]);
    gl.compile_shader(shader);
    let mut status = [0_i32];
    gl.get_shaderiv(shader, gl::COMPILE_STATUS, &mut status);
    let mut log = vec![0_u8; 4096];
    let written = gl.get_shader_info_log(shader, &mut log);
    assert_eq!(status[0], 1, "{}", String::from_utf8_lossy(&log[..usize::try_from(written).unwrap_or(0).min(log.len())]));
    shader
}

/// Makes a context, writes and reads back a buffer and an image, draws a triangle over the pixel buffer and reads it.
pub fn draws_and_reads_back<E: Egl, G: OnEgl<E>>(folder: Option<&Path>) {
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    let gl = &made.gl;
    let version = gl.get_string(gl::VERSION).expect("a version");
    assert!(version.starts_with(b"OpenGL ES 3"), "{}", String::from_utf8_lossy(&version));
    // A buffer written whole, then in part, and read back.
    let mut buffers = [0_u32; 2];
    gl.gen_buffers(&mut buffers);
    assert!(buffers.iter().all(|&buffer| buffer != 0));
    gl.bind_buffer(gl::ARRAY_BUFFER, buffers[0]);
    let bytes: Vec<u8> = (0..=u8::MAX).collect();
    gl.buffer_data(gl::ARRAY_BUFFER, i64::try_from(bytes.len()).unwrap_or(0), Some(&bytes), gl::STATIC_DRAW);
    gl.buffer_sub_data(gl::ARRAY_BUFFER, 16, &[7; PIXEL]);
    let mut back = vec![0_u8; 32];
    assert!(gl.map_buffer_range_read(gl::ARRAY_BUFFER, 0, &mut back));
    let mut expected: Vec<u8> = (0..32).collect();
    expected[16..20].fill(7);
    assert_eq!(back, expected);
    // An image written and read back through a frame buffer.
    let mut textures = [0_u32];
    gl.gen_textures(&mut textures);
    gl.bind_texture(gl::TEXTURE_2D, textures[0]);
    gl.tex_storage_2d(gl::TEXTURE_2D, 1, gl::RGBA8, SIDE, SIDE);
    let pixels: Vec<u8> = (0..IMAGE).map(|at| u8::try_from(at % 251).unwrap_or(0)).collect();
    gl.tex_sub_image_2d(gl::TEXTURE_2D, 0, 0, 0, SIDE, SIDE, gl::RGBA, gl::UNSIGNED_BYTE, PixelUnpack::Bytes(&pixels));
    let mut framebuffers = [0_u32];
    gl.gen_framebuffers(&mut framebuffers);
    gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffers[0]);
    gl.framebuffer_texture_2d(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, gl::TEXTURE_2D, textures[0], 0);
    assert_eq!(gl.check_framebuffer_status(gl::FRAMEBUFFER), gl::FRAMEBUFFER_COMPLETE);
    let mut read = vec![0_u8; IMAGE];
    gl.read_pixels(0, 0, SIDE, SIDE, gl::RGBA, gl::UNSIGNED_BYTE, PixelPack::Bytes(&mut read));
    assert_eq!(read, pixels);
    // A triangle over the whole pixel buffer in one colour.
    gl.bind_framebuffer(gl::FRAMEBUFFER, 0);
    let vertex = compiled(gl, gl::VERTEX_SHADER, "#version 300 es\nlayout(location = 0) in vec2 at;\nvoid main() { gl_Position = vec4(at, 0.0, 1.0); }\n");
    let fragment = compiled(
        gl,
        gl::FRAGMENT_SHADER,
        "#version 300 es\nprecision mediump float;\nuniform vec4 colour;\nout vec4 drawn;\nvoid main() { drawn = colour; }\n",
    );
    let program = gl.create_program();
    gl.attach_shader(program, vertex);
    gl.attach_shader(program, fragment);
    gl.link_program(program);
    let mut linked = [0_i32];
    gl.get_programiv(program, gl::LINK_STATUS, &mut linked);
    assert_eq!(linked[0], 1);
    gl.use_program(program);
    let colour = gl.get_uniform_location(program, c"colour");
    assert!(colour >= 0);
    gl.uniform4fv(colour, &[0.0, 1.0, 0.0, 1.0]);
    let mut arrays = [0_u32];
    gl.gen_vertex_arrays(&mut arrays);
    gl.bind_vertex_array(arrays[0]);
    gl.bind_buffer(gl::ARRAY_BUFFER, buffers[1]);
    let triangle: Vec<u8> = [-1.0_f32, -1.0, 3.0, -1.0, -1.0, 3.0].iter().flat_map(|value| value.to_ne_bytes()).collect();
    gl.buffer_data(gl::ARRAY_BUFFER, i64::try_from(triangle.len()).unwrap_or(0), Some(&triangle), gl::STATIC_DRAW);
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer(0, 2, gl::FLOAT, false, 0, 0);
    gl.viewport(0, 0, SIDE, SIDE);
    gl.draw_arrays(gl::TRIANGLES, 0, 3);
    let fence = gl.fence_sync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0).expect("a fence");
    let waited = gl.client_wait_sync(&fence, gl::SYNC_FLUSH_COMMANDS_BIT, SECOND);
    assert!(waited == gl::ALREADY_SIGNALED || waited == gl::CONDITION_SATISFIED, "{waited:#x}");
    let mut status = [0_i32];
    gl.get_synciv(&fence, gl::SYNC_STATUS, &mut status);
    assert_eq!(status[0].cast_unsigned(), gl::SIGNALED);
    gl.delete_sync(&fence);
    let mut drawn = vec![0_u8; IMAGE];
    gl.read_pixels(0, 0, SIDE, SIDE, gl::RGBA, gl::UNSIGNED_BYTE, PixelPack::Bytes(&mut drawn));
    assert!(drawn.chunks(PIXEL).all(|pixel| pixel == [0, u8::MAX, 0, u8::MAX]), "{drawn:?}");
    assert_eq!(gl.get_error(), gl::NO_ERROR);
    gl.delete_textures(&textures);
    gl.delete_buffers(&buffers);
    gl.delete_framebuffers(&framebuffers);
    gl.delete_vertex_arrays(&arrays);
    gl.delete_program(program);
    gl.delete_shader(vertex);
    gl.delete_shader(fragment);
    assert!(made.egl.make_current(&made.display, None, None, None));
    assert!(made.egl.destroy_context(&made.display, &made.context));
}

/// Asserts that the next error is the one the glue refused with, and that the refusal left nothing pending.
fn refused<G: OpenGlEs>(gl: &G, error: u32, what: &str) {
    assert_eq!(gl.get_error(), error, "{what}");
    assert_eq!(gl.get_error(), gl::NO_ERROR, "{what}: one error only");
}

/// Every check the glue makes that no registry attribute expresses refuses what would reach memory not lent.
pub fn refuses_what_would_reach_memory<E: Egl, G: OnEgl<E>>(folder: Option<&Path>) {
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    let gl = &made.gl;
    let mut textures = [0_u32];
    gl.gen_textures(&mut textures);
    gl.bind_texture(gl::TEXTURE_2D, textures[0]);
    gl.tex_storage_2d(gl::TEXTURE_2D, 1, gl::RGBA8, SIDE, SIDE);
    let short = vec![0_u8; IMAGE - 1];
    gl.tex_sub_image_2d(gl::TEXTURE_2D, 0, 0, 0, SIDE, SIDE, gl::RGBA, gl::UNSIGNED_BYTE, PixelUnpack::Bytes(&short));
    refused(gl, gl::INVALID_VALUE, "pixels one byte short");
    // A row length of twice the width: the last row starts 3 rows of 8 pixels in.
    gl.pixel_storei(gl::UNPACK_ROW_LENGTH, 2 * SIDE);
    let whole = vec![0_u8; IMAGE];
    gl.tex_sub_image_2d(gl::TEXTURE_2D, 0, 0, 0, SIDE, SIDE, gl::RGBA, gl::UNSIGNED_BYTE, PixelUnpack::Bytes(&whole));
    refused(gl, gl::INVALID_VALUE, "pixels short of the row length");
    let rows = vec![0_u8; (3 * 2 * SIDE as usize + SIDE as usize) * PIXEL];
    gl.tex_sub_image_2d(gl::TEXTURE_2D, 0, 0, 0, SIDE, SIDE, gl::RGBA, gl::UNSIGNED_BYTE, PixelUnpack::Bytes(&rows));
    assert_eq!(gl.get_error(), gl::NO_ERROR, "pixels exactly as long as the row length asks");
    gl.pixel_storei(gl::UNPACK_ROW_LENGTH, 0);
    gl.tex_sub_image_2d(gl::TEXTURE_2D, 0, 0, 0, SIDE, SIDE, gl::RGBA, 0x1234, PixelUnpack::Bytes(&whole));
    refused(gl, gl::INVALID_ENUM, "a pixel type the glue does not know");
    gl.tex_sub_image_2d(gl::TEXTURE_2D, 0, 0, 0, SIDE, SIDE, gl::RGBA, gl::UNSIGNED_BYTE, PixelUnpack::Offset(0));
    refused(gl, gl::INVALID_OPERATION, "an offset with no unpack buffer");
    gl.compressed_tex_sub_image_2d(gl::TEXTURE_2D, 0, 0, 0, SIDE, SIDE, gl::COMPRESSED_RGBA8_ETC2_EAC, CompressedPixelUnpack::Offset { offset: 0, size: 16 });
    refused(gl, gl::INVALID_OPERATION, "a compressed offset with no unpack buffer");
    let mut framebuffers = [0_u32];
    gl.gen_framebuffers(&mut framebuffers);
    gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffers[0]);
    gl.framebuffer_texture_2d(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, gl::TEXTURE_2D, textures[0], 0);
    let mut read = vec![0_u8; IMAGE - 1];
    gl.read_pixels(0, 0, SIDE, SIDE, gl::RGBA, gl::UNSIGNED_BYTE, PixelPack::Bytes(&mut read));
    refused(gl, gl::INVALID_VALUE, "a read one byte short");
    gl.pixel_storei(gl::PACK_ALIGNMENT, 8);
    let mut odd = vec![0_u8; 3 * 3 * 2];
    gl.read_pixels(0, 0, 3, 2, gl::RGB, gl::UNSIGNED_BYTE, PixelPack::Bytes(&mut odd));
    refused(gl, gl::INVALID_VALUE, "a read short of the pack alignment");
    gl.read_pixels(0, 0, SIDE, SIDE, gl::RGBA, gl::UNSIGNED_BYTE, PixelPack::Offset(0));
    refused(gl, gl::INVALID_OPERATION, "a read offset with no pack buffer");
    // Vertex data: only offsets into a bound buffer, and no draw with an enabled attribute reading none.
    gl.bind_buffer(gl::ARRAY_BUFFER, 0);
    gl.vertex_attrib_pointer(0, 2, gl::FLOAT, false, 0, 0);
    refused(gl, gl::INVALID_OPERATION, "a vertex offset with no array buffer");
    let mut buffers = [0_u32; 2];
    gl.gen_buffers(&mut buffers);
    gl.bind_buffer(gl::ARRAY_BUFFER, buffers[0]);
    gl.buffer_data(gl::ARRAY_BUFFER, 64, None, gl::STATIC_DRAW);
    gl.vertex_attrib_pointer(1, 2, gl::FLOAT, false, 0, 0x7000_0000);
    gl.enable_vertex_attrib_array(1);
    gl.delete_buffers(&buffers[..1]);
    gl.draw_arrays(gl::TRIANGLES, 0, 3);
    refused(gl, gl::INVALID_OPERATION, "a draw whose enabled attribute lost its buffer");
    gl.draw_arrays_instanced(gl::TRIANGLES, 0, 3, 2);
    refused(gl, gl::INVALID_OPERATION, "an instanced draw whose enabled attribute lost its buffer");
    gl.disable_vertex_attrib_array(1);
    gl.bind_buffer(gl::ELEMENT_ARRAY_BUFFER, 0);
    gl.draw_elements_instanced(gl::TRIANGLES, 3, gl::UNSIGNED_SHORT, 0, 1);
    refused(gl, gl::INVALID_OPERATION, "indices with no element array buffer");
    // Lengths: a vector count must be exact, a bounded read gets room for its most.
    gl.uniform4fv(0, &[0.0; 5]);
    refused(gl, gl::INVALID_VALUE, "five floats for vec4s");
    gl.clear_bufferfv(gl::COLOR, 0, &[0.0]);
    assert_eq!(gl.get_error(), gl::NO_ERROR, "a colour clear with one value reads the glue's four");
    let mut formats = [0_i32; 4];
    gl.get_integerv(gl::COMPRESSED_TEXTURE_FORMATS, &mut formats);
    refused(gl, gl::INVALID_ENUM, "a value whose length is another query's");
    let mut one = [0_i32; 1];
    gl.get_integerv(gl::VIEWPORT, &mut one);
    assert_eq!(gl.get_error(), gl::NO_ERROR, "four values into room for one: the glue's room takes them");
    // Buffers read back only as far as they reach.
    gl.bind_buffer(gl::ARRAY_BUFFER, buffers[1]);
    gl.buffer_data(gl::ARRAY_BUFFER, 16, None, gl::STATIC_DRAW);
    let mut past = vec![0_u8; 64];
    assert!(!gl.map_buffer_range_read(gl::ARRAY_BUFFER, 0, &mut past), "a read past the buffer's end");
    assert_eq!(gl.get_error(), gl::INVALID_VALUE);
    gl.buffer_data(gl::ARRAY_BUFFER, 64, Some(&[0; 8]), gl::STATIC_DRAW);
    refused(gl, gl::INVALID_VALUE, "fewer bytes than the size");
    // Fences: a deleted one, or another context's, is no fence.
    let fence = gl.fence_sync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0).expect("a fence");
    gl.delete_sync(&fence);
    let mut status = [0_i32];
    gl.get_synciv(&fence, gl::SYNC_STATUS, &mut status);
    refused(gl, gl::INVALID_VALUE, "a deleted fence");
    let other = make::<E, G>(folder, &CONTEXT_ES3);
    let others = other.gl.fence_sync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0).expect("the other context's fence");
    gl.get_synciv(&others, gl::SYNC_STATUS, &mut status);
    refused(gl, gl::INVALID_VALUE, "another context's fence");
    // EGL: an attribute key with no value, and a destroyed context.
    let mut configs = vec![None; 1];
    assert_eq!(made.egl.choose_config(&made.display, &[egl::RED_SIZE], &mut configs), None);
    assert_eq!(made.egl.get_error(), egl::BAD_ATTRIBUTE);
    assert!(made.egl.destroy_context(&made.display, &made.context));
    let _ = gl.get_error();
    gl.flush();
    assert_eq!(gl.get_error(), gl::CONTEXT_LOST, "a call after the context was destroyed");
    assert!(!made.egl.destroy_context(&made.display, &made.context));
    assert_eq!(made.egl.get_error(), egl::BAD_CONTEXT, "a destroyed context destroyed again");
    assert!(made.egl.make_current(&made.display, None, None, None));
}

/// A context asked without robust buffer access gets it: the glue forces it.
pub fn robust_access_is_forced<E: Egl, G: OnEgl<E>>(folder: Option<&Path>) {
    let made = make::<E, G>(folder, &[egl::CONTEXT_MAJOR_VERSION, 3, egl::CONTEXT_OPENGL_ROBUST_ACCESS, egl::FALSE, egl::NONE]);
    let mut robust = [0_i32];
    made.gl.get_integerv(gl::CONTEXT_ROBUST_ACCESS_EXT, &mut robust);
    assert_eq!(robust[0], 1, "the context reads buffers robustly");
    assert_eq!(made.gl.get_error(), gl::NO_ERROR);
}

/// Calls every method of the trait, `rounds` times each with arguments picked by the round; nothing may end the
/// process, and the context still answers after.
pub fn every_method_is_called<E: Egl, G: OnEgl<E>>(folder: Option<&Path>, rounds: usize) {
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    let gl = &made.gl;
    let fence = gl.fence_sync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0).expect("a fence");
    for round in 0..rounds {
        for which in 0..METHODS {
            call_one(gl, &fence, which, round.wrapping_mul(METHODS).wrapping_add(which));
        }
    }
    while gl.get_error() != gl::NO_ERROR {}
    assert!(gl.get_string(gl::VERSION).is_some_and(|version| version.starts_with(b"OpenGL ES 3")));
}

#[macro_export]
macro_rules! tests {
    ($egl:ty, $gl:ty, $folder:expr) => {
        #[test]
        fn draws_and_reads_back() {
            $crate::draws_and_reads_back::<$egl, $gl>($folder.as_deref());
        }

        #[test]
        fn refuses_what_would_reach_memory() {
            $crate::refuses_what_would_reach_memory::<$egl, $gl>($folder.as_deref());
        }

        #[test]
        fn robust_access_is_forced() {
            $crate::robust_access_is_forced::<$egl, $gl>($folder.as_deref());
        }

        #[test]
        fn every_method_is_called() {
            $crate::every_method_is_called::<$egl, $gl>($folder.as_deref(), 8);
        }
    };
}
