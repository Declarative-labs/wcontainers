// Shaders for the tessellator's test (tests/tessellation_draw.rs): patches of one control point, a
// hull shader per domain, partitioning and output topology taking each patch's factors from a constant
// buffer by its primitive number, and a domain shader per domain whose output, streamed, says where
// each point lies and which patch made it.

cbuffer Factors : register(b0) {
    // Patch p's outer factors at 2p, its inner factors at 2p + 1.
    float4 factors[64];
};

struct ControlPoint {
    uint id : INDEX;
};

ControlPoint vs_patch(uint id : SV_VertexID) {
    ControlPoint c;
    c.id = id;
    return c;
}

struct TriFactors {
    float edge[3] : SV_TessFactor;
    float inside : SV_InsideTessFactor;
};

struct QuadFactors {
    float edge[4] : SV_TessFactor;
    float inside[2] : SV_InsideTessFactor;
};

struct LineFactors {
    float edge[2] : SV_TessFactor;
};

TriFactors tri_factors(uint p : SV_PrimitiveID) {
    TriFactors f;
    f.edge[0] = factors[2 * p].x;
    f.edge[1] = factors[2 * p].y;
    f.edge[2] = factors[2 * p].z;
    f.inside = factors[2 * p + 1].x;
    return f;
}

QuadFactors quad_factors(uint p : SV_PrimitiveID) {
    QuadFactors f;
    f.edge[0] = factors[2 * p].x;
    f.edge[1] = factors[2 * p].y;
    f.edge[2] = factors[2 * p].z;
    f.edge[3] = factors[2 * p].w;
    f.inside[0] = factors[2 * p + 1].x;
    f.inside[1] = factors[2 * p + 1].y;
    return f;
}

LineFactors line_factors(uint p : SV_PrimitiveID) {
    LineFactors f;
    f.edge[0] = factors[2 * p].x;
    f.edge[1] = factors[2 * p].y;
    return f;
}

#define HS(name, dom, mode, topology, constants) \
    [domain(dom)] [partitioning(mode)] [outputtopology(topology)] \
    [outputcontrolpoints(1)] [patchconstantfunc(constants)] \
    ControlPoint name(InputPatch<ControlPoint, 1> p, uint i : SV_OutputControlPointID) { return p[0]; }

