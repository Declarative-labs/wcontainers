// Direct3D 12 programs: pixel shaders reading each kind of root signature binding, a compute shader
// writing a texture through a descriptor table, and a compute shader adding into a buffer.
// dxc: ps_6_0 ps_constants, ps_6_0 ps_root_cbv, ps_6_0 ps_root_srv, ps_6_0 ps_table, ps_6_0 ps_static, ps_6_0 ps_bundle, cs_6_0 cs_image, cs_6_0 cs_add, ps_6_0 ps_image
struct ColorOut { float4 pos : SV_Position; float4 color : COLOR; };
cbuffer Constants : register(b0) { float4 constants; };
cbuffer RootBuffer : register(b1) { float4 root_cbv; };
StructuredBuffer<float4> root_srv : register(t0);
cbuffer TableBuffer : register(b2) { float4 table_cbv; };
Texture2D table_texture : register(t1);
SamplerState table_sampler : register(s0);
SamplerState static_sampler : register(s1);

float4 ps_constants(ColorOut i) : SV_Target { return constants * i.color; }
float4 ps_root_cbv(ColorOut i) : SV_Target { return root_cbv * i.color; }
float4 ps_root_srv(ColorOut i) : SV_Target { return root_srv[uint(i.pos.x) % 4] * i.color; }
float4 ps_table(ColorOut i) : SV_Target {
    return table_texture.Sample(table_sampler, i.pos.xy / 24.0) * table_cbv;
}
float4 ps_static(ColorOut i) : SV_Target { return table_texture.Sample(static_sampler, i.pos.xy / 24.0); }
float4 ps_bundle(ColorOut i) : SV_Target { return float4(i.color.rgb * constants.x, 1.0); }

RWTexture2D<float4> image : register(u0);
[numthreads(8, 8, 1)]
void cs_image(uint3 id : SV_DispatchThreadID) {
    image[id.xy] = float4(id.x / 64.0, id.y / 64.0, ((id.x * id.y) & 31) / 31.0, 1.0);
}
RWByteAddressBuffer sums : register(u1);
[numthreads(64, 1, 1)]
void cs_add(uint3 id : SV_DispatchThreadID) {
    uint old;
    sums.InterlockedAdd((id.x % 4) * 4, id.x + constants.x, old);
}
Texture2D<float4> shown : register(t1);
float4 ps_image(ColorOut i) : SV_Target { return shown.Load(int3(int2(i.pos.xy) % 64, 0)); }
