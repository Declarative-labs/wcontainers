// One texture read through two register spaces (dxc, -D SPACES: t0 in spaces 0 and 1, which Direct3D
// 12 binds to the same view), and the same program reading t0 twice for Direct3D 11 (fxc), so both
// pictures are alike. Drawn with own.hlsl's vs_transform.

cbuffer Camera : register(b0) {
    float4x4 world_view_projection;
    float4 tint;
};

struct VsOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint id : TEXCOORD1;
};

SamplerState linear_clamp : register(s0);
Texture2D<float4> first : register(t0);
#ifdef SPACES
Texture2D<float4> second : register(t0, space1);
#else
#define second first
#endif

float4 ps_twospaces(VsOut i) : SV_Target0 {
    return (first.Sample(linear_clamp, i.uv) + second.Sample(linear_clamp, i.uv.yx)) * 0.5 * tint;
}
