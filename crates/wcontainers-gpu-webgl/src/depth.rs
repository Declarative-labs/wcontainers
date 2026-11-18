//! Depth and stencil read back by drawing them into a colour texture
//! ([`wcontainers_gpu::SlowPath::DepthReadByDraw`]): WebGL 2's `readPixels` reads neither, so a
//! full-screen triangle fetches each texel and writes its stored value into an `r32uint` texture,
//! which reads back like any colour texture. Stencil is fetched as unsigned integers with
//! WEBGL_stencil_texturing; without it, eight draws count its bits into an `r8unorm` texture first
//! ([`wcontainers_gpu::SlowPath::StencilCopiedByDraw`]), which the same fetch reads with a scale of 255.

use glow::HasContext;

/// The program for one kind of source texture, and its uniforms.
#[derive(Debug)]
pub struct DepthRead {
    pub program: glow::Program,
    /// The region's origin x and y, and its layer; the level is the texture's base level, which the
    /// draw sets to the region's.
    pub at: glow::UniformLocation,
    /// What a depth of 1 is stored as (65535, 16777215), or 0 to keep a float's bits; None for
    /// stencil.
    pub scale: Option<glow::UniformLocation>,
}

/// A triangle covering the target, from the vertex number alone.
const VERTEX: &str = "#version 300 es
void main() {
    gl_Position = vec4(float((gl_VertexID & 1) << 2) - 1.0, float((gl_VertexID & 2) << 1) - 1.0, 0.0, 1.0);
}
";

/// The direction from a cube's centre to the centre of texel `t` of face `at.z` (GL's face order
/// and texel axes), for programs sampling a cube `src` towards a texel.
pub const TOWARDS: &str = "vec3 towards(ivec2 t) {
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
pub enum Shape {
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
        // adding 0.5 before truncating would round again (or not at all, fused).
        "scale == 0u ? floatBitsToUint(d) : min(uint(round(d * float(scale))), scale)"
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
{towards}void main() {{
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
pub fn build(gl: &glow::Context, shape: Shape, stencil: bool) -> Result<DepthRead, String> {
    let program = link(gl, &fragment(shape, stencil))?;
    // SAFETY: plain queries of a linked program.
    unsafe {
        let at = gl
            .get_uniform_location(program, "at")
            .ok_or("the depth read program has no uniform at")?;
        let scale = gl.get_uniform_location(program, "scale");
        Ok(DepthRead { program, at, scale })
    }
}

/// The program writing stencil from an `r8uint` texture, and its uniforms.
#[derive(Debug)]
pub struct StencilWrite {
    pub program: glow::Program,
    /// As [`DepthRead::at`]: the source's offset from the fragment, and its layer.
    pub at: glow::UniformLocation,
    /// The bit drawn: fragments whose source value lacks it are discarded; 0 discards none.
    pub bit: glow::UniformLocation,
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
pub fn build_stencil_write(gl: &glow::Context, array: bool) -> Result<StencilWrite, String> {
    let program = link(gl, &stencil_write(array))?;
    // SAFETY: plain queries of a linked program.
    unsafe {
        let at = gl.get_uniform_location(program, "at");
        let bit = gl.get_uniform_location(program, "bit");
        match (at, bit) {
            (Some(at), Some(bit)) => Ok(StencilWrite { program, at, bit }),
            _ => Err("the stencil write program lacks a uniform".into()),
        }
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
pub fn build_probe(gl: &glow::Context) -> Result<(glow::Program, glow::UniformLocation), String> {
    let program = link_with(gl, PROBE, ONE)?;
    // SAFETY: a plain query of a linked program.
    let z = unsafe { gl.get_uniform_location(program, "z") };
    Ok((program, z.ok_or("the probe program has no uniform z")?))
}

/// Builds the program the stencil bit draws use. Err holds GL's log.
pub fn build_bits(gl: &glow::Context) -> Result<glow::Program, String> {
    link(gl, ONE)
}

/// Links the full-screen vertex program with `fragment`. Err holds GL's log.
pub fn link(gl: &glow::Context, fragment: &str) -> Result<glow::Program, String> {
    link_with(gl, VERTEX, fragment)
}

fn link_with(gl: &glow::Context, vertex: &str, fragment: &str) -> Result<glow::Program, String> {
    // SAFETY: plain GL calls on the current context; the shaders are deleted once linked.
    unsafe {
        let program = gl.create_program()?;
        let mut shaders = Vec::new();
        for (stage, source) in [
            (glow::VERTEX_SHADER, vertex),
            (glow::FRAGMENT_SHADER, fragment),
        ] {
            let s = gl.create_shader(stage)?;
            gl.shader_source(s, source);
            gl.compile_shader(s);
            if !gl.get_shader_compile_status(s) {
                return Err(gl.get_shader_info_log(s));
            }
            gl.attach_shader(program, s);
            shaders.push(s);
        }
        gl.link_program(program);
        for s in shaders {
            gl.detach_shader(program, s);
            gl.delete_shader(s);
        }
        if !gl.get_program_link_status(program) {
            return Err(gl.get_program_info_log(program));
        }
        Ok(program)
    }
}

/// WEBGL_stencil_texturing's texture parameter and its two values.
pub const DEPTH_STENCIL_TEXTURE_MODE: u32 = 0x90EA;
pub const STENCIL_INDEX: u32 = 0x1901;
pub const DEPTH_COMPONENT: u32 = 0x1902;

/// What the stored depth of `format` is read as: the scale of [`DepthRead::scale`] and the bytes each
/// texel keeps (Direct3D's: 16-bit unorm, 24-bit unorm in the low bits of a 32-bit word, 32-bit float).
pub fn stored(format: wcontainers_gpu::Format) -> Option<(u32, usize)> {
    use wcontainers_gpu::Format as F;
    match format {
        F::Depth16Unorm => Some((65535, 2)),
        F::Depth24Plus | F::Depth24PlusStencil8 => Some((16_777_215, 4)),
        F::Depth32Float | F::Depth32FloatStencil8 => Some((0, 4)),
        _ => None,
    }
}
