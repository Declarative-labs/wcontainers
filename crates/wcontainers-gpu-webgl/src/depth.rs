//! Depth and stencil read back by drawing them into a colour texture
//! ([`wcontainers_gpu::SlowPath::DepthReadByDraw`]): WebGL 2's `readPixels` reads neither, so a
//! full-screen triangle fetches each texel and writes its stored value into an `r32uint` texture,
//! which reads back like any colour texture. Stencil is fetched as unsigned integers with
//! WEBGL_stencil_texturing.

use glow::HasContext;

/// The program for one kind of source texture, and its uniforms.
#[derive(Debug)]
pub struct DepthRead {
    pub program: glow::Program,
    /// The region's origin x and y, its layer and its level.
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

/// Fetches the texel under the fragment, moved by the region's origin, and writes its stored value.
/// Drawn with GL's own lower-left window origin, so the fragment's row is the texel's row.
fn fragment(array: bool, stencil: bool) -> String {
    let (sampler, coord) = match (array, stencil) {
        (false, false) => ("sampler2D", "p"),
        (true, false) => ("sampler2DArray", "ivec3(p, at.z)"),
        (false, true) => ("usampler2D", "p"),
        (true, true) => ("usampler2DArray", "ivec3(p, at.z)"),
    };
    let value = if stencil {
        "texelFetch(src, COORD, at.w).r"
    } else {
        "scale == 0u ? floatBitsToUint(d) : uint(d * float(scale) + 0.5)"
    }
    .replace("COORD", coord);
    format!(
        "#version 300 es
precision highp float;
precision highp int;
uniform highp {sampler} src;
uniform ivec4 at;
uniform uint scale;
out highp uvec4 o;
void main() {{
    ivec2 p = ivec2(gl_FragCoord.xy) + at.xy;
    {fetch}
    o = uvec4({value}, 0u, 0u, 0u);
}}
",
        fetch = if stencil {
            String::new()
        } else {
            format!("float d = texelFetch(src, {coord}, at.w).r;")
        },
    )
}

/// Builds the program for 2D (`array` false) or 2D array sources, of depth or of stencil. Err holds
/// GL's log.
pub fn build(gl: &glow::Context, array: bool, stencil: bool) -> Result<DepthRead, String> {
    // SAFETY: plain GL calls on the current context; the shaders are deleted once linked.
    unsafe {
        let program = gl.create_program()?;
        let mut shaders = Vec::new();
        for (stage, source) in [
            (glow::VERTEX_SHADER, VERTEX.to_string()),
            (glow::FRAGMENT_SHADER, fragment(array, stencil)),
        ] {
            let s = gl.create_shader(stage)?;
            gl.shader_source(s, &source);
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
        let at = gl
            .get_uniform_location(program, "at")
            .ok_or("the depth read program has no uniform at")?;
        let scale = gl.get_uniform_location(program, "scale");
        Ok(DepthRead { program, at, scale })
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
