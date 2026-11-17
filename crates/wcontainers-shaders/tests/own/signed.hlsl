// Shaders compiled by both fxc and dxc (compile.cmd), drawn by both routes against real Direct3D 11's
// picture of the fxc build: signed integer operations, texel offsets on loads, NaN and infinity tests,
// a loop left from inside a switch, and values a compiler packs into one register. They share
// own.hlsl's constants, texture and sampler, and read vs_transform's outputs.

cbuffer Camera : register(b0) {
    float4x4 world_view_projection;
    float4 tint;
};

Texture2D<float4> albedo : register(t0);
SamplerState linear_clamp : register(s0);

struct VsOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint id : TEXCOORD1;
};

// Signed division, remainder, arithmetic shift, conversions both ways, minimum, maximum and absolute
// value, on values that run negative, and an unsigned division of the same bits: each answer differs
// between signed and unsigned operations. Written as raw bits so the comparison is exact.
uint4 ps_signed(VsOut i) : SV_Target0 {
    int x = (int)i.position.x - 32;
    int y = (int)(i.uv.y * 40.0) - 20;
    int q = x / 3;
    int r = y % 5;
    int s = (x * 1000) >> 3;
    float f = (float)y * 0.25;
    int m = min(x, y) + max(x, -y) * 3 + abs(y);
    uint u = (uint)x / 7u;
    int t = (int)(f * 7.0 + tint.x);
    return uint4(asuint(q * 65536 + r), asuint(s), asuint(f) ^ asuint(t), asuint(m) ^ u);
}

// Loads with a texel offset, and a sample with one, from texels chosen by the position.
float4 ps_offset(VsOut i) : SV_Target0 {
    int2 at = (int2)i.position.xy & 3;
    float4 a = albedo.Load(int3(at, 0), int2(2, 1));
    float4 b = albedo.Load(int3(at.yx, 0), int2(-1, 3));
    float4 c = albedo.Sample(linear_clamp, i.uv * 0.5, int2(1, -2));
    return float4(a.x, b.y, c.z, (a.w + b.w + c.w) / 3.0);
}

// NaN and infinity from values made at run time: the square root of a negative number, and one over
// zero.
float4 ps_special(VsOut i) : SV_Target0 {
    float n = sqrt(i.uv.x * tint.x - 0.4);
    float v = 1.0 / floor(i.uv.y * 2.0 * tint.y);
    float2 w = float2(n, v);
    return float4(isnan(n) ? 1.0 : 0.25, isinf(v) ? 1.0 : 0.25,
                  any(isnan(w)) ? 0.5 : 0.0, any(isinf(-w)) ? 0.75 : 0.0);
}

// A loop whose switch has cases that leave the switch early from inside an if, and the loop left by a
// test after the switch.
float4 ps_cases(VsOut i) : SV_Target0 {
    uint px = (uint)i.position.x;
    uint py = (uint)i.position.y;
    float total = 0.0;
    float step = 0.0;
    [loop] for (uint k = 0; k < ((px >> 2) & 7) + 2; k++) {
        [forcecase] switch ((k * 3 + (py >> 1)) & 3) {
        case 0:
            total += 0.07;
            break;
        case 1:
            if (total > 0.2) {
                step += 0.1;
                break;
            }
            total += 0.11;
            break;
        case 2:
            if (step > 0.15) {
                total -= 0.05;
                break;
            }
            step += 0.03;
            break;
        default:
            total *= 0.85;
            break;
        }
        if (total + step > 0.9)
            break;
    }
    return float4(total, step, frac(total * 5.0), 1.0);
}

struct PackOut {
    float4 position : SV_Position;
    float2 a : TEXCOORD0;
    float2 b : TEXCOORD1;
    float c : TEXCOORD2;
};

// Two two-component values and one more, which compilers may share registers between.
PackOut vs_pack(float3 position : POSITION, float2 uv : TEXCOORD0) {
    PackOut o;
    o.position = mul(float4(position, 1.0), world_view_projection);
    o.a = uv;
    o.b = frac(position.xy * 2.3);
    o.c = uv.x * uv.y;
    return o;
}

float4 ps_pack(PackOut i) : SV_Target0 {
    return float4(i.a.x, i.b.y, i.c, i.a.y * i.b.x);
}
