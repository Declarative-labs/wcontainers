// Resource binding tier 1's counts bound in one compute program (tests/answers.rs): 128 shader
// resource views, 14 constant buffers, 16 samplers and 8 unordered access views. Writes the sum of
// every texture's texel, of a sample through each sampler, of every constant and of every other
// view's first element.
Texture2D<float> t[128] : register(t0);
SamplerState s[16] : register(s0);
cbuffer c0 : register(b0) { float v0; };
cbuffer c1 : register(b1) { float v1; };
cbuffer c2 : register(b2) { float v2; };
cbuffer c3 : register(b3) { float v3; };
cbuffer c4 : register(b4) { float v4; };
cbuffer c5 : register(b5) { float v5; };
cbuffer c6 : register(b6) { float v6; };
cbuffer c7 : register(b7) { float v7; };
cbuffer c8 : register(b8) { float v8; };
cbuffer c9 : register(b9) { float v9; };
cbuffer c10 : register(b10) { float v10; };
cbuffer c11 : register(b11) { float v11; };
cbuffer c12 : register(b12) { float v12; };
cbuffer c13 : register(b13) { float v13; };
RWBuffer<uint> result : register(u0);
RWBuffer<uint> u1 : register(u1);
RWBuffer<uint> u2 : register(u2);
RWBuffer<uint> u3 : register(u3);
RWBuffer<uint> u4 : register(u4);
RWBuffer<uint> u5 : register(u5);
RWBuffer<uint> u6 : register(u6);
RWBuffer<uint> u7 : register(u7);

[numthreads(1, 1, 1)] void main()
{
    float loaded = 0;
    [unroll] for (int i = 0; i < 128; ++i)
        loaded += t[i].Load(int3(0, 0, 0));
    float sampled = 0;
    [unroll] for (int j = 0; j < 16; ++j)
        sampled += t[j].SampleLevel(s[j], float2(0.5, 0.5), 0);
    result[0] = asuint(loaded);
    result[1] = asuint(sampled);
    result[2] = asuint(v0 + v1 + v2 + v3 + v4 + v5 + v6 + v7 + v8 + v9 + v10 + v11 + v12 + v13);
    result[3] = u1[0] + u2[0] + u3[0] + u4[0] + u5[0] + u6[0] + u7[0];
}
