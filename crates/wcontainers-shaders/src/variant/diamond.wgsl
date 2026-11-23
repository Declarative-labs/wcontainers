// Direct3D's aliased line rule, the diamond exit: whether the line from `ends.xy` to `ends.zw`
// covers the pixel whose centre is `centre`, all in window pixels with y down, the ends on
// Direct3D's grid of 1/256 pixel. The pixel's diamond holds the points less than half a pixel from
// its centre (the distances along x and y added) and, of its outline, the two lower edges and the
// bottom corner, and for a y-major line also the right corner. The line covers the pixel where it
// leaves the diamond: it meets the diamond and its end lies outside. Exact for ends within 32,768
// pixels of the centre: positions are integers of the grid, and a line's side of a corner is the
// sign of a cross product computed in halves of 12 bits.
fn diamond_exit(ends: vec4<f32>, centre: vec2<f32>) -> bool {
    const STEPS: f32 = 256.0;
    const HALF: i32 = 128;
    const SPLIT: u32 = 12u;
    const LOW: i32 = 4095;
    let p = vec2<i32>(round((ends.xy - centre) * STEPS));
    let q = vec2<i32>(round((ends.zw - centre) * STEPS));
    let d = q - p;
    let y_major = abs(d.y) > abs(d.x);
    let sp = abs(p.x) + abs(p.y);
    let in_p = sp < HALF || (sp == HALF && (p.y > 0 || (y_major && p.x == HALF)));
    let sq = abs(q.x) + abs(q.y);
    let in_q = sq < HALF || (sq == HALF && (q.y > 0 || (y_major && q.x == HALF)));
    if in_q {
        return false;
    }
    if in_p {
        return true;
    }
    // The sign of d x (corner - p) for the top, bottom, left and right corners.
    let corners = array<vec2<i32>, 4>(
        vec2<i32>(0, -HALF),
        vec2<i32>(0, HALF),
        vec2<i32>(-HALF, 0),
        vec2<i32>(HALF, 0),
    );
    var sides: array<i32, 4>;
    for (var k = 0u; k < 4u; k++) {
        let r = corners[k] - p;
        // d.x * r.y - d.y * r.x as high * 2^24 + middle * 2^12 + low.
        let a = vec4<i32>(d.x, r.y, d.y, r.x);
        let high = a >> vec4<u32>(SPLIT);
        let low = a & vec4<i32>(LOW);
        var h = high.x * high.y - high.z * high.w;
        var m = high.x * low.y + low.x * high.y - high.z * low.w - low.z * high.w;
        var l = low.x * low.y - low.z * low.w;
        m += l >> SPLIT;
        l &= LOW;
        h += m >> SPLIT;
        m &= LOW;
        sides[k] = select(sign(h), select(0, 1, (m | l) != 0), h == 0);
    }
    var hit = false;
    // Where the line crosses the vertical through the centre between its ends: inside below the top
    // corner, up to and with the bottom corner.
    if d.x != 0 && sign(p.x) * sign(q.x) <= 0 {
        let s = sign(d.x);
        hit = sides[0] * s < 0 && sides[1] * s >= 0;
    }
    // Where it crosses the horizontal through the centre: right of the left corner, left of the right
    // corner, or on it for a y-major line.
    if d.y != 0 && sign(p.y) * sign(q.y) <= 0 {
        let s = sign(d.y);
        let right = sides[3] * s;
        hit = hit || (sides[2] * s > 0 && (right < 0 || (y_major && right == 0)));
    }
    return hit;
}
