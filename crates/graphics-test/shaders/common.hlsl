// Shared by many scenes: coloured vertices, and a full-target triangle with texture coordinates.
// fxc: vs_4_0 vs_color, ps_4_0 ps_color, vs_4_0 vs_full, vs_4_0_level_9_1 vs_color_9_1, ps_4_0_level_9_1 ps_color_9_1
// dxc: vs_6_0 vs_color, ps_6_0 ps_color, vs_6_0 vs_full
struct ColorIn { float3 pos : POSITION; float4 color : COLOR; };
struct ColorOut { float4 pos : SV_Position; float4 color : COLOR; };
ColorOut vs_color(ColorIn i) { ColorOut o; o.pos = float4(i.pos, 1.0); o.color = i.color; return o; }
float4 ps_color(ColorOut i) : SV_Target { return i.color; }
ColorOut vs_color_9_1(ColorIn i) { ColorOut o; o.pos = float4(i.pos, 1.0); o.color = i.color; return o; }
float4 ps_color_9_1(ColorOut i) : SV_Target { return i.color; }

struct FullOut { float4 pos : SV_Position; float2 uv : TEXCOORD; };
// Three vertices from SV_VertexID covering the target; uv runs 0 to 1 over it, top to bottom.
FullOut vs_full(uint id : SV_VertexID) {
    FullOut o;
    float2 uv = float2((id << 1) & 2, id & 2);
    o.pos = float4(uv * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    o.uv = uv;
    return o;
}
