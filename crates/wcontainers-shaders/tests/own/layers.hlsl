// A pixel shader reading its layer and viewport index: real Direct3D's from a geometry shader that
// writes them, ours from the draw's run (the helper values a front sets when it splits a draw per
// layer and viewport).

struct Layered {
    float4 position : SV_Position;
    uint layer : SV_RenderTargetArrayIndex;
    uint viewport : SV_ViewportArrayIndex;
};

// Each triangle into each of four layers, through viewport 0 or 1 by layer.
[instance(4)]
[maxvertexcount(3)]
void gs_layers(triangle float4 p[3] : SV_Position, uint instance : SV_GSInstanceID,
               inout TriangleStream<Layered> stream) {
    for (uint i = 0; i < 3; i++) {
        Layered v;
        v.position = p[i];
        v.layer = instance;
        v.viewport = instance % 2;
        stream.Append(v);
    }
}

float4 ps_layer(float4 position : SV_Position, uint layer : SV_RenderTargetArrayIndex,
                uint viewport : SV_ViewportArrayIndex) : SV_Target0 {
    return float4(layer / 4.0, viewport * 0.5 + 0.25, frac(position.x / 16.0), 1.0);
}
