// Direct3D's fixed-function tessellator as compute, ported from the reference tessellator Microsoft
// published under the MIT licence (carried in Mesa as src/gallium/auxiliary/tessellator). Each patch is
// tessellated by one invocation, in the reference's fixed-point arithmetic, so the domain points and
// their order are the reference's. Three passes over one program, chosen by `tv.mode`:
// 0 counts each patch's points and indices, 1 (one workgroup) turns the counts into each patch's first
// point and index and writes the indexed indirect draw's arguments, 2 writes the points and indices.

struct Values {
    // Patches in the draw, all API instances together.
    patches: u32,
    mode: u32,
    // Triangle (0), quad (1) or isoline (2).
    domain: u32,
    // Integer (0), powers of two (1), fractional odd (2), fractional even (3).
    partitioning: u32,
    // Points (0), lines (1), clockwise (2) or counter-clockwise (3) triangles.
    output: u32,
    // Patches per API instance.
    primitives: u32,
    pad0: u32,
    pad1: u32,
}

@group(1) @binding(0) var<uniform> tv: Values;
// The hull stage's output: a header (`crate::stage::tess`), then one record per patch.
@group(1) @binding(1) var<storage, read> patches: array<u32>;
// The indexed indirect draw's five words, the point total, two unused, then two words per patch:
// its point and index counts after pass 0, its first point and index after pass 1.
@group(1) @binding(2) var<storage, read_write> counts: array<u32>;
// Each point: u and v (f32 bits), the patch's number in the draw and its primitive number.
@group(1) @binding(3) var<storage, read_write> points: array<vec4<u32>>;
// The draw's indices: point numbers in the draw.
@group(1) @binding(4) var<storage, read_write> indices: array<u32>;

const HEADER_PATCH_WORDS: u32 = 0u;
const HEADER_FACTOR_BASE: u32 = 3u;
const HEADER_WORDS: u32 = 72u;
const COUNTS_BASE: u32 = 8u;

const FXP_FRACTION_BITS: u32 = 16u;
const FXP_FRACTION_MASK: u32 = 0x0000ffffu;
const FXP_INTEGER_MASK: u32 = 0x7fff0000u;
const FXP_ONE: u32 = 0x10000u;
const FXP_ONE_THIRD: u32 = 0x5555u;
const FXP_TWO_THIRDS: u32 = 0xaaaau;
const FXP_ONE_HALF: u32 = 0x8000u;

const EVEN: u32 = 0u;
const ODD: u32 = 1u;

const OUTPUT_POINT: u32 = 0u;
const OUTPUT_LINE: u32 = 1u;
const OUTPUT_TRIANGLE_CW: u32 = 2u;

const DIAGONALS_INSIDE_TO_OUTSIDE: u32 = 0u;
const DIAGONALS_INSIDE_TO_OUTSIDE_EXCEPT_MIDDLE: u32 = 1u;
const DIAGONALS_MIRRORED: u32 = 2u;

// 2^-16, the smallest positive fixed-point fraction.
const EPSILON: f32 = 0.0000152587890625;
const MIN_ODD_PLUS_HALF_EPSILON: f32 = 1.00000762939453125;

const RECIPROCAL = array<u32, 65>(
    0xffffffffu,
    0x10000u, 0x8000u, 0x5555u, 0x4000u, 0x3333u, 0x2aabu, 0x2492u, 0x2000u,
    0x1c72u, 0x199au, 0x1746u, 0x1555u, 0x13b1u, 0x1249u, 0x1111u, 0x1000u,
    0xf0fu, 0xe39u, 0xd79u, 0xccdu, 0xc31u, 0xba3u, 0xb21u, 0xaabu,
    0xa3du, 0x9d9u, 0x97bu, 0x925u, 0x8d4u, 0x889u, 0x842u, 0x800u,
    0x7c2u, 0x788u, 0x750u, 0x71cu, 0x6ebu, 0x6bdu, 0x690u, 0x666u,
    0x63eu, 0x618u, 0x5f4u, 0x5d1u, 0x5b0u, 0x591u, 0x572u, 0x555u,
    0x539u, 0x51fu, 0x505u, 0x4ecu, 0x4d5u, 0x4beu, 0x4a8u, 0x492u,
    0x47eu, 0x46au, 0x457u, 0x444u, 0x432u, 0x421u, 0x410u, 0x400u,
);

// Where vertex i of a half edge ends up at the largest factor in the ruler function's split order,
// and the loop bounds over it for each half factor.
const FINAL_POINT_POSITION = array<i32, 33>(
    0, 32, 16, 8, 17, 4, 18, 9, 19, 2, 20, 10, 21, 5, 22, 11, 23,
    1, 24, 12, 25, 6, 26, 13, 27, 3, 28, 14, 29, 7, 30, 15, 31,
);
const LOOP_START = array<i32, 33>(
    1, 1, 17, 9, 9, 5, 5, 5, 5, 3, 3, 3, 3, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
);
const LOOP_END = array<i32, 33>(
    0, 0, 17, 17, 25, 25, 25, 25, 29, 29, 29, 29, 29, 29, 29, 29, 31, 31, 31, 31, 31, 31, 31, 31, 31,
    31, 31, 31, 31, 31, 31, 31, 32,
);

