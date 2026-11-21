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

// Arrays of constant buffers and typed buffers, two declared without a length (sized by the root
// signature's ranges), each element picked at run time: invocation i writes element i of the views
// it picks, so every result is determined.
struct Pick
{
    uint4 v;
};
ConstantBuffer<Pick> picks[2] : register(b0);
Buffer<uint> sources[3] : register(t0);
Buffer<uint> rest[] : register(t3);
RWBuffer<uint> targets[2] : register(u0);
RWBuffer<uint> more[] : register(u2);

[numthreads(8, 1, 1)]
void cs_buffer_array(uint3 id : SV_DispatchThreadID)
{
    uint i = id.x;
    uint4 p = picks[i % 2].v;
    targets[(p.x + i) % 2][i] = sources[(p.y + i) % 3][i] + rest[(p.z + i) % 2][i];
    more[(p.w + i) % 2][i] = sources[2][i] ^ rest[i % 2][i];
}

// Arrays of typed buffers alone, one of each kind declared without a length (sized by the root
// signature's ranges), each element picked at run time from a constant: invocation i writes element
// i of the view it picks, so every result is determined.
cbuffer choice : register(b0)
{
    uint4 pick;
};
Buffer<uint> loaded[3] : register(t0);
Buffer<uint> loaded_rest[] : register(t3);
RWBuffer<uint> stored[] : register(u0);

[numthreads(8, 1, 1)]
void cs_typed_array(uint3 id : SV_DispatchThreadID)
{
    uint i = id.x;
    stored[(pick.x + i) % 2][i] = loaded[(pick.y + i) % 3][i] + loaded_rest[(pick.z + i) % 2][i];
    stored[2][i] = loaded[2][i] ^ loaded_rest[(pick.w + i) % 2][i];
}
