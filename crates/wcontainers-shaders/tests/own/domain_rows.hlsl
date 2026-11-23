// A hull shader writing each control point's array of rows, and a domain shader reading them at an
// index known only at run time and at a constant one, different components of each, so fxc declares
// an index range whose component mask has a gap (tests/tessellation_draw.rs). Every component the
// hull writes differs from the others, so a component read from the wrong place shows.

cbuffer Factors : register(b0) {
    // Patch p's outer factors at 2p, its inner factors at 2p + 1.
    float4 factors[64];
};

struct ControlPoint {
    uint id : INDEX;
};

struct RowsPoint {
    float3 arr[3] : ARR;
};

struct RowsFactors {
    float edge[4] : SV_TessFactor;
    float inside[2] : SV_InsideTessFactor;
};

RowsFactors rows_factors(uint p : SV_PrimitiveID) {
    RowsFactors f;
    f.edge[0] = factors[2 * p].x;
    f.edge[1] = factors[2 * p].y;
    f.edge[2] = factors[2 * p].z;
    f.edge[3] = factors[2 * p].w;
    f.inside[0] = factors[2 * p + 1].x;
    f.inside[1] = factors[2 * p + 1].y;
    return f;
}

[domain("quad")] [partitioning("integer")] [outputtopology("triangle_cw")]
[outputcontrolpoints(2)] [patchconstantfunc("rows_factors")]
RowsPoint hs_rows_gap(InputPatch<ControlPoint, 1> ip, uint i : SV_OutputControlPointID,
                      uint p : SV_PrimitiveID) {
    RowsPoint o;
    [unroll] for (uint r = 0; r < 3; r++) {
        o.arr[r] = float3(ip[0].id + 0.25 * r, 10.0 * i + r, -3.0 * r - i - p - 0.5);
    }
    return o;
}

struct Streamed {
    float4 position : SV_Position;
    float3 location : DOMAIN;
    uint primitive : PRIMITIVE;
};

[domain("quad")]
Streamed ds_rows_gap(RowsFactors f, float2 uv : SV_DomainLocation, uint p : SV_PrimitiveID,
                     const OutputPatch<RowsPoint, 2> c) {
    uint k = uv.x > 0.5f ? 1u : 0u;
    Streamed s;
    s.position = float4(uv, 0.0, 1.0);
    s.location = float3(uv, c[1 - k].arr[k + 1].z + c[k].arr[2].x);
    s.primitive = p;
    return s;
}
