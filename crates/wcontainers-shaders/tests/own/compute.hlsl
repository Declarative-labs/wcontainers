// Atomic operations on shared memory and storage buffers, and append, consume and counter views.
// Every result written is the same whatever order the invocations run in: values returned by an
// atomic mark slots, so the slots marked are what is compared.

RWByteAddressBuffer raw : register(u0);
RWStructuredBuffer<int> signed_words : register(u1);
RWStructuredBuffer<uint> slots : register(u2);
AppendStructuredBuffer<uint> appended : register(u3);
ConsumeStructuredBuffer<uint> consumed : register(u4);

groupshared uint shared_words[8];

[numthreads(64, 1, 1)]
void cs_atomics(uint t : SV_GroupIndex, uint3 group : SV_GroupID, uint3 id : SV_DispatchThreadID) {
    if (t < 8) {
        shared_words[t] = (t == 2 || t == 5) ? 0xffffffffu : 0u;
    }
    GroupMemoryBarrierWithGroupSync();
    uint v = id.x * 2654435761u;

    // Shared memory: the values an add returns are 0 to 63 once each.
    uint old;
    InterlockedAdd(shared_words[0], 1, old);
    raw.Store(256 + 4 * (group.x * 64 + old), 1);
    InterlockedMax(shared_words[1], v);
    InterlockedMin(shared_words[2], v);
    InterlockedOr(shared_words[3], 1u << (t % 29));
    InterlockedXor(shared_words[4], v);
    InterlockedAnd(shared_words[5], ~(1u << (t % 31)));
    // A maximum made of compare-exchanges, which returns what it found.
    uint seen = shared_words[6];
    [allow_uav_condition] for (;;) {
        uint prior;
        InterlockedCompareExchange(shared_words[6], seen, max(seen, v ^ 0x55555555u), prior);
        if (prior == seen) {
            break;
        }
        seen = prior;
    }
    // The values an exchange returns, and the last one left, are 0 to 64 once each.
    uint was;
    InterlockedExchange(shared_words[7], t + 1, was);
    raw.Store(1280 + 4 * (group.x * 65 + was), 1);
    GroupMemoryBarrierWithGroupSync();
    if (t == 0) {
        raw.Store(1280 + 4 * (group.x * 65 + shared_words[7]), 1);
    }
    if (t < 7) {
        raw.Store(4 * (group.x * 8 + t), shared_words[t]);
    }

    // A storage buffer, across groups.
    raw.InterlockedAdd(2560, v);
    raw.InterlockedMax(2564, v);
    raw.InterlockedMin(2568, v);
    raw.InterlockedOr(2572, 1u << (id.x % 32));
    raw.InterlockedXor(2576, v);
    raw.InterlockedAnd(2580, ~(1u << (id.x % 30)));
    raw.InterlockedCompareStore(2584, 0, 7);
    uint found = raw.Load(2588);
    [allow_uav_condition] for (;;) {
        uint prior;
        raw.InterlockedCompareExchange(2588, found, max(found, v ^ 0xaaaaaaaau), prior);
        if (prior == found) {
            break;
        }
        found = prior;
    }
    uint swapped;
    raw.InterlockedExchange(2592, id.x + 1, swapped);
    raw.Store(4096 + 4 * swapped, 1);
    InterlockedMin(signed_words[0], int(v));
    InterlockedMax(signed_words[1], int(v));
    InterlockedAdd(signed_words[2], -int(t));

    // Counters: each invocation takes a slot; consumes one value and appends it changed.
    uint n = slots.IncrementCounter();
    slots[n] = 1;
    uint c = consumed.Consume();
    appended.Append(c * 3 + 1);
}

// A counter taken back: the values a decrement returns mark slots too.
RWStructuredBuffer<uint> returned : register(u0);

[numthreads(32, 1, 1)]
void cs_decrement(uint3 id : SV_DispatchThreadID) {
    uint n = returned.DecrementCounter();
    returned[n] = returned[n] + 2;
}

// Storage textures of several formats and shapes: written, and one read and written again.
RWTexture2D<float4> colour : register(u0);
RWTexture2D<float> level : register(u1);
RWTexture2DArray<uint4> layers : register(u2);
RWTexture2D<int2> pairs : register(u3);

[numthreads(8, 8, 1)]
void cs_images(uint3 id : SV_DispatchThreadID) {
    uint width, height;
    colour.GetDimensions(width, height);
    float2 uv = (id.xy + 0.5) / float2(width, height);
    colour[id.xy] = float4(uv, 1.0 - uv.x, 0.25 + uv.y * 0.5);
    level[id.xy] = level[id.xy] * 2.0 + uv.x;
    uint w, h, n;
    layers.GetDimensions(w, h, n);
    for (uint l = 0; l < n; l++) {
        layers[uint3(id.xy, l)] = uint4(id.xy, l, w * h + n);
    }
    pairs[id.xy] = int2(int(id.x) - int(id.y), -int(id.x * 3));
}

// Structured group-shared memory of members of several types (a float, an unsigned integer, a
// float pair, indexed at run time), written by one invocation and read by another.
struct Shared {
    float f;
    uint u;
    float2 v;
};
groupshared Shared shared_structs[64];

[numthreads(64, 1, 1)]
void cs_shared(uint t : SV_GroupIndex, uint3 group : SV_GroupID) {
    shared_structs[t].f = t * 0.5 + group.x;
    shared_structs[t].u = t * 3 + 1;
    shared_structs[t].v = float2(t, -float(t) * 0.25);
    GroupMemoryBarrierWithGroupSync();
    Shared s = shared_structs[(t * 7 + 5) % 64];
    raw.Store4(16 * (group.x * 64 + t), uint4(asuint(s.f), s.u, asuint(s.v)));
}
