// Sampling each texture kind over the target's uv: 1D and its arrays, 2D arrays, 3D, cubes and their
// arrays, a mip chain by explicit level, and exact texels by Load.
// fxc: ps_5_0 ps_1d, ps_5_0 ps_1d_array, ps_5_0 ps_2d_array, ps_5_0 ps_3d, ps_5_0 ps_cube, ps_5_0 ps_cube_array, ps_5_0 ps_mips, ps_5_0 ps_load
// dxc: ps_6_0 ps_1d, ps_6_0 ps_1d_array, ps_6_0 ps_2d_array, ps_6_0 ps_3d, ps_6_0 ps_cube, ps_6_0 ps_cube_array, ps_6_0 ps_mips, ps_6_0 ps_load
struct FullOut { float4 pos : SV_Position; float2 uv : TEXCOORD; };
SamplerState smp : register(s0);
Texture1D t1 : register(t0);
Texture1DArray t1a : register(t0);
Texture2DArray t2a : register(t0);
Texture3D t3 : register(t0);
TextureCube tc : register(t0);
TextureCubeArray tca : register(t0);
Texture2D t2 : register(t0);

float4 ps_1d(FullOut i) : SV_Target { return t1.SampleLevel(smp, i.uv.x * 1.5, i.uv.y * 4.0); }
float4 ps_1d_array(FullOut i) : SV_Target { return t1a.Sample(smp, float2(i.uv.x, floor(i.uv.y * 4.0))); }
float4 ps_2d_array(FullOut i) : SV_Target {
    float layer = floor(i.uv.x * 2.0) + 2.0 * floor(i.uv.y * 2.0);
    return t2a.Sample(smp, float3(frac(i.uv * 2.0), layer));
}
float4 ps_3d(FullOut i) : SV_Target { return t3.Sample(smp, float3(i.uv, frac(i.uv.x + i.uv.y))); }
float3 direction(float2 uv) {
    float a = uv.x * 6.2831853, b = (uv.y - 0.5) * 3.1415926;
    return float3(cos(b) * cos(a), sin(b), cos(b) * sin(a));
}
float4 ps_cube(FullOut i) : SV_Target { return tc.Sample(smp, direction(i.uv)); }
float4 ps_cube_array(FullOut i) : SV_Target {
    float2 uv = float2(frac(i.uv.x * 2.0), i.uv.y);
    return tca.Sample(smp, float4(direction(uv), floor(i.uv.x * 2.0)));
}
float4 ps_mips(FullOut i) : SV_Target { return t2.SampleLevel(smp, i.uv * 2.0, i.uv.y * 6.0); }
float4 ps_load(FullOut i) : SV_Target { return t2.Load(int3(int2(i.pos.xy) % 32, 0)); }
