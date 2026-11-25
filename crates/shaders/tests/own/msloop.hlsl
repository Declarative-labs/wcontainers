// Shader model 4.0 loads of a multisampled texture: an unrolled loop picking the sample, and the
// coordinates swapped.

Texture2DMS<float4, 1> t : register(t0);

float4 ps_msloop(float4 pos : SV_Position) : SV_Target
{
    int i;
    float4 o;
    for (i = 0; i < 1; i++)
    {
        o = t.Load(pos.xy, i);
    }
    return o;
}

float4 ps_msswap(float4 pos : SV_Position) : SV_Target
{
    return t.Load(pos.yx, 0);
}
