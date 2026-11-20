// Values handed through the hull stage (tests/tessellation_draw.rs, tests/geometry.rs): hull shaders
// taking three control points and giving four, each control point holding the position, a vector of
// numbers and an array of rows; a patch constant function reading the input and the output patch and
// giving values beside the factors (an array among them); a domain shader checking every value it is
// handed and moving its location where one is not what the hull gave; and an instanced geometry shader
// after the domain stage reading its primitive's number.

cbuffer Factors : register(b0) {
    // Patch p's outer factors at 2p, its inner factors at 2p + 1.
    float4 factors[64];
};

struct ControlPoint {
    uint id : INDEX;
};

struct HullPoint {
    float4 place : SV_Position;
    uint4 code : CODE;
    float2 rows[2] : ROWS;
};

struct ValueFactors {
    float edge[3] : SV_TessFactor;
    float inside : SV_InsideTessFactor;
    uint4 sum : SUM;
    float extra[2] : EXTRA;
};

ValueFactors value_factors(InputPatch<ControlPoint, 3> ip, OutputPatch<HullPoint, 4> op,
                           uint p : SV_PrimitiveID) {
    ValueFactors f;
    [unroll] for (uint i = 0; i < 3; i++) {
        uint k = ip[i].id - 3 * p;
        f.edge[i] = factors[2 * p][i] * (k == i ? 1.0 : 0.5);
    }
    f.inside = factors[2 * p + 1].x;
    f.sum = uint4(ip[0].id + ip[1].id + ip[2].id, op[3].code.x, op[1].code.y, p);
    f.extra[0] = op[2].rows[1].y;
    f.extra[1] = op[3].place.y + float(p) + 0.5;
    return f;
}

#define VALUES_HS(name, mode, topology) \
    [domain("tri")] [partitioning(mode)] [outputtopology(topology)] \
    [outputcontrolpoints(4)] [patchconstantfunc("value_factors")] \
    HullPoint name(InputPatch<ControlPoint, 3> ip, uint i : SV_OutputControlPointID, \
                   uint p : SV_PrimitiveID) { \
        HullPoint o; \
        o.place = float4(ip[i % 3].id, i, p, 1.5); \
        o.code = uint4(ip[i % 3].id, i, p, ip[(i + 1) % 3].id); \
        o.rows[0] = float2(i, p); \
        o.rows[1] = float2(ip[0].id, 7.0 + i); \
        return o; \
    }

VALUES_HS(hs_values_odd_ccw, "fractional_odd", "triangle_ccw")
VALUES_HS(hs_values_even_cw, "fractional_even", "triangle_cw")
VALUES_HS(hs_values_integer_point, "integer", "point")

struct Streamed {
    float4 position : SV_Position;
    float3 location : DOMAIN;
    uint primitive : PRIMITIVE;
};

[domain("tri")]
Streamed ds_values(ValueFactors f, float3 uvw : SV_DomainLocation, uint p : SV_PrimitiveID,
                   const OutputPatch<HullPoint, 4> c) {
    bool ok = true;
    [unroll] for (uint i = 0; i < 4; i++) {
        ok = ok && all(c[i].place == float4(3 * p + i % 3, i, p, 1.5));
        ok = ok && all(c[i].code == uint4(3 * p + i % 3, i, p, 3 * p + (i + 1) % 3));
        ok = ok && all(c[i].rows[0] == float2(i, p));
        ok = ok && all(c[i].rows[1] == float2(3 * p, 7.0 + i));
    }
    ok = ok && all(f.sum == uint4(9 * p + 3, 3 * p, 1, p));
    ok = ok && f.extra[0] == 9.0 && f.extra[1] == 3.0 + float(p) + 0.5;
    Streamed s;
    s.position = float4(uvw, 1.0);
    s.location = uvw;
    if (!ok) {
        s.location = uvw.zxy + 2.0;
    }
    s.primitive = p;
    return s;
}

// Each primitive twice, once per instance, the second's vertices rotated; the location moved where the
// primitive's number the stage reads is not the patch's.
[instance(2)]
[maxvertexcount(3)]
void gs_values_tess(triangle Streamed t[3], uint prim : SV_PrimitiveID, uint inst : SV_GSInstanceID,
                    inout TriangleStream<Streamed> stream) {
    for (uint i = 0; i < 3; i++) {
        Streamed s = t[(i + inst) % 3];
        if (prim != t[0].primitive) {
            s.location = s.location + 4.0;
        }
        stream.Append(s);
    }
}
