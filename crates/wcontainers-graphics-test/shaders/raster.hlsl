// A geometry shader sending each triangle to eight viewports, with a colour per viewport.
// fxc: vs_5_0 vs, gs_5_0 gs, ps_5_0 ps
// dxc: vs_6_0 vs, gs_6_0 gs, ps_6_0 ps
struct ColorIn { float3 pos : POSITION; float4 color : COLOR; };
struct ColorOut { float4 pos : SV_Position; float4 color : COLOR; };
ColorOut vs(ColorIn i) { ColorOut o; o.pos = float4(i.pos, 1.0); o.color = i.color; return o; }
struct GsOut { float4 pos : SV_Position; float4 color : COLOR; uint viewport : SV_ViewportArrayIndex; };
[maxvertexcount(24)]
void gs(triangle ColorOut i[3], inout TriangleStream<GsOut> s) {
    for (uint v = 0; v < 8; v++) {
        for (uint k = 0; k < 3; k++) {
            GsOut o;
            o.pos = i[k].pos;
            o.color = float4(i[k].color.rgb * (0.3 + 0.1 * v), 1.0);
            o.viewport = v;
            s.Append(o);
        }
        s.RestartStrip();
    }
}
float4 ps(GsOut i) : SV_Target { return i.color; }
