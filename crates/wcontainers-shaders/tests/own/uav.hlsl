// A four-channel float storage texture written one half at a time, which shader model 6 allows
// as a load and a store per half.
RWTexture2D<float4> u : register(u1);

float4 ps_halves() : SV_Target
{
    u[uint2(0, 0)].xy = float2(1, 2);
    u[uint2(0, 0)].zw = float2(3, 4);
    return 0;
}
