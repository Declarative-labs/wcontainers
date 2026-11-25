// A triangle covering the target and each vertex's number, for the hull shader (tests/registers.rs).
struct V
{
    float4 p : P;
    float k : K;
};

V main(uint id : SV_VertexID)
{
    V o;
    float2 p = float2((id << 1) & 2, id & 2) * float2(2, -2) + float2(-1, 1);
    o.p = float4(p, 0, 1);
    o.k = id;
    return o;
}
