// A geometry shader reading a typed buffer by its primitive's number, between lines.hlsl's vs_line and
// ps_line, compiled by fxc (gs_5_0) and dxc (gs_6_0) by compile.cmd. The view holds four elements, so
// every fifth primitive reads past it (zero).

struct LineOut {
    float4 position : SV_Position;
    float4 colour : COLOR0;
    nointerpolation uint id : ID0;
};

Buffer<float4> offsets : register(t0);

[maxvertexcount(3)]
void gs_typed(triangle LineOut input[3], uint primitive : SV_PrimitiveID,
              inout TriangleStream<LineOut> output) {
    float4 o = offsets[primitive % 5];
    for (uint k = 0; k < 3; k++) {
        LineOut v = input[k];
        v.position.xy += o.xy;
        v.colour = float4(v.colour.rgb * o.z, o.w);
        output.Append(v);
    }
}
