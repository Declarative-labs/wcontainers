// A strip as an indexed list, as one workgroup: the draw's places are split into 256 runs; a scan
// over the workgroup gives each run the start of the strip it begins in, each run counts the
// primitives starting in it, a second scan gives each run its first primitive, and each writes its
// primitives' indices in the API's order. A restart index ends a strip (indexed draws only). The
// triangles of each strip alternate, odd ones taking their last two vertices swapped (i, i + 2,
// i + 1), which keeps the winding and the first vertex. A triangle strip with adjacency takes its
// triangles from the strip's even places and their adjacent vertices from the odd ones, as
// Direct3D does. The last invocation writes the listed indices' total in word 0.

struct Values {
    // The API draw's places (indices or vertices), and its first.
    count: u32,
    first: u32,
    // Bytes per index, 2 or 4; 0 for a draw without indices, whose places are its vertices.
    index_bytes: u32,
    // Bytes from the bound index words' start to the API's index buffer offset.
    index_offset: u32,
    // Vertices per primitive: 2, 3, 4, or 6 for triangles with adjacency.
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
const TRIANGLE_ADJACENCY_VERTICES: u32 = 6u;
const RUNS: u32 = 256u;

// Each run's primitives, then scanned: those of the runs up to it.
var<workgroup> scan: array<u32, 256>;
// One past the last restart index's place in each run (0 for none), then scanned by maximum: the
// first place of the strip each run ends in.
var<workgroup> strip_start: array<u32, 256>;

fn index_at(i: u32) -> u32 {
    if values.index_bytes == 0u {
        return values.first + i;
    }
    let byte = values.index_offset + (values.first + i) * values.index_bytes;
    let word = indices[byte / 4u];
    if values.index_bytes == 4u {
        return word;
    }
    return (word >> ((byte & 2u) * 8u)) & 0xffffu;
}

fn is_restart(i: u32) -> bool {
    return values.index_bytes != 0u && index_at(i) == values.restart;
}

// Whether places `i` to `i + n` are in the draw and none is a restart.
fn whole(i: u32, n: u32) -> bool {
    if i + n > values.count {
        return false;
    }
    for (var k = 0u; k < n; k++) {
        if is_restart(i + k) {
            return false;
        }
    }
    return true;
}

// Whether a primitive starts at place `i` of a strip starting at `start`: a triangle with adjacency
// only at the strip's even places.
fn starts(i: u32, start: u32) -> bool {
    if values.vertices == TRIANGLE_ADJACENCY_VERTICES && ((i - start) & 1u) != 0u {
        return false;
    }
    return whole(i, values.vertices);
}

// The place of vertex `k` of the triangle with adjacency at place `i` of a strip starting at
// `start`, in Direct3D's order: a vertex, the vertex adjacent to the edge from it to the next, the
// next, and so on.
fn adjacent_place(i: u32, start: u32, k: u32) -> u32 {
    let first = i == start;
    // The last triangle of its strip: no triangle starts two places on.
    let last = !whole(i, TRIANGLE_ADJACENCY_VERTICES + 2u);
    let odd = (((i - start) / 2u) & 1u) != 0u;
    // Offsets from `i` of the triangle's vertices and of the vertices adjacent to its edges, in
    // the order (v0, adjacent to v0 v1, v1, adjacent to v1 v2, v2, adjacent to v2 v0). An odd
    // triangle keeps its first vertex and takes the other two swapped, as a strip's triangles do.
    var v = vec3<u32>(0u, 2u, 4u);
    // Adjacent to the edge from the strip's vertex before: 1 on for the first triangle.
    var before = i - 2u;
    if first {
        before = i + 1u;
    }
    // Adjacent to the edge towards the strip's vertex after: 6 on, or 5 for the last triangle.
    var after = i + 6u;
    if last {
        after = i + 5u;
    }
    var a01 = before;
    var a12 = after;
    var a20 = i + 3u;
    if odd {
        v = vec3<u32>(0u, 4u, 2u);
        a01 = i + 3u;
        a12 = after;
        a20 = before;
    }
    switch k {
        case 0u: { return i + v.x; }
        case 1u: { return a01; }
        case 2u: { return i + v.y; }
        case 3u: { return a12; }
        case 4u: { return i + v.z; }
        default: { return a20; }
    }
}

// One step of the workgroup's inclusive scans. The steps are written out, not looped: Direct3D's
// shader compiler unrolls a loop holding a barrier and cannot unroll the form it gets.
fn sum_step(t: u32, d: u32) {
    let add = select(0u, scan[max(t, d) - d], t >= d);
    workgroupBarrier();
    scan[t] += add;
    workgroupBarrier();
}

fn max_step(t: u32, d: u32) {
    let start = select(0u, strip_start[max(t, d) - d], t >= d);
    workgroupBarrier();
    strip_start[t] = max(strip_start[t], start);
    workgroupBarrier();
}

@compute @workgroup_size(256)
fn main(@builtin(local_invocation_index) t: u32) {
    let per = (values.count + RUNS - 1u) / RUNS;
    let begin = min(t * per, values.count);
    let end = min(begin + per, values.count);
    var cut = 0u;
    for (var i = begin; i < end; i++) {
        if is_restart(i) {
            cut = i + 1u;
        }
    }
    strip_start[t] = cut;
    workgroupBarrier();
    max_step(t, 1u);
    max_step(t, 2u);
    max_step(t, 4u);
    max_step(t, 8u);
    max_step(t, 16u);
    max_step(t, 32u);
    max_step(t, 64u);
    max_step(t, 128u);
    let run_start = select(0u, strip_start[max(t, 1u) - 1u], t > 0u);
    var start = run_start;
    var n = 0u;
    for (var i = begin; i < end; i++) {
        if is_restart(i) {
            start = i + 1u;
        }
        if starts(i, start) {
            n += 1u;
        }
    }
    scan[t] = n;
    workgroupBarrier();
    sum_step(t, 1u);
    sum_step(t, 2u);
    sum_step(t, 4u);
    sum_step(t, 8u);
    sum_step(t, 16u);
    sum_step(t, 32u);
    sum_step(t, 64u);
    sum_step(t, 128u);
    var at = values.header + (scan[t] - n) * values.vertices;
    start = run_start;
    for (var i = begin; i < end; i++) {
        if is_restart(i) {
            start = i + 1u;
        }
        if !starts(i, start) {
            continue;
        }
        let odd = values.vertices == TRIANGLE_VERTICES && ((i - start) & 1u) != 0u;
        for (var k = 0u; k < values.vertices; k++) {
            var v = i + k;
            if values.vertices == TRIANGLE_ADJACENCY_VERTICES {
                v = adjacent_place(i, start, k);
            } else if odd && k != 0u {
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
