// An array of textures, one element picked by the pixel's column, one by a constant.
Texture2D t[4] : register(t0);
SamplerState s : register(s0);

float4 ps_array(float4 position : SV_Position, float2 uv : TEXCOORD0) : SV_Target
{
    uint i = uint(position.x) % 4;
    return t[i].Sample(s, uv) + t[2].Load(int3(0, 0, 0));
}
