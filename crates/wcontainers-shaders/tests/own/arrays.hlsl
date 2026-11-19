// An array of textures, one element picked by the pixel's column, one by a constant.
Texture2D t[4] : register(t0);
SamplerState s : register(s0);

float4 ps_array(float4 position : SV_Position, float2 uv : TEXCOORD0) : SV_Target
{
    uint i = uint(position.x) % 4;
    return t[i].Sample(s, uv) + t[2].Load(int3(0, 0, 0));
}

// An array of storage textures: each invocation reads its texel of an element picked at run time
// and stores into two others picked by its place. Only invocation (x, y) touches texel (x, y), so every result is determined.
RWTexture2D<uint> images[4] : register(u0);

[numthreads(8, 8, 1)]
void cs_image_array(uint3 id : SV_DispatchThreadID)
{
    uint before = images[(id.x + 2 * id.y) % 4][id.xy];
    uint i = (id.x + id.y) % 4;
    images[i][id.xy] = before + id.x * 16 + id.y + 1;
    images[3 - i][id.xy] = before ^ 30001;
}
