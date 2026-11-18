// Pixel shaders that write depth: any value, and values the rasterizer's depth may only grow or
// shrink towards.
// fxc: ps_4_0 ps_depth, ps_5_0 ps_greater, ps_5_0 ps_less
// dxc: ps_6_0 ps_depth, ps_6_0 ps_greater, ps_6_0 ps_less
struct ColorOut { float4 pos : SV_Position; float4 color : COLOR; };
struct Out { float4 color : SV_Target; float depth : SV_Depth; };
Out ps_depth(ColorOut i) {
    Out o;
    o.color = i.color;
    o.depth = frac(i.pos.x / 16.0) * 0.5 + i.pos.y / 512.0;
    return o;
}
// Conservative depth needs the position interpolated at the centroid.
struct CentroidIn { noperspective centroid float4 pos : SV_Position; float4 color : COLOR; };
struct Greater { float4 color : SV_Target; float depth : SV_DepthGreaterEqual; };
Greater ps_greater(CentroidIn i) {
    Greater o;
    o.color = i.color;
    o.depth = i.pos.z + frac(i.pos.x / 8.0) * 0.25;
    return o;
}
struct Less { float4 color : SV_Target; float depth : SV_DepthLessEqual; };
Less ps_less(CentroidIn i) {
    Less o;
    o.color = i.color;
    o.depth = i.pos.z - frac(i.pos.y / 8.0) * 0.25;
    return o;
}
