// Six small shaders written for the shaders crate's own check, compiled by fxc on the desktop:
//   fxc /nologo /T vs_5_0 /E vs_transform /Fo vs_transform.dxbc own.hlsl   (and so on for each)

cbuffer Camera : register(b0) {
    float4x4 world_view_projection;
    float4 tint;
};

struct VsOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint id : TEXCOORD1;
};

// A transform by a matrix from a constant buffer, passing a texture coordinate on.
VsOut vs_transform(float3 position : POSITION, float2 uv : TEXCOORD0, uint id : SV_VertexID) {
    VsOut o;
    o.position = mul(float4(position, 1.0), world_view_projection);
    o.uv = uv;
    o.id = id;
    return o;
}

Texture2D<float4> albedo : register(t0);
SamplerState linear_clamp : register(s0);

// A texture sampled and tinted, with alpha testing.
float4 ps_textured(VsOut i) : SV_Target0 {
    float4 c = albedo.Sample(linear_clamp, i.uv) * tint;
    clip(c.a - 0.5);
    return c;
}

// Integer work: shifts, masks and a switch on the vertex number.
uint4 ps_integer(VsOut i) : SV_Target0 {
    uint x = (i.id * 2654435761u) >> 7;
    switch (i.id & 3) {
    case 0: x ^= 0xff; break;
    case 1: x = reversebits(x); break;
    default: x = firstbithigh(x) | (x << 3); break;
    }
    return uint4(x, x & 0xffff, countbits(x), i.id);
}

// A loop whose trip count comes from a constant, carrying values around it.
float4 ps_loop(VsOut i) : SV_Target0 {
    float sum = 0.0;
    float weight = 1.0;
    for (int k = 0; k < (int)tint.w; k++) {
        sum += albedo.SampleLevel(linear_clamp, i.uv + float2(k * 0.01, 0.0), 0).r * weight;
        weight *= 0.5;
        if (sum > 4.0)
            break;
    }
    return float4(sum, weight, 0.0, 1.0);
}

// A full-screen triangle from SV_VertexID, as engines draw post-processing passes.
float4 vs_fullscreen(uint id : SV_VertexID) : SV_Position {
    float2 uv = float2((id << 1) & 2, id & 2);
    return float4(uv * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
}

// A colour from the pixel's window position, for drawing with the full-screen triangle.
float4 ps_window(float4 position : SV_Position) : SV_Target0 {
    float2 cell = frac(position.xy / 16.0);
    return float4(cell, position.x > position.y ? 1.0 : 0.25, 1.0);
}
