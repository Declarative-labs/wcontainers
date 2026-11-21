// A geometry shader writing each triangle and a moved copy to stream output, and a pass-through
// for drawing the written vertices back with DrawAuto.
// fxc: gs_4_0 gs, vs_4_0 vs_back
// dxc: gs_6_0 gs, vs_6_0 vs_back
struct ColorOut { float4 pos : SV_Position; float4 color : COLOR; };
[maxvertexcount(6)]
void gs(triangle ColorOut t[3], inout TriangleStream<ColorOut> s) {
    for (uint copy = 0; copy < 2; copy++) {
        for (uint k = 0; k < 3; k++) {
            ColorOut o = t[k];
            o.pos.xy = o.pos.xy * 0.6 + float2(copy * 0.5 - 0.4, copy * -0.3);
            o.color.rgb = copy ? o.color.bgr : o.color.rgb;
            s.Append(o);
        }
        s.RestartStrip();
    }
}
struct BackIn { float4 pos : POSITION; float4 color : COLOR; };
ColorOut vs_back(BackIn i) { ColorOut o; o.pos = float4(i.pos.xy * float2(-1.0, 1.0), i.pos.zw); o.color = i.color; return o; }
