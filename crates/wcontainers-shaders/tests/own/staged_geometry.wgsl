// gs_cut of geometry.hlsl as another route hands it to `staged::geometry`: its stage values private
// variables, each vertex it emits a call of `emit` and each strip's end a call of `cut`, both empty
// and declared before the program's own function.

var<private> position_in: array<vec4<f32>, 3>;
var<private> colour_in: array<vec4<f32>, 3>;
var<private> ids_in: array<vec2<u32>, 3>;
var<private> primitive: u32;
var<private> position_out: vec4<f32>;
var<private> colour: vec4<f32>;
var<private> ids: vec4<u32>;

fn emit() {}

fn cut() {}

fn gs_cut() {
    let copies = primitive % 3u;
    let centre = (position_in[0] + position_in[1] + position_in[2]) / 3.0;
    for (var c = 0u; c < copies; c++) {
        let scale = 1.0 - f32(c) * 0.4;
        for (var i = 0u; i < 3u; i++) {
            position_out = mix(centre, position_in[i], scale);
            colour = colour_in[i] * vec4(1.0, scale, 1.0, 1.0);
            ids = vec4(ids_in[i], primitive, c);
            emit();
        }
        if c == 1u {
            position_out = centre + vec4(0.0, -0.2, 0.0, 0.0);
            colour = vec4(1.0, 1.0, 0.0, 1.0);
            ids = vec4(ids_in[0], primitive, 7u);
            emit();
        }
        cut();
    }
}
