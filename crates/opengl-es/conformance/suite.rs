// Written by tools/api-crate-generator from Khronos's gl.xml (and ANGLE's gl_angle_ext.xml for ANGLE's extensions), pinned in src/khronos/write.rs. Do not edit.
//! The tests every implementation of wcontainers-opengl-es over wcontainers-egl runs, written against the traits
//! alone: a context made, drawn with and read back; every check of the implementation's own, each shown refused by
//! the implementation itself (not the library), with every buffer a heap allocation of exactly the length named or a
//! part of a longer one whose rest the call must not reach; every method called; a query's result read as soon as it
//! ended. Every test that needs only OpenGL ES has a form taking contexts however they were made (`*_on`, run by name
//! with `run_on_any_context`), for an implementation with no EGL (a browser's WebGL 2).

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

/// `make` for a library that may lie: no answer is checked, and None when a context cannot be had.
fn reach<E: Egl, G: OnEgl<E>>(library: &E) -> Option<(E::Display, E::Context, G)> {
    let display = library.get_display()?;
    library.initialize(&display)?;
    let _ = library.bind_api(egl::OPENGL_ES_API.cast_unsigned());
    let mut configs = vec![None; 8];
    library.choose_config(&display, &[egl::SURFACE_TYPE, egl::PBUFFER_BIT, egl::RENDERABLE_TYPE, egl::OPENGL_ES3_BIT, egl::NONE], &mut configs)?;
    let config = configs.into_iter().flatten().next()?;
    let context = library.create_context(&display, &config, None, &CONTEXT_ES3)?;
    let surface = library.create_pbuffer_surface(&display, &config, &[egl::WIDTH, SIDE, egl::HEIGHT, SIDE, egl::NONE])?;
    let gl = G::new(library, &display, &context, Some(&surface), Some(&surface))?;
    Some((display, context, gl))
}

/// `reach`, tried until it gives a context (a lying library refuses some of the calls at random), with the library
/// loaded once.
fn reached<E: Egl, G: OnEgl<E>>(folder: Option<&Path>) -> Made<E, G> {
    let library = (0..1000).find_map(|_| E::load(folder)).expect("EGL loads");
    let (display, context, gl) = (0..1000).find_map(|_| reach::<E, G>(&library)).expect("a context within 1000 tries");
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
    draws_and_reads_back_on(&made.gl);
    assert!(made.egl.make_current(&made.display, None, None, None));
    assert!(made.egl.destroy_context(&made.display, &made.context));
}

/// `draws_and_reads_back` on a context made by any means, whose default frame buffer is SIDE by SIDE pixels.
pub fn draws_and_reads_back_on<G: OpenGlEs>(gl: &G) {
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
}

/// Asserts that the next error is the one the glue refused with, and that the refusal left nothing pending.
fn refused<G: OpenGlEs>(gl: &G, error: u32, what: &str) {
    assert_eq!(gl.get_error(), error, "{what}");
    assert_eq!(gl.get_error(), gl::NO_ERROR, "{what}: one error only");
}

/// How a test tells the implementation's own refusals from the library's errors: the number of calls each refused
/// itself so far (the OpenGL ES context's, and EGL's on the calling thread). A check removed from the implementation
/// leaves the count where it was, even where the library refuses the call with the same error.
pub struct Counts<E, G> {
    pub gl: fn(&G) -> u64,
    pub egl: fn(&E) -> u64,
}

/// The count of the calls an implementation refused itself (`Counts::gl`), or none for one whose checks another
/// makes (a browser checks every WebGL call): then a refusal is any error, with nothing else pending.
pub type Refusals<G> = Option<fn(&G) -> u64>;

/// Asserts that the call was refused by the implementation itself, with the error, and that nothing else is pending;
/// with no count, that it was refused with some error.
fn glue_refuses<G: OpenGlEs>(gl: &G, refusals: Refusals<G>, error: u32, what: &str, call: impl FnOnce()) {
    let Some(count) = refusals else {
        call();
        assert_ne!(gl.get_error(), gl::NO_ERROR, "{what}: refused");
        assert_eq!(gl.get_error(), gl::NO_ERROR, "{what}: one error only");
        return;
    };
    let before = count(gl);
    call();
    assert_eq!(count(gl), before + 1, "{what}: refused by the implementation itself");
    refused(gl, error, what);
}

/// Asserts that the call passed the implementation's checks and the library took it without an error.
fn glue_passes<G: OpenGlEs>(gl: &G, refusals: Refusals<G>, what: &str, call: impl FnOnce()) {
    let before = refusals.map(|count| count(gl));
    call();
    assert_eq!(refusals.map(|count| count(gl)), before, "{what}: not refused");
    assert_eq!(gl.get_error(), gl::NO_ERROR, "{what}: no error");
}

/// Asserts that the EGL call was refused by the implementation itself, with the error.
fn egl_refuses<E: Egl, G>(egl: &E, counts: &Counts<E, G>, error: i32, what: &str, call: impl FnOnce() -> bool) {
    let before = (counts.egl)(egl);
    let succeeded = call();
    assert!(!succeeded, "{what}: refused");
    assert_eq!((counts.egl)(egl), before + 1, "{what}: refused by the implementation itself");
    assert_eq!(egl.get_error(), error, "{what}");
}

/// The values in a heap allocation of exactly their length: a call that reads or writes past it reaches memory the
/// allocator guards (AddressSanitizer reports it), or the values a test placed after a part it gives.
fn heap<T: Clone>(values: &[T]) -> Vec<T> {
    values.to_vec()
}

/// A value no call of these tests writes, kept past the end of what a call is given: a call that wrote or read past
/// its slice changes it, or reads it back.
const LEFT: i32 = 0x5EED_5EED;
/// `LEFT` as an unsigned value.
const LEFT_WORD: u32 = 0x5EED_5EED;
/// A clear colour's first channel, given alone; the three after it in memory are `BEYOND`.
const GIVEN: f32 = 0.25;
/// What follows the one value a clear is given: read only by a call that reads past its slice.
const BEYOND: f32 = 0.75;
/// An integer clear's first channel, given alone; `LEFT` follows it.
const GIVEN_INTEGER: i32 = 5;
/// A value no format, type or attachment is: the library refuses it, so a call that reads it shows.
const NO_VALUE: u32 = 0x1234;

/// Pixel transfers read and write only the caller's bytes, as many as the pixel store state makes the transfer, and
/// only with no pixel buffer bound; an offset only with one bound; a format or type the implementation does not know
/// is refused. Every buffer given is a heap allocation of exactly the length named.
pub fn refuses_pixels_past_what_was_given<E: Egl, G: OnEgl<E>>(folder: Option<&Path>, counts: &Counts<E, G>) {
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    refuses_pixels_past_what_was_given_on(&made.gl, Some(counts.gl));
}

/// `refuses_pixels_past_what_was_given` on a context made by any means.
pub fn refuses_pixels_past_what_was_given_on<G: OpenGlEs>(gl: &G, refusals: Refusals<G>) {
    let side = usize::try_from(SIDE).unwrap_or(0);
    let row = side * PIXEL;
    let mut textures = [0_u32; 2];
    gl.gen_textures(&mut textures);
    gl.bind_texture(gl::TEXTURE_2D, textures[0]);
    gl.tex_storage_2d(gl::TEXTURE_2D, 1, gl::RGBA8, SIDE, SIDE);
    gl.bind_texture(gl::TEXTURE_2D_ARRAY, textures[1]);
    gl.tex_storage_3d(gl::TEXTURE_2D_ARRAY, 1, gl::RGBA8, SIDE, SIDE, 2);
    let unpack = |bytes: usize| vec![0_u8; bytes];
    let two = |pixels: &[u8]| gl.tex_sub_image_2d(gl::TEXTURE_2D, 0, 0, 0, SIDE, SIDE, gl::RGBA, gl::UNSIGNED_BYTE, PixelUnpack::Bytes(pixels));
    let three = |pixels: &[u8]| gl.tex_sub_image_3d(gl::TEXTURE_2D_ARRAY, 0, 0, 0, 0, SIDE, SIDE, 2, gl::RGBA, gl::UNSIGNED_BYTE, PixelUnpack::Bytes(pixels));
    glue_refuses(gl, refusals, gl::INVALID_VALUE, "pixels one byte short", || two(&unpack(IMAGE - 1)));
    glue_passes(gl, refusals, "pixels exactly as long", || two(&unpack(IMAGE)));
    // Each pixel store value moves the end of what is read; the caller's bytes must reach it. Skipped pixels are
    // given rows one pixel longer than the width: WebGL 2 refuses pixels skipped past a row's end (§5.35), which OpenGL
    // ES reads from the next row.
    let stores: [(&[(u32, i32)], usize); 3] = [
        // Rows of twice the width: the last row starts 3 rows of 8 pixels in.
        (&[(gl::UNPACK_ROW_LENGTH, 2 * SIDE)], (3 * 2 * side + side) * PIXEL),
        (&[(gl::UNPACK_SKIP_ROWS, 1)], IMAGE + row),
        // Rows of 5 pixels, one skipped: the last row's pixels start 3 rows and one pixel in.
        (&[(gl::UNPACK_ROW_LENGTH, SIDE + 1), (gl::UNPACK_SKIP_PIXELS, 1)], (3 * (side + 1) + 1 + side) * PIXEL),
    ];
    for (values, needed) in stores {
        for &(name, value) in values {
            gl.pixel_storei(name, value);
        }
        let name = values.last().map_or(0, |(name, _)| *name);
        glue_refuses(gl, refusals, gl::INVALID_VALUE, &format!("pixels short of the store value {name:#x}"), || two(&unpack(needed - 1)));
        glue_passes(gl, refusals, &format!("pixels as long as the store value {name:#x} asks"), || two(&unpack(needed)));
        for &(name, _) in values {
            gl.pixel_storei(name, 0);
        }
    }
    // Rows of 3 pixels aligned to 8 bytes: 16 bytes apart, the last 12 long.
    gl.pixel_storei(gl::UNPACK_ALIGNMENT, 8);
    let aligned = |pixels: &[u8]| gl.tex_sub_image_2d(gl::TEXTURE_2D, 0, 0, 0, 3, 2, gl::RGBA, gl::UNSIGNED_BYTE, PixelUnpack::Bytes(pixels));
    glue_refuses(gl, refusals, gl::INVALID_VALUE, "pixels short of the unpack alignment", || aligned(&unpack(3 * 2 * PIXEL)));
    glue_passes(gl, refusals, "pixels as long as the unpack alignment asks", || aligned(&unpack(16 + 3 * PIXEL)));
    gl.pixel_storei(gl::UNPACK_ALIGNMENT, 4);
    // Images: two of them, then each taller by the image height, then one skipped.
    glue_refuses(gl, refusals, gl::INVALID_VALUE, "one image for two", || three(&unpack(IMAGE)));
    glue_passes(gl, refusals, "two images", || three(&unpack(2 * IMAGE)));
    for (name, value) in [(gl::UNPACK_IMAGE_HEIGHT, 2 * SIDE), (gl::UNPACK_SKIP_IMAGES, 1)] {
        gl.pixel_storei(name, value);
        glue_refuses(gl, refusals, gl::INVALID_VALUE, &format!("two images short of the store value {name:#x}"), || three(&unpack(2 * IMAGE)));
        glue_passes(gl, refusals, &format!("images as long as the store value {name:#x} asks"), || three(&unpack(3 * IMAGE)));
        gl.pixel_storei(name, 0);
    }
    let whole = unpack(IMAGE);
    glue_refuses(gl, refusals, gl::INVALID_ENUM, "a pixel type the implementation does not know", || {
        gl.tex_sub_image_2d(gl::TEXTURE_2D, 0, 0, 0, SIDE, SIDE, gl::RGBA, NO_VALUE, PixelUnpack::Bytes(&whole));
    });
    glue_refuses(gl, refusals, gl::INVALID_ENUM, "a pixel format the implementation does not know", || {
        gl.tex_sub_image_2d(gl::TEXTURE_2D, 0, 0, 0, SIDE, SIDE, NO_VALUE, gl::UNSIGNED_BYTE, PixelUnpack::Bytes(&whole));
    });
    // Offsets need an unpack buffer; bytes need none.
    let offset = u64::try_from(IMAGE).unwrap_or(0);
    let compressed = CompressedPixelUnpack::Offset { offset, size: 16 };
    glue_refuses(gl, refusals, gl::INVALID_OPERATION, "an offset with no unpack buffer", || {
        gl.tex_sub_image_2d(gl::TEXTURE_2D, 0, 0, 0, SIDE, SIDE, gl::RGBA, gl::UNSIGNED_BYTE, PixelUnpack::Offset(offset));
    });
    glue_refuses(gl, refusals, gl::INVALID_OPERATION, "an image offset with no unpack buffer", || {
        gl.tex_sub_image_3d(gl::TEXTURE_2D_ARRAY, 0, 0, 0, 0, SIDE, SIDE, 1, gl::RGBA, gl::UNSIGNED_BYTE, PixelUnpack::Offset(offset));
    });
    glue_refuses(gl, refusals, gl::INVALID_OPERATION, "a compressed offset with no unpack buffer", || {
        gl.compressed_tex_sub_image_2d(gl::TEXTURE_2D, 0, 0, 0, SIDE, SIDE, gl::COMPRESSED_RGBA8_ETC2_EAC, compressed);
    });
    glue_refuses(gl, refusals, gl::INVALID_OPERATION, "a compressed image offset with no unpack buffer", || {
        gl.compressed_tex_sub_image_3d(gl::TEXTURE_2D_ARRAY, 0, 0, 0, 0, SIDE, SIDE, 1, gl::COMPRESSED_RGBA8_ETC2_EAC, compressed);
    });
    let mut buffers = [0_u32; 2];
    gl.gen_buffers(&mut buffers);
    gl.bind_buffer(gl::PIXEL_UNPACK_BUFFER, buffers[0]);
    gl.buffer_data(gl::PIXEL_UNPACK_BUFFER, i64::try_from(4 * IMAGE).unwrap_or(0), None, gl::STREAM_READ);
    let bytes = unpack(16);
    glue_refuses(gl, refusals, gl::INVALID_OPERATION, "bytes with an unpack buffer bound", || two(&whole));
    glue_refuses(gl, refusals, gl::INVALID_OPERATION, "image bytes with an unpack buffer bound", || three(&unpack(2 * IMAGE)));
    glue_refuses(gl, refusals, gl::INVALID_OPERATION, "compressed bytes with an unpack buffer bound", || {
        gl.compressed_tex_sub_image_2d(gl::TEXTURE_2D, 0, 0, 0, SIDE, SIDE, gl::COMPRESSED_RGBA8_ETC2_EAC, CompressedPixelUnpack::Bytes(&bytes));
    });
    glue_refuses(gl, refusals, gl::INVALID_OPERATION, "compressed image bytes with an unpack buffer bound", || {
        gl.compressed_tex_sub_image_3d(gl::TEXTURE_2D_ARRAY, 0, 0, 0, 0, SIDE, SIDE, 1, gl::COMPRESSED_RGBA8_ETC2_EAC, CompressedPixelUnpack::Bytes(&bytes));
    });
    glue_passes(gl, refusals, "an offset into the unpack buffer bound", || {
        gl.tex_sub_image_2d(gl::TEXTURE_2D, 0, 0, 0, SIDE, SIDE, gl::RGBA, gl::UNSIGNED_BYTE, PixelUnpack::Offset(0));
    });
    gl.bind_buffer(gl::PIXEL_UNPACK_BUFFER, 0);
    // Reads back: the same rules for the pack state and buffer.
    let mut framebuffers = [0_u32];
    gl.gen_framebuffers(&mut framebuffers);
    gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffers[0]);
    gl.framebuffer_texture_2d(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, gl::TEXTURE_2D, textures[0], 0);
    let read = |bytes: &mut [u8]| gl.read_pixels(0, 0, SIDE, SIDE, gl::RGBA, gl::UNSIGNED_BYTE, PixelPack::Bytes(bytes));
    glue_refuses(gl, refusals, gl::INVALID_VALUE, "a read one byte short", || read(&mut unpack(IMAGE - 1)));
    let mut kept = vec![LEFT_WORD.to_ne_bytes()[0]; IMAGE + row];
    glue_passes(gl, refusals, "a read exactly as long", || read(&mut kept[..IMAGE]));
    assert!(kept[IMAGE..].iter().all(|&byte| byte == LEFT_WORD.to_ne_bytes()[0]), "a read wrote past its bytes");
    let stores: [(u32, i32); 3] = [(gl::PACK_ROW_LENGTH, 2 * SIDE), (gl::PACK_SKIP_ROWS, 1), (gl::PACK_SKIP_PIXELS, 1)];
    for (name, value) in stores {
        gl.pixel_storei(name, value);
        glue_refuses(gl, refusals, gl::INVALID_VALUE, &format!("a read short of the store value {name:#x}"), || read(&mut unpack(IMAGE)));
        gl.pixel_storei(name, 0);
    }
    gl.pixel_storei(gl::PACK_ALIGNMENT, 8);
    glue_refuses(gl, refusals, gl::INVALID_VALUE, "a read short of the pack alignment", || {
        gl.read_pixels(0, 0, 3, 2, gl::RGBA, gl::UNSIGNED_BYTE, PixelPack::Bytes(&mut unpack(3 * 2 * PIXEL)));
    });
    gl.pixel_storei(gl::PACK_ALIGNMENT, 4);
    glue_refuses(gl, refusals, gl::INVALID_ENUM, "a read of a type the implementation does not know", || {
        gl.read_pixels(0, 0, SIDE, SIDE, gl::RGBA, NO_VALUE, PixelPack::Bytes(&mut unpack(IMAGE)));
    });
    glue_refuses(gl, refusals, gl::INVALID_OPERATION, "a read offset with no pack buffer", || {
        gl.read_pixels(0, 0, SIDE, SIDE, gl::RGBA, gl::UNSIGNED_BYTE, PixelPack::Offset(offset));
    });
    gl.bind_buffer(gl::PIXEL_PACK_BUFFER, buffers[1]);
    gl.buffer_data(gl::PIXEL_PACK_BUFFER, i64::try_from(IMAGE).unwrap_or(0), None, gl::STREAM_READ);
    glue_refuses(gl, refusals, gl::INVALID_OPERATION, "read bytes with a pack buffer bound", || read(&mut unpack(IMAGE)));
    glue_passes(gl, refusals, "a read into the pack buffer bound", || {
        gl.read_pixels(0, 0, SIDE, SIDE, gl::RGBA, gl::UNSIGNED_BYTE, PixelPack::Offset(0));
    });
    gl.bind_buffer(gl::PIXEL_PACK_BUFFER, 0);
}

