// Stream output (Direct3D's stream output stage) as one workgroup: the units of work (geometry
// invocations, or the API's primitives when no geometry shader runs, or the primitives a tessellation
// stage listed) are split into 256 runs; each
// counts its runs' whole primitives, a scan over the workgroup gives each run its first primitive in
// the stream, and each writes its primitives' vertices into the buffers, element by element, in the
// order the stream has them. Writing stops for every buffer at the first primitive one of them cannot
// hold, and nothing is written where no buffer of the stream is bound; the filled sizes advance by
// what was written. A geometry shader's streams run one after another, each with its own buffers.

struct Entry {
    // Buffer (bits 0 to 7), first component (8 to 15), component count (16 to 23).
    packed: u32,
    // Bytes from the start of the vertex in its buffer.
    offset: u32,
    // The record's four-word slot the components come from.
    slot: u32,
    pad: u32,
}

struct Values {
    // Geometry invocations, or primitives times API instances.
    units: u32,
    // 1: units are geometry invocations, whose primitives are in their index slots.
    geometry: u32,
    // Vertices per primitive: 1, 2 or 3.
    vertices: u32,
    // Primitives come from a strip (units are API primitives).
    strip: u32,
    // Index slots per geometry invocation.
    slots: u32,
    // The word where index slots start.
    index_base: u32,
    // Words per record.
    record_words: u32,
    entries: u32,
    // API primitives per instance, and API vertices per instance (units are API primitives).
    primitives: u32,
    count: u32,
    // 1: units are the primitives listed in `list` (their index total first, then from word 4 each
    // primitive's records); `units` is then unused.
    listed: u32,
    // The stream: its statistics go to words 4 + 2 * stream and 5 + 2 * stream of `filled`.
    stream: u32,
    // 1: units are API primitives of an indexed strip made a list, those of each instance past the
    // index total in the first word of `list` holding none.
    listed_total: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
    // Bytes per vertex in each buffer; 0 where no buffer is bound.
    strides: vec4<u32>,
    // Bytes of each bound buffer.
    sizes: vec4<u32>,
    table: array<Entry, 64>,
}

@group(1) @binding(0) var<uniform> so: Values;
@group(1) @binding(1) var<storage, read> data: array<u32>;
// The four buffers' filled sizes in bytes, then for each stream the primitives written and those the
// draw made.
@group(1) @binding(2) var<storage, read_write> filled: array<u32>;
@group(1) @binding(3) var<storage, read_write> out0: array<u32>;
@group(1) @binding(4) var<storage, read_write> out1: array<u32>;
@group(1) @binding(5) var<storage, read_write> out2: array<u32>;
@group(1) @binding(6) var<storage, read_write> out3: array<u32>;
@group(1) @binding(7) var<storage, read> list: array<u32>;

const LIST_HEADER: u32 = 4u;

var<workgroup> scan: array<u32, 256>;
var<workgroup> capacity: u32;
var<workgroup> start: array<u32, 4>;

fn store(buffer: u32, index: u32, value: u32) {
    switch buffer {
        case 0u: { out0[index] = value; }
        case 1u: { out1[index] = value; }
        case 2u: { out2[index] = value; }
        default: { out3[index] = value; }
    }
}

// Writes record `record` as vertex `at` of the stream.
fn write_vertex(record: u32, at: u32) {
    for (var e = 0u; e < so.entries; e++) {
        let entry = so.table[e];
        let buffer = entry.packed & 0xffu;
        let first = (entry.packed >> 8u) & 0xffu;
        let count = (entry.packed >> 16u) & 0xffu;
        let byte = start[buffer] + at * so.strides[buffer] + entry.offset;
        let source = record * so.record_words + entry.slot * 4u + first;
        for (var c = 0u; c < count; c++) {
            store(buffer, byte / 4u + c, data[source + c]);
        }
    }
}

// Writes primitive `index` of the stream if every buffer holds it.
fn write_primitive(index: u32, records: vec3<u32>) {
    if index >= capacity {
        return;
    }
    // Components named, not indexed by a loop: Direct3D's shader compiler cannot unroll that loop.
    let at = index * so.vertices;
    write_vertex(records.x, at);
    if so.vertices > 1u {
        write_vertex(records.y, at + 1u);
    }
    if so.vertices > 2u {
        write_vertex(records.z, at + 2u);
    }
}

