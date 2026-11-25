// Geometry shaders: points grown into quads, triangles with adjacency coloured by their neighbours,
// several strips per input with primitive ids (and the system's ids without a geometry shader), and instancing into layers of an array target.
// fxc: vs_4_0 vs, gs_4_0 gs_points, gs_4_0 gs_adjacency, gs_4_0 gs_strips, gs_5_0 gs_layers, ps_4_0 ps, ps_4_0 ps_primitive, ps_4_0 ps_id
// dxc: vs_6_0 vs, gs_6_0 gs_points, gs_6_0 gs_adjacency, gs_6_0 gs_strips, gs_6_0 gs_layers, ps_6_0 ps, ps_6_0 ps_primitive, ps_6_0 ps_id
struct ColorIn { float3 pos : POSITION; float4 color : COLOR; };
struct ColorOut { float4 pos : SV_Position; float4 color : COLOR; };
ColorOut vs(ColorIn i) { ColorOut o; o.pos = float4(i.pos, 1.0); o.color = i.color; return o; }
float4 ps(ColorOut i) : SV_Target { return i.color; }

[maxvertexcount(4)]
void gs_points(point ColorOut p[1], inout TriangleStream<ColorOut> s) {
    float size = 0.04 + 0.12 * p[0].color.r;
    for (uint k = 0; k < 4; k++) {
        ColorOut o;
        o.pos = p[0].pos + float4((k & 1) ? size : -size, (k & 2) ? size : -size, 0.0, 0.0);
        o.color = float4(p[0].color.rgb, 1.0) * (0.6 + 0.1 * k);
        s.Append(o);
    }
}

[maxvertexcount(3)]
void gs_adjacency(triangleadj ColorOut t[6], inout TriangleStream<ColorOut> s) {
    for (uint k = 0; k < 6; k += 2) {
        ColorOut o;
        o.pos = t[k].pos;
        o.color = float4(abs(t[k + 1].pos.xy - t[k].pos.xy), t[k].color.b, 1.0);
        s.Append(o);
    }
}

struct PrimOut { float4 pos : SV_Position; float4 color : COLOR; uint id : PRIM; };
[maxvertexcount(12)]
void gs_strips(triangle ColorOut t[3], uint id : SV_PrimitiveID, inout TriangleStream<PrimOut> s) {
    for (uint copy = 0; copy < 3; copy++) {
        for (uint k = 0; k < 3; k++) {
            PrimOut o;
            o.pos = t[k].pos * float4(0.5, 0.5, 1.0, 1.0) + float4(copy * 0.3 - 0.5, -0.4 + 0.25 * copy, 0.0, 0.0);
            o.color = t[k].color;
            o.id = id * 3 + copy;
            s.Append(o);
        }
        s.RestartStrip();
    }
}
float4 ps_primitive(PrimOut i) : SV_Target { return float4(i.color.rgb * (0.5 + 0.08 * (i.id % 6)), 1.0); }
// Without a geometry shader the pixel shader gets the primitive id from the system.
float4 ps_id(ColorOut i, uint id : SV_PrimitiveID) : SV_Target { return float4(i.color.rgb * (0.4 + 0.3 * (id % 3)), 1.0); }

struct LayerOut { float4 pos : SV_Position; float4 color : COLOR; uint layer : SV_RenderTargetArrayIndex; };
[instance(3)]
[maxvertexcount(3)]
void gs_layers(triangle ColorOut t[3], uint instance : SV_GSInstanceID, inout TriangleStream<LayerOut> s) {
    for (uint k = 0; k < 3; k++) {
        LayerOut o;
        float turn = instance * 2.0944;
        float2 p = t[k].pos.xy;
        o.pos = float4(p.x * cos(turn) - p.y * sin(turn), p.x * sin(turn) + p.y * cos(turn), t[k].pos.zw);
        o.color = t[k].color.gbra;
        o.layer = instance;
        s.Append(o);
    }
}
