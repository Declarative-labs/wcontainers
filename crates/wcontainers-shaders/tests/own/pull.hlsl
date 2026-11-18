// Shaders for pulled vertex inputs (tests/pull_draw.rs): one point per vertex and instance, at pixel
// (vertex, instance), carrying the bits of one input to the target.
struct PullIn {
    float4 a : A;
    int4 b : B;
    float4 c : C;
    float4 d : D;
    float4 e : E;
    float4 f : F;
    float4 g : G;
    float4 h : H;
    uint4 i : I;
    uint4 l : L;
    int4 j : J;
    float4 k : K;
    float4 m : M;
    uint vertex : SV_VertexID;
    uint instance : SV_InstanceID;
};

struct PullOut {
    float4 position : SV_Position;
    nointerpolation uint4 value : VALUE;
};

PullOut vs_pull(PullIn v) {
    PullOut o;
    o.position = float4((v.vertex + 0.5) / 32.0 - 1.0, 1.0 - (v.instance + 0.5) / 32.0, 0.0, 1.0);
    uint which = v.instance % 13;
    uint4 x = asuint(v.a);
    x = which == 1 ? asuint(v.b) : x;
    x = which == 2 ? asuint(v.c) : x;
    x = which == 3 ? asuint(v.d) : x;
    x = which == 4 ? asuint(v.e) : x;
    x = which == 5 ? asuint(v.f) : x;
    x = which == 6 ? asuint(v.g) : x;
    x = which == 7 ? asuint(v.h) : x;
    x = which == 8 ? v.i : x;
    x = which == 9 ? v.l : x;
    x = which == 10 ? asuint(v.j) : x;
    x = which == 11 ? asuint(v.k) : x;
    x = which == 12 ? asuint(v.m) : x;
    o.value = x;
    return o;
}

uint4 ps_pull(PullOut i) : SV_Target0 {
    return i.value;
}
