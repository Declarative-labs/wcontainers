// Loads of each component kind written out as words, for textures bound through views of another
// kind than declared (variant/view_kind.rs).
Texture2D<float4> floats : register(t0);
Texture2D<uint4> uints : register(t1);
Texture2D<int4> ints : register(t2);
RWTexture2D<uint4> loaded : register(u0);

[numthreads(8, 8, 1)]
void cs_view_kinds(uint3 id : SV_DispatchThreadID) {
    int3 at = int3(id.xy, 0);
    loaded[id.xy] = asuint(floats.Load(at));
    loaded[id.xy + uint2(8, 0)] = uints.Load(at);
    loaded[id.xy + uint2(16, 0)] = asuint(ints.Load(at));
}

// A sample through a view of another kind, which the conversion refuses by name.
SamplerState linear_clamp : register(s0);

[numthreads(8, 8, 1)]
void cs_view_sampled(uint3 id : SV_DispatchThreadID) {
    loaded[id.xy] = asuint(floats.SampleLevel(linear_clamp, id.xy / 8.0, 0.0));
}