// One factor's placement of points along an edge.
struct Ctx {
    inv_floor: u32,
    inv_ceil: u32,
    half_fraction: u32,
    num_half: i32,
    split: i32,
}

var<private> parity: u32;
var<private> original_parity: u32;
var<private> writing: bool;
var<private> point_base: u32;
var<private> index_base: u32;
var<private> patch_number: u32;
var<private> primitive: u32;
var<private> num_points: i32;
var<private> num_indices: i32;

// The reference's two ways of renumbering indices while stitching a ring's last edge.
var<private> patched: bool;
var<private> inside_delta: i32;
var<private> inside_bad: i32;
var<private> inside_replacement: i32;
var<private> outside_patch_base: i32;
var<private> outside_delta: i32;
var<private> outside_bad: i32;
var<private> outside_replacement: i32;
var<private> patched2: bool;
var<private> base_to_invert: i32;
var<private> inversion_end: i32;
var<private> corner_bad: i32;
var<private> corner_replacement: i32;

var<workgroup> sum_points: array<u32, 64>;
var<workgroup> sum_indices: array<u32, 64>;

fn hw_integer() -> bool {
    return tv.partitioning < 2u;
}

fn odd() -> bool {
    return parity == ODD;
}

fn is_nan(a: f32) -> bool {
    let bits = bitcast<u32>(a);
    return (bits & 0x7f800000u) == 0x7f800000u && (bits & 0x007fffffu) != 0u;
}

fn flush(a: f32) -> f32 {
    let bits = bitcast<u32>(a);
    if (bits & 0x7fffffffu) < 0x00800000u {
        return bitcast<f32>(bits & 0x80000000u);
    }
    return a;
}

fn tess_min(a: f32, b: f32) -> f32 {
    let fa = flush(a);
    let fb = flush(b);
    if is_nan(fb) {
        return a;
    }
    if fa == 0.0 && fb == 0.0 {
        return select(b, a, (bitcast<u32>(fa) & 0x80000000u) != 0u);
    }
    return select(b, a, fa < fb);
}

fn tess_max(a: f32, b: f32) -> f32 {
    let fa = flush(a);
    let fb = flush(b);
    if is_nan(fb) {
        return a;
    }
    if fa == 0.0 && fb == 0.0 {
        return select(b, a, (bitcast<u32>(fb) & 0x80000000u) != 0u);
    }
    return select(b, a, fa >= fb);
}

// An edge factor that culls its patch: not greater than zero, or NaN. Read from the bits, as a
// comparison on the GPU may flush a denormal to zero, which the reference keeps.
fn culls(f: f32) -> bool {
    let bits = bitcast<u32>(f);
    return bits == 0u || (bits & 0x80000000u) != 0u || is_nan(f);
}

fn clamp_factor(f: f32, lower: f32, upper: f32) -> f32 {
    return tess_min(upper, tess_max(lower, f));
}

// An f32 as unsigned 15.16 fixed point: NaN and negatives zero, too large the largest, rounded as the
// reference rounds.
fn float_to_fixed(input: f32) -> u32 {
    let bits = bitcast<u32>(input);
    let exponent = i32((bits >> 23u) & 0xffu) - 127;
    if exponent == 128 && (bits & 0x007fffffu) != 0u {
        return 0u;
    }
    if (bits & 0x80000000u) != 0u {
        return 0u;
    }
    if bits >= (142u << 23u) {
        return 0xffffffffu;
    }
    if exponent < -17 {
        return 0u;
    }
    var out = (bits & 0x007fffffu) | 0x00800000u;
    let extra = 7 - exponent;
    if extra >= 0 {
        let lsb = 1u << u32(extra);
        let mask = lsb - 1u;
        let half = lsb >> 1u;
        if (out & lsb) != 0u || (out & mask) > half {
            out += half;
        }
        return out >> u32(extra);
    }
    return out << u32(-extra);
}

fn fixed_to_float(x: u32) -> f32 {
    return f32(x >> FXP_FRACTION_BITS) + f32(x & FXP_FRACTION_MASK) / 65536.0;
}

fn fxp_ceil(x: u32) -> u32 {
    if (x & FXP_FRACTION_MASK) != 0u {
        return (x & FXP_INTEGER_MASK) + FXP_ONE;
    }
    return x;
}

fn fxp_floor(x: u32) -> u32 {
    return x & FXP_INTEGER_MASK;
}

fn is_even(f: f32) -> bool {
    return (i32(f) & 1) == 0;
}

fn remove_msb(v: i32) -> i32 {
    if v <= 0 {
        return 0;
    }
    return v & ~(1 << firstLeadingBit(u32(v)));
}

fn num_points_for(f: u32) -> i32 {
    if odd() {
        return i32((fxp_ceil(FXP_ONE_HALF + (f + 1u) / 2u) * 2u) >> FXP_FRACTION_BITS);
    }
    return i32((fxp_ceil((f + 1u) / 2u) * 2u) >> FXP_FRACTION_BITS) + 1;
}