/// Vertex data comes only from buffers: an offset with no array or element array buffer bound is refused, and so is
/// every draw while an enabled attribute has no buffer (its offset would be read as an address).
pub fn refuses_vertex_reads_without_a_buffer<E: Egl, G: OnEgl<E>>(folder: Option<&Path>, counts: &Counts<E, G>) {
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    refuses_vertex_reads_without_a_buffer_on(&made.gl, Some(counts.gl));
}

/// `refuses_vertex_reads_without_a_buffer` on a context made by any means.
pub fn refuses_vertex_reads_without_a_buffer_on<G: OpenGlEs>(gl: &G, refusals: Refusals<G>) {
    let mut most = [0_i32];
    gl.get_integerv(gl::MAX_VERTEX_ATTRIBS, &mut most);
    // The last attribute, so that a check of fewer attributes than the context has shows.
    let last = u32::try_from(most[0] - 1).expect("at least one vertex attribute");
    gl.bind_buffer(gl::ARRAY_BUFFER, 0);
    glue_refuses(gl, refusals, gl::INVALID_OPERATION, "a vertex offset with no array buffer", || gl.vertex_attrib_pointer(last, 2, gl::FLOAT, false, 0, 0));
    glue_refuses(gl, refusals, gl::INVALID_OPERATION, "an integer vertex offset with no array buffer", || gl.vertex_attrib_i_pointer(last, 2, gl::INT, 0, 0));
    let mut buffers = [0_u32; 2];
    gl.gen_buffers(&mut buffers);
    gl.bind_buffer(gl::ARRAY_BUFFER, buffers[0]);
    gl.buffer_data(gl::ARRAY_BUFFER, 64, None, gl::STATIC_DRAW);
    glue_passes(gl, refusals, "a vertex offset into the array buffer", || gl.vertex_attrib_pointer(last, 2, gl::FLOAT, false, 0, 0x7000_0000));
    gl.enable_vertex_attrib_array(last);
    gl.bind_buffer(gl::ELEMENT_ARRAY_BUFFER, buffers[1]);
    gl.buffer_data(gl::ELEMENT_ARRAY_BUFFER, 64, None, gl::STATIC_DRAW);
    // Deleting the bound buffer leaves the attribute reading client memory at its offset.
    gl.delete_buffers(&buffers[..1]);
    glue_refuses(gl, refusals, gl::INVALID_OPERATION, "a draw whose enabled attribute lost its buffer", || gl.draw_arrays(gl::TRIANGLES, 0, 3));
    glue_refuses(gl, refusals, gl::INVALID_OPERATION, "an instanced draw whose enabled attribute lost its buffer", || {
        gl.draw_arrays_instanced(gl::TRIANGLES, 0, 3, 2);
    });
    glue_refuses(gl, refusals, gl::INVALID_OPERATION, "an indexed draw whose enabled attribute lost its buffer", || {
        gl.draw_elements_instanced(gl::TRIANGLES, 3, gl::UNSIGNED_SHORT, 0, 1);
    });
    gl.disable_vertex_attrib_array(last);
    gl.bind_buffer(gl::ELEMENT_ARRAY_BUFFER, 0);
    glue_refuses(gl, refusals, gl::INVALID_OPERATION, "indices with no element array buffer", || {
        gl.draw_elements_instanced(gl::TRIANGLES, 3, gl::UNSIGNED_SHORT, 0x7000_0000, 1);
    });
}

/// A fragment shader with one uniform of every type a `uniform*v` call sets, each used.
const EVERY_UNIFORM: &str = "#version 300 es
precision highp float;
precision highp int;
uniform float f1; uniform vec2 f2; uniform vec3 f3; uniform vec4 f4;
uniform int i1; uniform ivec2 i2; uniform ivec3 i3; uniform ivec4 i4;
uniform uint u1; uniform uvec2 u2; uniform uvec3 u3; uniform uvec4 u4;
uniform mat2 m2; uniform mat2x3 m23; uniform mat2x4 m24; uniform mat3 m3; uniform mat3x2 m32; uniform mat3x4 m34;
uniform mat4 m4; uniform mat4x2 m42; uniform mat4x3 m43;
out vec4 drawn;
void main() {
    float sum = f1 + f2.x + f3.x + f4.x + float(i1 + i2.x + i3.x + i4.x) + float(u1 + u2.x + u3.x + u4.x);
    sum += m2[0][0] + m23[0][0] + m24[0][0] + m3[0][0] + m32[0][0] + m34[0][0] + m4[0][0] + m42[0][0] + m43[0][0];
    drawn = vec4(sum);
}
";

/// Every `uniform*v` call's count is its slice's length over the values of one element: exact lengths pass (one
/// element of a uniform that is not an array; a count of more would be the library's error), a length that does not
/// divide is refused.
pub fn counts_are_the_slices<E: Egl, G: OnEgl<E>>(folder: Option<&Path>, counts: &Counts<E, G>) {
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    counts_are_the_slices_on(&made.gl, Some(counts.gl));
}

/// `counts_are_the_slices` on a context made by any means.
pub fn counts_are_the_slices_on<G: OpenGlEs>(gl: &G, refusals: Refusals<G>) {
    let vertex = compiled(gl, gl::VERTEX_SHADER, "#version 300 es\nvoid main() { gl_Position = vec4(0.0); }\n");
    let fragment = compiled(gl, gl::FRAGMENT_SHADER, EVERY_UNIFORM);
    let program = gl.create_program();
    gl.attach_shader(program, vertex);
    gl.attach_shader(program, fragment);
    gl.link_program(program);
    gl.use_program(program);
    let at = |name: &core::ffi::CStr| {
        let location = gl.get_uniform_location(program, name);
        assert!(location >= 0, "{name:?} is active");
        location
    };
    type Floats<G> = fn(&G, i32, &[f32]);
    type Matrices<G> = fn(&G, i32, bool, &[f32]);
    let floats: [(&core::ffi::CStr, usize, Floats<G>); 4] =
        [(c"f1", 1, G::uniform1fv), (c"f2", 2, G::uniform2fv), (c"f3", 3, G::uniform3fv), (c"f4", 4, G::uniform4fv)];
    for (name, by, call) in floats {
        let location = at(name);
        glue_passes(gl, refusals, &format!("{name:?} given one element"), || call(gl, location, &vec![0.0; by]));
        if by > 1 {
            glue_refuses(gl, refusals, gl::INVALID_VALUE, &format!("{name:?} given a length that does not divide"), || call(gl, location, &vec![0.0; by + 1]));
        }
    }
    let integers: [(&core::ffi::CStr, usize, fn(&G, i32, &[i32])); 4] =
        [(c"i1", 1, G::uniform1iv), (c"i2", 2, G::uniform2iv), (c"i3", 3, G::uniform3iv), (c"i4", 4, G::uniform4iv)];
    for (name, by, call) in integers {
        let location = at(name);
        glue_passes(gl, refusals, &format!("{name:?} given one element"), || call(gl, location, &vec![0; by]));
        if by > 1 {
            glue_refuses(gl, refusals, gl::INVALID_VALUE, &format!("{name:?} given a length that does not divide"), || call(gl, location, &vec![0; by + 1]));
        }
    }
    let words: [(&core::ffi::CStr, usize, fn(&G, i32, &[u32])); 4] =
        [(c"u1", 1, G::uniform1uiv), (c"u2", 2, G::uniform2uiv), (c"u3", 3, G::uniform3uiv), (c"u4", 4, G::uniform4uiv)];
    for (name, by, call) in words {
        let location = at(name);
        glue_passes(gl, refusals, &format!("{name:?} given one element"), || call(gl, location, &vec![0; by]));
        if by > 1 {
            glue_refuses(gl, refusals, gl::INVALID_VALUE, &format!("{name:?} given a length that does not divide"), || call(gl, location, &vec![0; by + 1]));
        }
    }
    let matrices: [(&core::ffi::CStr, usize, Matrices<G>); 9] = [
        (c"m2", 4, G::uniform_matrix2fv),
        (c"m23", 6, G::uniform_matrix2x3fv),
        (c"m24", 8, G::uniform_matrix2x4fv),
        (c"m3", 9, G::uniform_matrix3fv),
        (c"m32", 6, G::uniform_matrix3x2fv),
        (c"m34", 12, G::uniform_matrix3x4fv),
        (c"m4", 16, G::uniform_matrix4fv),
        (c"m42", 8, G::uniform_matrix4x2fv),
        (c"m43", 12, G::uniform_matrix4x3fv),
    ];
    for (name, by, call) in matrices {
        let location = at(name);
        glue_passes(gl, refusals, &format!("{name:?} given one element"), || call(gl, location, false, &vec![0.0; by]));
        glue_refuses(gl, refusals, gl::INVALID_VALUE, &format!("{name:?} given a length that does not divide"), || call(gl, location, false, &vec![0.0; by + 1]));
    }
}

/// Calls that write a count of values write only as many as the slice holds: each is given an empty or short part of
/// a longer allocation, and what follows it must be left as it was. Calls that read a count read only as many: the
/// value after the part given is one the library would refuse, or an object the call would delete.
pub fn counts_reach_only_the_slices<E: Egl, G: OnEgl<E>>(folder: Option<&Path>, counts: &Counts<E, G>) {
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    counts_reach_only_the_slices_on(&made.gl, Some(counts.gl));
}

