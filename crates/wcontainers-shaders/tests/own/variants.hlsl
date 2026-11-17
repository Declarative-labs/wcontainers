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

struct IdOut {
    float4 position : SV_Position;
    nointerpolation uint2 ids : TEXCOORD0;
};

// The grid, each instance in its own half, carrying the vertex and instance numbers on: for draws
// with non-zero bases.
IdOut vs_ids(float3 position : POSITION, uint vertex : SV_VertexID, uint instance : SV_InstanceID) {
    IdOut o;
    o.position = float4(position.x * 0.5 - 0.5 + instance, position.y, 0.0, 1.0);
    o.ids = uint2(vertex, instance);
    return o;
}

// A colour from the triangle's number in the draw (the same for its three vertices, whichever is the
// provoking one) and the instance number.
float4 ps_ids(IdOut i) : SV_Target0 {
    uint triangle_number = i.ids.x / 3;
    return float4((triangle_number % 16) / 15.0, (triangle_number / 16) / 15.0, i.ids.y / 3.0, 1.0);
}
