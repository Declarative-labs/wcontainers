// Shaders for the variant tests (tests/variant_draw.rs), with own.hlsl's inputs.
#include "own.hlsl"

struct DualOut {
    float4 colour : SV_Target0;
    float4 factor : SV_Target1;
};

// Two outputs for dual-source blending: the texture's colour and a factor from the coordinate.
DualOut ps_dual(VsOut i) {
    DualOut o;
    o.colour = albedo.Sample(linear_clamp, i.uv) * 0.5;
    o.factor = float4(saturate(i.uv), 1.0 - saturate(i.uv.x), 0.25);
    return o;
}

// The texture's level count and size, as GetDimensions gives them.
float4 ps_levels(VsOut i) : SV_Target0 {
    uint w, h, levels;
    albedo.GetDimensions(0, w, h, levels);
    return float4(levels / 8.0, w / 16.0, h / 32.0, saturate(i.uv.x));
}

// The targets' sample count.
float4 ps_samples(VsOut i) : SV_Target0 {
    return float4(GetRenderTargetSampleCount() / 8.0, saturate(i.uv), 1.0);
}
