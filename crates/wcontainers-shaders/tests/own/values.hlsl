// Shaders compiled by both fxc and dxc (compile.cmd), drawn by both routes against real Direct3D 11's
// picture of the fxc build: a loop whose next trip uses values the last one computed, float to integer
// conversions of special values from a table, and fine and coarse derivatives. They share own.hlsl's
// constants and its vertex shader's outputs.

cbuffer Camera : register(b0) {
    float4x4 world_view_projection;
    float4 tint;
};

struct VsOut {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint id : TEXCOORD1;
};

// vs_transform's product, one row a trip, through a loop the compilers keep.
VsOut vs_carry(float3 position : POSITION, float2 uv : TEXCOORD0, uint id : SV_VertexID) {
    float4 h = float4(position, 1.0);
    float4 r = 0.0;
    [loop] for (uint k = 0; k < 4; k++) {
        r += h[k] * world_view_projection[k];
    }
    VsOut o;
    o.position = r;
    o.uv = uv;
    o.id = id;
    return o;
}

// Values Direct3D converts to integers by its own rule: NaN to 0, the limits at and past them. The
// tint gives NaN, both infinities and 2^31 at run time; the rest are finite.
// Components are picked by selects: dxil-spirv reads a component picked by index through a view of
// the constant buffer that WebGPU cannot bind.
float special(uint k) {
    float4 row = (k & 4) != 0 ? float4(-2147483904.0, 4294967296.0, -1.5, 3000000000.0) : tint;
    uint c = k & 3;
    return c == 0 ? row.x : c == 1 ? row.y : c == 2 ? row.z : row.w;
}

// Conversions of special values, picked by the pixel, and of products that vary across the target.
uint4 ps_ftoi(VsOut i) : SV_Target0 {
    float a = special(uint(i.position.x) % 8);
    float b = special(uint(i.position.y) % 8) * (i.uv.x + 1.0);
    return uint4(asuint(int(a)), uint(a), asuint(int(b)) * 7 + uint(b), uint(-b));
}

// The same values from one table, which holds NaN and the infinities as constants (fxc's immediate
// constant buffer).
static const float specials[8] = {
    asfloat(0x7fc00000), asfloat(0x7f800000), asfloat(0xff800000), 2147483648.0,
    -2147483904.0, 4294967296.0, -1.5, 3000000000.0
};

uint4 ps_table(VsOut i) : SV_Target0 {
    float a = specials[uint(i.position.x) % 8];
    float b = specials[uint(i.position.y) % 8] * (i.uv.x + 1.0);
    return uint4(asuint(int(a)), uint(a), asuint(int(b)) * 7 + uint(b), uint(-b));
}

// A product of the pixel's coordinates, whose fine and coarse derivatives differ.
float pixel_product(VsOut i) {
    return i.position.x * i.position.y * 0.25 + i.uv.x;
}

uint4 ps_deriv(VsOut i) : SV_Target0 {
    float v = pixel_product(i);
    return asuint(float4(ddx_fine(v), ddy_fine(v), ddx_coarse(v), ddy_coarse(v)));
}

// Coarse and unspecified derivatives only, which GLSL ES's plain ones may stand for.
uint4 ps_coarse(VsOut i) : SV_Target0 {
    float v = pixel_product(i);
    return asuint(float4(ddx_coarse(v), ddy_coarse(v), ddx(v), fwidth(v)));
}

// Helper lanes seen through derivatives: each pixel shows whether its quad neighbours are helpers,
// outside the triangle or discarded. The dxc build (shader model 6.6) discards first and asks
// IsHelperLane(); the fxc build takes a lane without coverage as a helper, adds the lanes it will
// discard, and discards them after the derivatives. Both read the coverage.
uint4 ps_helper(VsOut i, uint coverage : SV_Coverage) : SV_Target0 {
    uint2 p = uint2(i.position.xy);
    bool gone = (p.x * 7 + p.y * 3) % 5 == 0;
#if __SHADER_TARGET_MAJOR >= 6
    if (gone) {
        discard;
    }
    float h = IsHelperLane() ? 1.0 : 0.0;
#else
    float h = coverage == 0 || gone ? 1.0 : 0.0;
#endif
    uint4 r = uint4(asuint(ddx_fine(h)), asuint(ddy_fine(h)), asuint(h), coverage);
#if __SHADER_TARGET_MAJOR < 6
    if (gone) {
        discard;
    }
#endif
    return r;
}
