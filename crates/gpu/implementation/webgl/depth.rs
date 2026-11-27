//! Depth and stencil read back by drawing them into a colour texture
//! ([`wcontainers_gpu::slow::SlowPath::DepthReadByDraw`]): WebGL 2's `readPixels` reads neither, so a
//! full-screen triangle fetches each texel and writes its stored value into an `r32uint` texture,
//! which reads back like any colour texture. Stencil is fetched as unsigned integers with
//! WEBGL_stencil_texturing; without it, eight draws count its bits into an `r8unorm` texture first
//! ([`wcontainers_gpu::slow::SlowPath::StencilCopiedByDraw`]), which the same fetch reads with a scale of 255.

use crate::platform;
use std::sync::OnceLock;
use wcontainers_gpu::format::Format;
use wcontainers_opengl_es::OpenGlEs;
use wcontainers_opengl_es::constants as gles;

/// The program for one kind of source texture, and its uniforms.
#[derive(Debug)]
pub(crate) struct DepthRead {
    pub(crate) program: u32,
    /// The region's origin x and y, and its layer; the level is the texture's base level, which the
    /// draw sets to the region's.
    pub(crate) at: i32,
    /// What a depth of 1 is stored as (65535, 16777215), or 0 to keep a float's bits; None for
    /// stencil.
    pub(crate) scale: Option<i32>,
    /// The device's conversion of 24-bit depth to a float ([`UNORM24`]); None for stencil.
    pub(crate) reciprocal: Option<i32>,
}

/// The 24-bit value stored for depth `d` as sampled, by the device's own conversion: the uniform
/// `reciprocal24` is 0 where the device samples a stored c as the float nearest c / (2^24 - 1) (as
/// Direct3D asks), else the float it multiplies c by (llvmpipe: the float nearest 1 / (2^24 - 1)); the
/// back end's probe at creation sets it. c 2^-24 is exact and c / (2^24 - 1) lies above it by more than
/// half a float step and at most one, so the nearest float is c 2^-24's bits plus one; under both
/// conversions d is at least
/// c / (2^24 - 1), so the product d (2^24 - 1), rounded once, is at least c, and its whole part `e` is c
/// or one high (in the upper part of each binade neighbouring values give one float). So e and the value
/// below it are each converted as the device does, the one giving d's bits kept (e where neither does).
/// A sampled depth is at most 1, so e is at most 2^24 - 1; at 0, `e - 1u` wraps to a word whose float
/// is above 1, which no sampled depth has. Words the device samples to one float are told apart by
/// [`EQUAL`].
pub(crate) const UNORM24: &str = "uniform highp float reciprocal24;
uint sampled24(uint c) {
    if (reciprocal24 != 0.0) return floatBitsToUint(float(c) * reciprocal24);
    return c == 0u ? 0u : floatBitsToUint(float(c) * exp2(-24.0)) + 1u;
}
uint unorm24(float d) {
    uint e = uint(d * 16777215.0);
    uint bits = floatBitsToUint(d);
    return sampled24(e) != bits && sampled24(e - 1u) == bits ? e - 1u : e;
}
";

/// The comparison pass after a read of 24-bit depth by sampling, where the device samples some stored
/// words to the float of another ([`colliding`]): drawn with the depth texture as the depth attachment
/// and the depth test EQUAL, at the depth stored as `word` (`depth`, the float nearest
/// word / (2^24 - 1), which the device stores as that word), so only texels holding `word` write it.
/// The device compares the stored integers, which sampling cannot tell apart.
const EQUAL: &str = "#version 300 es
precision highp float;
uniform highp uint word;
uniform highp float depth;
out highp uvec4 o;
void main() {
    gl_FragDepth = depth;
    o = uvec4(word, 0u, 0u, 0u);
}
";

/// The comparison pass's program and its uniforms ([`EQUAL`]).
#[derive(Debug)]
pub(crate) struct EqualPass {
    pub(crate) program: u32,
    pub(crate) word: i32,
    pub(crate) depth: i32,
}

/// Builds the comparison pass's program. Err holds GL's log.
pub(crate) fn build_equal(gl: &impl OpenGlEs) -> Result<EqualPass, String> {
    let program = link(gl, EQUAL)?;
    let uniform = |name| {
        platform::location(gl, program, name)
            .ok_or_else(|| format!("the comparison pass has no uniform {name}"))
    };
    Ok(EqualPass {
        program,
        word: uniform("word")?,
        depth: uniform("depth")?,
    })
}

