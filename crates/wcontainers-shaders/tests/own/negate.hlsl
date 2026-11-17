// Unsigned negation as compilers write it, compiled by both fxc and dxc (compile.cmd) and drawn by
// both routes: fxc writes `a - b * c` on unsigned values as `imad -b, c, a`, dxc writes `0 - x` and the
// remainder of a signed division as `sub 0, x`. Shares own.hlsl's constants and vertex outputs.

cbuffer Camera : register(b0) {
    float4x4 world_view_projection;
    float4 tint;
};

struct VsOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint id : TEXCOORD1;
};

uint4 ps_negate(VsOut i) : SV_Target0 {
    uint px = (uint)i.position.x;
    uint py = (uint)i.position.y;
    int sx = (int)px - 31;
    int table[3] = { 5, -7, 11 };
    int pick = table[abs(sx % 3)];
    return uint4(i.id - px * py, 0u - px, (uint)(sx % 3), (uint)(pick * sx));
}
