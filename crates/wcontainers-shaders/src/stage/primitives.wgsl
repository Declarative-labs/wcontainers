// Primitive numbers for a pixel program that reads its primitive's: one invocation per API primitive
// copies its vertices' records, in the API's order, into a record each of its own, followed by a
// slot holding the primitive's number in its instance (Direct3D's SV_PrimitiveID). Triangles of a
// strip are listed with their first vertex leading and their winding kept, as the cull program does.

struct Values {
    // API primitives times API instances.
    units: u32,
    // API primitives per instance.
    primitives: u32,
    // API vertices per instance: records per instance.
    count: u32,
    // Vertices per primitive: 1, 2 or 3.
    vertices: u32,
    // Primitives come from a strip.
    strip: u32,
    // Words per vertex record, and per copied record (one slot more).
    in_words: u32,
    out_words: u32,
    pad: u32,
}

@group(1) @binding(0) var<uniform> values: Values;
@group(1) @binding(1) var<storage, read> records: array<u32>;
@group(1) @binding(2) var<storage, read_write> copies: array<u32>;

const TRIANGLE_VERTICES: u32 = 3u;
const SLOT_WORDS: u32 = 4u;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let unit = id.x;
    if unit >= values.units {
        return;
    }
    let instance = unit / values.primitives;
    let primitive = unit % values.primitives;
    let n = values.vertices;
    let strip = values.strip != 0u;
    var first = primitive * n;
    if strip {
        first = primitive;
    }
    for (var k = 0u; k < n; k++) {
        var v = first + k;
        // An odd triangle of a strip: (first, first + 2, first + 1).
        if strip && n == TRIANGLE_VERTICES && (primitive & 1u) == 1u && k != 0u {
            v = first + TRIANGLE_VERTICES - k;
        }
        let from = (instance * values.count + v) * values.in_words;
        let to = (unit * n + k) * values.out_words;
        for (var w = 0u; w < values.in_words; w++) {
            copies[to + w] = records[from + w];
        }
        copies[to + values.in_words] = primitive;
        for (var w = 1u; w < SLOT_WORDS; w++) {
            copies[to + values.in_words + w] = 0u;
        }
    }
}
