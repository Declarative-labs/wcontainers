// Compute programs each using one ability of a view's format (tests/answers.rs), for the component
// kind KIND (float4, uint4 or int4) and its scalar SCALAR; `use_<name>` picks the program. Each
// writes what it read to `result` as bits.
RWByteAddressBuffer result : register(u0);
SamplerState linear_sampler : register(s0);
SamplerComparisonState compare_sampler : register(s1);
static const float2 middle = float2(0.5, 0.5);
static const float reference = 0.5;
static const int2 first = int2(0, 0);

#if defined(use_load)
Texture2D<KIND> t : register(t0);
[numthreads(1, 1, 1)] void main() { result.Store4(0, asuint(t.Load(int3(first, 0)))); }
#elif defined(use_sample)
Texture2D<float4> t : register(t0);
[numthreads(1, 1, 1)] void main() { result.Store4(0, asuint(t.SampleLevel(linear_sampler, middle, 0))); }
#elif defined(use_compare)
Texture2D<float> t : register(t0);
[numthreads(1, 1, 1)] void main() { result.Store(0, asuint(t.SampleCmpLevelZero(compare_sampler, middle, reference))); }
#elif defined(use_gather)
Texture2D<float4> t : register(t0);
[numthreads(1, 1, 1)] void main() { result.Store4(0, asuint(t.Gather(linear_sampler, middle))); }
#elif defined(use_gather_compare)
Texture2D<float> t : register(t0);
[numthreads(1, 1, 1)] void main() { result.Store4(0, asuint(t.GatherCmp(compare_sampler, middle, reference))); }
#elif defined(use_multisample_load)
Texture2DMS<KIND> t : register(t0);
[numthreads(1, 1, 1)] void main() { result.Store4(0, asuint(t.Load(first, 0))); }
#elif defined(use_buffer_load)
Buffer<KIND> t : register(t0);
[numthreads(1, 1, 1)] void main() { result.Store4(0, asuint(t.Load(0))); }
#elif defined(use_store)
RWTexture2D<KIND> u : register(u1);
[numthreads(1, 1, 1)] void main() { u[first] = (KIND)1; }
#elif defined(use_buffer_store)
RWBuffer<KIND> u : register(u1);
[numthreads(1, 1, 1)] void main() { u[0] = (KIND)1; }
#elif defined(use_typed_load)
RWTexture2D<SCALAR> u : register(u1);
[numthreads(1, 1, 1)] void main() { result.Store(0, asuint(u[first])); }
#else
// The atomics: each the previous values its operations return, then the texel's last value.
RWTexture2D<SCALAR> u : register(u1);
[numthreads(1, 1, 1)] void main()
{
    SCALAR a = 0, b = 0, c = 0;
#if defined(use_atomic_add)
    InterlockedAdd(u[first], 5, a);
#elif defined(use_atomic_bitwise)
    InterlockedOr(u[first], 6, a);
    InterlockedAnd(u[first], 3, b);
    InterlockedXor(u[first], 1, c);
#elif defined(use_atomic_compare)
    InterlockedCompareStore(u[first], 0, 7);
    InterlockedCompareExchange(u[first], 7, 9, a);
#elif defined(use_atomic_exchange)
    InterlockedExchange(u[first], (SCALAR)4, a);
#elif defined(use_atomic_signed_min_or_max)
    InterlockedMax(u[first], 3, a);
    InterlockedMin(u[first], -2, b);
#elif defined(use_atomic_unsigned_min_or_max)
    InterlockedMax(u[first], 8u, a);
    InterlockedMin(u[first], 2u, b);
#endif
    result.Store4(0, asuint(KIND(a, b, c, u[first])));
}
#endif
