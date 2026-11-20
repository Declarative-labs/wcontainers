// An indexed strip as an indexed list, as one workgroup: the draw's index positions are split into 256
// runs; each counts the primitives starting in its run (those whose vertices hold no restart index),
// a scan over the workgroup gives each run its first primitive and the strip its last run's restart
// left it in, and each writes its primitives' indices in the API's order. A restart index ends a
// strip; the triangles of each strip alternate, odd ones taking their last two vertices swapped
// (i, i + 2, i + 1), which keeps the winding and the first vertex. The last invocation writes the
// listed indices' total in word 0.

struct Values {
    // The API draw's indices, and its first index.
    count: u32,
    first: u32,
    // Bytes per index, 2 or 4.
    index_bytes: u32,
    // Bytes from the bound index words' start to the API's index buffer offset.
    index_offset: u32,
    // Vertices per primitive: 2, 3 or 4.
    vertices: u32,
    // The index that ends a strip.
    restart: u32,
    // The word of the list where the first index goes.
    header: u32,
    pad: u32,
}

@group(1) @binding(0) var<uniform> values: Values;
@group(1) @binding(1) var<storage, read> indices: array<u32>;
@group(1) @binding(2) var<storage, read_write> list: array<u32>;

const TRIANGLE_VERTICES: u32 = 3u;
const RUNS: u32 = 256u;

// Each run's primitives, then scanned: those of the runs up to it.
var<workgroup> scan: array<u32, 256>;
// One past the last restart index's position in each run (0 for none), then scanned by maximum: the
// first position of the strip each run ends in.
var<workgroup> strip_start: array<u32, 256>;

fn index_at(i: u32) -> u32 {
    let byte = values.index_offset + (values.first + i) * values.index_bytes;
    let word = indices[byte / 4u];
    if values.index_bytes == 4u {
        return word;
    }
    return (word >> ((byte & 2u) * 8u)) & 0xffffu;
}

// Whether a primitive starts at position `i`: its vertices are in the draw and none is a restart.
fn starts(i: u32) -> bool {
    if i + values.vertices > values.count {
        return false;
    }
    for (var k = 0u; k < values.vertices; k++) {
        if index_at(i + k) == values.restart {
            return false;
        }
    }
    return true;
}

// One step of the workgroup's inclusive scans. The steps are written out, not looped: Direct3D's
// shader compiler unrolls a loop holding a barrier and cannot unroll the form it gets.
fn scan_step(t: u32, d: u32) {
    let before = max(t, d) - d;
    let add = select(0u, scan[before], t >= d);
    let start = select(0u, strip_start[before], t >= d);
    workgroupBarrier();
    scan[t] += add;
    strip_start[t] = max(strip_start[t], start);
    workgroupBarrier();
}

@compute @workgroup_size(256)
fn main(@builtin(local_invocation_index) t: u32) {
    let per = (values.count + RUNS - 1u) / RUNS;
    let begin = min(t * per, values.count);
    let end = min(begin + per, values.count);
    var n = 0u;
    var cut = 0u;
    for (var i = begin; i < end; i++) {
        if index_at(i) == values.restart {
            cut = i + 1u;
        }
        if starts(i) {
            n += 1u;
        }
    }
    scan[t] = n;
    strip_start[t] = cut;
    workgroupBarrier();
    scan_step(t, 1u);
    scan_step(t, 2u);
    scan_step(t, 4u);
    scan_step(t, 8u);
    scan_step(t, 16u);
    scan_step(t, 32u);
    scan_step(t, 64u);
    scan_step(t, 128u);
    var at = values.header + (scan[t] - n) * values.vertices;
    var start = select(0u, strip_start[max(t, 1u) - 1u], t > 0u);
    for (var i = begin; i < end; i++) {
        if index_at(i) == values.restart {
            start = i + 1u;
        }
        if !starts(i) {
            continue;
        }
        let odd = values.vertices == TRIANGLE_VERTICES && ((i - start) & 1u) != 0u;
        for (var k = 0u; k < values.vertices; k++) {
            var v = i + k;
            if odd && k != 0u {
                v = i + TRIANGLE_VERTICES - k;
            }
            list[at + k] = index_at(v);
        }
        at += values.vertices;
    }
    if t == RUNS - 1u {
        list[0] = scan[t] * values.vertices;
    }
}
