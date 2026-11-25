// More shaders written for the shaders crate's own check, compiled by fxc on the desktop by compile.cmd.
// They share own.hlsl's constants, texture and sampler.

cbuffer Camera : register(b0) {
    float4x4 world_view_projection;
    float4 tint;
};

Texture2D<float4> albedo : register(t0);
SamplerState linear_clamp : register(s0);

struct VsOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint id : TEXCOORD1;
};

struct ShadeOut {
    float4 position : SV_Position;
    float4 colour : COLOR0;
    float2 uv : TEXCOORD0;
};

// A transform passing on a colour that differs at every vertex, for pixel shaders that interpolate it
// in other ways than the default.
ShadeOut vs_shade(float3 position : POSITION, float2 uv : TEXCOORD0) {
    ShadeOut o;
    o.position = mul(float4(position, 1.0), world_view_projection);
    o.colour = float4(frac(position.xy * 1.7 + 0.3), uv.x * 0.5, 1.0);
    o.uv = uv;
    return o;
}

// The colour of the provoking vertex, uninterpolated.
float4 ps_flat(float4 position : SV_Position, nointerpolation float4 colour : COLOR0) : SV_Target0 {
    return colour;
}

// The colour at the centroid and the coordinate interpolated without perspective.
float4 ps_shade(float4 position : SV_Position, centroid float4 colour : COLOR0,
                noperspective float2 uv : TEXCOORD0) : SV_Target0 {
    return float4(colour.rg, frac(uv * 3.0));
}

// The pixel's depth and its clip w.
float4 ps_depth(float4 position : SV_Position) : SV_Target0 {
    return float4(position.z, frac(position.w * 4.0), 1.0 / position.w - 0.5, 1.0);
}

// A local array indexed by a value known only when the shader runs.
float4 ps_local(VsOut i) : SV_Target0 {
    float local[6];
    for (int k = 0; k < 6; k++)
        local[k] = albedo.SampleLevel(linear_clamp, float2(k / 6.0, i.uv.y), 0).g + k * 0.1;
    uint w = (uint)(saturate(i.uv.x) * 5.0);
    return float4(local[w], local[5 - w], frac(i.uv * 2.0));
}

// The texture's size at its first level.
float4 ps_size(VsOut i) : SV_Target0 {
    uint w, h;
    albedo.GetDimensions(w, h);
    return float4(w / 16.0, h / 32.0, frac(i.uv));
}
