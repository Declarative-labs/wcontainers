// A component of a constant buffer picked at run time (fxc and dxc): dxc reads such components
// through a view of the buffer as scalars. Reads vs_transform's outputs and own.hlsl's constants.

cbuffer Camera : register(b0) {
    float4x4 world_view_projection;
    float4 tint;
};

struct VsOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint id : TEXCOORD1;
};

// The tint's component and a matrix element, each picked by the pixel's position; written as raw
// bits so the comparison is exact.
uint4 ps_component(VsOut i) : SV_Target0 {
    uint column = (uint)i.position.x & 3;
    uint row = ((uint)i.position.y >> 2) & 3;
    float t = tint[column];
    float m = world_view_projection[row][column];
    float n = world_view_projection[column][3 - row];
    return uint4(asuint(t), asuint(m), asuint(n), column + row * 4);
}
