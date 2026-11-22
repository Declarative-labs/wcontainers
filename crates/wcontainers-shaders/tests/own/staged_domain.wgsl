// The domain shaders of tessellation.hlsl as another route hands them to `staged::domain`: inputs
// private variables (the domain location as OpenGL's three coordinates), outputs the entry point's
// own. One entry point per domain; the test keeps the one it hands over.

var<private> coordinate: vec3<f32>;
var<private> primitive: u32;

struct Streamed {
    @builtin(position) position: vec4<f32>,
    @location(1) location: vec3<f32>,
    @location(2) @interpolate(flat) primitive: u32,
}

@vertex
fn ds_tri() -> Streamed {
    return Streamed(vec4(coordinate, 1.0), coordinate, primitive);
}

@vertex
fn ds_quad() -> Streamed {
    return Streamed(vec4(coordinate.xy, 0.0, 1.0), vec3(coordinate.xy, 0.0), primitive);
}

@vertex
fn ds_line() -> Streamed {
    return Streamed(vec4(coordinate.xy, 0.0, 1.0), vec3(coordinate.xy, 0.0), primitive);
}
