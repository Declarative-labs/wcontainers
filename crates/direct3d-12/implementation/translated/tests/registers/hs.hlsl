// Passes a triangle patch on whole, tessellated once (tests/registers.rs).
struct V
{
    float4 p : P;
    float k : K;
};

struct C
{
    float e[3] : SV_TessFactor;
    float i : SV_InsideTessFactor;
};

C constants(InputPatch<V, 3> ip)
{
    C c;
    c.e[0] = 1;
    c.e[1] = 1;
    c.e[2] = 1;
    c.i = 1;
    return c;
}

[domain("tri")]
[partitioning("integer")]
[outputtopology("triangle_cw")]
[outputcontrolpoints(3)]
[patchconstantfunc("constants")]
V main(InputPatch<V, 3> ip, uint n : SV_OutputControlPointID)
{
    return ip[n];
}
