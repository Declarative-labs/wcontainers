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
