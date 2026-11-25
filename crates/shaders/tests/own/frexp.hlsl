// frexp of constants of any bits (NaNs, infinities, zeros, subnormals), the mantissa and the
// exponent written out as bits, a pair an invocation.

cbuffer Operands : register(b0) {
    float4 operands[16];
};

RWStructuredBuffer<uint> parts : register(u0);

[numthreads(64, 1, 1)]
void cs_frexp(uint i : SV_DispatchThreadID) {
    float exponent;
    float mantissa = frexp(operands[i / 4][i % 4], exponent);
    parts[2 * i] = asuint(mantissa);
    parts[2 * i + 1] = asuint(exponent);
}
