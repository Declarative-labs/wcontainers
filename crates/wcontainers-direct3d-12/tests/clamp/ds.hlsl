// Places each tessellated vertex in its triangle, with the first corner's colour.
struct V { float4 p : SV_Position; float4 c : COLOR; };
struct F { float e[3] : SV_TessFactor; float i : SV_InsideTessFactor; };
[domain("tri")] V main(F f, float3 b : SV_DomainLocation, const OutputPatch<V, 3> p) {
    V o; o.p = p[0].p * b.x + p[1].p * b.y + p[2].p * b.z; o.c = p[0].c; return o; }
