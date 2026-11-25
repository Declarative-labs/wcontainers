// Typed buffers: loads through views of any format the test binds, their lengths, elements past
// the length, stores converted into the bound view's format (the values stored are written to the
// results too), and atomics on a 32-bit view.

Buffer<float4> floats : register(t0);
Buffer<uint4> uints : register(t1);
Buffer<int4> ints : register(t2);
Buffer<float2> pairs : register(t3);
RWBuffer<float4> written : register(u0);
RWBuffer<uint> counts : register(u1);
RWStructuredBuffer<uint4> results : register(u2);

[numthreads(64, 1, 1)]
void cs_texels(uint i : SV_DispatchThreadID) {
    uint n0, n1, n2, n3, n4, n5;
    floats.GetDimensions(n0);
    uints.GetDimensions(n1);
    ints.GetDimensions(n2);
    pairs.GetDimensions(n3);
    written.GetDimensions(n4);
    counts.GetDimensions(n5);
    results[i * 6 + 0] = asuint(floats[i]);
    results[i * 6 + 1] = uints[i];
    results[i * 6 + 2] = asuint(ints[i]);
    results[i * 6 + 3] = uint4(asuint(pairs[i]), n0, n1);
    results[i * 6 + 4] = uint4(n2, n3, n4, n5);

    // Values from tiny to past a half float's range, both signs, and infinities and a NaN.
    float scale = asfloat((i % 40 + 103) << 23);
    float4 v = (float(i) - 60.0) * float4(0.013, 1.7, -0.29, 3.1) * scale;
    if (i == 5) {
        v.x = asfloat(0x7f800000u);
    }
    if (i == 6) {
        v.y = asfloat(0xff800000u);
    }
    if (i == 7) {
        v.z = asfloat(0x7fc00000u);
    }
    written[i] = v;
    results[i * 6 + 5] = asuint(v);

    InterlockedAdd(counts[i % 16], i + 1);
    InterlockedMax(counts[16 + i % 8], i * 2654435761u);
    InterlockedOr(counts[24 + i % 8], 1u << (i % 32));
    if (i >= 32) {
        counts[i] = counts[i] * 3 + 1;
    }
}

// Typed buffers in a pixel shader, as WebGL 2 reads them through textures: pixel (x, y) of a 64 by
// 64 target reads element (x / 2 + 32 y) % 160, of the float view in even columns and the unsigned
// one in odd ones; pixel (0, 0) gives the two lengths.
uint4 ps_texels(float4 position : SV_Position) : SV_Target0 {
    uint2 p = uint2(position.xy);
    uint i = (p.x / 2 + 32 * p.y) % 160;
    if (i == 0) {
        uint n, m;
        floats.GetDimensions(n);
        uints.GetDimensions(m);
        return uint4(n, m, 0, 0);
    }
    return p.x % 2 == 0 ? asuint(floats[i]) : uints[i];
}

// Storage textures read and written through buffers of their texels: atomics on a 32-bit unsigned
// texture, a float texture read and written, a 2D array and a 3D texture of formats the test picks
// written, and their sizes; some invocations reach past the views.
RWTexture2D<uint> marks : register(u3);
RWTexture2D<float> heights : register(u4);
RWTexture2DArray<float4> layered : register(u5);
RWTexture3D<uint4> volume : register(u6);

[numthreads(8, 8, 1)]
void cs_through(uint3 id : SV_DispatchThreadID) {
    uint w, h, l, vw, vh, vd;
    layered.GetDimensions(w, h, l);
    volume.GetDimensions(vw, vh, vd);
    // Each texel takes one kind of atomic, so the result does not depend on their order.
    InterlockedAdd(marks[uint2(id.x % 8, id.y % 4)], id.x + 1);
    InterlockedMax(marks[uint2(id.y, 4 + id.x % 3)], id.x * 2654435761u);
    uint before;
    InterlockedOr(marks[uint2(id.x % 8, 7)], 1u << id.y, before);
    heights[id.xy] = heights[id.xy] * 2.0 + float(w + h * 100 + l * 10000);
    // Exact in f32 however the arithmetic is ordered or fused.
    float4 v = float4(int3(id.xyx) - 7, 1) * float4(0.25, 0.5, -0.125, 2.0) * float(vw + vh + vd);
    layered[uint3(id.xy, id.x % 4)] = v;
    volume[uint3(id.x, id.y, id.x % 3)] = uint4(id.x, id.y, vd, 300 - id.x * 37);
}

// A storage texture loaded and stored whole (tests bind an `A8_UNORM` view, which loads give as
// (0, 0, 0, a)); the stored alpha depends on every component loaded, and no code it gives lies
// halfway between two.
RWTexture2D<unorm float4> alpha : register(u0);

[numthreads(8, 8, 1)]
void cs_alpha(uint3 id : SV_DispatchThreadID) {
    float4 v = alpha[id.xy];
    alpha[id.xy] = float4(v.w, v.x, v.y, v.w * 0.75 + 0.125 + (v.x + v.y + v.z) * 0.5);
}

// A vertex shader reading a typed buffer per vertex and instance, with the draw's bases.
float4 vs_texels(uint v : SV_VertexID, uint n : SV_InstanceID) : SV_Position {
    return floats[v] + floats[n].yxwz;
}
