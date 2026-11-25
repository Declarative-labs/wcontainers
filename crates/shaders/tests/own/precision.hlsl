// Minimum-precision constants read and written out widened, a word each: vectors packed as the
// compiler lays them out, and arrays (an element a row) picked by a constant and at run time.
// fxc (shader model 5.0) holds them in 32 bits, dxc (shader model 6.0, no 16-bit types) in 16.

cbuffer Packed : register(b0) {
    min16float4 f4;
    min16float3 f3;
    min16float2 f2;
    min16float f1;
    min16uint4 u4;
    min16int3 i3;
    min16uint u1;
    min16int2 i2;
    min16uint2 u2;
};

cbuffer Rows : register(b1) {
    min16float fa[4];
    min16int ia[4];
    uint pick;
};

RWStructuredBuffer<uint> widened : register(u0);

[numthreads(1, 1, 1)]
void cs_min16() {
    uint n = 0;
    widened[n++] = asuint((float)f4.x);
    widened[n++] = asuint((float)f4.y);
    widened[n++] = asuint((float)f4.z);
    widened[n++] = asuint((float)f4.w);
    widened[n++] = asuint((float)f3.x);
    widened[n++] = asuint((float)f3.y);
    widened[n++] = asuint((float)f3.z);
    widened[n++] = asuint((float)f2.x);
    widened[n++] = asuint((float)f2.y);
    widened[n++] = asuint((float)f1);
    widened[n++] = (uint)u4.x;
    widened[n++] = (uint)u4.y;
    widened[n++] = (uint)u4.z;
    widened[n++] = (uint)u4.w;
    widened[n++] = asuint((int)i3.x);
    widened[n++] = asuint((int)i3.y);
    widened[n++] = asuint((int)i3.z);
    widened[n++] = (uint)u1;
    widened[n++] = asuint((int)i2.x);
    widened[n++] = asuint((int)i2.y);
    widened[n++] = (uint)u2.x;
    widened[n++] = (uint)u2.y;
    widened[n++] = asuint((float)fa[2]);
    widened[n++] = asuint((int)ia[1]);
    [loop] for (uint k = 0; k < 4; k++) {
        widened[n + 2 * k] = asuint((float)fa[(pick + k) & 3]);
        widened[n + 2 * k + 1] = asuint((int)ia[(pick + 3 * k) & 3]);
    }
}
