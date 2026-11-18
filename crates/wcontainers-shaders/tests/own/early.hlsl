// Early depth and stencil tests: the pixel shader with [earlydepthstencil] marks each pixel it runs
// for in a storage buffer, so only fragments that pass the tests leave marks; the triangle's
// corners come from the constant buffer.

cbuffer corners : register(b0) {
    float4 corner[3];
};

RWStructuredBuffer<uint> marks : register(u1);

float4 vs_early(uint v : SV_VertexID) : SV_Position {
    return corner[v];
}

[earlydepthstencil]
float4 ps_early(float4 p : SV_Position) : SV_Target {
    uint at = uint(p.y) * 64 + uint(p.x);
    InterlockedAdd(marks[at], 1);
    return float4(p.z, 0.25, 0.5, 1);
}

float4 ps_mark(float4 p : SV_Position) : SV_Target {
    return float4(0, 1, 0, 1);
}

// Every pixel takes a signed minimum and maximum and an unsigned one of the same words, so one
// signedness runs as a compare-exchange loop; some pixels discard first, and the triangle's edges
// inside the target give helper lanes. Each pixel that ran marks its place after the atomics (2
// where the minimum gave back its own value, which no other pixel holds).
RWByteAddressBuffer words : register(u2);

float4 ps_contend(float4 p : SV_Position) : SV_Target {
    uint at = uint(p.y) * 64 + uint(p.x);
    int v = int(at * 2654435761u);
    if (at % 7 == 3) {
        discard;
    }
    int low;
    words.InterlockedMin(0, v, low);
    words.InterlockedMax(4, v);
    words.InterlockedMin(8, at * 40503u);
    words.InterlockedMax(12, at * 40503u);
    words.Store(16 + 4 * at, 1 + uint(low == v));
    return float4(ddx(p.x), ddy(p.y), 0.5, 1);
}