/// `counts_reach_only_the_slices` on a context made by any means.
pub fn counts_reach_only_the_slices_on<G: OpenGlEs>(gl: &G, refusals: Refusals<G>) {
    type Names<G> = fn(&G, &mut [u32]);
    let makers: [(&str, Names<G>); 7] = [
        ("gen_buffers", G::gen_buffers),
        ("gen_framebuffers", G::gen_framebuffers),
        ("gen_queries", G::gen_queries),
        ("gen_renderbuffers", G::gen_renderbuffers),
        ("gen_samplers", G::gen_samplers),
        ("gen_textures", G::gen_textures),
        ("gen_vertex_arrays", G::gen_vertex_arrays),
    ];
    for (what, call) in makers {
        let mut names = heap(&[LEFT_WORD; 2]);
        glue_passes(gl, refusals, what, || call(gl, &mut names[..1]));
        assert_ne!(names[0], LEFT_WORD, "{what} wrote its one name");
        assert_eq!(names[1], LEFT_WORD, "{what} wrote past its slice");
    }
    // Info logs: a shader that does not compile, and a program that does not link, each have a long one.
    let shader = gl.create_shader(gl::FRAGMENT_SHADER);
    gl.shader_source(shader, &[b"not a shader".as_slice()]);
    gl.compile_shader(shader);
    let program = gl.create_program();
    gl.attach_shader(program, shader);
    gl.link_program(program);
    let mut log = heap(&[LEFT_WORD.to_ne_bytes()[0]; 64]);
    glue_passes(gl, refusals, "get_shader_info_log", || {
        let _ = gl.get_shader_info_log(shader, &mut log[..PIXEL]);
    });
    assert!(log[PIXEL..].iter().all(|&byte| byte == LEFT_WORD.to_ne_bytes()[0]), "get_shader_info_log wrote past its slice");
    glue_passes(gl, refusals, "get_program_info_log", || {
        let _ = gl.get_program_info_log(program, &mut log[..PIXEL]);
    });
    assert!(log[PIXEL..].iter().all(|&byte| byte == LEFT_WORD.to_ne_bytes()[0]), "get_program_info_log wrote past its slice");
    let mut values = heap(&[LEFT; 4]);
    glue_passes(gl, refusals, "get_internalformativ", || gl.get_internalformativ(gl::RENDERBUFFER, gl::RGBA8, gl::SAMPLES, &mut values[..0]));
    assert!(values.iter().all(|&value| value == LEFT), "get_internalformativ wrote past its slice");
    let fence = gl.fence_sync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0).expect("a fence");
    glue_passes(gl, refusals, "get_synciv", || {
        let _ = gl.get_synciv(&fence, gl::SYNC_STATUS, &mut values[..0]);
    });
    assert!(values.iter().all(|&value| value == LEFT), "get_synciv wrote past its slice");
    gl.delete_sync(&fence);
    // Deletions: the second object is bound, and only the first is given.
    type Bind<G> = fn(&G, u32);
    type Delete<G> = fn(&G, &[u32]);
    let objects: [(&str, Names<G>, Bind<G>, u32, Delete<G>); 6] = [
        ("delete_buffers", G::gen_buffers, |gl, name| gl.bind_buffer(gl::ARRAY_BUFFER, name), gl::ARRAY_BUFFER_BINDING, G::delete_buffers),
        ("delete_textures", G::gen_textures, |gl, name| gl.bind_texture(gl::TEXTURE_2D, name), gl::TEXTURE_BINDING_2D, G::delete_textures),
        ("delete_framebuffers", G::gen_framebuffers, |gl, name| gl.bind_framebuffer(gl::FRAMEBUFFER, name), gl::FRAMEBUFFER_BINDING, G::delete_framebuffers),
        ("delete_renderbuffers", G::gen_renderbuffers, |gl, name| gl.bind_renderbuffer(gl::RENDERBUFFER, name), gl::RENDERBUFFER_BINDING, G::delete_renderbuffers),
        ("delete_samplers", G::gen_samplers, |gl, name| gl.bind_sampler(0, name), gl::SAMPLER_BINDING, G::delete_samplers),
        ("delete_vertex_arrays", G::gen_vertex_arrays, |gl, name| gl.bind_vertex_array(name), gl::VERTEX_ARRAY_BINDING, G::delete_vertex_arrays),
    ];
    for (what, make_names, bind, binding, delete) in objects {
        let mut names = vec![0_u32; 2];
        make_names(gl, &mut names);
        bind(gl, names[1]);
        glue_passes(gl, refusals, what, || delete(gl, &names[..1]));
        let mut bound = [0_i32];
        gl.get_integerv(binding, &mut bound);
        assert_eq!(bound[0].cast_unsigned(), names[1], "{what} deleted past its slice");
        bind(gl, 0);
        delete(gl, &names[1..]);
    }
    let mut queries = vec![0_u32; 2];
    gl.gen_queries(&mut queries);
    gl.begin_query(gl::ANY_SAMPLES_PASSED, queries[1]);
    glue_passes(gl, refusals, "delete_queries", || gl.delete_queries(&queries[..1]));
    glue_passes(gl, refusals, "the query not given still ends", || gl.end_query(gl::ANY_SAMPLES_PASSED));
    // Lists of values: the one after the part given is a value the library refuses.
    let mut textures = [0_u32];
    gl.gen_textures(&mut textures);
    gl.bind_texture(gl::TEXTURE_2D, textures[0]);
    gl.tex_storage_2d(gl::TEXTURE_2D, 1, gl::RGBA8, SIDE, SIDE);
    let mut framebuffers = [0_u32];
    gl.gen_framebuffers(&mut framebuffers);
    gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffers[0]);
    gl.framebuffer_texture_2d(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, gl::TEXTURE_2D, textures[0], 0);
    // Names of outputs to capture: only the one given.
    let captured = compiled(gl, gl::VERTEX_SHADER, "#version 300 es\nout float captured;\nvoid main() { captured = 1.0; gl_Position = vec4(0.0); }\n");
    let coloured = compiled(gl, gl::FRAGMENT_SHADER, "#version 300 es\nprecision mediump float;\nout vec4 drawn;\nvoid main() { drawn = vec4(1.0); }\n");
    let capturing = gl.create_program();
    gl.attach_shader(capturing, captured);
    gl.attach_shader(capturing, coloured);
    glue_passes(gl, refusals, "transform_feedback_varyings", || gl.transform_feedback_varyings(capturing, &[c"captured"], gl::INTERLEAVED_ATTRIBS));
    gl.link_program(capturing);
    let mut linked = [0_i32];
    gl.get_programiv(capturing, gl::LINK_STATUS, &mut linked);
    assert_eq!(linked[0], 1, "transform_feedback_varyings read past its names");
    let attachments = heap(&[gl::COLOR_ATTACHMENT0, NO_VALUE]);
    glue_passes(gl, refusals, "draw_buffers", || gl.draw_buffers(&attachments[..1]));
    glue_passes(gl, refusals, "invalidate_framebuffer", || gl.invalidate_framebuffer(gl::FRAMEBUFFER, &attachments[..1]));
}

/// Calls whose output the glue takes into room of its own copy out only as many values as the caller's slice holds,
/// and calls whose input it copies into room of its own read only that: a clear given one channel clears with that
/// channel and zeros, never with the values after it in the caller's memory.
pub fn rooms_keep_calls_inside<E: Egl, G: OnEgl<E>>(folder: Option<&Path>, counts: &Counts<E, G>) {
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    rooms_keep_calls_inside_on(&made.gl, Some(counts.gl));
}

/// `rooms_keep_calls_inside` on a context made by any means.
pub fn rooms_keep_calls_inside_on<G: OpenGlEs>(gl: &G, refusals: Refusals<G>) {
    let mut integers = vec![LEFT; 8];
    glue_passes(gl, refusals, "get_integerv into nothing", || gl.get_integerv(gl::VIEWPORT, &mut integers[..0]));
    assert!(integers.iter().all(|&value| value == LEFT), "get_integerv wrote past its slice");
    glue_passes(gl, refusals, "get_integerv into one", || gl.get_integerv(gl::VIEWPORT, &mut integers[..1]));
    assert_eq!(integers[0], 0, "the viewport's x");
    assert!(integers[1..].iter().all(|&value| value == LEFT), "get_integerv wrote past its slice");
    let mut floats = vec![-1.0_f32; 8];
    glue_passes(gl, refusals, "get_floatv", || gl.get_floatv(gl::VIEWPORT, &mut floats[..0]));
    assert!(floats.iter().all(|&value| value < 0.0), "get_floatv wrote past its slice");
    let mut wide = vec![i64::from(LEFT); 8];
    glue_passes(gl, refusals, "get_integer64v", || gl.get_integer64v(gl::VIEWPORT, &mut wide[..0]));
    assert!(wide.iter().all(|&value| value == i64::from(LEFT)), "get_integer64v wrote past its slice");
    // The colour write mask is four trues.
    let mut flags = vec![false; 8];
    glue_passes(gl, refusals, "get_booleanv", || gl.get_booleanv(gl::COLOR_WRITEMASK, &mut flags[..0]));
    assert!(flags.iter().all(|&flag| !flag), "get_booleanv wrote past its slice");
    for name in [gl::COMPRESSED_TEXTURE_FORMATS, gl::PROGRAM_BINARY_FORMATS, gl::SHADER_BINARY_FORMATS] {
        glue_refuses(gl, refusals, gl::INVALID_ENUM, &format!("get_integerv of the list {name:#x}"), || gl.get_integerv(name, &mut integers));
        glue_refuses(gl, refusals, gl::INVALID_ENUM, &format!("get_floatv of the list {name:#x}"), || gl.get_floatv(name, &mut floats));
        glue_refuses(gl, refusals, gl::INVALID_ENUM, &format!("get_integer64v of the list {name:#x}"), || gl.get_integer64v(name, &mut wide));
        glue_refuses(gl, refusals, gl::INVALID_ENUM, &format!("get_booleanv of the list {name:#x}"), || gl.get_booleanv(name, &mut flags));
    }
    integers.fill(LEFT);
    glue_passes(gl, refusals, "get_vertex_attribiv", || gl.get_vertex_attribiv(0, gl::CURRENT_VERTEX_ATTRIB, &mut integers[..0]));
    let program = gl.create_program();
    glue_passes(gl, refusals, "get_programiv", || gl.get_programiv(program, gl::LINK_STATUS, &mut integers[..0]));
    let shader = gl.create_shader(gl::VERTEX_SHADER);
    glue_passes(gl, refusals, "get_shaderiv", || gl.get_shaderiv(shader, gl::SHADER_TYPE, &mut integers[..0]));
    assert!(integers.iter().all(|&value| value == LEFT), "a get wrote past its slice");
    let mut queries = [0_u32];
    gl.gen_queries(&mut queries);
    gl.begin_query(gl::ANY_SAMPLES_PASSED, queries[0]);
    if gl.get_queryiv_ext(gl::ANY_SAMPLES_PASSED, gl::CURRENT_QUERY, &mut integers[..0]).is_some() {
        assert_eq!(gl.get_error(), gl::NO_ERROR);
    }
    gl.end_query(gl::ANY_SAMPLES_PASSED);
    gl.finish();
    let mut words = heap(&[LEFT_WORD; 8]);
    glue_passes(gl, refusals, "get_query_objectuiv", || gl.get_query_objectuiv(queries[0], gl::QUERY_RESULT_AVAILABLE, &mut words[..0]));
    assert!(words.iter().all(|&value| value == LEFT_WORD), "get_query_objectuiv wrote past its slice");
    let mut results = heap(&[u64::from(LEFT_WORD); 8]);
    let _ = gl.get_query_objectui64v_ext(queries[0], gl::QUERY_RESULT, &mut results[..0]);
    while gl.get_error() != gl::NO_ERROR {}
    assert!(results.iter().all(|&value| value == u64::from(LEFT_WORD)), "get_query_objectui64v_ext wrote past its slice");
    assert!(integers.iter().all(|&value| value == LEFT), "get_queryiv_ext wrote past its slice");
    // Clears: one channel given, followed in memory by values the clear must not read.
    let mut textures = [0_u32; 3];
    gl.gen_textures(&mut textures);
    let mut framebuffers = [0_u32];
    gl.gen_framebuffers(&mut framebuffers);
    gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffers[0]);
    for (texture, format) in textures.iter().zip([gl::RGBA8, gl::RGBA32I, gl::RGBA32UI]) {
        gl.bind_texture(gl::TEXTURE_2D, *texture);
        gl.tex_storage_2d(gl::TEXTURE_2D, 1, format, SIDE, SIDE);
    }
    gl.framebuffer_texture_2d(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, gl::TEXTURE_2D, textures[0], 0);
    let colour = heap(&[GIVEN, BEYOND, BEYOND, BEYOND]);
    glue_passes(gl, refusals, "clear_bufferfv given one channel", || gl.clear_bufferfv(gl::COLOR, 0, &colour[..1]));
    let mut pixel = vec![0_u8; PIXEL];
    gl.read_pixels(0, 0, 1, 1, gl::RGBA, gl::UNSIGNED_BYTE, PixelPack::Bytes(&mut pixel));
    assert!((63..=64).contains(&pixel[0]) && pixel[1..] == [0, 0, 0], "clear_bufferfv read past its one channel: {pixel:?}");
    glue_passes(gl, refusals, "clear_bufferfv given exactly one channel", || gl.clear_bufferfv(gl::COLOR, 0, &heap(&[GIVEN])));
    let mut channels = vec![0_u8; PIXEL * PIXEL];
    gl.framebuffer_texture_2d(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, gl::TEXTURE_2D, textures[1], 0);
    let signed = heap(&[GIVEN_INTEGER, LEFT, LEFT, LEFT]);
    glue_passes(gl, refusals, "clear_bufferiv given one channel", || gl.clear_bufferiv(gl::COLOR, 0, &signed[..1]));
    gl.read_pixels(0, 0, 1, 1, gl::RGBA_INTEGER, gl::INT, PixelPack::Bytes(&mut channels));
    let read: Vec<i32> = channels.chunks(PIXEL).map(|bytes| i32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])).collect();
    assert_eq!(read, [GIVEN_INTEGER, 0, 0, 0], "clear_bufferiv read past its one channel");
    glue_passes(gl, refusals, "clear_bufferiv given exactly one channel", || gl.clear_bufferiv(gl::COLOR, 0, &heap(&[GIVEN_INTEGER])));
    gl.framebuffer_texture_2d(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, gl::TEXTURE_2D, textures[2], 0);
    let unsigned = heap(&[GIVEN_INTEGER.cast_unsigned(), LEFT_WORD, LEFT_WORD, LEFT_WORD]);
    glue_passes(gl, refusals, "clear_bufferuiv given one channel", || gl.clear_bufferuiv(gl::COLOR, 0, &unsigned[..1]));
    gl.read_pixels(0, 0, 1, 1, gl::RGBA_INTEGER, gl::UNSIGNED_INT, PixelPack::Bytes(&mut channels));
    let read: Vec<u32> = channels.chunks(PIXEL).map(|bytes| u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])).collect();
    assert_eq!(read, [GIVEN_INTEGER.cast_unsigned(), 0, 0, 0], "clear_bufferuiv read past its one channel");
    glue_passes(gl, refusals, "clear_bufferuiv given exactly one channel", || gl.clear_bufferuiv(gl::COLOR, 0, &heap(&[GIVEN_INTEGER.cast_unsigned()])));
    // A buffer's data is read only as far as the bytes given; fewer than the size is refused.
    let mut buffers = [0_u32];
    gl.gen_buffers(&mut buffers);
    gl.bind_buffer(gl::ARRAY_BUFFER, buffers[0]);
    let data = heap(&[1_u8; 64]);
    glue_refuses(gl, refusals, gl::INVALID_VALUE, "fewer bytes than the size", || gl.buffer_data(gl::ARRAY_BUFFER, 64, Some(&data[..8]), gl::STATIC_DRAW));
    glue_passes(gl, refusals, "as many bytes as the size", || gl.buffer_data(gl::ARRAY_BUFFER, 8, Some(&data[..8]), gl::STATIC_DRAW));
    let mut past = vec![0_u8; 64];
    assert!(!gl.map_buffer_range_read(gl::ARRAY_BUFFER, 0, &mut past), "a read past the buffer's end");
    assert_eq!(gl.get_error(), gl::INVALID_VALUE);
}

/// A fence is used only by the context that made it, and only until it is deleted: each is refused by the
/// implementation itself, also while this context holds a live fence another context's would be mistaken for.
pub fn fences_are_the_contexts_own<E: Egl, G: OnEgl<E>>(folder: Option<&Path>, counts: &Counts<E, G>) {
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    let other = make::<E, G>(folder, &CONTEXT_ES3);
    fences_are_the_contexts_own_on(&made.gl, &other.gl, Some(counts.gl));
}

