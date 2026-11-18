// A texture both compared against and read plainly, as a shadow map often is.

Texture2D<float> shadow : register(t0);
SamplerState plain : register(s0);
SamplerComparisonState less : register(s1);

float4 ps_shadow(float4 pos : SV_Position, float2 uv : TEXCOORD) : SV_Target
{
    float lit = shadow.SampleCmp(less, uv, 0.5);
    float flat = shadow.SampleCmpLevelZero(less, uv, 0.25);
    float near = shadow.Sample(plain, uv);
    float top = shadow.SampleLevel(plain, uv, 0);
    float texel = shadow.Load(int3(pos.xy, 0));
    float4 four = shadow.Gather(plain, uv);
    return float4(lit + flat, near + top, texel, four.w);
}