/// The stored 24-bit words a read by sampling ([`UNORM24`]) gives as another word on a device that
/// samples a stored c as c times `reciprocal` (0: the float nearest c / (2^24 - 1), where every word
/// samples to its own float and none is listed). Found by converting every word as the device does
/// and reading the float back as the read does; on llvmpipe's product, 0xfffffe alone (it samples to
/// 1.0, as 0xffffff does). The scan of the first reciprocal asked is kept for the process.
pub(crate) fn colliding(reciprocal: f32) -> Vec<u32> {
    static FIRST: OnceLock<(u32, Vec<u32>)> = OnceLock::new();
    if reciprocal == 0.0 {
        return Vec::new();
    }
    let (bits, words) = FIRST.get_or_init(|| (reciprocal.to_bits(), scan(reciprocal)));
    if *bits == reciprocal.to_bits() {
        words.clone()
    } else {
        scan(reciprocal)
    }
}

/// [`colliding`]'s scan of every word, for a reciprocal other than 0.
fn scan(reciprocal: f32) -> Vec<u32> {
    let sampled = |c: u32| (c as f32 * reciprocal).to_bits();
    let read = |bits: u32| {
        let e = ((f32::from_bits(bits) * Format::UNORM24_MAX as f32).round() as u32)
            .min(Format::UNORM24_MAX);
        if sampled(e) != bits && e.checked_sub(1).is_some_and(|below| sampled(below) == bits) {
            e - 1
        } else {
            e
        }
    };
    (0..=Format::UNORM24_MAX)
        .filter(|&c| read(sampled(c)) != c)
        .collect()
}

/// The float nearest `word` / (2^24 - 1): word 2^-24 is exact and the quotient more than half and at
/// most one of its float steps above it, so it is word 2^-24's bits plus one (zero stays zero).
pub(crate) fn nearest(word: u32) -> f32 {
    if word == 0 {
        0.0
    } else {
        f32::from_bits((word as f32 * (1.0 / (1u32 << 24) as f32)).to_bits() + 1)
    }
}

/// A triangle covering the target, from the vertex number alone.
const VERTEX: &str = "#version 300 es
void main() {
    gl_Position = vec4(float((gl_VertexID & 1) << 2) - 1.0, float((gl_VertexID & 2) << 1) - 1.0, 0.0, 1.0);
}
";

/// The direction from a cube's centre to the centre of texel `t` of face `at.z` (GL's face order
/// and texel axes), for programs sampling a cube `src` towards a texel.
pub(crate) const TOWARDS: &str = "vec3 towards(ivec2 t) {
    vec2 c = (vec2(t) + 0.5) / float(textureSize(src, 0).x) * 2.0 - 1.0;
    switch (at.z) {
        case 0: return vec3(1.0, -c.y, -c.x);
        case 1: return vec3(-1.0, -c.y, c.x);
        case 2: return vec3(c.x, 1.0, c.y);
        case 3: return vec3(c.x, -1.0, -c.y);
        case 4: return vec3(c.x, -c.y, 1.0);
        default: return vec3(-c.x, -c.y, -1.0);
    }
}
";

/// The shape of a source texture: 2D, 2D array (`at.z` the layer) or cube (`at.z` the face).
#[derive(Clone, Copy, Debug)]
pub(crate) enum Shape {
    D2,
    Array,
    Cube,
}

/// Fetches the texel under the fragment, moved by the region's origin, and writes its stored value.
/// Drawn with GL's own lower-left window origin, so the fragment's row is the texel's row. GLSL ES
/// 3.00 fetches no texel of a cube, so a cube is sampled, nearest, towards the texel's centre.
fn fragment(shape: Shape, stencil: bool) -> String {
    let u = if stencil { "u" } else { "" };
    let (sampler, coord) = match shape {
        Shape::D2 => (format!("{u}sampler2D"), "p"),
        Shape::Array => (format!("{u}sampler2DArray"), "ivec3(p, at.z)"),
        Shape::Cube => (format!("{u}samplerCube"), "p"),
    };
    let fetch = match shape {
        Shape::Cube => "textureLod(src, towards(p), 0.0)",
        _ => "texelFetch(src, COORD, 0)",
    }
    .replace("COORD", coord);
    let value = if stencil {
        "FETCH.r"
    } else {
        // The product rounds once to the nearest float, which from 2^23 up is the nearest integer:
        // adding 0.5 before truncating would round again (or not at all, fused). 24-bit depth is
        // checked against the device's conversion ([`UNORM24`]).
        "scale == 0u ? floatBitsToUint(d) : scale == 16777215u ? unorm24(d) \
         : min(uint(round(d * float(scale))), scale)"
    }
    .replace("FETCH", &fetch);
    format!(
        "#version 300 es
precision highp float;
precision highp int;
uniform highp {sampler} src;
uniform ivec3 at;
uniform uint scale;
out highp uvec4 o;
{unorm24}{towards}void main() {{
    ivec2 p = ivec2(gl_FragCoord.xy) + at.xy;
    {fetch}
    o = uvec4({value}, 0u, 0u, 0u);
}}
",
        towards = if matches!(shape, Shape::Cube) {
            TOWARDS
        } else {
            ""
        },
        unorm24 = if stencil { "" } else { UNORM24 },
        fetch = if stencil {
            String::new()
        } else {
            format!("float d = {fetch}.r;")
        },
    )
}