fn context(f: u32) -> Ctx {
    var c: Ctx;
    var half = (f + 1u) / 2u;
    if odd() || half == FXP_ONE_HALF {
        half += FXP_ONE_HALF;
    }
    let floor_half = fxp_floor(half);
    let ceil_half = fxp_ceil(half);
    c.half_fraction = half - floor_half;
    c.num_half = i32(ceil_half >> FXP_FRACTION_BITS);
    if ceil_half == floor_half {
        c.split = c.num_half + 1;
    } else if odd() {
        if floor_half == FXP_ONE {
            c.split = 0;
        } else {
            c.split = (remove_msb(i32(floor_half >> FXP_FRACTION_BITS) - 1) << 1u) + 1;
        }
    } else {
        c.split = (remove_msb(i32(floor_half >> FXP_FRACTION_BITS)) << 1u) + 1;
    }
    var floor_segments = (floor_half * 2u) >> FXP_FRACTION_BITS;
    var ceil_segments = (ceil_half * 2u) >> FXP_FRACTION_BITS;
    if odd() {
        floor_segments -= 1u;
        ceil_segments -= 1u;
    }
    c.inv_floor = RECIPROCAL[floor_segments];
    c.inv_ceil = RECIPROCAL[ceil_segments];
    return c;
}

fn place(c: Ctx, at: i32) -> u32 {
    var point = at;
    var flip = false;
    if point >= c.num_half {
        point = (c.num_half << 1u) - point;
        if odd() {
            point -= 1;
        }
        flip = true;
    }
    if point == c.num_half {
        return FXP_ONE_HALF;
    }
    let on_ceil = u32(point);
    var on_floor = on_ceil;
    if point > c.split {
        on_floor -= 1u;
    }
    let floor_location = on_floor * c.inv_floor;
    let ceil_location = on_ceil * c.inv_ceil;
    var location = floor_location * (FXP_ONE - c.half_fraction) + ceil_location * c.half_fraction;
    location = (location + FXP_ONE_HALF) >> FXP_FRACTION_BITS;
    if flip {
        location = FXP_ONE - location;
    }
    return location;
}

fn define_point(u: u32, v: u32, offset: i32) {
    if writing {
        let p = vec4<u32>(
            bitcast<u32>(fixed_to_float(u)),
            bitcast<u32>(fixed_to_float(v)),
            patch_number,
            primitive,
        );
        points[point_base + u32(offset)] = p;
    }
}

fn patch_index(index: i32) -> i32 {
    var i = index;
    if patched {
        if i >= outside_patch_base {
            if i == outside_bad {
                i = outside_replacement;
            } else {
                i += outside_delta;
            }
        } else {
            if i == inside_bad {
                i = inside_replacement;
            } else {
                i += inside_delta;
            }
        }
    } else if patched2 {
        if i >= base_to_invert {
            if i == corner_bad {
                i = corner_replacement;
            } else {
                i = inversion_end - i;
            }
        } else if i == corner_bad {
            i = corner_replacement;
        }
    }
    return i;
}

fn define_index(index: i32, offset: i32) {
    let i = patch_index(index);
    if writing {
        indices[index_base + u32(offset)] = point_base + u32(i);
    }
}

fn clockwise_triangle(i0: i32, i1: i32, i2: i32, base: i32) {
    define_index(i0, base);
    if tv.output == OUTPUT_TRIANGLE_CW {
        define_index(i1, base + 1);
        define_index(i2, base + 2);
    } else {
        define_index(i2, base + 1);
        define_index(i1, base + 2);
    }
}

fn dump_points() {
    for (var p = 0; p < num_points; p++) {
        define_index(p, num_indices);
        num_indices += 1;
    }
}

fn dump_lines() {
    for (var p = 1; p < num_points; p++) {
        define_index(p - 1, num_indices);
        define_index(p, num_indices + 1);
        num_indices += 2;
    }
}

fn stitch_regular(trapezoid: bool, diagonals: u32, base_offset: i32, inside_points: i32,
                  inside_base: i32, outside_base: i32) {
    var base = base_offset;
    var inside = inside_base;
    var outside = outside_base;
    if trapezoid {
        clockwise_triangle(outside, outside + 1, inside, base);
        base += 3;
        outside += 1;
    }
    // Each quad between the rows is two triangles in one of three shapes: inside to outside (a),
    // outside to inside (b) or mirrored (c). The middle quad of `DIAGONALS_INSIDE_TO_OUTSIDE_EXCEPT_MIDDLE`
    // and the first half of `DIAGONALS_MIRRORED` take c. One loop with the shape chosen per quad keeps
    // the program small enough for fxc, which fails on one loop per shape.
    let half = inside_points / 2;
    let middle = max(half - 1, 0);
    var quads = max(inside_points - 1, 0);
    if diagonals == DIAGONALS_INSIDE_TO_OUTSIDE_EXCEPT_MIDDLE {
        quads = middle + 1 + max(inside_points - middle - 2, 0);
    }
    for (var q = 0; q < quads; q++) {
        var shape = 0u;
        if diagonals == DIAGONALS_INSIDE_TO_OUTSIDE_EXCEPT_MIDDLE {
            shape = select(1u, 2u, q == middle);
        } else if diagonals == DIAGONALS_MIRRORED {
            shape = select(0u, 2u, q < half);
        }
        let first = select(select(vec3<i32>(outside, outside + 1, inside),
                                  vec3<i32>(outside, inside + 1, inside), shape == 2u),
                           vec3<i32>(inside, outside, outside + 1), shape == 0u);
        let second = select(vec3<i32>(inside, outside + 1, inside + 1),
                            vec3<i32>(outside, outside + 1, inside + 1), shape == 2u);
        clockwise_triangle(first.x, first.y, first.z, base);
        clockwise_triangle(second.x, second.y, second.z, base + 3);
        base += 6;
        inside += 1;
        outside += 1;
    }
    if trapezoid {
        clockwise_triangle(outside, outside + 1, inside, base);
    }
}