// The records of triangle (or line, or point) `i` of a strip whose last three vertices are a, b, c:
// odd triangles take their last two vertices swapped, (i, i + 2, i + 1).
fn strip_primitive(i: u32, a: u32, b: u32, c: u32) -> vec3<u32> {
    var r = vec3<u32>(a, b, c);
    if so.vertices == 1u {
        r = vec3<u32>(c, 0u, 0u);
    } else if so.vertices == 2u {
        r = vec3<u32>(b, c, 0u);
    } else if (i & 1u) != 0u {
        r = vec3<u32>(a, c, b);
    }
    return r;
}

// Walks a geometry invocation's index slots; writes its primitives from stream primitive `first` when
// `write`, and returns how many it has.
fn walk(unit: u32, first: u32, write: bool) -> u32 {
    var length = 0u;
    var primitives = 0u;
    var a = 0u;
    var b = 0u;
    let base = so.index_base + unit * so.slots;
    for (var s = 0u; s < so.slots; s++) {
        let v = data[base + s];
        if v == 0xffffffffu {
            length = 0u;
            continue;
        }
        length += 1u;
        if length >= so.vertices {
            if write {
                write_primitive(first + primitives, strip_primitive(length - so.vertices, a, b, v));
            }
            primitives += 1u;
        }
        a = b;
        b = v;
    }
    return primitives;
}

// The records of API primitive `unit` (primitive-major within each instance), or of listed
// primitive `unit`.
fn api_primitive(unit: u32) -> vec3<u32> {
    if so.listed != 0u {
        let at = LIST_HEADER + unit * so.vertices;
        let last = LIST_HEADER + list[0] - 1u;
        return vec3<u32>(list[at], list[min(at + 1u, last)], list[min(at + 2u, last)]);
    }
    let instance = unit / so.primitives;
    let p = unit % so.primitives;
    let first = instance * so.count;
    if so.strip == 0u {
        return vec3<u32>(first + p * so.vertices) + vec3<u32>(0u, 1u, 2u);
    }
    if so.vertices == 3u && (p & 1u) != 0u {
        return vec3<u32>(first + p) + vec3<u32>(0u, 2u, 1u);
    }
    return vec3<u32>(first + p) + vec3<u32>(0u, 1u, 2u);
}

// Whether API primitive `unit` is one of the draw's: false past an indexed strip's listed primitives.
fn present(unit: u32) -> bool {
    return so.listed_total == 0u || unit % so.primitives < list[0] / so.vertices;
}

// One step of the workgroup's inclusive scan. The steps are written out, not looped: Direct3D's
// shader compiler unrolls a loop holding a barrier and cannot unroll the form it gets.
fn scan_step(t: u32, d: u32) {
    let add = select(0u, scan[max(t, d) - d], t >= d);
    workgroupBarrier();
    scan[t] += add;
    workgroupBarrier();
}

@compute @workgroup_size(256)
fn main(@builtin(local_invocation_index) t: u32) {
    if t == 0u {
        var room = 0xffffffffu;
        var bound = false;
        for (var b = 0u; b < 4u; b++) {
            start[b] = filled[b];
            let stride = so.strides[b] * so.vertices;
            if stride != 0u {
                room = min(room, select(0u, (so.sizes[b] - start[b]) / stride, so.sizes[b] > start[b]));
                bound = true;
            }
        }
        capacity = select(0u, room, bound);
    }
    // Each invocation takes a run of units, counts their primitives, and after the scan writes them
    // from the primitives of the runs before it.
    let units = select(so.units, list[0] / so.vertices, so.listed != 0u);
    let per = (units + 255u) / 256u;
    let begin = min(t * per, units);
    let end = min(begin + per, units);
    var n = 0u;
    for (var unit = begin; unit < end; unit++) {
        n += select(select(0u, 1u, present(unit)), walk(unit, 0u, false), so.geometry != 0u);
    }
    scan[t] = n;
    workgroupBarrier();
    scan_step(t, 1u);
    scan_step(t, 2u);
    scan_step(t, 4u);
    scan_step(t, 8u);
    scan_step(t, 16u);
    scan_step(t, 32u);
    scan_step(t, 64u);
    scan_step(t, 128u);
    var at = scan[t] - n;
    for (var unit = begin; unit < end; unit++) {
        if so.geometry != 0u {
            at += walk(unit, at, true);
        } else if present(unit) {
            write_primitive(at, api_primitive(unit));
            at += 1u;
        }
    }
    if t == 0u {
        let made = scan[255];
        let written = min(made, capacity);
        for (var b = 0u; b < 4u; b++) {
            filled[b] = start[b] + written * so.vertices * so.strides[b];
        }
        filled[4u + 2u * so.stream] = written;
        filled[5u + 2u * so.stream] = made;
    }
}
