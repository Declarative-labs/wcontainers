// The hull shaders of tessellation.hlsl as another route hands them to `staged::hull`: their stage
// values private variables, the control point function passing its one control point on, and a patch
// constant function per domain taking the patch's factors from the constant buffer by its primitive
// number. The tessellation factors are arrays as OpenGL declares them; the constant buffer's binding
// is set from the route's reflection.

struct Factors {
    factors: array<vec4<f32>, 64>,
}

@group(0) @binding(0) var<uniform> constants: Factors;

var<private> id_in: array<u32, 1>;
var<private> id_out: array<u32, 1>;
var<private> point: u32;
var<private> primitive: u32;
var<private> outer: array<f32, 4>;
var<private> inner: array<f32, 2>;

fn control_point() {
    id_out[point] = id_in[0];
}

fn tri_factors() {
    let edge = constants.factors[2u * primitive];
    outer[0] = edge.x;
    outer[1] = edge.y;
    outer[2] = edge.z;
    inner[0] = constants.factors[2u * primitive + 1u].x;
}

fn quad_factors() {
    let edge = constants.factors[2u * primitive];
    let inside = constants.factors[2u * primitive + 1u];
    outer[0] = edge.x;
    outer[1] = edge.y;
    outer[2] = edge.z;
    outer[3] = edge.w;
    inner[0] = inside.x;
    inner[1] = inside.y;
}

fn line_factors() {
    let edge = constants.factors[2u * primitive];
    outer[0] = edge.x;
    outer[1] = edge.y;
}