fn stitch_transition(base_offset: i32, inside_base: i32, inside_half_points: i32, inside_parity: u32,
                     outside_base: i32, outside_half_points: i32, outside_parity: u32) {
    var base = base_offset;
    var inside_half = inside_half_points;
    var outside_half = outside_half_points;
    if inside_parity == ODD {
        inside_half -= 1;
    }
    if outside_parity == ODD {
        outside_half -= 1;
    }
    var outside = outside_base;
    var inside = inside_base;
    let first = min(LOOP_START[inside_half], LOOP_START[outside_half]);
    let last = max(LOOP_END[inside_half], LOOP_END[outside_half]);
    if FINAL_POINT_POSITION[0] < outside_half {
        clockwise_triangle(outside, outside + 1, inside, base);
        base += 3;
        outside += 1;
    }
    for (var i = first; i <= last; i++) {
        if FINAL_POINT_POSITION[i] < inside_half {
            clockwise_triangle(inside, outside, inside + 1, base);
            base += 3;
            inside += 1;
        }
        if FINAL_POINT_POSITION[i] < outside_half {
            clockwise_triangle(outside, outside + 1, inside, base);
            base += 3;
            outside += 1;
        }
    }
    if inside_parity != outside_parity || inside_parity == ODD {
        if inside_parity == outside_parity {
            clockwise_triangle(inside, outside, inside + 1, base);
            clockwise_triangle(inside + 1, outside, outside + 1, base + 3);
            base += 6;
            inside += 1;
            outside += 1;
        } else if inside_parity == EVEN {
            clockwise_triangle(inside, outside, outside + 1, base);
            base += 3;
            outside += 1;
        } else {
            clockwise_triangle(inside, outside, inside + 1, base);
            base += 3;
            inside += 1;
        }
    }
    for (var i = last; i >= first; i--) {
        if FINAL_POINT_POSITION[i] < outside_half {
            clockwise_triangle(outside, outside + 1, inside, base);
            base += 3;
            outside += 1;
        }
        if FINAL_POINT_POSITION[i] < inside_half {
            clockwise_triangle(inside, outside, inside + 1, base);
            base += 3;
            inside += 1;
        }
    }
    if FINAL_POINT_POSITION[0] < outside_half {
        clockwise_triangle(outside, outside + 1, inside, base);
    }
}

// The bounds factors are clamped to, by partitioning.
fn bounds() -> vec2<f32> {
    switch tv.partitioning {
        case 3u: {
            return vec2<f32>(2.0, 64.0);
        }
        case 2u: {
            return vec2<f32>(1.0, 63.0);
        }
        default: {
            return vec2<f32>(1.0, 64.0);
        }
    }
}

fn factor(i: u32) -> f32 {
    let at = HEADER_WORDS + patch_number * patches[HEADER_PATCH_WORDS] + patches[HEADER_FACTOR_BASE];
    return bitcast<f32>(patches[at + i]);
}

