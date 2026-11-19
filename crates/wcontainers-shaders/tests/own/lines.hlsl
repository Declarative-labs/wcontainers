// Lines drawn with Direct3D's alpha antialiasing: the vertex shader passes a position and a colour
// on, the pixel shader writes the colour, whose alpha Direct3D multiplies by the line's coverage.

struct LineOut {
    float4 position : SV_Position;
    float4 colour : COLOR0;
    nointerpolation uint id : ID0;
};

LineOut vs_line(float3 position : POSITION, float4 colour : COLOR0, uint vertex : SV_VertexID) {
    LineOut o;
    o.position = float4(position, 1.0);
    o.colour = colour;
    o.id = vertex;
    return o;
}

float4 ps_line(LineOut i) : SV_Target0 {
    return float4(i.colour.rgb, i.colour.a * (1.0 - (i.id % 2) / 4.0));
}

// The primitive's number from the system (no geometry shader) beside the vertex shader's values.
float4 ps_primitive(LineOut i, uint primitive : SV_PrimitiveID) : SV_Target0 {
    return float4((primitive % 16) / 15.0, i.colour.g, (i.id % 8) / 7.0, 1.0);
}
