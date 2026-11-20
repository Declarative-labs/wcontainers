// Vertex shaders writing a storage texture, a typed storage buffer and a raw storage buffer, compiled
// by fxc (vs_5_0) and dxc (vs_6_0) by compile.cmd.

RWTexture2D<uint> texels : register(u1);
RWBuffer<uint> typed : register(u1);
RWByteAddressBuffer raw : register(u1);

float4 vs_store_texture(uint v : SV_VertexID) : SV_Position {
    texels[uint2(v, 0)] = v;
    return float4(0, 0, 0, 1);
}

float4 vs_store_typed(uint v : SV_VertexID) : SV_Position {
    typed[v] = v;
    return float4(0, 0, 0, 1);
}

float4 vs_store_raw(uint v : SV_VertexID) : SV_Position {
    raw.Store(v * 4, v);
    return float4(0, 0, 0, 1);
}