fn quad() {
    var outer = array<f32, 4>(factor(0u), factor(1u), factor(2u), factor(3u));
    var inner = array<f32, 2>(factor(4u), factor(5u));
    if culls(outer[0]) || culls(outer[1]) || culls(outer[2]) || culls(outer[3]) {
        return;
    }
    var b = bounds();
    for (var e = 0; e < 4; e++) {
        outer[e] = clamp_factor(outer[e], b.x, b.y);
        if hw_integer() {
            outer[e] = ceil(outer[e]);
        }
    }
    if tv.partitioning == 2u {
        var frame = inner[0] > MIN_ODD_PLUS_HALF_EPSILON || inner[1] > MIN_ODD_PLUS_HALF_EPSILON;
        for (var e = 0; e < 4; e++) {
            frame = frame || outer[e] > MIN_ODD_PLUS_HALF_EPSILON;
        }
        if frame {
            b.x = 1.0 + EPSILON;
        }
    }
    for (var a = 0; a < 2; a++) {
        inner[a] = clamp_factor(inner[a], b.x, b.y);
        if hw_integer() {
            inner[a] = ceil(inner[a]);
        }
    }
    var outer_parity: array<u32, 4>;
    var inner_parity: array<u32, 2>;
    var outer_fxp: array<u32, 4>;
    var inner_fxp: array<u32, 2>;
    for (var e = 0; e < 4; e++) {
        outer_parity[e] = select(original_parity, select(ODD, EVEN, is_even(outer[e])), hw_integer());
        outer_fxp[e] = float_to_fixed(outer[e]);
    }
    for (var a = 0; a < 2; a++) {
        let even = is_even(inner[a]) || inner[a] == 1.0;
        inner_parity[a] = select(original_parity, select(ODD, EVEN, even), hw_integer());
        inner_fxp[a] = float_to_fixed(inner[a]);
    }
    if hw_integer() || original_parity == ODD {
        if inner_fxp[0] == FXP_ONE && inner_fxp[1] == FXP_ONE && outer_fxp[0] == FXP_ONE
            && outer_fxp[1] == FXP_ONE && outer_fxp[2] == FXP_ONE && outer_fxp[3] == FXP_ONE {
            define_point(0u, 0u, 0);
            define_point(FXP_ONE, 0u, 1);
            define_point(FXP_ONE, FXP_ONE, 2);
            define_point(0u, FXP_ONE, 3);
            num_points = 4;
            if tv.output == OUTPUT_POINT {
                dump_points();
            } else if tv.output == OUTPUT_LINE {
                dump_lines();
            } else {
                clockwise_triangle(0, 1, 3, 0);
                clockwise_triangle(1, 2, 3, 3);
                num_indices = 6;
            }
            return;
        }
    }
    var outer_ctx: array<Ctx, 4>;
    var inner_ctx: array<Ctx, 2>;
    var outer_points: array<i32, 4>;
    var inner_points: array<i32, 2>;
    for (var e = 0; e < 4; e++) {
        parity = outer_parity[e];
        outer_ctx[e] = context(outer_fxp[e]);
        outer_points[e] = num_points_for(outer_fxp[e]);
        num_points += outer_points[e];
    }
    num_points -= 4;
    for (var a = 0; a < 2; a++) {
        parity = inner_parity[a];
        inner_ctx[a] = context(inner_fxp[a]);
        let least = select(3, 4, inner_parity[a] == ODD);
        inner_points[a] = max(least, num_points_for(inner_fxp[a]));
    }
    let inside_edge_base = num_points;
    num_points += (inner_points[0] - 2) * (inner_points[1] - 2);

    // Points: the outer ring clockwise from the top left, the inner rings, then the middle row.
    var offset = 0;
    for (var e = 0; e < 4; e++) {
        let end = outer_points[e] - 1;
        parity = outer_parity[e];
        for (var p = 0; p < end; p++) {
            let q = select(end - p, p, e == 1 || e == 2);
            let param = place(outer_ctx[e], q);
            if (e & 1) != 0 {
                define_point(param, select(0u, FXP_ONE, e == 3), offset);
            } else {
                define_point(select(0u, FXP_ONE, e == 2), param, offset);
            }
            offset += 1;
        }
    }
    let rings = min(inner_points[0], inner_points[1]) >> 1u;
    for (var ring = 1; ring < rings; ring++) {
        let start = ring;
        var ends = array<i32, 2>(inner_points[0] - 1 - start, inner_points[1] - 1 - start);
        for (var e = 0; e < 4; e++) {
            let p0 = e & 1;
            let p1 = (e + 1) & 1;
            let perpendicular = select(ends[p0], start, e < 2);
            parity = inner_parity[p0];
            let perp = place(inner_ctx[p0], perpendicular);
            parity = inner_parity[p1];
            for (var p = start; p < ends[p1]; p++) {
                let q = select(ends[p1] - (p - start), p, e == 1 || e == 2);
                let param = place(inner_ctx[p1], q);
                if p1 != 0 {
                    define_point(perp, param, offset);
                } else {
                    define_point(param, perp, offset);
                }
                offset += 1;
            }
        }
    }
    if inner_points[0] > inner_points[1] && inner_parity[1] == EVEN {
        let end = inner_points[0] - 1 - rings;
        parity = inner_parity[0];
        for (var p = rings; p <= end; p++) {
            define_point(place(inner_ctx[0], p), FXP_ONE_HALF, offset);
            offset += 1;
        }
    } else if inner_points[1] >= inner_points[0] && inner_parity[0] == EVEN {
        let end = inner_points[1] - 1 - rings;
        parity = inner_parity[1];
        for (var p = end; p >= rings; p--) {
            define_point(FXP_ONE_HALF, place(inner_ctx[1], p), offset);
            offset += 1;
        }
    }
    if tv.output == OUTPUT_POINT {
        dump_points();
        return;
    }
    if tv.output == OUTPUT_LINE {
        dump_lines();
        return;
    }

    // Triangles: each ring stitched to the one inside it, then the middle strip.
    var rows = array<i32, 2>((inner_points[0] + 1) >> 1u, (inner_points[1] + 1) >> 1u);
    let stitch_rings = min(rows[0], rows[1]);
    var degenerate = array<i32, 2>(
        select(-1, rows[1] - 1, inner_parity[1] == EVEN),
        select(-1, rows[0] - 1, inner_parity[0] == EVEN),
    );
    var half_points = array<i32, 4>(outer_ctx[0].num_half, outer_ctx[1].num_half,
                                    outer_ctx[2].num_half, outer_ctx[3].num_half);
    var edge_parity = outer_parity;
    var edge_points = outer_points;
    var inside_base_offset = inside_edge_base;
    var outside_base_offset = 0;
    for (var ring = 1; ring < stitch_rings; ring++) {
        var inside_points = array<i32, 2>(inner_points[0] - 2 * ring, inner_points[1] - 2 * ring);
        let edge0_inside = inside_base_offset;
        let edge0_outside = outside_base_offset;
        for (var e = 0; e < 4; e++) {
            let pa = (e + 1) & 1;
            let triangles = inside_points[pa] + edge_points[e] - 2;
            var inside_at = inside_base_offset;
            var outside_at = outside_base_offset;
            if e == 3 {
                if ring == degenerate[pa] {
                    base_to_invert = inside_base_offset + 1;
                    corner_bad = outside_base_offset + edge_points[e] - 1;
                    corner_replacement = edge0_outside;
                    inversion_end = (base_to_invert << 1u) - 1;
                    inside_at = base_to_invert;
                    outside_at = outside_base_offset;
                    patched2 = true;
                } else {
                    inside_delta = inside_base_offset;
                    inside_bad = inside_points[pa] - 1;
                    inside_replacement = edge0_inside;
                    outside_patch_base = inside_bad + 1;
                    outside_delta = outside_base_offset - outside_patch_base;
                    outside_bad = outside_patch_base + edge_points[e] - 1;
                    outside_replacement = edge0_outside;
                    inside_at = 0;
                    outside_at = outside_patch_base;
                    patched = true;
                }
            } else if e == 2 && ring == degenerate[pa] {
                base_to_invert = inside_base_offset;
                corner_bad = -1;
                corner_replacement = -1;
                inversion_end = base_to_invert << 1u;
                inside_at = base_to_invert;
                outside_at = outside_base_offset;
                patched2 = true;
            }
            if ring == 1 {
                stitch_transition(num_indices, inside_at, inner_ctx[pa].num_half, inner_parity[pa],
                                  outside_at, half_points[e], edge_parity[e]);
            } else {
                stitch_regular(true, DIAGONALS_MIRRORED, num_indices, inside_points[pa], inside_at,
                               outside_at);
            }
            patched = false;
            patched2 = false;
            num_indices += triangles * 3;
            outside_base_offset += edge_points[e] - 1;
            if e == 2 && ring == degenerate[pa] {
                inside_base_offset -= inside_points[pa] - 1;
            } else {
                inside_base_offset += inside_points[pa] - 1;
            }
            edge_points[e] = inside_points[pa];
        }
        if ring == 1 {
            for (var e = 0; e < 4; e++) {
                half_points[e] = inner_ctx[e & 1].num_half;
                edge_parity[e] = inner_parity[e & 1];
            }
        }
    }
    if inner_points[0] > inner_points[1] && inner_parity[1] == ODD {
        patched2 = true;
        let quads = (((inner_points[0] >> 1u) - (inner_points[1] >> 1u)) << 1u)
            + select(1, 2, inner_parity[0] == EVEN);
        base_to_invert = outside_base_offset + quads + 2;
        corner_bad = base_to_invert;
        corner_replacement = outside_base_offset;
        inversion_end = base_to_invert + base_to_invert + quads;
        stitch_regular(false, DIAGONALS_INSIDE_TO_OUTSIDE, num_indices, quads + 1, base_to_invert,
                       outside_base_offset + 1);
        patched2 = false;
        num_indices += quads * 6;
    } else if inner_points[1] >= inner_points[0] && inner_parity[0] == ODD {
        patched2 = true;
        let quads = (((inner_points[1] >> 1u) - (inner_points[0] >> 1u)) << 1u)
            + select(1, 2, inner_parity[1] == EVEN);
        base_to_invert = outside_base_offset + quads + 1;
        corner_bad = -1;
        inversion_end = base_to_invert + base_to_invert + quads;
        let diagonals = select(DIAGONALS_INSIDE_TO_OUTSIDE_EXCEPT_MIDDLE, DIAGONALS_INSIDE_TO_OUTSIDE,
                               inner_parity[1] == EVEN);
        stitch_regular(false, diagonals, num_indices, quads + 1, base_to_invert,
                       outside_base_offset);
        patched2 = false;
        num_indices += quads * 6;
    }
}

