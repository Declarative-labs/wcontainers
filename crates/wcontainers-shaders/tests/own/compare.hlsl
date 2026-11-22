// Float comparisons, minimum, maximum and saturate of operand pairs among which are NaNs, infinities
// and signed zeros, so the converted program's NaN rules can be compared with Direct3D's (an
// unordered `ne`, the ordered rest, min and max giving the number over a NaN), by a compute shader
// and by a pixel shader reading vs_transform's outputs and own.hlsl's constants. Built with IEEE
// strictness, so neither compiler folds a negated comparison into another.

cbuffer Camera : register(b0) {
    float4x4 world_view_projection;
    float4 tint;
};

struct VsOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint id : TEXCOORD1;
};

StructuredBuffer<uint> operands : register(t0);
RWStructuredBuffer<uint> results : register(u0);

// The float's bits, with every NaN as one, since Direct3D leaves a NaN result's bits open.
uint bits(float x) {
    uint b = asuint(x);
    return (b & 0x7fffffff) > 0x7f800000 ? 0x7fc00000 : b;
}

// Each comparison's result as one bit.
uint compared(float a, float b) {
    uint r = 0;
    r |= a == b ? 1u : 0u;
    r |= a != b ? 2u : 0u;
    r |= a < b ? 4u : 0u;
    r |= a <= b ? 8u : 0u;
    r |= a > b ? 16u : 0u;
    r |= a >= b ? 32u : 0u;
    r |= !(a < b) ? 64u : 0u;
    r |= !(a <= b) ? 128u : 0u;
    r |= !(a > b) ? 256u : 0u;
    r |= !(a >= b) ? 512u : 0u;
    r |= (a < b || a > b) ? 1024u : 0u;
    r |= !(a < b || a > b) ? 2048u : 0u;
    bool2 ne = float2(a, b) != float2(b, 1.0);
    r |= ne.x ? 4096u : 0u;
    r |= ne.y ? 8192u : 0u;
    r |= any(float2(a, b) != float2(b, a)) ? 16384u : 0u;
    r |= (a != b && b != 1.0) ? 32768u : 0u;
    return r;
}

[numthreads(64, 1, 1)]
void cs_compare(uint i : SV_DispatchThreadID) {
    float a = asfloat(operands[2 * i]);
    float b = asfloat(operands[2 * i + 1]);
    uint r = compared(a, b);
    [branch] if (a != b) {
        results[4 * i] = r | 65536u;
    } else {
        results[4 * i] = r;
    }
    results[4 * i + 1] = bits(min(a, b));
    results[4 * i + 2] = bits(max(a, b));
    // A float as a condition: true unless it equals zero (NaN is true).
    [branch] if (b) {
        results[4 * i + 3] = bits(saturate(a));
    } else {
        results[4 * i + 3] = 1u;
    }
}

// The special values, made at run time (the tint is one), so no compiler folds them.
static const uint specials[8] = { 0x7fc00000, 0xffc00001, 0x7f800000, 0xff800000,
                                  0x80000000, 0x3f800000, 0xbf800000, 0x3f000000 };

// The same rules for the pair of special values picked by the pixel's position.
uint4 ps_compare(VsOut i) : SV_Target0 {
    float a = asfloat(specials[(uint)i.position.x & 7]) * tint.x;
    float b = asfloat(specials[(uint)i.position.y & 7]) * tint.y;
    return uint4(compared(a, b), bits(min(a, b)), bits(max(a, b)), bits(saturate(a)));
}
