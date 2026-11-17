// Compute shaders on unordered access views: a typed texture, structured and raw buffers, group
// shared memory with barriers, atomics, append and consume buffers and a hidden counter; and a pixel
// shader counting overlaps with atomics.
// fxc: cs_5_0 cs_texture, cs_5_0 cs_structured, cs_5_0 cs_shared, cs_5_0 cs_atomic, cs_5_0 cs_append, cs_5_0 cs_consume, cs_5_0 cs_counter, ps_5_0 ps_overlap
RWTexture2D<float4> image : register(u0);
[numthreads(8, 8, 1)]
void cs_texture(uint3 id : SV_DispatchThreadID, uint3 group : SV_GroupID) {
    image[id.xy] = float4(id.x / 64.0, id.y / 64.0, ((id.x ^ id.y) & 15) / 15.0, (group.x + group.y) % 2 ? 1.0 : 0.5);
}

struct Item { float2 a; uint b; };
StructuredBuffer<Item> input : register(t0);
RWStructuredBuffer<Item> output : register(u0);
[numthreads(64, 1, 1)]
void cs_structured(uint3 id : SV_DispatchThreadID) {
    Item i = input[id.x];
    i.a = i.a * 2.0 + float(id.x);
    i.b = i.b ^ (id.x * 2654435761u);
    output[id.x] = i;
}

RWByteAddressBuffer raw : register(u0);
groupshared uint partial[64];
[numthreads(64, 1, 1)]
void cs_shared(uint index : SV_GroupIndex, uint3 group : SV_GroupID) {
    partial[index] = index * 3 + group.x * 1000;
    GroupMemoryBarrierWithGroupSync();
    for (uint s = 32; s > 0; s >>= 1) {
        if (index < s) partial[index] += partial[index + s];
        GroupMemoryBarrierWithGroupSync();
    }
    if (index == 0) raw.Store(group.x * 4, partial[0]);
}

[numthreads(64, 1, 1)]
void cs_atomic(uint3 id : SV_DispatchThreadID) {
    uint old;
    raw.InterlockedAdd((id.x % 8) * 4, id.x, old);
    raw.InterlockedMax(32, (id.x * 7) % 1000);
    raw.InterlockedMin(36, 5000 - id.x);
    raw.InterlockedOr(40, 1u << (id.x % 32));
    raw.InterlockedXor(44, id.x * 17);
}

AppendStructuredBuffer<uint> appended : register(u0);
[numthreads(64, 1, 1)]
void cs_append(uint3 id : SV_DispatchThreadID) {
    if (id.x % 3 == 0) appended.Append(id.x * 10);
}

ConsumeStructuredBuffer<uint> consumed : register(u0);
RWByteAddressBuffer total : register(u1);
[numthreads(16, 1, 1)]
void cs_consume(uint3 id : SV_DispatchThreadID) {
    uint v = consumed.Consume();
    uint old;
    total.InterlockedAdd(0, v, old);
    total.InterlockedAdd(4, 1, old);
}

RWStructuredBuffer<uint> slots : register(u0);
[numthreads(32, 1, 1)]
void cs_counter(uint3 id : SV_DispatchThreadID) {
    if (id.x % 4 != 1) slots[slots.IncrementCounter()] = id.x;
}

RWTexture2D<uint> overlaps : register(u1);
struct ColorOut { float4 pos : SV_Position; float4 color : COLOR; };
float4 ps_overlap(ColorOut i) : SV_Target {
    uint old;
    InterlockedAdd(overlaps[uint2(i.pos.xy)], 1, old);
    return i.color;
}