/// `fences_are_the_contexts_own` on two contexts made by any means.
pub fn fences_are_the_contexts_own_on<G: OpenGlEs>(gl: &G, other: &G, refusals: Refusals<G>) {
    let live = gl.fence_sync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0).expect("a fence");
    let deleted = gl.fence_sync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0).expect("a second fence");
    glue_passes(gl, refusals, "a fence deleted", || gl.delete_sync(&deleted));
    let mut status = vec![0_i32; 1];
    glue_refuses(gl, refusals, gl::INVALID_VALUE, "get_synciv of a deleted fence", || {
        let _ = gl.get_synciv(&deleted, gl::SYNC_STATUS, &mut status);
    });
    glue_refuses(gl, refusals, gl::INVALID_VALUE, "client_wait_sync of a deleted fence", || {
        let _ = gl.client_wait_sync(&deleted, 0, 0);
    });
    glue_refuses(gl, refusals, gl::INVALID_VALUE, "delete_sync of a deleted fence", || gl.delete_sync(&deleted));
    // The other context's first fence: this context's first is live.
    let others = other.fence_sync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0).expect("the other context's fence");
    glue_refuses(gl, refusals, gl::INVALID_VALUE, "get_synciv of another context's fence", || {
        let _ = gl.get_synciv(&others, gl::SYNC_STATUS, &mut status);
    });
    glue_refuses(gl, refusals, gl::INVALID_VALUE, "client_wait_sync of another context's fence", || {
        let _ = gl.client_wait_sync(&others, 0, 0);
    });
    glue_refuses(gl, refusals, gl::INVALID_VALUE, "delete_sync of another context's fence", || gl.delete_sync(&others));
    glue_passes(gl, refusals, "this context's live fence", || {
        let _ = gl.get_synciv(&live, gl::SYNC_STATUS, &mut status);
    });
    other.delete_sync(&others);
    gl.delete_sync(&live);
}

/// EGL's handles are used only with the library that made them and only until destroyed, attribute lists are read only
/// as far as the slice and up to their first key EGL_NONE, and a context's calls after it is destroyed are refused.
pub fn egl_handles_are_the_librarys_own<E: Egl, G: OnEgl<E>>(folder: Option<&Path>, counts: &Counts<E, G>) {
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    let (egl, display) = (&made.egl, &made.display);
    let mut configs = vec![None; 256];
    let list = vec![egl::RED_SIZE];
    egl_refuses(egl, counts, egl::BAD_ATTRIBUTE, "a key with no value", || egl.choose_config(display, &list, &mut configs).is_some());
    let followed = heap(&[egl::RED_SIZE, 8, egl::NONE]);
    egl_refuses(egl, counts, egl::BAD_ATTRIBUTE, "a key whose value lies past the slice", || egl.choose_config(display, &followed[..1], &mut configs).is_some());
    let wanted = vec![egl::SURFACE_TYPE, egl::PBUFFER_BIT, egl::RENDERABLE_TYPE, egl::OPENGL_ES3_BIT, egl::NONE];
    let found = egl.choose_config(display, &wanted, &mut configs).expect("configurations");
    let found = usize::try_from(found).unwrap_or(0);
    assert!(found > 0 && found < configs.len(), "{found} configurations into room for {}", configs.len());
    assert!(configs[found..].iter().all(Option::is_none), "only the configurations written are handles");
    let config = configs[0].clone().expect("a configuration");
    // A list with no EGL_NONE, followed in memory by a pair EGL would refuse: it ends at the slice.
    let unended = heap(&[egl::WIDTH, SIDE, egl::HEIGHT, SIDE, egl::WIDTH, -1, egl::NONE]);
    let surface = egl.create_pbuffer_surface(display, &config, &unended[..4]).expect("a list ended at the slice");
    // Another library's handles: the same EGL loaded again.
    let second = E::load(folder).expect("EGL loads again");
    let second_display = second.get_display().expect("its display");
    assert!(second.initialize(&second_display).is_some());
    let mut second_configs = vec![None; 1];
    assert!(second.choose_config(&second_display, &wanted, &mut second_configs).is_some_and(|found| found > 0));
    let second_config = second_configs[0].clone().expect("its configuration");
    let second_context = second.create_context(&second_display, &second_config, None, &CONTEXT_ES3).expect("its context");
    let second_surface = second.create_pbuffer_surface(&second_display, &second_config, &[egl::WIDTH, 1, egl::HEIGHT, 1, egl::NONE]).expect("its surface");
    let other = &second_display;
    let refuse = |what: &str, call: &dyn Fn() -> bool| egl_refuses(egl, counts, egl::BAD_DISPLAY, what, call);
    refuse("choose_config on another library's display", &|| egl.choose_config(other, &wanted, &mut vec![None; 1]).is_some());
    refuse("create_context on another library's display", &|| egl.create_context(other, &config, None, &CONTEXT_ES3).is_some());
    refuse("create_pbuffer_surface on another library's display", &|| egl.create_pbuffer_surface(other, &config, &[egl::NONE]).is_some());
    refuse("destroy_context on another library's display", &|| egl.destroy_context(other, &made.context));
    refuse("destroy_surface on another library's display", &|| egl.destroy_surface(other, &surface));
    refuse("get_config_attrib on another library's display", &|| egl.get_config_attrib(other, &config, egl::RED_SIZE).is_some());
    refuse("initialize on another library's display", &|| egl.initialize(other).is_some());
    refuse("make_current on another library's display", &|| egl.make_current(other, None, None, None));
    refuse("query_string on another library's display", &|| egl.query_string(Some(other), egl::VENDOR).is_some());
    refuse("query_surface on another library's display", &|| egl.query_surface(other, &surface, egl::WIDTH).is_some());
    refuse("swap_buffers on another library's display", &|| egl.swap_buffers(other, &surface));
    refuse("swap_interval on another library's display", &|| egl.swap_interval(other, 0));
    let refuse = |error: i32, what: &str, call: &dyn Fn() -> bool| egl_refuses(egl, counts, error, what, call);
    refuse(egl::BAD_CONFIG, "create_context of another library's configuration", &|| egl.create_context(display, &second_config, None, &CONTEXT_ES3).is_some());
    refuse(egl::BAD_CONFIG, "create_pbuffer_surface of another library's configuration", &|| {
        egl.create_pbuffer_surface(display, &second_config, &[egl::NONE]).is_some()
    });
    refuse(egl::BAD_CONFIG, "get_config_attrib of another library's configuration", &|| egl.get_config_attrib(display, &second_config, egl::RED_SIZE).is_some());
    refuse(egl::BAD_CONTEXT, "create_context sharing another library's context", &|| egl.create_context(display, &config, Some(&second_context), &CONTEXT_ES3).is_some());
    refuse(egl::BAD_CONTEXT, "make_current of another library's context", &|| egl.make_current(display, Some(&surface), Some(&surface), Some(&second_context)));
    refuse(egl::BAD_SURFACE, "make_current drawing to another library's surface", &|| egl.make_current(display, Some(&second_surface), Some(&surface), Some(&made.context)));
    refuse(egl::BAD_SURFACE, "make_current reading another library's surface", &|| egl.make_current(display, Some(&surface), Some(&second_surface), Some(&made.context)));
    refuse(egl::BAD_SURFACE, "query_surface of another library's surface", &|| egl.query_surface(display, &second_surface, egl::WIDTH).is_some());
    refuse(egl::BAD_SURFACE, "swap_buffers of another library's surface", &|| egl.swap_buffers(display, &second_surface));
    refuse(egl::BAD_SURFACE, "destroy_surface of another library's surface", &|| egl.destroy_surface(display, &second_surface));
    refuse(egl::BAD_CONTEXT, "destroy_context of another library's context", &|| egl.destroy_context(display, &second_context));
    // Destroyed handles.
    assert!(egl.destroy_surface(display, &surface));
    refuse(egl::BAD_SURFACE, "query_surface of a destroyed surface", &|| egl.query_surface(display, &surface, egl::WIDTH).is_some());
    refuse(egl::BAD_SURFACE, "swap_buffers of a destroyed surface", &|| egl.swap_buffers(display, &surface));
    refuse(egl::BAD_SURFACE, "make_current drawing to a destroyed surface", &|| egl.make_current(display, Some(&surface), None, Some(&made.context)));
    refuse(egl::BAD_SURFACE, "destroy_surface of a destroyed surface", &|| egl.destroy_surface(display, &surface));
    let gl = &made.gl;
    // Destroyed while current on this thread: EGL keeps it until released, the implementation refuses it at once.
    let _ = gl.get_error();
    assert!(egl.destroy_context(display, &made.context));
    glue_refuses(gl, Some(counts.gl), gl::CONTEXT_LOST, "a call after the context current on the thread was destroyed", || gl.flush());
    assert!(egl.make_current(display, None, None, None));
    refuse(egl::BAD_CONTEXT, "destroy_context of a destroyed context", &|| egl.destroy_context(display, &made.context));
    refuse(egl::BAD_CONTEXT, "make_current of a destroyed context", &|| egl.make_current(display, None, None, Some(&made.context)));
    refuse(egl::BAD_CONTEXT, "create_context sharing a destroyed context", &|| egl.create_context(display, &config, Some(&made.context), &CONTEXT_ES3).is_some());
    let _ = gl.get_error();
    glue_refuses(gl, Some(counts.gl), gl::CONTEXT_LOST, "a call after the context was destroyed", || gl.flush());
    assert!(second.make_current(&second_display, None, None, None));
    assert!(second.destroy_surface(&second_display, &second_surface));
    assert!(second.destroy_context(&second_display, &second_context));
}

/// A context asked without robust buffer access gets it: the glue forces it.
pub fn robust_access_is_forced<E: Egl, G: OnEgl<E>>(folder: Option<&Path>) {
    let made = make::<E, G>(folder, &[egl::CONTEXT_MAJOR_VERSION, 3, egl::CONTEXT_OPENGL_ROBUST_ACCESS, egl::FALSE, egl::NONE]);
    let mut robust = [0_i32];
    made.gl.get_integerv(gl::CONTEXT_ROBUST_ACCESS_EXT, &mut robust);
    assert_eq!(robust[0], 1, "the context reads buffers robustly");
    assert_eq!(made.gl.get_error(), gl::NO_ERROR);
}

/// An extension's command answers None unless the context names the extension.
pub fn extensions_answer_only_when_named<E: Egl, G: OnEgl<E>>(folder: Option<&Path>) {
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    extensions_answer_only_when_named_on(&made.gl);
}

/// `extensions_answer_only_when_named` on a context made by any means.
pub fn extensions_answer_only_when_named_on<G: OpenGlEs>(gl: &G) {
    let named = named_extensions(gl);
    let names_any = |wanted: &[&str]| wanted.iter().any(|name| names(&named, name));
    extension_calls(gl, &names_any);
    while gl.get_error() != gl::NO_ERROR {}
}

/// A program drawing in one colour, and a vertex array of one triangle at `at` (x, y of its first corner; the
/// triangle at -1, -1 covers the whole SIDE by SIDE pixel buffer).
fn triangle<G: OpenGlEs>(gl: &G, at: [f32; 2]) -> [u32; 5] {
    let vertex = compiled(gl, gl::VERTEX_SHADER, "#version 300 es\nlayout(location = 0) in vec2 at;\nvoid main() { gl_Position = vec4(at, 0.0, 1.0); }\n");
    let fragment = compiled(gl, gl::FRAGMENT_SHADER, "#version 300 es\nprecision mediump float;\nout vec4 drawn;\nvoid main() { drawn = vec4(1.0); }\n");
    let program = gl.create_program();
    gl.attach_shader(program, vertex);
    gl.attach_shader(program, fragment);
    gl.link_program(program);
    gl.use_program(program);
    let mut arrays = [0_u32];
    gl.gen_vertex_arrays(&mut arrays);
    gl.bind_vertex_array(arrays[0]);
    let mut buffers = [0_u32];
    gl.gen_buffers(&mut buffers);
    gl.bind_buffer(gl::ARRAY_BUFFER, buffers[0]);
    let [x, y] = at;
    let corners: Vec<u8> = [x, y, x + 4.0, y, x, y + 4.0].iter().flat_map(|value| value.to_ne_bytes()).collect();
    gl.buffer_data(gl::ARRAY_BUFFER, i64::try_from(corners.len()).unwrap_or(0), Some(&corners), gl::STATIC_DRAW);
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer(0, 2, gl::FLOAT, false, 0, 0);
    gl.viewport(0, 0, SIDE, SIDE);
    [program, vertex, fragment, arrays[0], buffers[0]]
}

/// A query's result read as soon as the query ended is its result, as OpenGL ES's QUERY_RESULT waits for it: an
/// occlusion query over a triangle drawn on the pixel buffer passed samples, one over a triangle drawn outside it
/// none.
pub fn query_results_wait<E: Egl, G: OnEgl<E>>(folder: Option<&Path>) {
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    query_results_wait_on(&made.gl);
}

/// `query_results_wait` on a context made by any means, whose default frame buffer is SIDE by SIDE pixels.
pub fn query_results_wait_on<G: OpenGlEs>(gl: &G) {
    let mut queries = [0_u32; 2];
    gl.gen_queries(&mut queries);
    let mut made = Vec::new();
    for (query, at) in queries.iter().zip([[-1.0, -1.0], [8.0, 8.0]]) {
        let objects = triangle(gl, at);
        gl.begin_query(gl::ANY_SAMPLES_PASSED, *query);
        gl.draw_arrays(gl::TRIANGLES, 0, 3);
        gl.end_query(gl::ANY_SAMPLES_PASSED);
        made.push(objects);
    }
    let read = |query: u32| {
        let mut result = [LEFT_WORD];
        gl.get_query_objectuiv(query, gl::QUERY_RESULT, &mut result);
        result[0]
    };
    assert_eq!(read(queries[0]), 1, "the triangle on the pixel buffer passed samples");
    assert_eq!(read(queries[1]), 0, "the triangle outside it passed none");
    assert_eq!(gl.get_error(), gl::NO_ERROR);
    gl.delete_queries(&queries);
    for [program, vertex, fragment, array, buffer] in made {
        gl.delete_vertex_arrays(&[array]);
        gl.delete_buffers(&[buffer]);
        gl.delete_program(program);
        gl.delete_shader(vertex);
        gl.delete_shader(fragment);
    }
}

/// The tests a context made by any means takes (each `*_on` above), by name.
pub const ON_ANY_CONTEXT: [&str; 12] = [
    "draws_and_reads_back",
    "refuses_pixels_past_what_was_given",
    "refuses_vertex_reads_without_a_buffer",
    "counts_are_the_slices",
    "counts_reach_only_the_slices",
    "rooms_keep_calls_inside",
    "fences_are_the_contexts_own",
    "extensions_answer_only_when_named",
    "stencil_faces_are_separate",
    "extension_values_are_taken",
    "query_results_wait",
    "every_method_is_called",
];

