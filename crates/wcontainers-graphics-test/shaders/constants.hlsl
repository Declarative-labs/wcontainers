// Constant buffers: fourteen slots read at once, and one large buffer read through windows of it.
// fxc: ps_5_0 ps_slots, ps_5_0 ps_window
cbuffer B0 : register(b0) { float4 a0; };
cbuffer B1 : register(b1) { float4 a1[16]; };
cbuffer B2 : register(b2) { float4 a2[256]; };
cbuffer B3 : register(b3) { float4 a3[4096]; };
cbuffer B4 : register(b4) { float4 a4; float2 b4; float c4; };
cbuffer B5 : register(b5) { float4 a5; };
cbuffer B6 : register(b6) { float4 a6; };
cbuffer B7 : register(b7) { float4 a7; };
cbuffer B8 : register(b8) { float4 a8; };
cbuffer B9 : register(b9) { float4 a9; };
cbuffer B10 : register(b10) { float4 a10; };
cbuffer B11 : register(b11) { float4 a11; };
cbuffer B12 : register(b12) { float4 a12; };
cbuffer B13 : register(b13) { float4 a13; };
struct FullOut { float4 pos : SV_Position; float2 uv : TEXCOORD; };
float4 ps_slots(FullOut i) : SV_Target {
    uint2 p = uint2(i.pos.xy);
    float4 v = a0 + a1[p.x % 16] + a2[(p.x + p.y * 7) % 256] + a3[(p.x * 64 + p.y) % 4096];
    v += float4(b4, c4, 0.0) * a4;
    v += (a5 + a6 + a7 + a8 + a9 + a10 + a11 + a12 + a13) * float(p.y % 2);
    return frac(v);
}
cbuffer Window : register(b0) { float4 window[64]; };
float4 ps_window(FullOut i) : SV_Target { return window[uint(i.pos.x) % 64]; }
