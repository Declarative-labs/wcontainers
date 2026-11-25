// Cull distances a geometry shader writes (Direct3D drops a primitive when all its vertices are below
// zero on one of them): one invocation per geometry invocation walks the drawn stream's index slots
// (strips ended by the restart index) and lists each primitive's vertices in the strip's order, or
// the empty record's number for each when the primitive is dropped; its list slots past its
// primitives hold the empty record's. Triangles of a strip are listed with their first vertex leading
// and their winding kept. Invocation 0 also writes the empty record: a position outside every clip
// volume and zeros.

struct Values {
    // Geometry invocations.
    invocations: u32,
    // Index slots per invocation.
    index_slots: u32,
    // List slots per invocation, each a primitive's.
    primitives: u32,
    // Vertices per primitive: 1, 2 or 3.
    vertices: u32,
    // Words per record.
    record_words: u32,
    // The word of a record where the cull distances start, and how many there are.
    cull_word: u32,
    cull_count: u32,
    // The word where the drawn stream's index slots start, and where the list starts.
    index_base: u32,
    list_base: u32,
    // The empty record's number.
    empty: u32,
    pad0: u32,
    pad1: u32,
}

// The records, the index slots, then the list.
@group(1) @binding(0) var<uniform> values: Values;
@group(1) @binding(1) var<storage, read_write> data: array<u32>;

const RESTART: u32 = 0xffffffffu;
const MOST_VERTICES: u32 = 3u;
// The position's w, which no clip volume holds.
const OUTSIDE_W: f32 = -1.0;
const W_COMPONENT: u32 = 3u;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let unit = id.x;
    if unit == 0u {
        let empty = values.empty * values.record_words;
        for (var w = 0u; w < values.record_words; w++) {
            data[empty + w] = 0u;
        }
        data[empty + W_COMPONENT] = bitcast<u32>(OUTSIDE_W);
    }
    if unit >= values.invocations {
        return;
    }
    let n = values.vertices;
    let first = values.index_base + unit * values.index_slots;
    let list = values.list_base + unit * values.primitives * n;
    // The strip's last vertices, the newest last, and how many it has so far.
    var newest: array<u32, MOST_VERTICES>;
    var run = 0u;
    var listed = 0u;
    for (var i = 0u; i < values.index_slots; i++) {
        let v = data[first + i];
        if v == RESTART {
            run = 0u;
            continue;
        }
        newest[0] = newest[1];
        newest[1] = newest[2];
        newest[2] = v;
        run++;
        if run < n || listed == values.primitives {
            continue;
        }
        var records: array<u32, MOST_VERTICES>;
        for (var k = 0u; k < n; k++) {
            records[k] = newest[MOST_VERTICES - n + k];
        }
        // An odd triangle of a strip: (first, first + 2, first + 1).
        if n == MOST_VERTICES && ((run - n) & 1u) == 1u {
            let second = records[1];
            records[1] = records[2];
            records[2] = second;
        }
        var dropped = false;
        for (var d = 0u; d < values.cull_count; d++) {
            var below = true;
            for (var k = 0u; k < n; k++) {
                let at = records[k] * values.record_words + values.cull_word + d;
                below = below && bitcast<f32>(data[at]) < 0.0;
            }
            dropped = dropped || below;
        }
        for (var k = 0u; k < n; k++) {
            data[list + listed * n + k] = select(records[k], values.empty, dropped);
        }
        listed++;
    }
    for (; listed < values.primitives; listed++) {
        for (var k = 0u; k < n; k++) {
            data[list + listed * n + k] = values.empty;
        }
    }
}
