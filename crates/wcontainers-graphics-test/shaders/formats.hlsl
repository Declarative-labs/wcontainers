// Gradients for render targets of every kind: floats running past 0 and 1, and unsigned and signed
// integers within 8 bits.
// fxc: ps_5_0 ps_float, ps_5_0 ps_uint, ps_5_0 ps_sint
// dxc: ps_6_0 ps_float, ps_6_0 ps_uint, ps_6_0 ps_sint
struct FullOut { float4 pos : SV_Position; float2 uv : TEXCOORD; };
float4 ps_float(FullOut i) : SV_Target {
    float2 t = i.uv;
    return float4(t.x * 2.5 - 0.75, t.y * 3.0 - 1.0, (t.x + t.y) * 0.5, 1.0 - t.x * t.y * 1.5);
}
uint4 ps_uint(FullOut i) : SV_Target {
    uint2 p = uint2(i.pos.xy);
    return uint4(p.x * 4, p.y * 3, (p.x ^ p.y) & 63, 200);
}
int4 ps_sint(FullOut i) : SV_Target {
    int2 p = int2(i.pos.xy);
    return int4(p.x * 4 - 128, p.y * 3 - 96, p.x - p.y, -7);
}