/// Writes 1 everywhere: blended by the constant colour, each stencil bit's draw adds its weight.
const ONE: &str = "#version 300 es
precision highp float;
out vec4 o;
void main() {
    o = vec4(1.0);
}
";

/// Builds the program for sources of `shape`, of depth or of stencil. Err holds GL's log.
pub(crate) fn build(gl: &impl OpenGlEs, shape: Shape, stencil: bool) -> Result<DepthRead, String> {
    let program = link(gl, &fragment(shape, stencil))?;
    let at =
        platform::location(gl, program, "at").ok_or("the depth read program has no uniform at")?;
    let scale = platform::location(gl, program, "scale");
    let reciprocal = platform::location(gl, program, "reciprocal24");
    Ok(DepthRead {
        program,
        at,
        scale,
        reciprocal,
    })
}

/// The program writing stencil from an `r8uint` texture, and its uniforms.
#[derive(Debug)]
pub(crate) struct StencilWrite {
    pub(crate) program: u32,
    /// As [`DepthRead::at`]: the source's offset from the fragment, and its layer.
    pub(crate) at: i32,
    /// The bit drawn: fragments whose source value lacks it are discarded; 0 discards none.
    pub(crate) bit: i32,
}

/// Keeps the fragments whose source texel holds `bit`, for draws that set that stencil bit.
fn stencil_write(array: bool) -> String {
    let (sampler, coord) = if array {
        ("usampler2DArray", "ivec3(p, at.z)")
    } else {
        ("usampler2D", "p")
    };
    format!(
        "#version 300 es
precision highp float;
precision highp int;
uniform highp {sampler} src;
uniform ivec3 at;
uniform uint bit;
void main() {{
    ivec2 p = ivec2(gl_FragCoord.xy) + at.xy;
    if ((texelFetch(src, {coord}, 0).r & bit) != bit) discard;
}}
"
    )
}

/// Builds the stencil write program for 2D (`array` false) or 2D array sources. Err holds GL's log.
pub(crate) fn build_stencil_write(gl: &impl OpenGlEs, array: bool) -> Result<StencilWrite, String> {
    let program = link(gl, &stencil_write(array))?;
    let at = platform::location(gl, program, "at");
    let bit = platform::location(gl, program, "bit");
    match (at, bit) {
        (Some(at), Some(bit)) => Ok(StencilWrite { program, at, bit }),
        _ => Err("the stencil write program lacks a uniform".into()),
    }
}

/// A triangle covering the target at clip depth `z`, for the 24-bit depth probe.
const PROBE: &str = "#version 300 es
uniform float z;
void main() {
    gl_Position = vec4(float((gl_VertexID & 1) << 2) - 1.0, float((gl_VertexID & 2) << 1) - 1.0, z, 1.0);
}
";

/// Builds the 24-bit depth probe's program and its uniform `z`. Err holds GL's log.
pub(crate) fn build_probe(gl: &impl OpenGlEs) -> Result<(u32, i32), String> {
    let program = link_with(gl, PROBE, ONE)?;
    let z = platform::location(gl, program, "z");
    Ok((program, z.ok_or("the probe program has no uniform z")?))
}

/// Triangle A of the suite's top-left rule test (in pixels of a 4 by 4 target, (0.5, 0.5), (3.5, 0.5),
/// (0.5, 3.5) with y down): a top edge and a left edge through pixel centres, drawn as the interface's
/// row 0 at the top. Under Direct3D's top-left rule it covers texel (0, 0); under the bottom-left rule a
/// rasterizer may take with GL's upper-left origin, it does not.
const EDGE_PROBE: &str = "#version 300 es
void main() {
    const vec2 corners[3] = vec2[3](vec2(-0.75, 0.75), vec2(0.75, 0.75), vec2(-0.75, -0.75));
    gl_Position = vec4(corners[gl_VertexID], 0.0, 1.0);
}
";

