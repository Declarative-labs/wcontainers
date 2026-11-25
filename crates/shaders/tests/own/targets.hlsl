// A target narrower than its format: two components into a four-component target, whose other two
// Direct3D leaves undefined. Reads vs_transform's outputs.

struct VsOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint id : TEXCOORD1;
};

uint2 ps_two(VsOut i) : SV_Target0 {
    return uint2(i.position.xy) ^ uint2(i.id, i.id * 3u);
}
