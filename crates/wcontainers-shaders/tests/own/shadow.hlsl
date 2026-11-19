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

// The same texture read whole, four components, as a view with a component mapping is: each pixel
// takes the load or the level-zero sample by its column; the comparison only makes it a depth texture
// (tested against a bound the compilers cannot fold, which no comparison result reaches).
Texture2D<float4> wide : register(t0);

float4 ps_shadow_wide(float4 pos : SV_Position, float2 uv : TEXCOORD) : SV_Target
{
    float lit = wide.SampleCmpLevelZero(less, uv, 0.5);
    float4 texel = wide.Load(int3(pos.xy, 0));
    float4 near = wide.SampleLevel(plain, uv, 0);
    float4 read = (uint(pos.x) & 1) ? texel : near;
    return lit > uv.x + 2 ? lit.xxxx : read;
}
