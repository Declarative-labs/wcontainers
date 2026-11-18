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
