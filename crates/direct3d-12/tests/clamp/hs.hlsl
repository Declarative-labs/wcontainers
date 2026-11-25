// Passes each triangle on once.
struct V { float4 p : SV_Position; float4 c : COLOR; };
struct F { float e[3] : SV_TessFactor; float i : SV_InsideTessFactor; };
F constants() { F f; f.e[0] = 1.0; f.e[1] = 1.0; f.e[2] = 1.0; f.i = 1.0; return f; }
[domain("tri")] [partitioning("integer")] [outputtopology("triangle_cw")]
[outputcontrolpoints(3)] [patchconstantfunc("constants")]
V main(InputPatch<V, 3> p, uint i : SV_OutputControlPointID) { return p[i]; }
