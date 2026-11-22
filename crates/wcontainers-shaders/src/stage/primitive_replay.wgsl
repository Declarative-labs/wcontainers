// Each API primitive's vertices again on a back end without compute, for capture: one invocation per
// vertex of each primitive, its vertex record read from the records texture in the capture's order
// (strips written as lists, instance after instance). With cull distances (Direct3D drops a
// primitive when all its vertices are below zero on one of them) a dropped primitive's vertices are
// written as the empty record, a position outside every clip volume and zeros; numbered, each record
// is followed by a slot holding the primitive's number in its instance (Direct3D's SV_PrimitiveID).
// `SLOTS` (four-word slots of a written record: the position, the locations, the clip distances and
// the number's slot if numbered) and `RECORD_SLOTS` (those of a captured record: the position, the
// locations, the clip distances and the cull distances) are declared before this text, the entry
// point after it.

struct Values {
    // Vertices per primitive: 1, 2 or 3.
    vertices: u32,
    // Cull distances, from the first word of the slot after the clip distances of a captured record.
    cull_count: u32,
    // API primitives per instance.
    primitives: u32,
    // 1 when each record is followed by the number's slot.
    numbered: u32,
}

@group(1) @binding(0) var<uniform> values: Values;
@group(1) @binding(2) var records: texture_2d<u32>;

const SLOT_WORDS: u32 = 4u;
// The position's w, which no clip volume holds.
const OUTSIDE_W: f32 = -1.0;

// Slot `s` of captured record `record`.
fn slot(record: u32, s: u32) -> vec4<u32> {
    let at = record * RECORD_SLOTS + s;
    let width = textureDimensions(records).x;
    return textureLoad(records, vec2<u32>(at % width, at / width), 0);
}

fn replayed(vertex: u32) -> array<vec4<u32>, SLOTS> {
    let n = values.vertices;
    let unit = vertex / n;
    let first = unit * n;
    // Slots copied from the record; the cull distances follow them.
    let copied = SLOTS - values.numbered;
    var dropped = false;
    for (var d = 0u; d < values.cull_count; d++) {
        var below = true;
        for (var k = 0u; k < n; k++) {
            let words = slot(first + k, copied + d / SLOT_WORDS);
            below = below && bitcast<f32>(words[d % SLOT_WORDS]) < 0.0;
        }
        dropped = dropped || below;
    }
    var out: array<vec4<u32>, SLOTS>;
    if dropped {
        out[0] = bitcast<vec4<u32>>(vec4<f32>(0.0, 0.0, 0.0, OUTSIDE_W));
        return out;
    }
    for (var s = 0u; s < copied; s++) {
        out[s] = slot(vertex, s);
    }
    if values.numbered != 0u {
        out[SLOTS - 1u] = vec4<u32>(unit % values.primitives, 0u, 0u, 0u);
    }
    return out;
}
