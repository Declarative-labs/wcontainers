// 1D textures and 1D arrays loaded, sampled (with an offset and gradients), sized and stored, which
// the fronts hold as 2D textures one texel high.
Texture1D<float> row : register(t0);
Texture1DArray<float> rows : register(t1);
SamplerState linear_clamp : register(s0);
RWTexture1D<float> row_out : register(u0);
RWTexture1DArray<uint> rows_out : register(u1);

[numthreads(64, 1, 1)]
void cs_one_d(uint3 id : SV_DispatchThreadID) {
    uint width, levels, array_width, layers;
    row.GetDimensions(0, width, levels);
    rows.GetDimensions(array_width, layers);
    float u = (id.x + 0.25) / width;
    row_out[id.x] = row.Load(int2(id.x, 0)) + row.SampleLevel(linear_clamp, u, 0, 1) * 2.0
        + rows.SampleLevel(linear_clamp, float2(u, id.x % layers), 0) * 4.0
        + row.SampleGrad(linear_clamp, u, 0.5 / width, 0.25 / width) * 8.0;
    uint stored_out, stored_layers;
    rows_out.GetDimensions(stored_out, stored_layers);
    rows_out[uint2(id.x, id.x % stored_layers)] = width + levels * 100 + array_width * 1000 + layers * 100000 + stored_out * 1000000 + id.x;
}