fn tri() {
    var outer = array<f32, 3>(factor(0u), factor(1u), factor(2u));
    var inner = factor(4u);
    if culls(outer[0]) || culls(outer[1]) || culls(outer[2]) {
        return;
    }
    var b = bounds();
    for (var e = 0; e < 3; e++) {
        outer[e] = clamp_factor(outer[e], b.x, b.y);
        if hw_integer() {
            outer[e] = ceil(outer[e]);
        }
    }
    if tv.partitioning == 2u {
        if outer[0] > MIN_ODD_PLUS_HALF_EPSILON || outer[1] > MIN_ODD_PLUS_HALF_EPSILON
            || outer[2] > MIN_ODD_PLUS_HALF_EPSILON {
            b.x = 1.0 + EPSILON;
        }
    }
    inner = clamp_factor(inner, b.x, b.y);
    if hw_integer() {
        inner = ceil(inner);
    }
    var outer_parity: array<u32, 3>;
    var outer_fxp: array<u32, 3>;
    for (var e = 0; e < 3; e++) {
        outer_parity[e] = select(original_parity, select(ODD, EVEN, is_even(outer[e])), hw_integer());
        outer_fxp[e] = float_to_fixed(outer[e]);
    }
    let inner_even = is_even(inner) || inner == 1.0;
    let inner_parity = select(original_parity, select(ODD, EVEN, inner_even), hw_integer());
    let inner_fxp = float_to_fixed(inner);
    if hw_integer() || original_parity == ODD {
        if inner_fxp == FXP_ONE && outer_fxp[0] == FXP_ONE && outer_fxp[1] == FXP_ONE
            && outer_fxp[2] == FXP_ONE {
            define_point(0u, FXP_ONE, 0);
            define_point(0u, 0u, 1);
            define_point(FXP_ONE, 0u, 2);
            num_points = 3;
            if tv.output == OUTPUT_POINT {
                dump_points();
            } else if tv.output == OUTPUT_LINE {
                dump_lines();
            } else {
                clockwise_triangle(0, 1, 2, 0);
                num_indices = 3;
            }
            return;
        }
    }
    var outer_ctx: array<Ctx, 3>;
    var outer_points: array<i32, 3>;
    for (var e = 0; e < 3; e++) {
        parity = outer_parity[e];
        outer_ctx[e] = context(outer_fxp[e]);
        outer_points[e] = num_points_for(outer_fxp[e]);
        num_points += outer_points[e];
    }
    num_points -= 3;
    parity = inner_parity;
    let inner_ctx = context(inner_fxp);
    let inner_points = max(select(3, 4, odd()), num_points_for(inner_fxp));
    let inside_edge_base = num_points;
    let interior_rings = (inner_points >> 1u) - 1;
    if odd() {
        num_points += 3 * (interior_rings * (interior_rings + 1) - interior_rings);
    } else {
        num_points += 3 * (interior_rings * (interior_rings + 1)) + 1;
    }

    var offset = 0;
    for (var e = 0; e < 3; e++) {
        let end = outer_points[e] - 1;
        parity = outer_parity[e];
        for (var p = 0; p < end; p++) {
            let q = select(end - p, p, (e & 1) != 0);
            let param = place(outer_ctx[e], q);
            if e == 0 {
                define_point(0u, param, offset);
            } else {
                define_point(param, select(0u, FXP_ONE - param, e == 2), offset);
            }
            offset += 1;
        }
    }
    parity = inner_parity;
    let rings = inner_points >> 1u;
    for (var ring = 1; ring < rings; ring++) {
        let start = ring;
        let end = inner_points - 1 - start;
        for (var e = 0; e < 3; e++) {
            var perp = place(inner_ctx, start);
            perp = (perp * FXP_TWO_THIRDS + FXP_ONE_HALF) >> FXP_FRACTION_BITS;
            for (var p = start; p < end; p++) {
                let q = select(end - (p - start), p, (e & 1) != 0);
                let param = place(inner_ctx, q);
                let along = param - (perp + 1u) / 2u;
                let u = select(along, perp, e == 0);
                let v = select(select(FXP_ONE - along - perp, perp, e == 1), along, e == 0);
                define_point(u, v, offset);
                offset += 1;
            }
        }
    }
    if !odd() {
        define_point(FXP_ONE_THIRD, FXP_ONE_THIRD, offset);
    }
    if tv.output == OUTPUT_POINT {
        dump_points();
        return;
    }
    if tv.output == OUTPUT_LINE {
        dump_lines();
        return;
    }

    let stitch_rings = (inner_points + 1) >> 1u;
    var half_points = array<i32, 3>(outer_ctx[0].num_half, outer_ctx[1].num_half, outer_ctx[2].num_half);
    var edge_parity = outer_parity;
    var edge_points = outer_points;
    var inside_base_offset = inside_edge_base;
    var outside_base_offset = 0;
    for (var ring = 1; ring < stitch_rings; ring++) {
        let inside_points = inner_points - 2 * ring;
        let edge0_inside = inside_base_offset;
        let edge0_outside = outside_base_offset;
        for (var e = 0; e < 3; e++) {
            let triangles = inside_points + edge_points[e] - 2;
            var inside_at = inside_base_offset;
            var outside_at = outside_base_offset;
            if e == 2 {
                inside_delta = inside_base_offset;
                inside_bad = inside_points - 1;
                inside_replacement = edge0_inside;
                outside_patch_base = inside_bad + 1;
                outside_delta = outside_base_offset - outside_patch_base;
                outside_bad = outside_patch_base + edge_points[e] - 1;
                outside_replacement = edge0_outside;
                patched = true;
                inside_at = 0;
                outside_at = outside_patch_base;
            }
            if ring == 1 {
                stitch_transition(num_indices, inside_at, inner_ctx.num_half, inner_parity,
                                  outside_at, half_points[e], edge_parity[e]);
            } else {
                stitch_regular(true, DIAGONALS_MIRRORED, num_indices, inside_points, inside_at,
                               outside_at);
            }
            if e == 2 {
                patched = false;
            }
            num_indices += triangles * 3;
            outside_base_offset += edge_points[e] - 1;
            inside_base_offset += inside_points - 1;
            edge_points[e] = inside_points;
        }
        if ring == 1 {
            for (var e = 0; e < 3; e++) {
                half_points[e] = inner_ctx.num_half;
                edge_parity[e] = inner_parity;
            }
        }
    }
    if odd() {
        clockwise_triangle(outside_base_offset, outside_base_offset + 1, outside_base_offset + 2,
                           num_indices);
        num_indices += 3;
    }
}

