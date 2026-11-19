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

// A loop running until a count or an escape, then the count divided and its fraction, as the
// Mandelbrot iteration of a sample program colours its pixels: per invocation a point's x and y
// and the iteration scale (float bits) in, the count, the quotient and its fraction out.
[numthreads(64, 1, 1)]
void cs_escape(uint i : SV_DispatchThreadID) {
    float2 coord = float2(asfloat(operands[3 * i]), asfloat(operands[3 * i + 1]));
    float scale = asfloat(operands[3 * i + 2]);
    uint most = (uint)scale * 4;
    uint count = 0;
    float2 start = coord;
    float2 sq;
    do {
        float2 next;
        sq = coord * coord;
        next.x = sq.x - sq.y;
        next.y = 2 * coord.y * coord.x;
        coord = next + start;
        count++;
    } while (count < most && (sq.x + sq.y) < 4.0);
    float q = (float)count / scale;
    quotients[3 * i] = count;
    quotients[3 * i + 1] = asuint(q);
    quotients[3 * i + 2] = asuint(frac(q));
}
