// Views of part of a resource or of another format of its typeless group.
// fxc: ps_5_0 ps_level0, ps_5_0 ps_array, ps_5_0 ps_uint, ps_5_0 ps_depth, ps_5_0 ps_stencil
struct FullOut { float4 pos : SV_Position; float2 uv : TEXCOORD; };
SamplerState smp : register(s0);
Texture2D t2 : register(t0);
Texture2DArray t2a : register(t0);
Texture2D<uint> tu : register(t0);
Texture2D<uint2> ts : register(t0);
float4 ps_level0(FullOut i) : SV_Target { return t2.SampleLevel(smp, i.uv, 0.0); }
float4 ps_array(FullOut i) : SV_Target { return t2a.SampleLevel(smp, float3(i.uv, floor(i.uv.x * 3.0)), 0.0); }
float4 ps_uint(FullOut i) : SV_Target {
    uint v = tu.Load(int3(int2(i.uv * 16.0), 0));
    return float4((v & 0xff) / 255.0, ((v >> 8) & 0xff) / 255.0, ((v >> 16) & 0xff) / 255.0, 1.0);
}
float4 ps_depth(FullOut i) : SV_Target { float d = t2.Load(int3(int2(i.uv * 32.0), 0)).r; return float4(d, d * d, 1.0 - d, 1.0); }
float4 ps_stencil(FullOut i) : SV_Target { uint s = ts.Load(int3(int2(i.uv * 32.0), 0)).g; return float4((s & 7) / 7.0, (s >> 3) / 31.0, 0.5, 1.0); }
