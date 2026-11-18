// Shaders for filtering in the program (tests/filter_draw.rs): a plane in perspective, near at the
// bottom (wider than the target) and six times as far at the top, so one draw magnifies and minifies,
// with a texture coordinate running from -1 to 2; the pixel shader samples in the way the constants
// pick.
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
    return t.Gather(s, i.uv);
}
