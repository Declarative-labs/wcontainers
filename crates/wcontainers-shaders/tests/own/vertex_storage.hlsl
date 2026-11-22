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

// A vertex shader writing a raw buffer and a typed texture beside the values it passes on (lines.hlsl's
// `LineOut`, drawn by `ps_line`). Each vertex writes words of itself alone, so they do not depend on
// how many times the device runs it.
RWByteAddressBuffer stored_words : register(u1);
RWTexture2D<uint> stored_texels : register(u2);

struct StoredOut {
    float4 position : SV_Position;
    float4 colour : COLOR0;
    nointerpolation uint id : ID0;
};

StoredOut vs_store_drawn(float3 position : POSITION, float4 colour : COLOR0, uint vertex : SV_VertexID,
                         uint instance : SV_InstanceID) {
    stored_words.Store2((instance * 32 + vertex) * 8, uint2(vertex | (instance << 16), asuint(position.x)));
    stored_texels[uint2(vertex, instance)] = asuint(colour.g) ^ vertex;
    StoredOut o;
    o.position = float4(position, 1.0);
    o.colour = colour;
    o.id = vertex;
    return o;
}