/// Runs the test of `ON_ANY_CONTEXT` named on contexts `make` makes, each test on new ones; `refusals` counts the
/// implementation's own refusals, or none for one whose checks another makes. False for a name not there.
pub fn run_on_any_context<G: OpenGlEs>(name: &str, make: &dyn Fn() -> G, refusals: Refusals<G>) -> bool {
    match name {
        "draws_and_reads_back" => draws_and_reads_back_on(&make()),
        "refuses_pixels_past_what_was_given" => refuses_pixels_past_what_was_given_on(&make(), refusals),
        "refuses_vertex_reads_without_a_buffer" => refuses_vertex_reads_without_a_buffer_on(&make(), refusals),
        "counts_are_the_slices" => counts_are_the_slices_on(&make(), refusals),
        "counts_reach_only_the_slices" => counts_reach_only_the_slices_on(&make(), refusals),
        "rooms_keep_calls_inside" => rooms_keep_calls_inside_on(&make(), refusals),
        "fences_are_the_contexts_own" => fences_are_the_contexts_own_on(&make(), &make(), refusals),
        "extensions_answer_only_when_named" => extensions_answer_only_when_named_on(&make()),
        "stencil_faces_are_separate" => stencil_faces_are_separate_on(&make(), refusals),
        "extension_values_are_taken" => extension_values_are_taken_on(&make(), refusals),
        "query_results_wait" => query_results_wait_on(&make()),
        "every_method_is_called" => every_method_is_called_on(&make(), EVERY_METHOD_ROUNDS),
        _ => return false,
    }
    true
}

/// The rounds `every_method_is_called` takes in the suite.
pub const EVERY_METHOD_ROUNDS: usize = 8;

/// The extensions WebGL gives under names of its own (the WebGL extension registry), by their OpenGL ES name.
const WEBGL_NAMES: [(&str, &str); 5] = [
    ("GL_ANGLE_polygon_mode", "WEBGL_polygon_mode"),
    ("GL_ANGLE_provoking_vertex", "WEBGL_provoking_vertex"),
    ("GL_EXT_disjoint_timer_query", "EXT_disjoint_timer_query_webgl2"),
    ("GL_EXT_clip_cull_distance", "WEBGL_clip_cull_distance"),
    ("GL_EXT_blend_func_extended", "WEBGL_blend_func_extended"),
];

/// The extensions the context names (get_stringi of EXTENSIONS).
fn named_extensions<G: OpenGlEs>(gl: &G) -> Vec<Vec<u8>> {
    let mut count = [0_i32];
    gl.get_integerv(gl::NUM_EXTENSIONS, &mut count);
    (0..count[0].cast_unsigned()).filter_map(|index| gl.get_stringi(gl::EXTENSIONS, index)).collect()
}

/// Whether the names hold the OpenGL ES extension `wanted`, by its own name or, as a WebGL context names it, by its
/// name less "GL_" or its WebGL name.
fn names(named: &[Vec<u8>], wanted: &str) -> bool {
    let webgl = WEBGL_NAMES.iter().find(|(name, _)| *name == wanted).map_or_else(|| wanted.trim_start_matches("GL_"), |(_, webgl)| *webgl);
    named.iter().any(|named| named.as_slice() == wanted.as_bytes() || named.as_slice() == webgl.as_bytes())
}

/// The front face's stencil reference value.
const FRONT_REFERENCE: i32 = 3;
/// The back face's stencil reference value.
const BACK_REFERENCE: i32 = 5;
/// The front face's stencil value mask.
const FRONT_MASK: u32 = 0xF0;
/// The back face's stencil value mask.
const BACK_MASK: u32 = 0x0F;

/// The front and back faces' stencil compare and operations are set apart (glStencilFuncSeparate,
/// glStencilOpSeparate), each face reads back as it was set, and neither call is refused by the implementation.
pub fn stencil_faces_are_separate<E: Egl, G: OnEgl<E>>(folder: Option<&Path>, counts: &Counts<E, G>) {
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    stencil_faces_are_separate_on(&made.gl, Some(counts.gl));
}

/// `stencil_faces_are_separate` on a context made by any means.
pub fn stencil_faces_are_separate_on<G: OpenGlEs>(gl: &G, refusals: Refusals<G>) {
    // A reference value reads back clamped to the stencil bits of the frame buffer drawn to: one with 8 of them.
    let mut renderbuffers = [0_u32];
    gl.gen_renderbuffers(&mut renderbuffers);
    gl.bind_renderbuffer(gl::RENDERBUFFER, renderbuffers[0]);
    gl.renderbuffer_storage(gl::RENDERBUFFER, gl::DEPTH24_STENCIL8, SIDE, SIDE);
    let mut framebuffers = [0_u32];
    gl.gen_framebuffers(&mut framebuffers);
    gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffers[0]);
    gl.framebuffer_renderbuffer(gl::FRAMEBUFFER, gl::DEPTH_STENCIL_ATTACHMENT, gl::RENDERBUFFER, renderbuffers[0]);
    let mut bits = [0_i32];
    gl.get_integerv(gl::STENCIL_BITS, &mut bits);
    assert_eq!(bits[0], 8, "the frame buffer's stencil bits");
    glue_passes(gl, refusals, "the front face's compare", || gl.stencil_func_separate(gl::FRONT, gl::LESS, FRONT_REFERENCE, FRONT_MASK));
    glue_passes(gl, refusals, "the back face's compare", || gl.stencil_func_separate(gl::BACK, gl::GEQUAL, BACK_REFERENCE, BACK_MASK));
    glue_passes(gl, refusals, "the front face's operations", || gl.stencil_op_separate(gl::FRONT, gl::KEEP, gl::INCR, gl::DECR));
    glue_passes(gl, refusals, "the back face's operations", || gl.stencil_op_separate(gl::BACK, gl::INVERT, gl::REPLACE, gl::ZERO));
    let read = |name: u32| {
        let mut value = [LEFT];
        gl.get_integerv(name, &mut value);
        value[0]
    };
    let expected = [
        (gl::STENCIL_FUNC, gl::LESS.cast_signed()),
        (gl::STENCIL_REF, FRONT_REFERENCE),
        (gl::STENCIL_VALUE_MASK, FRONT_MASK.cast_signed()),
        (gl::STENCIL_BACK_FUNC, gl::GEQUAL.cast_signed()),
        (gl::STENCIL_BACK_REF, BACK_REFERENCE),
        (gl::STENCIL_BACK_VALUE_MASK, BACK_MASK.cast_signed()),
        (gl::STENCIL_FAIL, gl::KEEP.cast_signed()),
        (gl::STENCIL_PASS_DEPTH_FAIL, gl::INCR.cast_signed()),
        (gl::STENCIL_PASS_DEPTH_PASS, gl::DECR.cast_signed()),
        (gl::STENCIL_BACK_FAIL, gl::INVERT.cast_signed()),
        (gl::STENCIL_BACK_PASS_DEPTH_FAIL, gl::REPLACE.cast_signed()),
        (gl::STENCIL_BACK_PASS_DEPTH_PASS, gl::ZERO.cast_signed()),
    ];
    for (name, value) in expected {
        assert_eq!(read(name), value, "stencil state {name:#x}");
    }
    assert_eq!(gl.get_error(), gl::NO_ERROR);
    gl.bind_framebuffer(gl::FRAMEBUFFER, 0);
    gl.delete_framebuffers(&framebuffers);
    gl.delete_renderbuffers(&renderbuffers);
}

/// The values the four extensions of the WebGL back end add are the registry's: where the context names an
/// extension, the call taking its value passes the implementation and the library takes it (a wrong number is
/// INVALID_ENUM). An extension the context does not name is printed and skipped.
pub fn extension_values_are_taken<E: Egl, G: OnEgl<E>>(folder: Option<&Path>, counts: &Counts<E, G>) {
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    extension_values_are_taken_on(&made.gl, Some(counts.gl));
}

/// `extension_values_are_taken` on a context made by any means.
pub fn extension_values_are_taken_on<G: OpenGlEs>(gl: &G, refusals: Refusals<G>) {
    let named = named_extensions(gl);
    let names = |wanted: &str| {
        let found = names(&named, wanted);
        if !found {
            println!("NOT NAMED {wanted}: its values are not tried");
        }
        found
    };
    let mut samplers = [0_u32];
    gl.gen_samplers(&mut samplers);
    if names("GL_EXT_clip_cull_distance") {
        // OpenGL ES asks an enabled clip distance with glIsEnabled only, which the set does not carry: taken by both
        // calls without an error is what a wrong number (INVALID_ENUM) fails.
        glue_passes(gl, refusals, "enable CLIP_DISTANCE0_EXT", || gl.enable(gl::CLIP_DISTANCE0_EXT));
        glue_passes(gl, refusals, "disable CLIP_DISTANCE0_EXT", || gl.disable(gl::CLIP_DISTANCE0_EXT));
    }
    if names("GL_EXT_texture_filter_anisotropic") {
        let mut most = [0.0_f32];
        glue_passes(gl, refusals, "MAX_TEXTURE_MAX_ANISOTROPY_EXT", || gl.get_floatv(gl::MAX_TEXTURE_MAX_ANISOTROPY_EXT, &mut most));
        assert!(most[0] >= 2.0, "the extension promises at least 2, read {}", most[0]);
        glue_passes(gl, refusals, "TEXTURE_MAX_ANISOTROPY_EXT", || gl.sampler_parameterf(samplers[0], gl::TEXTURE_MAX_ANISOTROPY_EXT, 2.0));
    }
    if names("GL_EXT_texture_mirror_clamp_to_edge") {
        glue_passes(gl, refusals, "MIRROR_CLAMP_TO_EDGE_EXT", || {
            gl.sampler_parameteri(samplers[0], gl::TEXTURE_WRAP_S, gl::MIRROR_CLAMP_TO_EDGE_EXT.cast_signed());
        });
    }
    if names("GL_EXT_blend_func_extended") {
        glue_passes(gl, refusals, "the second source's factors", || {
            gl.blend_func_separate(gl::SRC1_COLOR_EXT, gl::ONE_MINUS_SRC1_COLOR_EXT, gl::SRC1_ALPHA_EXT, gl::ONE_MINUS_SRC1_ALPHA_EXT);
        });
        let read = |name: u32| {
            let mut value = [LEFT];
            gl.get_integerv(name, &mut value);
            value[0].cast_unsigned()
        };
        assert_eq!(read(gl::BLEND_SRC_RGB), gl::SRC1_COLOR_EXT);
        assert_eq!(read(gl::BLEND_DST_RGB), gl::ONE_MINUS_SRC1_COLOR_EXT);
        assert_eq!(read(gl::BLEND_SRC_ALPHA), gl::SRC1_ALPHA_EXT);
        assert_eq!(read(gl::BLEND_DST_ALPHA), gl::ONE_MINUS_SRC1_ALPHA_EXT);
    }
    gl.delete_samplers(&samplers);
    assert_eq!(gl.get_error(), gl::NO_ERROR);
}

/// Calls every method of the trait, `rounds` times each with arguments picked by the round; nothing may end the
/// process, and the context still answers after.
pub fn every_method_is_called<E: Egl, G: OnEgl<E>>(folder: Option<&Path>, rounds: usize) {
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    every_method_is_called_on(&made.gl, rounds);
}

/// `every_method_is_called` on a context made by any means.
pub fn every_method_is_called_on<G: OpenGlEs>(gl: &G, rounds: usize) {
    let fence = gl.fence_sync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0).expect("a fence");
    for round in 0..rounds {
        for which in 0..METHODS {
            call_one(gl, &fence, which, round.wrapping_mul(METHODS).wrapping_add(which));
        }
    }
    while gl.get_error() != gl::NO_ERROR {}
    assert!(gl.get_string(gl::VERSION).is_some_and(|version| version.starts_with(b"OpenGL ES 3")));
}

/// The next value of a seeded sequence (xorshift): the fuzzing's only source of choice, so a seed replays a run.
fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// One of the values, picked by the sequence.
fn chosen<T: Clone>(values: &[T], state: &mut u64) -> Option<T> {
    if values.is_empty() {
        return None;
    }
    let at = usize::try_from(next(state) % u64::try_from(values.len()).unwrap_or(1)).unwrap_or(0);
    values.get(at).cloned()
}

/// The attribute lists a hostile caller gives EGL: well formed, a key with no value, no EGL_NONE, empty, keys EGL
/// refuses, values out of range.
const LISTS: [&[i32]; 7] = [
    &[egl::NONE],
    &[],
    &[egl::RED_SIZE],
    &[egl::WIDTH, 1, egl::HEIGHT, 1],
    &[egl::CONTEXT_MAJOR_VERSION, 3, egl::NONE],
    &[0x7FFF_FFFF, -1, egl::NONE],
    &[egl::WIDTH, -1, egl::HEIGHT, i32::MAX, egl::NONE, 5],
];

/// The EGL methods the fuzzing calls.
const EGL_METHODS: u64 = 15;
/// Of every 1024 calls, how many are EGL's (each may make a context, which is slow on a real library).
const EGL_SHARE: u64 = 8;
/// The most contexts and surfaces the fuzzing keeps; past it, it destroys the oldest.
const KEPT: usize = 8;

/// EGL's handles a hostile caller holds: its own, another library's, and destroyed ones.
struct Handles<E: Egl> {
    displays: Vec<E::Display>,
    configs: Vec<E::Config>,
    contexts: Vec<E::Context>,
    surfaces: Vec<E::Surface>,
}

