// Shaders for the stage helper's geometry tests (tests/geometry_draw.rs): a vertex shader, geometry
// shaders of each input and output primitive, and a pixel shader showing what reaches it.

cbuffer Geometry : register(b0) {
    float4 size;
    float4 tint;
};

struct GsIn {
    float4 position : SV_Position;
    float4 colour : COLOR0;
    nointerpolation uint2 ids : TEXCOORD0;
};

struct GsOut {
    float4 position : SV_Position;
    float4 colour : COLOR0;
    nointerpolation uint4 ids : TEXCOORD0;
};

GsIn vs_geometry(float3 position : POSITION, float4 colour : COLOR0, uint vertex : SV_VertexID,
                 uint instance : SV_InstanceID) {
    GsIn o;
    o.position = float4(position.x + instance * 0.25, position.y, position.z, 1.0);
    o.colour = colour;
    o.ids = uint2(vertex, instance);
    return o;
}

// Each point becomes a quad of the constant's size, coloured by the point and its primitive number.
[maxvertexcount(4)]
void gs_quads(point GsIn p[1], uint primitive : SV_PrimitiveID, inout TriangleStream<GsOut> stream) {
    GsOut o;
    o.ids = uint4(p[0].ids, primitive, 0);
    for (uint i = 0; i < 4; i++) {
        float2 corner = float2(i & 1, i >> 1) * 2.0 - 1.0;
        o.position = p[0].position + float4(corner * size.xy, 0.0, 0.0);
        o.colour = p[0].colour * tint + float4(0.0, 0.0, (primitive % 4) / 4.0, 0.0);
        stream.Append(o);
    }
}

// Each triangle becomes none, one or two triangles by its primitive number, the second made smaller
// after a cut and drawn as a strip of four vertices.
[maxvertexcount(8)]
void gs_cut(triangle GsIn t[3], uint primitive : SV_PrimitiveID, inout TriangleStream<GsOut> stream) {
    uint copies = primitive % 3;
    float4 centre = (t[0].position + t[1].position + t[2].position) / 3.0;
    for (uint c = 0; c < copies; c++) {
        float scale = 1.0 - c * 0.4;
        for (uint i = 0; i < 3; i++) {
            GsOut o;
            o.position = lerp(centre, t[i].position, scale);
            o.colour = t[i].colour * float4(1.0, scale, 1.0, 1.0);
            o.ids = uint4(t[i].ids, primitive, c);
            stream.Append(o);
        }
        if (c == 1) {
            GsOut o;
            o.position = centre + float4(0.0, -0.2, 0.0, 0.0);
            o.colour = float4(1.0, 1.0, 0.0, 1.0);
            o.ids = uint4(t[0].ids, primitive, 7);
            stream.Append(o);
        }
        stream.RestartStrip();
    }
}

// Each triangle drawn three times, once per geometry instance, moved and tinted by the instance.
[instance(3)]
[maxvertexcount(3)]
void gs_instances(triangle GsIn t[3], uint instance : SV_GSInstanceID,
                  inout TriangleStream<GsOut> stream) {
    for (uint i = 0; i < 3; i++) {
        GsOut o;
        o.position = t[i].position + float4(instance * 0.3 - 0.3, instance * 0.1, 0.0, 0.0);
        o.colour = t[i].colour * float4(instance / 2.0, 1.0, 1.0 - instance / 2.0, 1.0);
        o.ids = uint4(t[i].ids, instance, 0);
        stream.Append(o);
    }
}

// Each line becomes a line strip of three vertices through a midpoint pushed aside.
[maxvertexcount(3)]
void gs_lines(line GsIn l[2], inout LineStream<GsOut> stream) {
    GsOut o;
    o.ids = uint4(l[0].ids, l[1].ids);
    o.position = l[0].position;
    o.colour = l[0].colour;
    stream.Append(o);
    o.position = (l[0].position + l[1].position) * 0.5 + float4(size.x, size.y, 0.0, 0.0);
    o.colour = tint;
    stream.Append(o);
    o.position = l[1].position;
    o.colour = l[1].colour;
    stream.Append(o);
}

// Each point becomes up to four points, as many as its vertex number modulo five.
[maxvertexcount(4)]
void gs_points(point GsIn p[1], inout PointStream<GsOut> stream) {
    uint count = p[0].ids.x % 5;
    for (uint i = 0; i < count; i++) {
        GsOut o;
        o.position = p[0].position + float4(i * size.x, 0.0, 0.0, 0.0);
        o.colour = p[0].colour * tint;
        o.ids = uint4(p[0].ids, i, 0);
        stream.Append(o);
    }
}

struct GsBare {
    float4 colour : COLOR0;
    nointerpolation uint4 ids : TEXCOORD0;
};

// As gs_points, feeding stream output only: it writes no position.
[maxvertexcount(4)]
void gs_unplaced(point GsIn p[1], inout PointStream<GsBare> stream) {
    uint count = p[0].ids.x % 5;
    for (uint i = 0; i < count; i++) {
        GsBare o;
        o.colour = p[0].colour * tint + float4(p[0].position.xy, float(i), 0.0);
        o.ids = uint4(p[0].ids, i, 0);
        stream.Append(o);
    }
}