/// Builds the edge rule probe's program, writing 1 into every channel. Err holds GL's log.
pub(crate) fn build_edge_probe(gl: &impl OpenGlEs) -> Result<u32, String> {
    link_with(gl, EDGE_PROBE, ONE)
}

/// Builds the program the stencil bit draws use. Err holds GL's log.
pub(crate) fn build_bits(gl: &impl OpenGlEs) -> Result<u32, String> {
    link(gl, ONE)
}

/// Links the full-screen vertex program with `fragment`. Err holds GL's log.
pub(crate) fn link(gl: &impl OpenGlEs, fragment: &str) -> Result<u32, String> {
    link_with(gl, VERTEX, fragment)
}

fn link_with(gl: &impl OpenGlEs, vertex: &str, fragment: &str) -> Result<u32, String> {
    let program = gl.create_program();
    if program == 0 {
        return Err("no program object".into());
    }
    let mut shaders = Vec::new();
    for (stage, source) in [
        (gles::VERTEX_SHADER, vertex),
        (gles::FRAGMENT_SHADER, fragment),
    ] {
        let s = gl.create_shader(stage);
        gl.shader_source(s, &[source.as_bytes()]);
        gl.compile_shader(s);
        if !platform::compiled(gl, s) {
            return Err(platform::shader_log(gl, s));
        }
        gl.attach_shader(program, s);
        shaders.push(s);
    }
    gl.link_program(program);
    for s in shaders {
        gl.detach_shader(program, s);
        gl.delete_shader(s);
    }
    if !platform::linked(gl, program) {
        return Err(platform::program_log(gl, program));
    }
    Ok(program)
}

/// What the stored depth of `format` is read as: the scale of [`DepthRead::scale`] and the bytes each
/// texel keeps (Direct3D's: 16-bit unorm, 24-bit unorm in the low bits of a 32-bit word, 32-bit float).
pub(crate) fn stored(format: wcontainers_gpu::format::Format) -> Option<(u32, usize)> {
    use wcontainers_gpu::format::Format as F;
    match format {
        F::Depth16Unorm => Some((65535, 2)),
        F::Depth24Plus | F::Depth24PlusStencil8 => Some((16_777_215, 4)),
        F::Depth32Float | F::Depth32FloatStencil8 => Some((0, 4)),
        _ => None,
    }
}