/// Calls EGL method `which` with handles and lists picked by the sequence; made handles join the pool.
fn egl_call<E: Egl>(egl: &E, handles: &mut Handles<E>, which: u64, state: &mut u64) {
    let Some(display) = chosen(&handles.displays, state) else { return };
    let list = chosen(&LISTS, state).unwrap_or(&[]).to_vec();
    let room = usize::try_from(next(state) % 12).unwrap_or(0);
    match which {
        0 => {
            let _ = egl.bind_api(chosen(&[egl::OPENGL_ES_API.cast_unsigned(), 0, u32::MAX], state).unwrap_or(0));
        }
        1 => {
            let mut configs = vec![None; room];
            let _ = egl.choose_config(&display, &list, &mut configs);
            handles.configs.extend(configs.into_iter().flatten().take(2));
            handles.configs.truncate(4 * KEPT);
        }
        2 => {
            if let Some(config) = chosen(&handles.configs, state) {
                let share = if next(state).is_multiple_of(2) { chosen(&handles.contexts, state) } else { None };
                if let Some(context) = egl.create_context(&display, &config, share.as_ref(), &list) {
                    handles.contexts.push(context);
                }
            }
        }
        3 => {
            if let Some(config) = chosen(&handles.configs, state)
                && let Some(surface) = egl.create_pbuffer_surface(&display, &config, &list)
            {
                handles.surfaces.push(surface);
            }
        }
        4 => {
            if let Some(context) = chosen(&handles.contexts, state) {
                let _ = egl.destroy_context(&display, &context);
            }
        }
        5 => {
            if let Some(surface) = chosen(&handles.surfaces, state) {
                let _ = egl.destroy_surface(&display, &surface);
            }
        }
        6 => {
            if let Some(config) = chosen(&handles.configs, state) {
                let _ = egl.get_config_attrib(&display, &config, chosen(&[egl::RED_SIZE, egl::NONE, 0, -1], state).unwrap_or(0));
            }
        }
        7 => {
            let _ = egl.get_display();
        }
        8 => {
            let _ = egl.get_error();
        }
        9 => {
            let _ = egl.initialize(&display);
        }
        10 => {
            let draw = chosen(&handles.surfaces, state).filter(|_| next(state).is_multiple_of(2));
            let read = chosen(&handles.surfaces, state).filter(|_| next(state).is_multiple_of(2));
            let context = chosen(&handles.contexts, state).filter(|_| next(state).is_multiple_of(2));
            let _ = egl.make_current(&display, draw.as_ref(), read.as_ref(), context.as_ref());
        }
        11 => {
            let given = if next(state).is_multiple_of(2) { Some(&display) } else { None };
            let _ = egl.query_string(given, chosen(&[egl::VENDOR, egl::EXTENSIONS, 0], state).unwrap_or(0));
        }
        12 => {
            if let Some(surface) = chosen(&handles.surfaces, state) {
                let _ = egl.query_surface(&display, &surface, chosen(&[egl::WIDTH, 0], state).unwrap_or(0));
            }
        }
        13 => {
            if let Some(surface) = chosen(&handles.surfaces, state) {
                let _ = egl.swap_buffers(&display, &surface);
            }
        }
        _ => {
            let _ = egl.swap_interval(&display, chosen(&[0, 1, -1], state).unwrap_or(0));
        }
    }
    // Past the most kept, the oldest are destroyed (and stay in no pool: the destroyed ones kept are those the
    // calls above destroyed).
    while handles.contexts.len() > KEPT {
        let oldest = handles.contexts.remove(0);
        let _ = egl.make_current(&display, None, None, None);
        let _ = egl.destroy_context(&display, &oldest);
    }
    while handles.surfaces.len() > KEPT {
        let oldest = handles.surfaces.remove(0);
        let _ = egl.destroy_surface(&display, &oldest);
    }
}

/// A hostile caller: `calls` calls picked by the seed over every OpenGL ES method (on two contexts, with this
/// context's live fence, a deleted one and the other context's) and every EGL method (with this library's handles,
/// another library's, and destroyed ones). Nothing may end the process; returns the calls made, how many of them the
/// implementation refused itself (OpenGL ES's, EGL's: the rest reached the library), and the calls of each method.
pub fn fuzz<E: Egl, G: OnEgl<E>>(folder: Option<&Path>, seed: u64, calls: u64, counts: &Counts<E, G>) -> Fuzzed {
    let mut state = seed | 1;
    let made = reached::<E, G>(folder);
    let other = reached::<E, G>(folder);
    let live = made.gl.fence_sync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0);
    let deleted = made.gl.fence_sync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0);
    if let Some(fence) = &deleted {
        made.gl.delete_sync(fence);
    }
    let others = other.gl.fence_sync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0);
    let fences: Vec<G::Fence> = [live, deleted, others].into_iter().flatten().collect();
    let second = (0..1000).find_map(|_| E::load(folder)).expect("EGL loads again");
    let second_display = (0..1000).find_map(|_| second.get_display()).expect("its display");
    let _ = second.initialize(&second_display);
    let mut second_configs = vec![None; 2];
    let _ = second.choose_config(&second_display, &[egl::NONE], &mut second_configs);
    let mut handles = Handles::<E> {
        displays: vec![made.display.clone(), made.display.clone(), made.display.clone(), second_display],
        configs: second_configs.into_iter().flatten().collect(),
        contexts: Vec::new(),
        surfaces: Vec::new(),
    };
    let mut own = vec![None; 4];
    let _ = made.egl.choose_config(&made.display, &[egl::NONE], &mut own);
    handles.configs.extend(own.into_iter().flatten());
    let methods = u64::try_from(METHODS).unwrap_or(0);
    let mut gl_methods = vec![0_u64; METHODS];
    let mut egl_methods = vec![0_u64; usize::try_from(EGL_METHODS).unwrap_or(0)];
    for _ in 0..calls {
        let picked = next(&mut state);
        if picked % 1024 < EGL_SHARE {
            let which = (picked >> 10) % EGL_METHODS;
            if let Some(count) = usize::try_from(which).ok().and_then(|at| egl_methods.get_mut(at)) {
                *count += 1;
            }
            egl_call(&made.egl, &mut handles, which, &mut state);
            continue;
        }
        let gl = if (picked >> 10).is_multiple_of(4) { &other.gl } else { &made.gl };
        let Some(fence) = chosen(&fences, &mut state) else { continue };
        let which = usize::try_from((picked >> 12) % methods).unwrap_or(0);
        if let Some(count) = gl_methods.get_mut(which) {
            *count += 1;
        }
        call_one(gl, &fence, which, usize::try_from(next(&mut state) >> 1).unwrap_or(0));
    }
    let _ = made.egl.make_current(&made.display, None, None, None);
    Fuzzed { calls, refused: (counts.gl)(&made.gl) + (counts.gl)(&other.gl), egl_refused: (counts.egl)(&made.egl), gl_methods, egl_methods }
}

/// What `fuzz` did.
pub struct Fuzzed {
    /// The calls picked.
    pub calls: u64,
    /// OpenGL ES calls the implementation refused itself.
    pub refused: u64,
    /// EGL calls the implementation refused itself.
    pub egl_refused: u64,
    /// The calls of each OpenGL ES method, by its place in `call_one`.
    pub gl_methods: Vec<u64>,
    /// The calls of each EGL method, by its place in `egl_call`.
    pub egl_methods: Vec<u64>,
}

/// The number the environment names, else `default`: a run asked for by name takes its input from the environment,
/// and the suite's own run tests with the default (never nothing).
fn given(name: &str, default: u64) -> u64 {
    std::env::var(name).ok().map_or(default, |value| value.parse::<u64>().unwrap_or_else(|_| panic!("{name} names a number, not {value}")))
}

/// The seed and calls the suite's own run fuzzes with, and the draws it measures.
const FUZZ_SEED: u64 = 1;
const FUZZ_CALLS: u64 = 20_000;
const MEASURE_DRAWS: u64 = 1_000;

/// Runs `fuzz` with the seed and call count WCONTAINERS_FUZZ_SEED and WCONTAINERS_FUZZ_CALLS name (`FUZZ_SEED` and
/// `FUZZ_CALLS` where they name none).
pub fn fuzz_from_the_environment<E: Egl, G: OnEgl<E>>(folder: Option<&Path>, counts: &Counts<E, G>) {
    let (seed, calls) = (given("WCONTAINERS_FUZZ_SEED", FUZZ_SEED), given("WCONTAINERS_FUZZ_CALLS", FUZZ_CALLS));
    let fuzzed = fuzz::<E, G>(folder, seed, calls, counts);
    let (gl_never, egl_never) = (never_called(&fuzzed.gl_methods), never_called(&fuzzed.egl_methods));
    println!(
        "FUZZ seed {seed} calls {}, refused by the implementation itself: OpenGL ES {}, EGL {}; methods called: OpenGL ES {} of {METHODS}, EGL {} of {EGL_METHODS}",
        fuzzed.calls,
        fuzzed.refused,
        fuzzed.egl_refused,
        METHODS - gl_never.len(),
        fuzzed.egl_methods.len() - egl_never.len()
    );
    // What the run is for: every method called with hostile arguments, the process still standing, and some calls
    // refused by the implementation itself (its checks were reached).
    assert_eq!(fuzzed.calls, calls, "every call picked");
    assert!(gl_never.is_empty() && egl_never.is_empty(), "methods never called in seed {seed}: OpenGL ES {gl_never:?}, EGL {egl_never:?}");
    assert!(fuzzed.refused > 0 && fuzzed.egl_refused > 0, "the implementation refused no call in seed {seed}: {} {}", fuzzed.refused, fuzzed.egl_refused);
}

/// The places of the methods called no time.
fn never_called(counts: &[u64]) -> Vec<usize> {
    counts.iter().enumerate().filter(|(_, count)| **count == 0).map(|(at, _)| at).collect()
}

/// The nanoseconds each of `times` calls takes, timed together.
fn timed(times: u32, mut call: impl FnMut()) -> f64 {
    let start = std::time::Instant::now();
    for _ in 0..times {
        call();
    }
    start.elapsed().as_secs_f64() * 1e9 / f64::from(times)
}

/// What a draw costs: a triangle drawn `draws` times over the 4 by 4 pixel buffer with one enabled attribute, against
/// a call the library answers at once (glGetError) and a read of one attribute's state (glGetVertexAttribiv, which
/// the per-draw check makes twice per enabled attribute and once per other one).
pub fn draw_costs<E: Egl, G: OnEgl<E>>(folder: Option<&Path>, draws: u32) -> [f64; 3] {
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    let gl = &made.gl;
    let vertex = compiled(gl, gl::VERTEX_SHADER, "#version 300 es\nlayout(location = 0) in vec2 at;\nvoid main() { gl_Position = vec4(at, 0.0, 1.0); }\n");
    let fragment = compiled(gl, gl::FRAGMENT_SHADER, "#version 300 es\nprecision mediump float;\nout vec4 drawn;\nvoid main() { drawn = vec4(1.0); }\n");
    let program = gl.create_program();
    gl.attach_shader(program, vertex);
    gl.attach_shader(program, fragment);
    gl.link_program(program);
    gl.use_program(program);
    let mut buffers = [0_u32];
    gl.gen_buffers(&mut buffers);
    gl.bind_buffer(gl::ARRAY_BUFFER, buffers[0]);
    let triangle: Vec<u8> = [-1.0_f32, -1.0, 3.0, -1.0, -1.0, 3.0].iter().flat_map(|value| value.to_ne_bytes()).collect();
    gl.buffer_data(gl::ARRAY_BUFFER, i64::try_from(triangle.len()).unwrap_or(0), Some(&triangle), gl::STATIC_DRAW);
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer(0, 2, gl::FLOAT, false, 0, 0);
    gl.viewport(0, 0, SIDE, SIDE);
    gl.finish();
    let draw = timed(draws, || gl.draw_arrays(gl::TRIANGLES, 0, 3));
    gl.finish();
    assert_eq!(gl.get_error(), gl::NO_ERROR);
    // The draws timed drew: every pixel of the buffer is the program's white.
    let mut read = vec![0_u8; IMAGE];
    gl.read_pixels(0, 0, SIDE, SIDE, gl::RGBA, gl::UNSIGNED_BYTE, PixelPack::Bytes(&mut read));
    assert!(read.iter().all(|byte| *byte == u8::MAX), "the draws timed drew the triangle over the buffer: {read:?}");
    let answered = timed(draws, || {
        let _ = gl.get_error();
    });
    let mut room = [0_i32; 4];
    let state = timed(draws, || gl.get_vertex_attribiv(0, gl::VERTEX_ATTRIB_ARRAY_ENABLED, &mut room));
    [draw, answered, state]
}

/// Prints `draw_costs` for WCONTAINERS_MEASURE_DRAWS draws (`MEASURE_DRAWS` where it names none), five times.
pub fn measure_from_the_environment<E: Egl, G: OnEgl<E>>(folder: Option<&Path>) {
    let draws = u32::try_from(given("WCONTAINERS_MEASURE_DRAWS", MEASURE_DRAWS)).expect("WCONTAINERS_MEASURE_DRAWS fits 32 bits");
    let made = make::<E, G>(folder, &CONTEXT_ES3);
    let mut most = [0_i32];
    made.gl.get_integerv(gl::MAX_VERTEX_ATTRIBS, &mut most);
    for round in 0..5 {
        let [draw, answered, state] = draw_costs::<E, G>(folder, draws);
        assert!([draw, answered, state].iter().all(|cost| cost.is_finite() && *cost > 0.0), "every cost measured: {draw} {answered} {state}");
        println!("MEASURE round {round} draws {draws} attributes {}: draw {draw:.0} ns, glGetError {answered:.0} ns, glGetVertexAttribiv {state:.0} ns", most[0]);
    }
}

#[macro_export]
macro_rules! tests {
    ($egl:ty, $gl:ty, $folder:expr, $gl_refusals:expr, $egl_refusals:expr) => {
        fn counts() -> $crate::Counts<$egl, $gl> {
            $crate::Counts { gl: $gl_refusals, egl: $egl_refusals }
        }

        #[test]
        fn draws_and_reads_back() {
            $crate::draws_and_reads_back::<$egl, $gl>($folder.as_deref());
        }

        #[test]
        fn refuses_pixels_past_what_was_given() {
            $crate::refuses_pixels_past_what_was_given::<$egl, $gl>($folder.as_deref(), &counts());
        }

        #[test]
        fn refuses_vertex_reads_without_a_buffer() {
            $crate::refuses_vertex_reads_without_a_buffer::<$egl, $gl>($folder.as_deref(), &counts());
        }

        #[test]
        fn counts_are_the_slices() {
            $crate::counts_are_the_slices::<$egl, $gl>($folder.as_deref(), &counts());
        }

        #[test]
        fn counts_reach_only_the_slices() {
            $crate::counts_reach_only_the_slices::<$egl, $gl>($folder.as_deref(), &counts());
        }

        #[test]
        fn rooms_keep_calls_inside() {
            $crate::rooms_keep_calls_inside::<$egl, $gl>($folder.as_deref(), &counts());
        }

        #[test]
        fn fences_are_the_contexts_own() {
            $crate::fences_are_the_contexts_own::<$egl, $gl>($folder.as_deref(), &counts());
        }

        #[test]
        fn egl_handles_are_the_librarys_own() {
            $crate::egl_handles_are_the_librarys_own::<$egl, $gl>($folder.as_deref(), &counts());
        }

        #[test]
        fn robust_access_is_forced() {
            $crate::robust_access_is_forced::<$egl, $gl>($folder.as_deref());
        }

        #[test]
        fn extensions_answer_only_when_named() {
            $crate::extensions_answer_only_when_named::<$egl, $gl>($folder.as_deref());
        }

        #[test]
        fn stencil_faces_are_separate() {
            $crate::stencil_faces_are_separate::<$egl, $gl>($folder.as_deref(), &counts());
        }

        #[test]
        fn extension_values_are_taken() {
            $crate::extension_values_are_taken::<$egl, $gl>($folder.as_deref(), &counts());
        }

        #[test]
        fn fuzz_from_the_environment() {
            $crate::fuzz_from_the_environment::<$egl, $gl>($folder.as_deref(), &counts());
        }

        #[test]
        fn measure_from_the_environment() {
            $crate::measure_from_the_environment::<$egl, $gl>($folder.as_deref());
        }

        #[test]
        fn query_results_wait() {
            $crate::query_results_wait::<$egl, $gl>($folder.as_deref());
        }

        #[test]
        fn every_method_is_called() {
            $crate::every_method_is_called::<$egl, $gl>($folder.as_deref(), $crate::EVERY_METHOD_ROUNDS);
        }
    };
}

