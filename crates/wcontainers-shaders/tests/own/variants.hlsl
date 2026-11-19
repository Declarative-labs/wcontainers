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

// The accesses a per-resource minimum level-of-detail clamp changes, by the pixel's column: an implicit
// sample, a sample at level 0.25, and loads of the level the row's quarter picks (no bit fields, which
// GLSL ES 3.00 lacks).
float4 ps_minlod(VsOut i) : SV_Target0 {
    uint2 p = uint2(i.position.xy);
    float4 implicit_level = albedo.Sample(linear_clamp, i.uv);
    float4 given_level = albedo.SampleLevel(linear_clamp, i.uv, 0.25);
    uint level = min(uint(i.position.y / 16.0), 3);
    float4 loaded = albedo.Load(int3(int2(p % 8) >> level, level));
    return p.x < 21 ? implicit_level : (p.x < 42 ? given_level : loaded);
}

// A gather, which reads level 0 only.
float4 ps_mingather(VsOut i) : SV_Target0 {
    return albedo.GatherGreen(linear_clamp, i.uv);
}

struct ClipOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint id : TEXCOORD1;
    float2 clip : SV_ClipDistance0;
};

// vs_transform with two clip distances, a diagonal across the grid and a line below its middle, both
// cutting through triangles.
ClipOut vs_clip(float3 position : POSITION, float2 uv : TEXCOORD0, uint id : SV_VertexID) {
    ClipOut o;
    o.position = mul(float4(position, 1.0), world_view_projection);
    o.uv = uv;
    o.id = id;
    o.clip = float2(position.x + position.y * 0.5 + 0.1, position.y + 0.3);
    return o;
}

struct ClipSixOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint id : TEXCOORD1;
    float4 near : SV_ClipDistance0;
    float2 far : SV_ClipDistance1;
};

// vs_transform with six clip distances over two registers: a frame cut from each side of the grid
// and two diagonals cutting its corners.
ClipSixOut vs_clip_six(float3 position : POSITION, float2 uv : TEXCOORD0, uint id : SV_VertexID) {
    ClipSixOut o;
    o.position = mul(float4(position, 1.0), world_view_projection);
    o.uv = uv;
    o.id = id;
    o.near = float4(position.x + 0.7, 0.8 - position.x, position.y + 0.75, 0.6 - position.y);
    o.far = float2(position.x + position.y + 1.1, 0.9 - position.x + position.y);
    return o;
}

// vs_transform with depth stretched past the clip volume on both sides: z/w runs from -0.2 to 1.15
// across the grid, where vs_transform's runs from 0.4 to 0.67.
VsOut vs_deep(float3 position : POSITION, float2 uv : TEXCOORD0, uint id : SV_VertexID) {
    VsOut o = vs_transform(position, uv, id);
    o.position.z = o.position.z * 5.0 - o.position.w * 2.2;
    return o;
}

// A sample outside the texture across the grid, for addressing: u and v from -4.5 to 4.5, about one
// texel a pixel, so a texture with levels is sampled between levels 0 and 1.
float4 ps_mirror(VsOut i) : SV_Target0 {
    return albedo.Sample(linear_clamp, i.uv * 6.0 - 3.0) * tint;
}
