// The triangle scene for WebGPU, natively through wgpu and in the browser.
struct VsOut {
    @builtin(position) pos: vec4f,
    @location(0) color: vec3f,
}

@vertex
fn vs(@location(0) pos: vec2f, @location(1) color: vec3f) -> VsOut {
    return VsOut(vec4f(pos, 0.0, 1.0), color);
}

@fragment
fn ps(i: VsOut) -> @location(0) vec4f {
    return vec4f(i.color, 1.0);
}