fn isoline() {
    var density = factor(0u);
    var detail = factor(1u);
    if culls(density) || culls(detail) {
        return;
    }
    let b = bounds();
    density = clamp_factor(density, 1.0, 64.0);
    detail = clamp_factor(detail, b.x, b.y);
    var detail_parity = original_parity;
    if hw_integer() {
        detail = ceil(detail);
        detail_parity = select(ODD, EVEN, is_even(detail));
    }
    let detail_fxp = float_to_fixed(detail);
    parity = detail_parity;
    let detail_ctx = context(detail_fxp);
    let per_line = num_points_for(detail_fxp);
    density = ceil(density);
    let density_parity = select(ODD, EVEN, is_even(density));
    parity = density_parity;
    let density_fxp = float_to_fixed(density);
    let density_ctx = context(density_fxp);
    let lines = num_points_for(density_fxp) - 1;
    num_points = per_line * lines;
    var offset = 0;
    for (var line = 0; line < lines; line++) {
        for (var p = 0; p < per_line; p++) {
            parity = density_parity;
            let v = place(density_ctx, line);
            parity = detail_parity;
            let u = place(detail_ctx, p);
            define_point(u, v, offset);
            offset += 1;
        }
    }
    offset = 0;
    var index = 0;
    for (var line = 0; line < lines; line++) {
        for (var p = 0; p < per_line; p++) {
            if tv.output == OUTPUT_POINT {
                define_index(offset, index);
                index += 1;
            } else if p > 0 {
                define_index(offset - 1, index);
                define_index(offset, index + 1);
                index += 2;
            }
            offset += 1;
        }
    }
    num_indices = index;
}

