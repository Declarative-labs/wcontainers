// Tessellation of triangle, quad and isoline patches with each partitioning, factors from a constant
// buffer, and the domain shader bending the surface so the result shows every new vertex.
// fxc: vs_5_0 vs, hs_5_0 hs_tri, ds_5_0 ds_tri, hs_5_0 hs_quad_odd, hs_5_0 hs_quad_even, hs_5_0 hs_quad_pow2, hs_5_0 hs_quad_integer, ds_5_0 ds_quad, hs_5_0 hs_line, ds_5_0 ds_line, ps_5_0 ps
cbuffer Factors : register(b0) { float4 edges; float4 inside; };
struct Point { float3 pos : POSITION; float4 color : COLOR; };
Point vs(Point p) { return p; }
struct TriFactors { float edge[3] : SV_TessFactor; float inside : SV_InsideTessFactor; };
struct QuadFactors { float edge[4] : SV_TessFactor; float inside[2] : SV_InsideTessFactor; };
struct LineFactors { float edge[2] : SV_TessFactor; };
struct ColorOut { float4 pos : SV_Position; float4 color : COLOR; };

TriFactors tri_factors(InputPatch<Point, 3> p) {
    TriFactors f;
    f.edge[0] = edges.x; f.edge[1] = edges.y; f.edge[2] = edges.z; f.inside = inside.x;
    return f;
}
[domain("tri")] [partitioning("fractional_odd")] [outputtopology("triangle_cw")]
[outputcontrolpoints(3)] [patchconstantfunc("tri_factors")]
Point hs_tri(InputPatch<Point, 3> p, uint i : SV_OutputControlPointID) { return p[i]; }
[domain("tri")]
ColorOut ds_tri(TriFactors f, float3 uvw : SV_DomainLocation, const OutputPatch<Point, 3> p) {
    ColorOut o;
    float3 pos = uvw.x * p[0].pos + uvw.y * p[1].pos + uvw.z * p[2].pos;
    pos.xy *= 1.0 + 0.3 * uvw.x * uvw.y * uvw.z;
    o.pos = float4(pos, 1.0);
    o.color = uvw.x * p[0].color + uvw.y * p[1].color + uvw.z * p[2].color;
    return o;
}

QuadFactors quad_factors(InputPatch<Point, 4> p) {
    QuadFactors f;
    f.edge[0] = edges.x; f.edge[1] = edges.y; f.edge[2] = edges.z; f.edge[3] = edges.w;
    f.inside[0] = inside.x; f.inside[1] = inside.y;
    return f;
}
#define QUAD_HS(name, mode) \
    [domain("quad")] [partitioning(mode)] [outputtopology("triangle_ccw")] \
    [outputcontrolpoints(4)] [patchconstantfunc("quad_factors")] \
    Point name(InputPatch<Point, 4> p, uint i : SV_OutputControlPointID) { return p[i]; }
QUAD_HS(hs_quad_odd, "fractional_odd")
QUAD_HS(hs_quad_even, "fractional_even")
QUAD_HS(hs_quad_pow2, "pow2")
QUAD_HS(hs_quad_integer, "integer")
[domain("quad")]
ColorOut ds_quad(QuadFactors f, float2 uv : SV_DomainLocation, const OutputPatch<Point, 4> p) {
    ColorOut o;
    float3 top = lerp(p[0].pos, p[1].pos, uv.x), bottom = lerp(p[3].pos, p[2].pos, uv.x);
    float3 pos = lerp(top, bottom, uv.y);
    pos.y += 0.15 * sin(uv.x * 6.2831853);
    o.pos = float4(pos, 1.0);
    o.color = lerp(lerp(p[0].color, p[1].color, uv.x), lerp(p[3].color, p[2].color, uv.x), uv.y);
    return o;
}

LineFactors line_factors(InputPatch<Point, 2> p) {
    LineFactors f;
    f.edge[0] = edges.x; f.edge[1] = edges.y;
    return f;
}
[domain("isoline")] [partitioning("integer")] [outputtopology("line")]
[outputcontrolpoints(2)] [patchconstantfunc("line_factors")]
Point hs_line(InputPatch<Point, 2> p, uint i : SV_OutputControlPointID) { return p[i]; }
[domain("isoline")]
ColorOut ds_line(LineFactors f, float2 uv : SV_DomainLocation, const OutputPatch<Point, 2> p) {
    ColorOut o;
    float3 pos = lerp(p[0].pos, p[1].pos, uv.x);
    pos.y += uv.y * 1.2 - 0.6 + 0.2 * sin(uv.x * 9.0);
    o.pos = float4(pos, 1.0);
    o.color = lerp(p[0].color, p[1].color, uv.x);
    return o;
}
float4 ps(ColorOut i) : SV_Target { return i.color; }
