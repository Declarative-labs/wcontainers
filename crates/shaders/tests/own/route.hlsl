// Shaders compiled by both fxc and dxc (compile.cmd), drawn by both routes against real Direct3D 11's
// picture of the fxc build: a constant buffer indexed at run time, bound at the size it declares, and
// discards inside a loop followed by an implicit-derivative sample. They share own.hlsl's constants
// (read here as five vectors), texture and sampler, and read vs_transform's outputs.

cbuffer Camera : register(b0) {
    float4 rows[5];
};

Texture2D<float4> albedo : register(t0);
SamplerState linear_clamp : register(s0);

struct VsOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint id : TEXCOORD1;
};

// One of the five vectors, chosen by the pixel's column.
float4 ps_rows(VsOut i) : SV_Target0 {
    uint k = uint(i.position.x) % 5;
    return frac(rows[k] * 0.25 + i.uv.xyxy);
}

// Discards where a channel is dark, from inside a loop; the survivors sample again, which needs the
// discarded neighbours' derivatives.
float4 ps_clip(VsOut i) : SV_Target0 {
    float4 s = albedo.Sample(linear_clamp, i.uv);
    float c[4] = { s.x, s.y, s.z, s.w };
    [loop] for (uint k = 0; k < 4; k++) {
        if (c[k] < 0.25)
            discard;
        c[k] *= 0.5;
    }
    return float4(c[0], c[1], c[2], c[3]) + albedo.Sample(linear_clamp, i.uv * 2.0) * 0.5;
}