// Pass 1, one workgroup: each invocation takes a run of patches.
fn scan(l: u32) {
    let run = (tv.patches + 63u) / 64u;
    let first = min(l * run, tv.patches);
    let end = min(first + run, tv.patches);
    var p_sum = 0u;
    var i_sum = 0u;
    for (var p = first; p < end; p++) {
        p_sum += counts[COUNTS_BASE + 2u * p];
        i_sum += counts[COUNTS_BASE + 2u * p + 1u];
    }
    sum_points[l] = p_sum;
    sum_indices[l] = i_sum;
    workgroupBarrier();
    if l == 0u {
        var p_at = 0u;
        var i_at = 0u;
        for (var r = 0u; r < 64u; r++) {
            let np = sum_points[r];
            let ni = sum_indices[r];
            sum_points[r] = p_at;
            sum_indices[r] = i_at;
            p_at += np;
            i_at += ni;
        }
        counts[0] = i_at;
        counts[1] = 1u;
        counts[2] = 0u;
        counts[3] = 0u;
        counts[4] = 0u;
        counts[5] = p_at;
    }
    workgroupBarrier();
    var p_at = sum_points[l];
    var i_at = sum_indices[l];
    for (var p = first; p < end; p++) {
        let np = counts[COUNTS_BASE + 2u * p];
        let ni = counts[COUNTS_BASE + 2u * p + 1u];
        counts[COUNTS_BASE + 2u * p] = p_at;
        counts[COUNTS_BASE + 2u * p + 1u] = i_at;
        p_at += np;
        i_at += ni;
    }
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(local_invocation_index) l: u32) {
    if tv.mode == 1u {
        scan(l);
        return;
    }
    if id.x >= tv.patches {
        return;
    }
    patch_number = id.x;
    primitive = id.x % tv.primitives;
    writing = tv.mode == 2u;
    if writing {
        point_base = counts[COUNTS_BASE + 2u * id.x];
        index_base = counts[COUNTS_BASE + 2u * id.x + 1u];
    }
    original_parity = select(EVEN, ODD, tv.partitioning == 2u);
    parity = original_parity;
    switch tv.domain {
        case 0u: {
            tri();
        }
        case 1u: {
            quad();
        }
        default: {
            isoline();
        }
    }
    if !writing {
        counts[COUNTS_BASE + 2u * id.x] = u32(num_points);
        counts[COUNTS_BASE + 2u * id.x + 1u] = u32(num_indices);
    }
}
