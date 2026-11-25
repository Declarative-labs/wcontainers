// A multisampled array texture: ps_fill draws each layer, a value per sample; ps_msarray reads it
// back with the layer picked by the pixel's column at run time (past the layer count from column 3),
// the sample by its row, and the texture's dimensions in its last rows.

cbuffer Fill : register(b0) {
    float4 tint;
};

float4 ps_fill(float4 position : SV_Position, uint sample : SV_SampleIndex) : SV_Target0 {
    uint2 p = uint2(position.xy);
    return float4(((p.x * 37 + sample * 11) % 256) / 255.0, ((p.y * 53 + sample * 29) % 256) / 255.0,
                  sample / 3.0, tint.x);
}

Texture2DMSArray<float4> layers : register(t0);

float4 ps_msarray(float4 position : SV_Position) : SV_Target0 {
    uint2 p = uint2(position.xy);
    uint width, height, count, samples;
    layers.GetDimensions(width, height, count, samples);
    if (p.y >= 56) {
        return float4(width, height, count, samples) / 255.0;
    }
    return layers.Load(int3(p % uint2(width, height), p.x / 8), (p.y / 8) % samples);
}
