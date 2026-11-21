// Geometry shaders with arrays among their inputs and outputs (one register a row), and a vertex
// shader writing such arrays, for the stage helper's stream output tests (tests/geometry_draw.rs).

cbuffer Geometry : register(b0) {
    float4 size;
    float4 tint;
};

struct GsIn {
    float4 position : SV_Position;
    float4 colour : COLOR0;
    nointerpolation uint2 ids : TEXCOORD0;
};

struct RowsIn {
    float4 position : SV_Position;
    float4 colour[2] : COLOR0;
    nointerpolation uint2 ids[2] : TEXCOORD0;
};

// vs_geometry's values, with a second colour and a second pair of numbers in the arrays' second rows.
RowsIn vs_rows(float3 position : POSITION, float4 colour : COLOR0, uint vertex : SV_VertexID,
               uint instance : SV_InstanceID) {
    RowsIn o;
    o.position = float4(position.x + instance * 0.25, position.y, position.z, 1.0);
    o.colour[0] = colour;
    o.colour[1] = colour.wzyx * 0.5 + float4(position, 0.25);
    o.ids[0] = uint2(vertex, instance);
    o.ids[1] = uint2(vertex * 3 + 1, instance + 5);
    return o;
}

struct RowsLine {
    float4 position : SV_Position;
    float2 w[3] : TEXCOORD6;
    nointerpolation int n : TEXCOORD9;
    float f : TEXCOORD10;
};

// Each triangle, in two instances, becomes a strip of as many vertices as its primitive and instance
// numbers pick, then in the second instance a line of two; each vertex carries rows of every input
// vertex's arrays, one of them picked by the instance.
[instance(2)]
[maxvertexcount(6)]
void gs_rows_lines(triangle RowsIn t[3], uint primitive : SV_PrimitiveID,
                   uint inst : SV_GSInstanceID, inout LineStream<RowsLine> s) {
    uint count = (primitive + inst) % 4;
    for (uint i = 0; i < count; i++) {
        RowsLine o;
        o.position = t[(i + inst) % 3].position;
        [unroll] for (uint r = 0; r < 3; r++) {
            o.w[r] = float2(t[r].ids[1].x, r + i) + t[r].colour[(r + inst) % 2].xy;
        }
        o.n = -(int)(t[i % 3].ids[0].y + 10 * inst);
        o.f = t[2 - (i % 3)].colour[1].g;
        s.Append(o);
    }
    s.RestartStrip();
    if (inst == 1) {
        RowsLine o;
        o.position = t[0].position;
        o.w[0] = float2(primitive, 9.0);
        o.w[1] = o.w[0];
        o.w[2] = float2(9.0, primitive);
        o.n = 3;
        o.f = 0.25;
        s.Append(o);
        o.position = t[1].position;
        s.Append(o);
    }
}

// Not an array: Direct3D 11 on the RTX 3080 refuses stream output (E_OUTOFMEMORY) from a geometry
// shader of several streams whose first stream writes an array.
struct RowsPair {
    float2 a : TEXCOORD0;
    float2 b : TEXCOORD1;
};

struct RowsPlaced {
    float4 position : SV_Position;
    float3 c[2] : COLOR0;
    nointerpolation uint4 ids : TEXCOORD2;
};

struct RowsLast {
    nointerpolation uint x : TEXCOORD3;
    float y : TEXCOORD4;
    nointerpolation int z : TEXCOORD5;
};

// Each point written to three streams as its vertex number picks, an array and the position in
// the second.
[maxvertexcount(8)]
void gs_rows_streams(point GsIn p[1], uint primitive : SV_PrimitiveID,
                     inout PointStream<RowsPair> s0, inout PointStream<RowsPlaced> s1,
                     inout PointStream<RowsLast> s2) {
    uint v = p[0].ids.x;
    if (v % 2 == 0) {
        RowsPair o;
        o.a = p[0].position.xy;
        o.b = float2(v, primitive);
        s0.Append(o);
    }
    for (uint i = 0; i < v % 3; i++) {
        RowsPlaced o;
        o.position = p[0].position + float4(i * size.x, 0.0, 0.0, 0.0);
        o.c[0] = p[0].colour.rgb * tint.rgb;
        o.c[1] = float3(i, v, primitive);
        o.ids = uint4(p[0].ids, primitive, i);
        s1.Append(o);
    }
    if (v % 4 != 1) {
        RowsLast o;
        o.x = v * 7;
        o.y = p[0].colour.a;
        o.z = -(int)primitive;
        s2.Append(o);
    }
}
