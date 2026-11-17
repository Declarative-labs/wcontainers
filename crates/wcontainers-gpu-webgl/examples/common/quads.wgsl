// The quads scene: textured, coloured quads, as quads.hlsl of the graphics rig.
@group(1) @binding(32) var tex: texture_2d<f32>;
@group(1) @binding(16) var smp: sampler;

struct VsOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
    @location(1) color: vec4f,
}

@vertex
fn vs(@location(0) pos: vec3f, @location(1) uv: vec2f, @location(2) color: vec4f) -> VsOut {
    return VsOut(vec4f(pos, 1.0), uv, color);
}

@fragment
fn ps(i: VsOut) -> @location(0) vec4f {
    return textureSample(tex, smp, i.uv) * i.color;
}
