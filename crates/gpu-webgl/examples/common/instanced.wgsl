// The instanced scene: one small triangle placed and coloured by instance from a uniform buffer, as
// instanced.hlsl of the graphics rig.
struct Scene {
    placement: array<vec4f, 16>, // offset x, y; cosine and sine of the turn
    colors: array<vec4f, 16>,
    scale: vec4f,
}
@group(0) @binding(0) var<uniform> scene: Scene;

struct VsOut {
    @builtin(position) pos: vec4f,
    @location(0) color: vec4f,
}

@vertex
fn vs(@location(0) pos: vec2f, @builtin(instance_index) id: u32) -> VsOut {
    let p = scene.placement[id];
    let turned = vec2f(pos.x * p.z - pos.y * p.w, pos.x * p.w + pos.y * p.z);
    return VsOut(vec4f(turned * scene.scale.xy + p.xy, 0.0, 1.0), scene.colors[id]);
}

@fragment
fn ps(i: VsOut) -> @location(0) vec4f {
    return i.color;
}