/// Calls each extension's command; one answering while the context names none of its extensions fails.
fn extension_calls<G: OpenGlEs>(gl: &G, named: &dyn Fn(&[&str]) -> bool) {
    let seed = 0_usize;
    {
        let answered = gl.blend_equation_separatei_oes(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2))).is_some();
        assert!(named(&["GL_OES_draw_buffers_indexed"]) || !answered, "blend_equation_separatei_oes answered with none of its extensions named");
    }
    {
        let answered = gl.blend_func_separatei_oes(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)), pick(&WORDS, seed.wrapping_add(3)), pick(&WORDS, seed.wrapping_add(4))).is_some();
        assert!(named(&["GL_OES_draw_buffers_indexed"]) || !answered, "blend_func_separatei_oes answered with none of its extensions named");
    }
    {
        let answered = gl.clip_control_ext(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1))).is_some();
        assert!(named(&["GL_EXT_clip_control"]) || !answered, "clip_control_ext answered with none of its extensions named");
    }
    {
        let answered = gl.color_maski_oes(pick(&WORDS, seed.wrapping_add(0)), seed.wrapping_add(1).is_multiple_of(2), seed.wrapping_add(2).is_multiple_of(2), seed.wrapping_add(3).is_multiple_of(2), seed.wrapping_add(4).is_multiple_of(2)).is_some();
        assert!(named(&["GL_OES_draw_buffers_indexed"]) || !answered, "color_maski_oes answered with none of its extensions named");
    }
    {
        let answered = gl.disablei_oes(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1))).is_some();
        assert!(named(&["GL_OES_draw_buffers_indexed", "GL_OES_viewport_array"]) || !answered, "disablei_oes answered with none of its extensions named");
    }
    {
        let answered = gl.enablei_oes(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1))).is_some();
        assert!(named(&["GL_OES_draw_buffers_indexed", "GL_OES_viewport_array"]) || !answered, "enablei_oes answered with none of its extensions named");
    }
    {
        let mut params = vec![0_u64; length(seed.wrapping_add(2))];
        let answered = gl.get_query_objectui64v_ext(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), &mut params).is_some();
        assert!(named(&["GL_EXT_disjoint_timer_query"]) || !answered, "get_query_objectui64v_ext answered with none of its extensions named");
    }
    {
        let mut params = vec![0_i32; length(seed.wrapping_add(2))];
        let answered = gl.get_queryiv_ext(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), &mut params).is_some();
        assert!(named(&["GL_EXT_disjoint_timer_query", "GL_EXT_occlusion_query_boolean"]) || !answered, "get_queryiv_ext answered with none of its extensions named");
    }
    {
        let answered = gl.polygon_mode_angle(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1))).is_some();
        assert!(named(&["GL_ANGLE_polygon_mode"]) || !answered, "polygon_mode_angle answered with none of its extensions named");
    }
    {
        let answered = gl.polygon_offset_clamp_ext(pick(&FLOATS, seed.wrapping_add(0)), pick(&FLOATS, seed.wrapping_add(1)), pick(&FLOATS, seed.wrapping_add(2))).is_some();
        assert!(named(&["GL_EXT_polygon_offset_clamp"]) || !answered, "polygon_offset_clamp_ext answered with none of its extensions named");
    }
    {
        let answered = gl.provoking_vertex_angle(pick(&WORDS, seed.wrapping_add(0))).is_some();
        assert!(named(&["GL_ANGLE_provoking_vertex"]) || !answered, "provoking_vertex_angle answered with none of its extensions named");
    }
    {
        let answered = gl.query_counter_ext(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1))).is_some();
        assert!(named(&["GL_EXT_disjoint_timer_query"]) || !answered, "query_counter_ext answered with none of its extensions named");
    }
}

/// The number of the trait's methods `call_one` calls (154 of 156: glClientWaitSync and glFinish are called by the
/// scenarios, never with a picked timeout, which could wait for ever).
pub const METHODS: usize = 156;

