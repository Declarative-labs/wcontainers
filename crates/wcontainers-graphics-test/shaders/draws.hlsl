// Vertices from slot 0 offset and coloured per instance from slot 1; the vertex and instance ids
// shade each corner, so a wrong start vertex, base vertex or start instance shows.
// fxc: vs_5_0 vs, ps_5_0 ps
// dxc: vs_6_0 vs, ps_6_0 ps
struct VsIn {
    float2 pos : POSITION;
    float2 offset : OFFSET;
    float4 color : COLOR;
    uint vertex : SV_VertexID;
    uint instance : SV_InstanceID;
};
struct VsOut { float4 pos : SV_Position; float4 color : COLOR; };
VsOut vs(VsIn i) {
    VsOut o;
    o.pos = float4(i.pos + i.offset, 0.0, 1.0);
    o.color = i.color * (0.55 + 0.05 * (i.vertex % 10)) + float4(0.0, 0.0, 0.08 * (i.instance % 4), 0.0);
    return o;
}
float4 ps(VsOut i) : SV_Target { return i.color; }
