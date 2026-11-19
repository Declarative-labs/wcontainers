// Alpha-antialiased lines: one invocation per API line writes the four corners of a rectangle around
// it, reaching past its axis and its ends by the coverage filter's reach, and the two triangles'
// indices. A corner record is the line's vertex record at the point the corner stands beside (its
// interpolated slots taken along the line there, the others the line's first vertex's), its position
// moved across the line, then the coverage slot: the distance from the line's axis, the distance
// along it from its first end, and its length, in pixels. The line is first cut where it crosses the
// near plane (clip z = 0); a line wholly behind it, or of no length on the screen, gets four equal
// corners.

struct Values {
    // API lines times API instances.
    units: u32,
    // API lines per instance.
    primitives: u32,
    // API vertices per instance: records per instance.
    count: u32,
    // The lines come from a strip.
    strip: u32,
    // Words per vertex record, and per corner record (one slot more).
    in_words: u32,
    out_words: u32,
    // The word of the output where the index list starts.
    index_base: u32,
    // Slots after the position taken along the line (bit i: slot 1 + i), and those of them taken
    // without perspective.
    interpolated: u32,
    linear: u32,
    pad: u32,
    // Half the viewport's width and height, in pixels.
    half_width: f32,
    half_height: f32,
    // How far the rectangle reaches past the line's axis and past each end, in pixels.
    reach_across: f32,
    reach_along: f32,
    pad1: u32,
    pad2: u32,
}

@group(1) @binding(0) var<uniform> values: Values;
@group(1) @binding(1) var<storage, read> records: array<u32>;
@group(1) @binding(2) var<storage, read_write> corners: array<u32>;

const SLOT_WORDS: u32 = 4u;
const CORNERS: u32 = 4u;
const VERTICES: u32 = 2u;
// The corners' order in the two triangles.
const TRIANGLES: array<u32, 6> = array<u32, 6>(0u, 1u, 2u, 2u, 1u, 3u);
const TRIANGLE_INDICES: u32 = 6u;

fn position(record: u32) -> vec4<f32> {
    let at = record * values.in_words;
    return bitcast<vec4<f32>>(vec4<u32>(records[at], records[at + 1u], records[at + 2u], records[at + 3u]));
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let unit = id.x;
    if unit >= values.units {
        return;
    }
    let instance = unit / values.primitives;
    let line = unit % values.primitives;
    var first = line * VERTICES;
    if values.strip != 0u {
        first = line;
    }
    let r0 = instance * values.count + first;
    let r1 = r0 + 1u;
    let p0 = position(r0);
    let p1 = position(r1);

    // The part in front of the near plane, from t0 to t1 of the line in clip space.
    var t0 = 0.0;
    var t1 = 1.0;
    let behind0 = p0.z < 0.0;
    let behind1 = p1.z < 0.0;
    if behind0 != behind1 {
        let cut = p0.z / (p0.z - p1.z);
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

    let base = unit * CORNERS;
    for (var k = 0u; k < CORNERS; k++) {
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
        let at = (base + k) * values.out_words;
        let placed = bitcast<vec4<u32>>(vec4<f32>(point.xy + moved, point.zw));
        for (var c = 0u; c < SLOT_WORDS; c++) {
            corners[at + c] = placed[c];
        }
        // Along the API line: perspective-correct values at t, the others at s, of the kept part.
        let perspective = mix(t0, t1, t);
        let linear = mix(t0, t1, s);
        for (var w = SLOT_WORDS; w < values.in_words; w++) {
            let bit = 1u << (w / SLOT_WORDS - 1u);
            var word = records[r0 * values.in_words + w];
            if (values.interpolated & bit) != 0u {
                let u = select(perspective, linear, (values.linear & bit) != 0u);
                let v0 = bitcast<f32>(word);
                let v1 = bitcast<f32>(records[r1 * values.in_words + w]);
                word = bitcast<u32>(mix(v0, v1, u));
            }
            corners[at + w] = word;
        }
        let coverage = vec4<f32>(side * values.reach_across, pixels, extent, 0.0);
        let words = bitcast<vec4<u32>>(coverage);
        for (var c = 0u; c < SLOT_WORDS; c++) {
            corners[at + values.in_words + c] = words[c];
        }
    }
    for (var i = 0u; i < TRIANGLE_INDICES; i++) {
        corners[values.index_base + unit * TRIANGLE_INDICES + i] = base + TRIANGLES[i];
    }
}