/// Calls the trait's method numbered `which`, with arguments picked by `seed`.
fn call_one<G: OpenGlEs>(gl: &G, fence: &G::Fence, which: usize, seed: usize) {
    match which {
        0 => {
            gl.active_texture(pick(&WORDS, seed.wrapping_add(0)));
        }
        1 => {
            gl.attach_shader(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        2 => {
            gl.begin_query(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        3 => {
            gl.begin_transform_feedback(pick(&WORDS, seed.wrapping_add(0)));
        }
        4 => {
            gl.bind_buffer(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        5 => {
            gl.bind_buffer_base(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)));
        }
        6 => {
            gl.bind_buffer_range(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)), pick(&SIZES, seed.wrapping_add(3)), pick(&SIZES, seed.wrapping_add(4)));
        }
        7 => {
            gl.bind_framebuffer(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        8 => {
            gl.bind_renderbuffer(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        9 => {
            gl.bind_sampler(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        10 => {
            gl.bind_texture(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        11 => {
            gl.bind_vertex_array(pick(&WORDS, seed.wrapping_add(0)));
        }
        12 => {
            gl.blend_color(pick(&FLOATS, seed.wrapping_add(0)), pick(&FLOATS, seed.wrapping_add(1)), pick(&FLOATS, seed.wrapping_add(2)), pick(&FLOATS, seed.wrapping_add(3)));
        }
        13 => {
            gl.blend_equation_separate(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        14 => {
            gl.blend_func_separate(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)), pick(&WORDS, seed.wrapping_add(3)));
        }
        15 => {
            gl.blit_framebuffer(pick(&NUMBERS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)), pick(&NUMBERS, seed.wrapping_add(4)), pick(&NUMBERS, seed.wrapping_add(5)), pick(&NUMBERS, seed.wrapping_add(6)), pick(&NUMBERS, seed.wrapping_add(7)), pick(&WORDS, seed.wrapping_add(8)), pick(&WORDS, seed.wrapping_add(9)));
        }
        16 => {
            {
                let data = vec![0_u8; length(seed.wrapping_add(2))];
                gl.buffer_data(pick(&WORDS, seed.wrapping_add(0)), pick(&SIZES, seed.wrapping_add(1)), (seed.wrapping_add(2).is_multiple_of(2)).then_some(data.as_slice()), pick(&WORDS, seed.wrapping_add(3)));
            }
        }
        17 => {
            {
                let data = vec![0_u8; length(seed.wrapping_add(3))];
                gl.buffer_sub_data(pick(&WORDS, seed.wrapping_add(0)), pick(&SIZES, seed.wrapping_add(1)), &data);
            }
        }
        18 => {
            let _ = gl.check_framebuffer_status(pick(&WORDS, seed.wrapping_add(0)));
        }
        19 => {
            gl.clear_bufferfi(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&FLOATS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)));
        }
        20 => {
            {
                let value = vec![0.0_f32; length(seed.wrapping_add(2))];
                gl.clear_bufferfv(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), &value);
            }
        }
        21 => {
            {
                let value = vec![0_i32; length(seed.wrapping_add(2))];
                gl.clear_bufferiv(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), &value);
            }
        }
        22 => {
            {
                let value = vec![0_u32; length(seed.wrapping_add(2))];
                gl.clear_bufferuiv(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), &value);
            }
        }
        24 => {
            gl.color_mask(seed.wrapping_add(0).is_multiple_of(2), seed.wrapping_add(1).is_multiple_of(2), seed.wrapping_add(2).is_multiple_of(2), seed.wrapping_add(3).is_multiple_of(2));
        }
        25 => {
            gl.compile_shader(pick(&WORDS, seed.wrapping_add(0)));
        }
        26 => {
            {
                let data = vec![0_u8; length(seed.wrapping_add(8))];
                gl.compressed_tex_sub_image_2d(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)), pick(&NUMBERS, seed.wrapping_add(4)), pick(&NUMBERS, seed.wrapping_add(5)), pick(&WORDS, seed.wrapping_add(6)), if seed.wrapping_add(8).is_multiple_of(3) { CompressedPixelUnpack::Offset { offset: 0, size: 16 } } else { CompressedPixelUnpack::Bytes(&data) });
            }
        }
        27 => {
            {
                let data = vec![0_u8; length(seed.wrapping_add(10))];
                gl.compressed_tex_sub_image_3d(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)), pick(&NUMBERS, seed.wrapping_add(4)), pick(&NUMBERS, seed.wrapping_add(5)), pick(&NUMBERS, seed.wrapping_add(6)), pick(&NUMBERS, seed.wrapping_add(7)), pick(&WORDS, seed.wrapping_add(8)), if seed.wrapping_add(10).is_multiple_of(3) { CompressedPixelUnpack::Offset { offset: 0, size: 16 } } else { CompressedPixelUnpack::Bytes(&data) });
            }
        }
        28 => {
            gl.copy_buffer_sub_data(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&SIZES, seed.wrapping_add(2)), pick(&SIZES, seed.wrapping_add(3)), pick(&SIZES, seed.wrapping_add(4)));
        }
        29 => {
            gl.copy_tex_sub_image_2d(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)), pick(&NUMBERS, seed.wrapping_add(4)), pick(&NUMBERS, seed.wrapping_add(5)), pick(&NUMBERS, seed.wrapping_add(6)), pick(&NUMBERS, seed.wrapping_add(7)));
        }
        30 => {
            gl.copy_tex_sub_image_3d(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)), pick(&NUMBERS, seed.wrapping_add(4)), pick(&NUMBERS, seed.wrapping_add(5)), pick(&NUMBERS, seed.wrapping_add(6)), pick(&NUMBERS, seed.wrapping_add(7)), pick(&NUMBERS, seed.wrapping_add(8)));
        }
        31 => {
            let _ = gl.create_program();
        }
        32 => {
            let _ = gl.create_shader(pick(&WORDS, seed.wrapping_add(0)));
        }
        33 => {
            gl.cull_face(pick(&WORDS, seed.wrapping_add(0)));
        }
        34 => {
            {
                let buffers = vec![0_u32; length(seed.wrapping_add(1))];
                gl.delete_buffers(&buffers);
            }
        }
        35 => {
            {
                let framebuffers = vec![0_u32; length(seed.wrapping_add(1))];
                gl.delete_framebuffers(&framebuffers);
            }
        }
        36 => {
            gl.delete_program(pick(&WORDS, seed.wrapping_add(0)));
        }
        37 => {
            {
                let ids = vec![0_u32; length(seed.wrapping_add(1))];
                gl.delete_queries(&ids);
            }
        }
        38 => {
            {
                let renderbuffers = vec![0_u32; length(seed.wrapping_add(1))];
                gl.delete_renderbuffers(&renderbuffers);
            }
        }
        39 => {
            {
                let samplers = vec![0_u32; length(seed.wrapping_add(1))];
                gl.delete_samplers(&samplers);
            }
        }
        40 => {
            gl.delete_shader(pick(&WORDS, seed.wrapping_add(0)));
        }
        41 => {
            gl.delete_sync(fence);
        }
        42 => {
            {
                let textures = vec![0_u32; length(seed.wrapping_add(1))];
                gl.delete_textures(&textures);
            }
        }
        43 => {
            {
                let arrays = vec![0_u32; length(seed.wrapping_add(1))];
                gl.delete_vertex_arrays(&arrays);
            }
        }
        44 => {
            gl.depth_func(pick(&WORDS, seed.wrapping_add(0)));
        }
        45 => {
            gl.depth_mask(seed.wrapping_add(0).is_multiple_of(2));
        }
        46 => {
            gl.depth_rangef(pick(&FLOATS, seed.wrapping_add(0)), pick(&FLOATS, seed.wrapping_add(1)));
        }
        47 => {
            gl.detach_shader(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        48 => {
            gl.disable(pick(&WORDS, seed.wrapping_add(0)));
        }
        49 => {
            gl.disable_vertex_attrib_array(pick(&WORDS, seed.wrapping_add(0)));
        }
        50 => {
            gl.draw_arrays(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)));
        }
        51 => {
            gl.draw_arrays_instanced(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)));
        }
        52 => {
            {
                let bufs = vec![0_u32; length(seed.wrapping_add(1))];
                gl.draw_buffers(&bufs);
            }
        }
        53 => {
            gl.draw_elements_instanced(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)), pick(&SIZES, seed.wrapping_add(3)).unsigned_abs(), pick(&NUMBERS, seed.wrapping_add(4)));
        }
        54 => {
            gl.enable(pick(&WORDS, seed.wrapping_add(0)));
        }
        55 => {
            gl.enable_vertex_attrib_array(pick(&WORDS, seed.wrapping_add(0)));
        }
        56 => {
            gl.end_query(pick(&WORDS, seed.wrapping_add(0)));
        }
        57 => {
            gl.end_transform_feedback();
        }
        58 => {
            let _ = gl.fence_sync(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        60 => {
            gl.flush();
        }
        61 => {
            gl.framebuffer_renderbuffer(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)), pick(&WORDS, seed.wrapping_add(3)));
        }
        62 => {
            gl.framebuffer_texture_2d(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)), pick(&WORDS, seed.wrapping_add(3)), pick(&NUMBERS, seed.wrapping_add(4)));
        }
        63 => {
            gl.framebuffer_texture_layer(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)), pick(&NUMBERS, seed.wrapping_add(4)));
        }
        64 => {
            gl.front_face(pick(&WORDS, seed.wrapping_add(0)));
        }
        65 => {
            {
                let mut buffers = vec![0_u32; length(seed.wrapping_add(1))];
                gl.gen_buffers(&mut buffers);
            }
        }
        66 => {
            {
                let mut framebuffers = vec![0_u32; length(seed.wrapping_add(1))];
                gl.gen_framebuffers(&mut framebuffers);
            }
        }
        67 => {
            {
                let mut ids = vec![0_u32; length(seed.wrapping_add(1))];
                gl.gen_queries(&mut ids);
            }
        }
        68 => {
            {
                let mut renderbuffers = vec![0_u32; length(seed.wrapping_add(1))];
                gl.gen_renderbuffers(&mut renderbuffers);
            }
        }
        69 => {
            {
                let mut samplers = vec![0_u32; length(seed.wrapping_add(1))];
                gl.gen_samplers(&mut samplers);
            }
        }
        70 => {
            {
                let mut textures = vec![0_u32; length(seed.wrapping_add(1))];
                gl.gen_textures(&mut textures);
            }
        }
        71 => {
            {
                let mut arrays = vec![0_u32; length(seed.wrapping_add(1))];
                gl.gen_vertex_arrays(&mut arrays);
            }
        }
        72 => {
            gl.generate_mipmap(pick(&WORDS, seed.wrapping_add(0)));
        }
        73 => {
            {
                let mut data = vec![false; length(seed.wrapping_add(1))];
                gl.get_booleanv(pick(&WORDS, seed.wrapping_add(0)), &mut data);
            }
        }
        74 => {
            let _ = gl.get_error();
        }
        75 => {
            {
                let mut data = vec![0.0_f32; length(seed.wrapping_add(1))];
                gl.get_floatv(pick(&WORDS, seed.wrapping_add(0)), &mut data);
            }
        }
        76 => {
            {
                let mut data = vec![0_i64; length(seed.wrapping_add(1))];
                gl.get_integer64v(pick(&WORDS, seed.wrapping_add(0)), &mut data);
            }
        }
        77 => {
            {
                let mut data = vec![0_i32; length(seed.wrapping_add(1))];
                gl.get_integerv(pick(&WORDS, seed.wrapping_add(0)), &mut data);
            }
        }
        78 => {
            {
                let mut params = vec![0_i32; length(seed.wrapping_add(4))];
                gl.get_internalformativ(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)), &mut params);
            }
        }
        79 => {
            {
                let mut info_log = vec![0_u8; length(seed.wrapping_add(3))];
                let _ = gl.get_program_info_log(pick(&WORDS, seed.wrapping_add(0)), &mut info_log);
            }
        }
        80 => {
            {
                let mut params = vec![0_i32; length(seed.wrapping_add(2))];
                gl.get_programiv(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), &mut params);
            }
        }
        81 => {
            {
                let mut params = vec![0_u32; length(seed.wrapping_add(2))];
                gl.get_query_objectuiv(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), &mut params);
            }
        }
        82 => {
            {
                let mut info_log = vec![0_u8; length(seed.wrapping_add(3))];
                let _ = gl.get_shader_info_log(pick(&WORDS, seed.wrapping_add(0)), &mut info_log);
            }
        }
        83 => {
            {
                let mut params = vec![0_i32; length(seed.wrapping_add(2))];
                gl.get_shaderiv(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), &mut params);
            }
        }
        84 => {
            let _ = gl.get_string(pick(&WORDS, seed.wrapping_add(0)));
        }
        85 => {
            let _ = gl.get_stringi(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        86 => {
            {
                let mut values = vec![0_i32; length(seed.wrapping_add(4))];
                let _ = gl.get_synciv(fence, pick(&WORDS, seed.wrapping_add(1)), &mut values);
            }
        }
        87 => {
            let _ = gl.get_uniform_block_index(pick(&WORDS, seed.wrapping_add(0)), c"name");
        }
        88 => {
            let _ = gl.get_uniform_location(pick(&WORDS, seed.wrapping_add(0)), c"name");
        }
        89 => {
            {
                let mut params = vec![0_i32; length(seed.wrapping_add(2))];
                gl.get_vertex_attribiv(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), &mut params);
            }
        }
        90 => {
            {
                let attachments = vec![0_u32; length(seed.wrapping_add(2))];
                gl.invalidate_framebuffer(pick(&WORDS, seed.wrapping_add(0)), &attachments);
            }
        }
        91 => {
            gl.link_program(pick(&WORDS, seed.wrapping_add(0)));
        }
        92 => {
            gl.pixel_storei(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)));
        }
        93 => {
            gl.polygon_offset(pick(&FLOATS, seed.wrapping_add(0)), pick(&FLOATS, seed.wrapping_add(1)));
        }
        94 => {
            {
                let mut pixels = vec![0_u8; length(seed.wrapping_add(6))];
                gl.read_pixels(pick(&NUMBERS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)), pick(&WORDS, seed.wrapping_add(4)), pick(&WORDS, seed.wrapping_add(5)), if seed.wrapping_add(6).is_multiple_of(3) { PixelPack::Offset(0) } else { PixelPack::Bytes(&mut pixels) });
            }
        }
        95 => {
            gl.renderbuffer_storage(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)));
        }
        96 => {
            gl.renderbuffer_storage_multisample(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)), pick(&NUMBERS, seed.wrapping_add(4)));
        }
        97 => {
            gl.sampler_parameterf(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&FLOATS, seed.wrapping_add(2)));
        }
        98 => {
            gl.sampler_parameteri(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)));
        }
        99 => {
            gl.scissor(pick(&NUMBERS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)));
        }
        100 => {
            gl.shader_source(pick(&WORDS, seed.wrapping_add(0)), &[b"void main() {}".as_slice()]);
        }
        101 => {
            gl.stencil_func(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)));
        }
        102 => {
            gl.stencil_func_separate(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)), pick(&WORDS, seed.wrapping_add(3)));
        }
        103 => {
            gl.stencil_mask(pick(&WORDS, seed.wrapping_add(0)));
        }
        104 => {
            gl.stencil_op(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)));
        }
        105 => {
            gl.stencil_op_separate(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)), pick(&WORDS, seed.wrapping_add(3)));
        }
        106 => {
            gl.tex_parameteri(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)));
        }
        107 => {
            gl.tex_storage_2d(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)), pick(&NUMBERS, seed.wrapping_add(4)));
        }
        108 => {
            gl.tex_storage_3d(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)), pick(&NUMBERS, seed.wrapping_add(4)), pick(&NUMBERS, seed.wrapping_add(5)));
        }
        109 => {
            {
                let pixels = vec![0_u8; length(seed.wrapping_add(8))];
                gl.tex_sub_image_2d(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)), pick(&NUMBERS, seed.wrapping_add(4)), pick(&NUMBERS, seed.wrapping_add(5)), pick(&WORDS, seed.wrapping_add(6)), pick(&WORDS, seed.wrapping_add(7)), if seed.wrapping_add(8).is_multiple_of(3) { PixelUnpack::Offset(0) } else { PixelUnpack::Bytes(&pixels) });
            }
        }
        110 => {
            {
                let pixels = vec![0_u8; length(seed.wrapping_add(10))];
                gl.tex_sub_image_3d(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)), pick(&NUMBERS, seed.wrapping_add(4)), pick(&NUMBERS, seed.wrapping_add(5)), pick(&NUMBERS, seed.wrapping_add(6)), pick(&NUMBERS, seed.wrapping_add(7)), pick(&WORDS, seed.wrapping_add(8)), pick(&WORDS, seed.wrapping_add(9)), if seed.wrapping_add(10).is_multiple_of(3) { PixelUnpack::Offset(0) } else { PixelUnpack::Bytes(&pixels) });
            }
        }
        111 => {
            gl.transform_feedback_varyings(pick(&WORDS, seed.wrapping_add(0)), &[c"name"], pick(&WORDS, seed.wrapping_add(3)));
        }
        112 => {
            gl.uniform1f(pick(&NUMBERS, seed.wrapping_add(0)), pick(&FLOATS, seed.wrapping_add(1)));
        }
        113 => {
            {
                let value = vec![0.0_f32; length(seed.wrapping_add(2))];
                gl.uniform1fv(pick(&NUMBERS, seed.wrapping_add(0)), &value);
            }
        }
        114 => {
            gl.uniform1i(pick(&NUMBERS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)));
        }
        115 => {
            {
                let value = vec![0_i32; length(seed.wrapping_add(2))];
                gl.uniform1iv(pick(&NUMBERS, seed.wrapping_add(0)), &value);
            }
        }
        116 => {
            gl.uniform1ui(pick(&NUMBERS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        117 => {
            {
                let value = vec![0_u32; length(seed.wrapping_add(2))];
                gl.uniform1uiv(pick(&NUMBERS, seed.wrapping_add(0)), &value);
            }
        }
        118 => {
            gl.uniform2f(pick(&NUMBERS, seed.wrapping_add(0)), pick(&FLOATS, seed.wrapping_add(1)), pick(&FLOATS, seed.wrapping_add(2)));
        }
        119 => {
            {
                let value = vec![0.0_f32; length(seed.wrapping_add(2))];
                gl.uniform2fv(pick(&NUMBERS, seed.wrapping_add(0)), &value);
            }
        }
        120 => {
            {
                let value = vec![0_i32; length(seed.wrapping_add(2))];
                gl.uniform2iv(pick(&NUMBERS, seed.wrapping_add(0)), &value);
            }
        }
        121 => {
            {
                let value = vec![0_u32; length(seed.wrapping_add(2))];
                gl.uniform2uiv(pick(&NUMBERS, seed.wrapping_add(0)), &value);
            }
        }
        122 => {
            {
                let value = vec![0.0_f32; length(seed.wrapping_add(2))];
                gl.uniform3fv(pick(&NUMBERS, seed.wrapping_add(0)), &value);
            }
        }
        123 => {
            gl.uniform3i(pick(&NUMBERS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)));
        }
        124 => {
            {
                let value = vec![0_i32; length(seed.wrapping_add(2))];
                gl.uniform3iv(pick(&NUMBERS, seed.wrapping_add(0)), &value);
            }
        }
        125 => {
            {
                let value = vec![0_u32; length(seed.wrapping_add(2))];
                gl.uniform3uiv(pick(&NUMBERS, seed.wrapping_add(0)), &value);
            }
        }
        126 => {
            {
                let value = vec![0.0_f32; length(seed.wrapping_add(2))];
                gl.uniform4fv(pick(&NUMBERS, seed.wrapping_add(0)), &value);
            }
        }
        127 => {
            {
                let value = vec![0_i32; length(seed.wrapping_add(2))];
                gl.uniform4iv(pick(&NUMBERS, seed.wrapping_add(0)), &value);
            }
        }
        128 => {
            {
                let value = vec![0_u32; length(seed.wrapping_add(2))];
                gl.uniform4uiv(pick(&NUMBERS, seed.wrapping_add(0)), &value);
            }
        }
        129 => {
            gl.uniform_block_binding(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)));
        }
        130 => {
            {
                let value = vec![0.0_f32; length(seed.wrapping_add(3))];
                gl.uniform_matrix2fv(pick(&NUMBERS, seed.wrapping_add(0)), seed.wrapping_add(2).is_multiple_of(2), &value);
            }
        }
        131 => {
            {
                let value = vec![0.0_f32; length(seed.wrapping_add(3))];
                gl.uniform_matrix2x3fv(pick(&NUMBERS, seed.wrapping_add(0)), seed.wrapping_add(2).is_multiple_of(2), &value);
            }
        }
        132 => {
            {
                let value = vec![0.0_f32; length(seed.wrapping_add(3))];
                gl.uniform_matrix2x4fv(pick(&NUMBERS, seed.wrapping_add(0)), seed.wrapping_add(2).is_multiple_of(2), &value);
            }
        }
        133 => {
            {
                let value = vec![0.0_f32; length(seed.wrapping_add(3))];
                gl.uniform_matrix3fv(pick(&NUMBERS, seed.wrapping_add(0)), seed.wrapping_add(2).is_multiple_of(2), &value);
            }
        }
        134 => {
            {
                let value = vec![0.0_f32; length(seed.wrapping_add(3))];
                gl.uniform_matrix3x2fv(pick(&NUMBERS, seed.wrapping_add(0)), seed.wrapping_add(2).is_multiple_of(2), &value);
            }
        }
        135 => {
            {
                let value = vec![0.0_f32; length(seed.wrapping_add(3))];
                gl.uniform_matrix3x4fv(pick(&NUMBERS, seed.wrapping_add(0)), seed.wrapping_add(2).is_multiple_of(2), &value);
            }
        }
        136 => {
            {
                let value = vec![0.0_f32; length(seed.wrapping_add(3))];
                gl.uniform_matrix4fv(pick(&NUMBERS, seed.wrapping_add(0)), seed.wrapping_add(2).is_multiple_of(2), &value);
            }
        }
        137 => {
            {
                let value = vec![0.0_f32; length(seed.wrapping_add(3))];
                gl.uniform_matrix4x2fv(pick(&NUMBERS, seed.wrapping_add(0)), seed.wrapping_add(2).is_multiple_of(2), &value);
            }
        }
        138 => {
            {
                let value = vec![0.0_f32; length(seed.wrapping_add(3))];
                gl.uniform_matrix4x3fv(pick(&NUMBERS, seed.wrapping_add(0)), seed.wrapping_add(2).is_multiple_of(2), &value);
            }
        }
        139 => {
            gl.use_program(pick(&WORDS, seed.wrapping_add(0)));
        }
        140 => {
            gl.vertex_attrib_divisor(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        141 => {
            gl.vertex_attrib_i_pointer(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)), pick(&SIZES, seed.wrapping_add(4)).unsigned_abs());
        }
        142 => {
            gl.vertex_attrib_pointer(pick(&WORDS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)), seed.wrapping_add(3).is_multiple_of(2), pick(&NUMBERS, seed.wrapping_add(4)), pick(&SIZES, seed.wrapping_add(5)).unsigned_abs());
        }
        143 => {
            gl.viewport(pick(&NUMBERS, seed.wrapping_add(0)), pick(&NUMBERS, seed.wrapping_add(1)), pick(&NUMBERS, seed.wrapping_add(2)), pick(&NUMBERS, seed.wrapping_add(3)));
        }
        144 => {
            let _ = gl.blend_equation_separatei_oes(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)));
        }
        145 => {
            let _ = gl.blend_func_separatei_oes(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), pick(&WORDS, seed.wrapping_add(2)), pick(&WORDS, seed.wrapping_add(3)), pick(&WORDS, seed.wrapping_add(4)));
        }
        146 => {
            let _ = gl.clip_control_ext(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        147 => {
            let _ = gl.color_maski_oes(pick(&WORDS, seed.wrapping_add(0)), seed.wrapping_add(1).is_multiple_of(2), seed.wrapping_add(2).is_multiple_of(2), seed.wrapping_add(3).is_multiple_of(2), seed.wrapping_add(4).is_multiple_of(2));
        }
        148 => {
            let _ = gl.disablei_oes(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        149 => {
            let _ = gl.enablei_oes(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        150 => {
            {
                let mut params = vec![0_u64; length(seed.wrapping_add(2))];
                let _ = gl.get_query_objectui64v_ext(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), &mut params);
            }
        }
        151 => {
            {
                let mut params = vec![0_i32; length(seed.wrapping_add(2))];
                let _ = gl.get_queryiv_ext(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)), &mut params);
            }
        }
        152 => {
            let _ = gl.polygon_mode_angle(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        153 => {
            let _ = gl.polygon_offset_clamp_ext(pick(&FLOATS, seed.wrapping_add(0)), pick(&FLOATS, seed.wrapping_add(1)), pick(&FLOATS, seed.wrapping_add(2)));
        }
        154 => {
            let _ = gl.provoking_vertex_angle(pick(&WORDS, seed.wrapping_add(0)));
        }
        155 => {
            let _ = gl.query_counter_ext(pick(&WORDS, seed.wrapping_add(0)), pick(&WORDS, seed.wrapping_add(1)));
        }
        _ => {}
    }
}
