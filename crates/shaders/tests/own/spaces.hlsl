// A shader of our own using register spaces, compiled by fxc (ps_5_1) and dxc (ps_6_0) by compile.cmd.

SamplerState linear_clamp : register(s0);
Texture2D<float4> first : register(t0, space0);
Texture2D<float4> second : register(t0, space1);

// Two textures at one register in two register spaces (shader model 5.1 and 6).
float4 ps_spaces(float4 position : SV_Position, float2 uv : TEXCOORD0) : SV_Target0 {
    return first.Sample(linear_clamp, uv) + second.Sample(linear_clamp, uv);
}