#define SURFACE_HS(dom, name, constants) \
    HS(hs_##name##_integer_point, dom, "integer", "point", constants) \
    HS(hs_##name##_integer_cw, dom, "integer", "triangle_cw", constants) \
    HS(hs_##name##_integer_ccw, dom, "integer", "triangle_ccw", constants) \
    HS(hs_##name##_pow2_point, dom, "pow2", "point", constants) \
    HS(hs_##name##_pow2_cw, dom, "pow2", "triangle_cw", constants) \
    HS(hs_##name##_pow2_ccw, dom, "pow2", "triangle_ccw", constants) \
    HS(hs_##name##_odd_point, dom, "fractional_odd", "point", constants) \
    HS(hs_##name##_odd_cw, dom, "fractional_odd", "triangle_cw", constants) \
    HS(hs_##name##_odd_ccw, dom, "fractional_odd", "triangle_ccw", constants) \
    HS(hs_##name##_even_point, dom, "fractional_even", "point", constants) \
    HS(hs_##name##_even_cw, dom, "fractional_even", "triangle_cw", constants) \
    HS(hs_##name##_even_ccw, dom, "fractional_even", "triangle_ccw", constants)

SURFACE_HS("tri", tri, "tri_factors")
SURFACE_HS("quad", quad, "quad_factors")
HS(hs_line_integer_point, "isoline", "integer", "point", "line_factors")
HS(hs_line_integer_line, "isoline", "integer", "line", "line_factors")
HS(hs_line_pow2_point, "isoline", "pow2", "point", "line_factors")
HS(hs_line_pow2_line, "isoline", "pow2", "line", "line_factors")
HS(hs_line_odd_point, "isoline", "fractional_odd", "point", "line_factors")
HS(hs_line_odd_line, "isoline", "fractional_odd", "line", "line_factors")
HS(hs_line_even_point, "isoline", "fractional_even", "point", "line_factors")
HS(hs_line_even_line, "isoline", "fractional_even", "line", "line_factors")

struct Streamed {
    float4 position : SV_Position;
    float3 location : DOMAIN;
    uint primitive : PRIMITIVE;
};

[domain("tri")]
Streamed ds_tri(TriFactors f, float3 uvw : SV_DomainLocation, uint p : SV_PrimitiveID,
                const OutputPatch<ControlPoint, 1> c) {
    Streamed s;
    s.position = float4(uvw, 1.0);
    s.location = uvw;
    s.primitive = p;
    return s;
}

[domain("quad")]
Streamed ds_quad(QuadFactors f, float2 uv : SV_DomainLocation, uint p : SV_PrimitiveID,
                 const OutputPatch<ControlPoint, 1> c) {
    Streamed s;
    s.position = float4(uv, 0.0, 1.0);
    s.location = float3(uv, 0.0);
    s.primitive = p;
    return s;
}

[domain("isoline")]
Streamed ds_line(LineFactors f, float2 uv : SV_DomainLocation, uint p : SV_PrimitiveID,
                 const OutputPatch<ControlPoint, 1> c) {
    Streamed s;
    s.position = float4(uv, 0.0, 1.0);
    s.location = float3(uv, 0.0);
    s.primitive = p;
    return s;
}

// A domain shader taking no control points, after hull_none.hlsl's hull shader.
[domain("tri")]
Streamed ds_tri_none(TriFactors f, float3 uvw : SV_DomainLocation, uint p : SV_PrimitiveID) {
    Streamed s;
    s.position = float4(uvw, 1.0);
    s.location = uvw;
    s.primitive = p;
    return s;
}

// Geometry shaders after the domain stage, one per tessellator output: each passes its primitive on
// in reverse order with the location's components turned, then (lines and triangles) a second
// primitive of its edges' midpoints; the patch's number is kept.
Streamed turned(Streamed s) {
    s.location = s.location.yzx;
    return s;
}

Streamed midpoint(Streamed a, Streamed b) {
    Streamed s = a;
    s.position = (a.position + b.position) * 0.5;
    s.location = (a.location + b.location) * 0.5;
    return s;
}

[maxvertexcount(1)]
void gs_tess_point(point Streamed p[1], inout PointStream<Streamed> stream) {
    stream.Append(turned(p[0]));
}

[maxvertexcount(4)]
void gs_tess_line(line Streamed p[2], inout LineStream<Streamed> stream) {
    stream.Append(turned(p[1]));
    stream.Append(turned(p[0]));
    stream.RestartStrip();
    stream.Append(midpoint(p[0], p[1]));
    stream.Append(p[1]);
}

[maxvertexcount(6)]
void gs_tess_tri(triangle Streamed p[3], inout TriangleStream<Streamed> stream) {
    stream.Append(turned(p[2]));
    stream.Append(turned(p[1]));
    stream.Append(turned(p[0]));
    stream.RestartStrip();
    stream.Append(midpoint(p[0], p[1]));
    stream.Append(midpoint(p[1], p[2]));
    stream.Append(midpoint(p[2], p[0]));
}

// Domain shaders writing cull distances, drawn: patch p fills cell (p % 4, p / 4) of a 4 by 2 grid,
// coloured by its domain location. A primitive is dropped where all its points lie left of the line
// u = (1 + p % 4) / 8, or all past v = 5 / 8; the thresholds and the grid's scale are exact in binary,
// so each distance's sign is too.
struct Culled {
    float4 position : SV_Position;
    float4 color : COLOR;
    float2 cull : SV_CullDistance;
};

Culled culled(float3 l, uint p) {
    Culled s;
    s.position = float4((l.x + p % 4) * 0.5 - 1.0, l.y + p / 4 % 2 - 1.0, 0.5, 1.0);
    s.color = float4(l, 1.0);
    s.cull = float2(l.x - (1 + p % 4) * 0.125, 0.625 - l.y);
    return s;
}

[domain("tri")]
Culled ds_tri_cull(TriFactors f, float3 uvw : SV_DomainLocation, uint p : SV_PrimitiveID,
                   const OutputPatch<ControlPoint, 1> c) {
    return culled(uvw, p);
}

[domain("quad")]
Culled ds_quad_cull(QuadFactors f, float2 uv : SV_DomainLocation, uint p : SV_PrimitiveID,
                    const OutputPatch<ControlPoint, 1> c) {
    return culled(float3(uv, 0.0), p);
}

[domain("isoline")]
Culled ds_line_cull(LineFactors f, float2 uv : SV_DomainLocation, uint p : SV_PrimitiveID,
                    const OutputPatch<ControlPoint, 1> c) {
    return culled(float3(uv, 0.0), p);
}

float4 ps_cull(float4 position : SV_Position, float4 color : COLOR) : SV_Target {
    return color;
}
