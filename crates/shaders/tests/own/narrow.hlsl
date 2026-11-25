// 16-bit integers: ps_narrow in minimum precision (fxc and dxc, shader model 6.0: dxc gives 16-bit
// integers, fxc 32-bit registers), on values that stay inside 16 bits so both builds agree;
// ps_narrow16 in native 16-bit types (dxc, shader model 6.2 with -enable-16bit-types), on values
// that wrap at 16 bits. Each reads vs_transform's outputs and own.hlsl's constants.

cbuffer Camera : register(b0) {
    float4x4 world_view_projection;
    float4 tint;
};

struct VsOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint id : TEXCOORD1;
};

// Unsigned division, arithmetic shifts, comparisons, minimum, maximum, absolute value and conversions
// both ways, on values running negative (fxc has no signed division of minimum precision).
uint4 ps_narrow(VsOut i) : SV_Target0 {
    min16int x = (min16int)i.position.x - 32;
    min16int y = (min16int)(i.uv.y * 40.0) - 20;
    min16int q = x >> 2;
    min16int r = y & 5;
    min16int s = (x * 100) >> 3;
    min16uint u = (min16uint)(x + 40) / 7;
    min16int m = min(x, y) + max(x, -y) * 3 + abs(y);
    min16float f = (min16float)y * 0.25;
    bool less = x < y;
    int wide = (int)q * 65536 + (int)r;
    return uint4(asuint(wide), asuint((int)s), (uint)u * 2u + (less ? 1u : 0u),
                 asuint((int)m) ^ asuint((float)f));
}

#if __SHADER_TARGET_MAJOR >= 6 && __SHADER_TARGET_MINOR >= 2
// The same kinds of operations where results wrap at 16 bits. Only unsigned arithmetic wraps (signed
// overflow is undefined in HLSL); the signed operations read wrapped values: arithmetic shifts,
// division, comparisons, minimum, maximum and widening conversions.
uint4 ps_narrow16(VsOut i) : SV_Target0 {
    uint16_t ux = (uint16_t)((uint)i.position.x * 997u + 45536u);
    uint16_t uy = (uint16_t)((uint)(i.uv.y * 64.0) * 1201u);
    uint16_t a = ux * (uint16_t)300 + uy;
    uint16_t p = ux * uy;
    int16_t x = (int16_t)ux;
    int16_t y = (int16_t)uy;
    int16_t b = (int16_t)p;
    int16_t c = (int16_t)(uint16_t)(ux << (uint16_t)5) >> (int16_t)3;
    uint16_t e = uy >> (uint16_t)2;
    int16_t g = x / (int16_t)((uy & (uint16_t)7) + (uint16_t)1);
    bool less = b < c;
    return uint4((uint)a | ((uint)p << 16), asuint((int)c), asuint((int)g) ^ ((uint)e << 8),
                 asuint((int)min(x, y) - (int)max(b, c)) + (less ? 1u : 0u));
}
#endif