/// [`UNORM24`] run by the device for every 24-bit word under both conversions it may sample stored
/// depth by, against the values worked out here on the processor. A device takes only the conversion
/// its probe found (llvmpipe the product), so the other one's text is reached by no read on this
/// host; a draw runs both here.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::{UNORM24, link};
    use crate::platform;
    use wcontainers_gpu::StandaloneGpu;
    use wcontainers_opengl_es::{OpenGlEs, PixelPack, constants as gles};

    /// Every 24-bit word is run.
    const WORDS: u32 = 1 << 24;
    /// The width of the target, in texels.
    const WIDE: u32 = 4096;
    /// The rows one draw runs.
    const ROWS: u32 = 1024;
    /// The word llvmpipe's product samples to the float of another (1.0, as 0xffffff does).
    const PRODUCT_COLLIDING: u32 = 0xff_fffe;

    /// For the word under each fragment: the word, the sampled float's bits, the word read back from it.
    fn fragment() -> String {
        format!(
            "#version 300 es
precision highp float;
precision highp int;
uniform uint base;
out highp uvec4 o;
{UNORM24}void main() {{
    uint c = base + uint(gl_FragCoord.y) * {WIDE}u + uint(gl_FragCoord.x);
    uint s = sampled24(c);
    o = uvec4(c, s, unorm24(uintBitsToFloat(s)), 0u);
}}
"
        )
    }

    /// The bits of the float nearest c / (2^24 - 1): the quotient in f64, rounded once to f32.
    fn nearest_float(c: u32) -> u32 {
        ((f64::from(c) / f64::from(WORDS - 1)) as f32).to_bits()
    }

    /// The float llvmpipe multiplies a stored word by, the float nearest 1 / (2^24 - 1).
    fn product_reciprocal() -> f32 {
        (1.0 / f64::from(WORDS - 1)) as f32
    }

    /// The words whose sampled float or read word is not the right value, at most eight, as (word,
    /// sampled, read), how many in all, and how many words were drawn other than once.
    #[derive(Debug, Default, PartialEq, Eq)]
    struct Wrong {
        count: u32,
        first: Vec<(u32, u32, u32)>,
        not_once: u32,
    }

    /// Draws every word with `reciprocal24` set to `reciprocal` (0: the nearest float) and states, for
    /// every word c: the sampled float is the nearest float of c or c times the reciprocal; the read
    /// gives back c, but for the one word the product samples as another's float, which reads as that
    /// word (0xffffff).
    fn check(reciprocal: f32) -> Wrong {
        type Native = crate::Gl<wcontainers_opengl_es_direct::OpenGlEsContext>;
        let gpu = <Native as StandaloneGpu>::new(crate::GlConfig {
            egl: std::env::var_os("WEBGL_ANGLE_DIR").map(std::path::PathBuf::from),
            off: Vec::new(),
        })
        .expect("an OpenGL ES 3.0 context from WEBGL_ANGLE_DIR's EGL library or the system's");
        let gl = &gpu.gl;
        let program = link(gl, &fragment()).expect("the read's program links");
        let base = platform::location(gl, program, "base").expect("uniform base");
        let at = platform::location(gl, program, "reciprocal24").expect("uniform reciprocal24");
        let renderbuffer = platform::one(|n| gl.gen_renderbuffers(n)).expect("a renderbuffer");
        gl.bind_renderbuffer(gles::RENDERBUFFER, renderbuffer);
        gl.renderbuffer_storage(gles::RENDERBUFFER, gles::RGBA32UI, WIDE as i32, ROWS as i32);
        let framebuffer = platform::one(|n| gl.gen_framebuffers(n)).expect("a framebuffer");
        gl.bind_framebuffer(gles::FRAMEBUFFER, framebuffer);
        gl.framebuffer_renderbuffer(
            gles::FRAMEBUFFER,
            gles::COLOR_ATTACHMENT0,
            gles::RENDERBUFFER,
            renderbuffer,
        );
        for cap in [
            gles::SCISSOR_TEST,
            gles::DEPTH_TEST,
            gles::STENCIL_TEST,
            gles::BLEND,
            gles::CULL_FACE,
            gles::RASTERIZER_DISCARD,
        ] {
            gl.disable(cap);
        }
        gl.color_mask(true, true, true, true);
        gl.viewport(0, 0, WIDE as i32, ROWS as i32);
        gl.use_program(program);
        gl.uniform1f(at, reciprocal);
        let mut seen = vec![0u8; WORDS as usize];
        let mut wrong = Wrong::default();
        let mut texels = vec![0u8; (WIDE * ROWS * 16) as usize];
        for first in (0..WORDS).step_by((WIDE * ROWS) as usize) {
            gl.uniform1ui(base, first);
            gl.draw_arrays(gles::TRIANGLES, 0, 3);
            gl.read_pixels(
                0,
                0,
                WIDE as i32,
                ROWS as i32,
                gles::RGBA_INTEGER,
                gles::UNSIGNED_INT,
                PixelPack::Bytes(&mut texels),
            );
            for t in texels.chunks_exact(16) {
                let [c, s, r] =
                    [0, 4, 8].map(|k| u32::from_le_bytes([t[k], t[k + 1], t[k + 2], t[k + 3]]));
                let Some(once) = seen.get_mut(c as usize) else {
                    wrong.not_once += 1;
                    continue;
                };
                *once = once.saturating_add(1);
                let sampled = if reciprocal == 0.0 {
                    nearest_float(c)
                } else {
                    (c as f32 * reciprocal).to_bits()
                };
                let read = if reciprocal != 0.0 && c == PRODUCT_COLLIDING {
                    WORDS - 1
                } else {
                    c
                };
                if (s, r) != (sampled, read) {
                    wrong.count += 1;
                    if wrong.first.len() < 8 {
                        wrong.first.push((c, s, r));
                    }
                }
            }
        }
        wrong.not_once += seen.iter().filter(|&&n| n != 1).count() as u32;
        gl.use_program(0);
        gl.delete_program(program);
        gl.bind_framebuffer(gles::FRAMEBUFFER, 0);
        gl.delete_framebuffers(&[framebuffer]);
        gl.bind_renderbuffer(gles::RENDERBUFFER, 0);
        gl.delete_renderbuffers(&[renderbuffer]);
        wrong
    }

    #[test]
    fn every_24_bit_word_reads_back_as_itself_where_a_device_samples_the_nearest_float() {
        assert_eq!(check(0.0), Wrong::default());
    }

    #[test]
    fn every_24_bit_word_reads_back_as_itself_but_one_where_a_device_samples_the_product() {
        assert_eq!(check(product_reciprocal()), Wrong::default());
    }
}
