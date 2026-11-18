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
