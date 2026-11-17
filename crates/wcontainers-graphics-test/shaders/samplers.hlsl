// Sampling with a sampler per cell: a uv running past 0 and 1, and a scale growing down the cell so
// the level of detail and the anisotropy change; and comparison sampling of a depth texture.
// fxc: ps_5_0 ps_sample, ps_5_0 ps_compare, ps_5_0 ps_compare_zero, ps_5_0 ps_gather
cbuffer Cell : register(b0) { float4 rect; }; // left, top, width, height in pixels
struct FullOut { float4 pos : SV_Position; float2 uv : TEXCOORD; };
Texture2D tex : register(t0);
SamplerState smp : register(s0);
SamplerComparisonState cmp : register(s1);
float2 cell_uv(float4 pos) { return saturate((pos.xy - rect.xy) / rect.zw); }
float4 ps_sample(FullOut i) : SV_Target {
    float2 t = cell_uv(i.pos);
    float2 uv = (t - 0.33) * float2(3.0, 3.0 + 9.0 * t.y * t.y);
    return tex.Sample(smp, uv);
}
float4 ps_compare(FullOut i) : SV_Target {
    float2 t = cell_uv(i.pos);
    float v = tex.SampleCmp(cmp, t * 1.3 - 0.15, t.x);
    return float4(v, v * 0.5, 1.0 - v, 1.0);
}
float4 ps_compare_zero(FullOut i) : SV_Target {
    float2 t = cell_uv(i.pos);
    float v = tex.SampleCmpLevelZero(cmp, t, 1.0 - t.y);
    return float4(v, 1.0 - v, 0.3, 1.0);
}
float4 ps_gather(FullOut i) : SV_Target {
    float2 t = cell_uv(i.pos);
    return tex.GatherRed(smp, t * 1.2 - 0.1);
}
