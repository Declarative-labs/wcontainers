// Wave operations of shader model 6.0 in a pixel shader, compiled by dxc only (compile.cmd) and drawn
// against real Direct3D 12: lane counts, sums, prefixes, a ballot and reads of other lanes over the
// pixels each wave holds. Some pixels are discarded first, so their lanes and the helper lanes at the
// triangles' edges must take no part in the wave operations. Shares own.hlsl's constants and its
// vertex shader's outputs.

cbuffer Camera : register(b0) {
    float4x4 world_view_projection;
    float4 tint;
};

struct VsOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint id : TEXCOORD1;
};

uint4 ps_wave(VsOut i) : SV_Target0 {
    uint2 p = uint2(i.position.xy);
    if ((p.x * 7 + p.y * 3) % 5 == 0) {
        discard;
    }
    uint key = p.x + 64 * p.y + 1;
    uint lanes = WaveActiveCountBits(true);
    uint4 odd = WaveActiveBallot((key & 1) != 0);
    uint first = WaveReadLaneFirst(key);
    uint later = WaveReadLaneAt(key, WaveActiveMax(WaveGetLaneIndex()));
    uint all_same = WaveActiveAllEqual(i.id) ? 1u : 0u;
    uint largest = WaveActiveMax(key);
    return uint4(
        lanes | (WaveGetLaneIndex() << 8) | (WaveGetLaneCount() << 16) | (all_same << 24),
        WaveActiveSum(key) ^ (WavePrefixSum(key) << 7),
        countbits(odd.x) + countbits(odd.y) * 64 + (WavePrefixCountBits(key % 3 == 0) << 12),
        first ^ (largest << 5) ^ (later << 11) ^ (WaveIsFirstLane() ? 0x80000000u : 0u)
    );
}
