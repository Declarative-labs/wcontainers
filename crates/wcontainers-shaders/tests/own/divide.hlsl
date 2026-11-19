// Floating-point division of a converted integer, and the fraction of the quotient, written out
// as bits, so the converted program's precision can be compared with Direct3D's `div`.

StructuredBuffer<uint> operands : register(t0);
RWStructuredBuffer<uint> quotients : register(u0);

[numthreads(64, 1, 1)]
void cs_divide(uint i : SV_DispatchThreadID) {
    float q = (float)operands[2 * i] / asfloat(operands[2 * i + 1]);
    quotients[2 * i] = asuint(q);
    quotients[2 * i + 1] = asuint(frac(q));
}
