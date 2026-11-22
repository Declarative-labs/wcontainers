// Alpha-antialiased lines on a back end without compute: one invocation per corner of the two
// triangles of the rectangle around each API line, as `lines.wgsl` writes them, for capture. The lines'
// vertex records are read from the records texture, two a line in list order (the capture writes
// strips as lists, instance after instance). `SLOTS` (four-word slots of a corner record: the vertex
// record's, then the coverage slot) is declared before this text, the entry point after it.

struct Values {
    // Slots after the position taken along the line (bit i: slot 1 + i), and those of them taken
    // without perspective.
    interpolated: u32,
    linear: u32,
    // 1 where the records' clip depth is rewritten as `2z - w` (`CLIP_DEPTH_REWRITE`), whose near
    // plane is then z = -w; 0 otherwise.
    near_w: f32,
    pad: u32,
    // Half the viewport's width and height, in pixels.
    half_width: f32,
    half_height: f32,
    // How far the rectangle reaches past the line's axis and past each end, in pixels.
    reach_across: f32,
    reach_along: f32,
}

@group(1) @binding(0) var<uniform> values: Values;
@group(1) @binding(2) var records: texture_2d<u32>;

const VERTICES: u32 = 2u;
// The corners' order in the two triangles.
const TRIANGLES: array<u32, 6> = array<u32, 6>(0u, 1u, 2u, 2u, 1u, 3u);
const TRIANGLE_INDICES: u32 = 6u;

// Slot `s` of vertex record `record`, of `SLOTS - 1` slots each.
fn slot(record: u32, s: u32) -> vec4<u32> {
    let at = record * (SLOTS - 1u) + s;
    let width = textureDimensions(records).x;
    return textureLoad(records, vec2<u32>(at % width, at / width), 0);
}

fn line_corner(vertex: u32) -> array<vec4<u32>, SLOTS> {
    let unit = vertex / TRIANGLE_INDICES;
    let triangles = TRIANGLES;
    let k = triangles[vertex % TRIANGLE_INDICES];
    let r0 = unit * VERTICES;
    let r1 = r0 + 1u;
    let p0 = bitcast<vec4<f32>>(slot(r0, 0u));
    let p1 = bitcast<vec4<f32>>(slot(r1, 0u));

    // The part in front of the near plane, from t0 to t1 of the line in clip space.
    var t0 = 0.0;
    var t1 = 1.0;
    let near0 = p0.z + values.near_w * p0.w;
    let near1 = p1.z + values.near_w * p1.w;
    let behind0 = near0 < 0.0;
    let behind1 = near1 < 0.0;
    if behind0 != behind1 {
        let cut = near0 / (near0 - near1);
        if behind0 {
            t0 = cut;
        } else {
            t1 = cut;
        }
    }
    let q0 = mix(p0, p1, t0);
    let q1 = mix(p0, p1, t1);
    let half_size = vec2<f32>(values.half_width, values.half_height);
    let a = q0.xy / q0.w * half_size;
    let b = q1.xy / q1.w * half_size;
    let extent = distance(a, b);
    let dropped = (behind0 && behind1) || q0.w <= 0.0 || q1.w <= 0.0 || !(extent > 0.0);
    var along = vec2<f32>(1.0, 0.0);
    if !dropped {
        along = (b - a) / extent;
    }
    let across = vec2<f32>(-along.y, along.x);

    let end = k / 2u;
    let side = f32(k % 2u) * 2.0 - 1.0;
    // The point beside the corner: s of the way from q0 to q1 on the screen, t in clip space.
    var pixels = -values.reach_along;
    if end == 1u {
        pixels = extent + values.reach_along;
    }
    var s = 0.0;
    var t = 0.0;
    if !dropped {
        s = pixels / extent;
        t = s * q0.w / (s * q0.w + (1.0 - s) * q1.w);
    }
    let point = mix(q0, q1, t);
    var moved = vec2<f32>(0.0);
    if !dropped {
        moved = across * side * values.reach_across / half_size * point.w;
    }
    var out: array<vec4<u32>, SLOTS>;
    out[0] = bitcast<vec4<u32>>(vec4<f32>(point.xy + moved, point.zw));
    // Along the API line: perspective-correct values at t, the others at s, of the kept part.
    let perspective = mix(t0, t1, t);
    let linear = mix(t0, t1, s);
    for (var i = 1u; i < SLOTS - 1u; i++) {
        let bit = 1u << (i - 1u);
        var words = slot(r0, i);
        if (values.interpolated & bit) != 0u {
            let u = select(perspective, linear, (values.linear & bit) != 0u);
            let v0 = bitcast<vec4<f32>>(words);
            let v1 = bitcast<vec4<f32>>(slot(r1, i));
            words = bitcast<vec4<u32>>(mix(v0, v1, vec4<f32>(u)));
        }
        out[i] = words;
    }
    out[SLOTS - 1u] = bitcast<vec4<u32>>(vec4<f32>(side * values.reach_across, pixels, extent, 0.0));
    return out;
}