// Each triangle with adjacency becomes its own triangle (vertices 0, 2, 4), each vertex coloured by
// the adjacent vertex after it.
[maxvertexcount(3)]
void gs_adjacent(triangleadj GsIn t[6], uint primitive : SV_PrimitiveID,
                 inout TriangleStream<GsOut> stream) {
    for (uint i = 0; i < 6; i += 2) {
        GsOut o;
        o.position = t[i].position;
        o.colour = t[i + 1].colour;
        o.ids = uint4(t[i].ids.x, t[i + 1].ids.x, primitive, 0);
        stream.Append(o);
    }
}

// Each line with adjacency becomes a line strip through its four vertices.
[maxvertexcount(4)]
void gs_line_adjacent(lineadj GsIn l[4], uint primitive : SV_PrimitiveID,
                      inout LineStream<GsOut> stream) {
    for (uint i = 0; i < 4; i++) {
        GsOut o;
        o.position = l[i].position;
        o.colour = l[i].colour * tint;
        o.ids = uint4(l[i].ids, primitive, i);
        stream.Append(o);
    }
}
struct GsClipOut {
    float4 position : SV_Position;
    float4 colour : COLOR0;
    nointerpolation uint4 ids : TEXCOORD0;
    float2 clip : SV_ClipDistance0;
    float more : SV_ClipDistance1;
};

// Each triangle passed on with three clip distances: two planes across the picture, and one that
// drops every fourth primitive whole and cuts the others along a diagonal. No plane passes through
// pixel centres, where Direct3D's clipped edge would follow its fill rule.
[maxvertexcount(3)]
void gs_clip(triangle GsIn t[3], uint primitive : SV_PrimitiveID,
             inout TriangleStream<GsClipOut> stream) {
    for (uint i = 0; i < 3; i++) {
        GsClipOut o;
        o.position = t[i].position;
        o.colour = t[i].colour;
        o.ids = uint4(t[i].ids, primitive, 0);
        o.clip = float2(t[i].position.x + 0.3037, 0.4129 - t[i].position.y);
        o.more = primitive % 4 == 1 ? -1.0 : t[i].position.x - t[i].position.y + 0.5113;
        stream.Append(o);
    }
}


struct CullOut {
    float4 position : SV_Position;
    float4 colour : COLOR0;
    nointerpolation uint4 ids : TEXCOORD0;
    float3 cull : SV_CullDistance0;
    float clip : SV_ClipDistance0;
};

// vs_geometry's values as ps_geometry reads them, drawn without a geometry shader, with three cull
// distances (primitives wholly left of a line, wholly above another, or of even vertex numbers only)
// and a clip distance (a diagonal).
CullOut vs_cull(float3 position : POSITION, float4 colour : COLOR0, uint vertex : SV_VertexID,
                uint instance : SV_InstanceID) {
    CullOut o;
    o.position = float4(position.x + instance * 0.25, position.y, position.z, 1.0);
    o.colour = colour;
    o.ids = uint4(vertex, instance, 0, 0);
    o.cull = float3(position.x + 0.3, 0.2 - position.y, (vertex & 1) ? 1.0 : -1.0);
    o.clip = position.x - position.y + 0.9;
    return o;
}

float4 ps_geometry(GsOut i) : SV_Target0 {
    return float4(i.colour.rgb, 1.0) * 0.75 + float4((i.ids % 4) / 16.0);
}

struct StreamPlaced {
    float4 position : SV_Position;
    float4 colour : COLOR0;
};

struct StreamIds {
    nointerpolation uint4 ids : TEXCOORD0;
};

struct StreamMixed {
    nointerpolation uint4 ids : TEXCOORD0;
    float4 colour : COLOR0;
};

struct StreamPlace {
    float2 place : TEXCOORD1;
};

// Each point written to four streams, to each as many times as its vertex number picks: positioned
// points to stream 0, values alone to the others, two of them named in two streams.
[maxvertexcount(12)]
void gs_streams(point GsIn p[1], uint primitive : SV_PrimitiveID,
                inout PointStream<StreamPlaced> s0, inout PointStream<StreamIds> s1,
                inout PointStream<StreamMixed> s2, inout PointStream<StreamPlace> s3) {
    uint v = p[0].ids.x;
    for (uint i = 0; i < v % 3; i++) {
        StreamPlaced o;
        o.position = p[0].position + float4(i * size.x, 0.0, 0.0, 0.0);
        o.colour = p[0].colour * tint;
        s0.Append(o);
    }
    for (uint j = 0; j < (v / 3) % 4; j++) {
        StreamIds o;
        o.ids = uint4(p[0].ids, primitive, j);
        s1.Append(o);
    }
    if ((v & 1) == 0) {
        StreamMixed o;
        o.ids = uint4(primitive, v, p[0].ids.y, 7);
        o.colour = p[0].colour + tint;
        s2.Append(o);
        s2.RestartStrip();
    }
    for (uint k = 0; k < min(v % 5, 3); k++) {
        StreamPlace o;
        o.place = p[0].position.xy * float(k + 1);
        s3.Append(o);
    }
}
