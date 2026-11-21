// A domain shader reading one control point's array of rows at an index known only at run time and
// at a constant one, different components of each, so fxc declares an index range whose component
// mask has a gap (gap A23 of the shaders status: refused by name on DXBC).

struct RowsPoint {
    float3 arr[3] : ARR;
};

struct RowsFactors {
    float edge[4] : SV_TessFactor;
    float inside[2] : SV_InsideTessFactor;
};

struct RowsDomainOut {
    float4 position : SV_Position;
    float location : DOMAIN;
};

[domain("quad")]
RowsDomainOut ds_rows_gap(RowsFactors f, float2 uv : SV_DomainLocation,
                          const OutputPatch<RowsPoint, 2> c) {
    uint k = uv.x > 0.5f ? 1u : 0u;
    RowsDomainOut o;
    o.position = float4(uv, 0.0, 1.0);
    o.location = c[1 - k].arr[k + 1].z + c[k].arr[2].x;
    return o;
}
