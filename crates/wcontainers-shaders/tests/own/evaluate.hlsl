// Inputs evaluated elsewhere in the pixel (the centroid, a sample, an offset from the centre), and the
// position in a program run per sample, drawn against real Direct3D 11 at one and four samples. Each
// value goes through a steep wave, so a place a sixteenth of a pixel off shows in the picture.

cbuffer Camera : register(b0) {
    float4x4 world_view_projection;
    float4 tint;
};

struct EvalIo {
    float4 position : SV_Position;
    float4 colour : COLOR0;
    noperspective float2 uv : TEXCOORD1;
    sample float2 at_sample : TEXCOORD2;
};

EvalIo vs_evaluate(float3 position : POSITION, float2 uv : TEXCOORD0) {
    EvalIo o;
    o.position = mul(float4(position, 1.0), world_view_projection);
    o.colour = float4(uv, uv.x + uv.y, uv.y - uv.x) + 2.0;
    o.uv = uv * 1.5;
    o.at_sample = uv.yx;
    return o;
}

float wave(float v) {
    return sin(v * 48.0) * 0.5 + 0.5;
}

float4 ps_evaluate(EvalIo i, uint s : SV_SampleIndex) : SV_Target0 {
    float4 snapped = EvaluateAttributeSnapped(i.colour, int2(-8, 7));
    float2 linear_snapped = EvaluateAttributeSnapped(i.uv, int2(5, -3));
    float4 at_centroid = EvaluateAttributeCentroid(i.colour);
    float4 other = EvaluateAttributeAtSample(i.colour, (s + 1) % GetRenderTargetSampleCount());
    float2 from_sample = EvaluateAttributeSnapped(i.at_sample, int2(3, 2));
    // linear_snapped's x: fxc 10.1 compiles a read of its y from its x here.
    return float4(wave(snapped.x), wave(linear_snapped.x),
                  wave(at_centroid.z + other.w), wave(from_sample.x + i.position.x / 64.0));
}

// An input evaluated at the sample being run, less the same value interpolated at that sample: zero
// on Direct3D, any difference scaled up so that the smallest shows in the picture.
struct ExactIo {
    float4 position : SV_Position;
    float2 attr : TEXCOORD3;
    sample float2 at_sample : TEXCOORD4;
};

ExactIo vs_eval_exact(float3 position : POSITION, float2 uv : TEXCOORD0) {
    ExactIo o;
    o.position = mul(float4(position, 1.0), world_view_projection);
    o.attr = o.position.xy * 3.0 + uv;
    o.at_sample = o.attr;
    return o;
}

float4 ps_eval_exact(ExactIo i, uint s : SV_SampleIndex) : SV_Target0 {
    float2 apart = EvaluateAttributeAtSample(i.attr, s) - i.at_sample;
    return float4(0.5 + apart * 4294967296.0, 0.5, 1.0);
}
