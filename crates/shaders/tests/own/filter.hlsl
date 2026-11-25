// Shaders for filtering in the program (tests/filter_draw.rs): a plane in perspective, near at the
// bottom (wider than the target) and six times as far at the top, so one draw magnifies and minifies,
// with a texture coordinate running from -1 to 2; the pixel shader samples in the way the constants
// pick, or loads the texel under the coordinate wrapped into the texture.
cbuffer Params : register(b0) {
    uint mode;
    float value;
    float2 scale;
};
Texture2D<float4> t : register(t0);
SamplerState s : register(s0);

struct FilterOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
};

FilterOut vs_filter(uint id : SV_VertexID) {
    static const uint corners[6] = { 0, 1, 2, 2, 1, 3 };
    uint c = corners[id];
    float2 corner = float2(c & 1, c >> 1);
    float w = lerp(1.0, 6.0, corner.y);
    FilterOut o;
    o.position = float4((corner.x * 2.0 - 1.0) * 2.5, (corner.y * 2.0 - 1.0) * w, 0.0, w);
    o.uv = corner * 3.0 - 1.0;
    return o;
}

float4 ps_filter(FilterOut i) : SV_Target {
    if (mode == 0) {
        return t.Sample(s, i.uv);
    }
    if (mode == 1) {
        return t.SampleBias(s, i.uv, value);
    }
    if (mode == 2) {
        return t.SampleLevel(s, i.uv, value * (i.uv.x + 1.0));
    }
    if (mode == 3) {
        return t.SampleGrad(s, i.uv, ddx(i.uv) * scale.x, ddy(i.uv) * scale.y);
    }
    if (mode == 4) {
        return t.Sample(s, i.uv, int2(1, -2));
    }
    if (mode == 5) {
        return t.Gather(s, i.uv);
    }
    uint width, height;
    t.GetDimensions(width, height);
    return t.Load(int3(int2(frac(i.uv) * float2(width, height)), 0));
}

// Samples with Direct3D's per-sample clamp, one above the value, in the way the constants pick: an
// implicit level, a bias, or gradients.
float4 ps_clamp(FilterOut i) : SV_Target {
    float lowest = value + 1.0;
    if (mode == 0) {
        return t.Sample(s, i.uv, int2(0, 0), lowest);
    }
    if (mode == 1) {
        return t.SampleBias(s, i.uv, scale.x - 1.0, int2(0, 0), lowest);
    }
    return t.SampleGrad(s, i.uv, ddx(i.uv) * scale.x, ddy(i.uv) * scale.y, int2(0, 0), lowest);
}

// The level of detail an implicit sample takes, clamped and unclamped, at the coordinate and at the
// coordinate scaled by `scale` (an anisotropic footprint where the two scales differ).
float4 ps_lod(FilterOut i) : SV_Target {
    float2 scaled = i.uv * scale;
    return float4(t.CalculateLevelOfDetail(s, i.uv), t.CalculateLevelOfDetailUnclamped(s, i.uv),
                  t.CalculateLevelOfDetail(s, scaled), t.CalculateLevelOfDetailUnclamped(s, scaled));
}
