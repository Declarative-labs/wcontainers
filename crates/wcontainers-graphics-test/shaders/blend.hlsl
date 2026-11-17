// Two colours out of one pixel shader for dual-source blending, varying over each cell; and a
// gradient for the destination.
// fxc: vs_5_0 vs, ps_5_0 ps_dual, ps_5_0 ps_dest
// dxc: vs_6_0 vs, ps_6_0 ps_dual, ps_6_0 ps_dest
cbuffer Cell : register(b0) { float4 rect; }; // left, top, width, height in pixels
struct VsOut { float4 pos : SV_Position; };
VsOut vs(uint id : SV_VertexID) {
    VsOut o;
    float2 uv = float2((id << 1) & 2, id & 2);
    o.pos = float4(uv * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    return o;
}
struct Dual { float4 c0 : SV_Target0; float4 c1 : SV_Target1; };
Dual ps_dual(VsOut i) {
    float2 t = saturate((i.pos.xy - rect.xy) / rect.zw);
    Dual o;
    o.c0 = float4(0.9 * t.x + 0.05, 0.3, 0.8 * (1.0 - t.x) + 0.1, 0.15 + 0.7 * t.y);
    o.c1 = float4(0.25, 0.75 * t.y + 0.1, 0.5, 0.9 - 0.6 * t.x);
    return o;
}
float4 ps_dest(VsOut i) : SV_Target {
    float2 t = saturate((i.pos.xy - rect.xy) / rect.zw);
    return float4(0.2 + 0.6 * t.y, 0.5, 0.8 - 0.5 * t.y, 0.25 + 0.5 * t.x);
}
