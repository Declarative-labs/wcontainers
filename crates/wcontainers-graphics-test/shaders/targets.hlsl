// Four render targets written at once, of float, normalised and integer formats.
// fxc: ps_5_0 ps
// dxc: ps_6_0 ps
struct ColorOut { float4 pos : SV_Position; float4 color : COLOR; };
struct Out {
    float4 c0 : SV_Target0;
    float4 c1 : SV_Target1;
    float4 c2 : SV_Target2;
    uint4 c3 : SV_Target3;
};
Out ps(ColorOut i) {
    Out o;
    o.c0 = i.color;
    o.c1 = float4(i.color.bgr * 3.0 - 1.0, i.color.a);
    o.c2 = float4(1.0 - i.color.rgb, 0.5);
    o.c3 = uint4(uint(i.pos.x) * 3 + uint(i.pos.y) * 1000, 0, 0, 0);
    return o;
}
