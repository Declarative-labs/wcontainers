// Multisampling: shading once per sample, writing the coverage mask, and reading single samples of
// a multisampled texture.
// fxc: ps_5_0 ps_sample, ps_5_0 ps_coverage, vs_5_0 vs_full, ps_5_0 ps_load
// dxc: ps_6_0 ps_sample, ps_6_0 ps_coverage, ps_6_0 ps_load
struct ColorOut { float4 pos : SV_Position; float4 color : COLOR; };
float4 ps_sample(ColorOut i, uint sample : SV_SampleIndex) : SV_Target {
    return float4(i.color.rgb * (0.25 + 0.25 * sample), 1.0);
}
struct Coverage { float4 color : SV_Target; uint mask : SV_Coverage; };
Coverage ps_coverage(ColorOut i) {
    Coverage o;
    o.color = i.color;
    o.mask = (uint(i.pos.x) / 8 + uint(i.pos.y) / 8) & 0xf;
    return o;
}
Texture2DMS<float4, 4> samples : register(t0);
struct FullOut { float4 pos : SV_Position; float2 uv : TEXCOORD; };
FullOut vs_full(uint id : SV_VertexID) {
    FullOut o;
    float2 uv = float2((id << 1) & 2, id & 2);
    o.pos = float4(uv * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    o.uv = uv;
    return o;
}
// Each quarter of the target shows one sample of the texture.
float4 ps_load(FullOut i) : SV_Target {
    uint2 p = uint2(i.pos.xy);
    uint quarter = (p.x >= 64 ? 1 : 0) + (p.y >= 64 ? 2 : 0);
    return samples.Load(int2(p % 64) * 2, quarter);
}
