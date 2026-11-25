// A geometry shader taking patches of four control points, drawn without tessellation between
// lines.hlsl's vs_line and ps_line, compiled by fxc (gs_5_0) and dxc (gs_6_0) by compile.cmd. Each
// patch is a quadrilateral, its points picked at run time, its colour scaled by the patch's number.

struct LineOut {
    float4 position : SV_Position;
    float4 colour : COLOR0;
    nointerpolation uint id : ID0;
};

[maxvertexcount(4)]
void gs_patch(InputPatch<LineOut, 4> input, uint primitive : SV_PrimitiveID,
              inout TriangleStream<LineOut> output) {
    float scale = (primitive % 4 + 1) * 0.25;
    for (uint k = 0; k < 4; k++) {
        LineOut v = input[k ^ (k >> 1)];
        v.colour.rgb *= scale;
        output.Append(v);
    }
}
